//! The field: the current map, the camera, and metatile drawing into BG1–BG3.
//!
//! Each field BG is a 32x32-entry tilemap holding a 16x16-metatile ring: metatile (mx, my) lives
//! at entries (2·((mx + ox) & 15), 2·((my + oy) & 15)) for the ring origin (ox, oy), and the BG
//! scroll is (camera pixel + 16·origin) mod 256. Only the metatiles that come into view are drawn
//! when the camera moves. A connection crossing moves the origin with the coordinates, so what
//! is drawn stays valid and nothing is redrawn (Gen3 `LoadMapFromCameraTransition` does not
//! redraw either).
//!
//! Layer routing, tileset/palette loading and the NORMAL underlay mirror Gen3 `DrawMetatile` and
//! `LoadTilesetPalette` (`field_camera.c`, `fieldmap.c`).
//!
//! Tilesets must be `static`s shared by the maps that use them: on a connection switch, tileset
//! identity (`core::ptr::eq`) decides whether tiles and palettes are re-uploaded.

use crate::data::{
    GameData, LayerType, METATILES_IN_PRIMARY, MapData, MapId, PALS_IN_PRIMARY, PALS_TOTAL,
    TILES_IN_PRIMARY, Tileset, attr_layer, block_metatile,
};
use crate::grid;
use crate::hash::Fnv64;
use crate::video::{BgControl, Cmd, Frame, SCREEN_H, SCREEN_W, layout};
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

/// Metatiles per side of the ring buffer each field BG holds.
pub const RING: i16 = 16;
/// What Gen3 `DrawMetatile` writes to the bottom BG under NORMAL metatiles.
pub const NORMAL_UNDERLAY: u16 = 0x3014;
/// Field BGs in the order the per-layer arrays use: top, middle, bottom.
pub const FIELD_BGS: [u8; 3] = [
    layout::FIELD_TOP_BG,
    layout::FIELD_MIDDLE_BG,
    layout::FIELD_BOTTOM_BG,
];

/// The 4 tile entries (TL, TR, BL, BR) a metatile puts on each field BG, indexed like
/// [`FIELD_BGS`] (top, middle, bottom).
pub type Layers = [[u16; 4]; 3];

/// An inclusive rectangle of metatile cells.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Rect {
    x0: i16,
    y0: i16,
    x1: i16,
    y1: i16,
}

impl Rect {
    /// The cells intersecting the 240x160 view whose top-left map pixel is `camera_px`.
    fn view(camera_px: (i32, i32)) -> Rect {
        let (cx, cy) = camera_px;
        Rect {
            x0: cx.div_euclid(16) as i16,
            y0: cy.div_euclid(16) as i16,
            x1: (cx + SCREEN_W as i32 - 1).div_euclid(16) as i16,
            y1: (cy + SCREEN_H as i32 - 1).div_euclid(16) as i16,
        }
    }

    fn contains(self, x: i16, y: i16) -> bool {
        (self.x0..=self.x1).contains(&x) && (self.y0..=self.y1).contains(&y)
    }
}

pub struct Field {
    map: MapId,
    camera_px: (i32, i32),
    /// Cells currently drawn in the ring (`None` until the first draw).
    drawn: Option<Rect>,
    /// `setmetatile` overrides of the current map's own cells.
    overrides: BTreeMap<(i16, i16), u16>,
    /// Ring origin (see the module doc), each in `0..RING`.
    ring: (i16, i16),
}

impl Field {
    pub fn new(map: MapId) -> Self {
        Field {
            map,
            camera_px: (0, 0),
            drawn: None,
            overrides: BTreeMap::new(),
            ring: (0, 0),
        }
    }

    pub fn map(&self) -> MapId {
        self.map
    }

    /// Map pixel of the screen's top-left corner.
    pub fn camera(&self) -> (i32, i32) {
        self.camera_px
    }

    /// Enters `map` from scratch (warps, new game) without drawing: clears overrides, uploads
    /// both tilesets' tiles (at `FIELD_CHAR_BLOCK`: primary 0.., secondary 512..) and palettes
    /// (primary 0..6, secondary 6..13) and sets BG1–BG3 controls. Nothing is drawn until
    /// [`Field::redraw`], so on-load `set_block`s land before the first draw (Gen3 runs on-load
    /// scripts in `InitMap` before `DrawWholeMapView`).
    pub fn enter(
        &mut self,
        game: &impl GameData,
        map: MapId,
        camera_px: (i32, i32),
        f: &mut Frame,
    ) {
        self.map = map;
        self.overrides.clear();
        for bg in FIELD_BGS {
            f.cmds.push(Cmd::Bg {
                layer: bg,
                control: BgControl {
                    enabled: true,
                    priority: layout::BG_PRIORITY[bg as usize],
                    char_block: layout::FIELD_CHAR_BLOCK,
                    screen_block: layout::SCREEN_BLOCKS[bg as usize],
                },
            });
        }
        let m = game.map(map);
        upload_primary(m.primary, f);
        upload_secondary(m.secondary, f);
        self.camera_px = camera_px;
        self.drawn = None;
        self.ring = (0, 0);
    }

    /// Re-uploads the current map's tilesets and palettes and BG1–BG3 controls, and invalidates
    /// the view, after something else owned the screen (a battle, the starter screen). Map,
    /// camera, ring origin and `setmetatile` overrides are kept; the next
    /// [`Field::set_camera`] redraws the whole view (Gen3 `CB2_ReturnToField` reloads the map
    /// graphics and calls `DrawWholeMapView`). Bulk: call it on a black frame.
    pub fn restore(&mut self, game: &impl GameData, f: &mut Frame) {
        let (camera, ring, overrides) = (
            self.camera_px,
            self.ring,
            core::mem::take(&mut self.overrides),
        );
        self.enter(game, self.map, camera, f);
        self.ring = ring;
        self.overrides = overrides;
    }

    /// [`Field::enter`] then [`Field::redraw`].
    pub fn load(&mut self, game: &impl GameData, map: MapId, camera_px: (i32, i32), f: &mut Frame) {
        self.enter(game, map, camera_px, f);
        self.redraw(game, f);
    }

    /// Moves the camera (map pixel of the screen's top-left corner), draws the metatiles that
    /// come into view and sets BG1–BG3 scroll in `f.scroll`.
    pub fn set_camera(&mut self, game: &impl GameData, camera_px: (i32, i32), f: &mut Frame) {
        self.camera_px = camera_px;
        let view = Rect::view(camera_px);
        let old = self.drawn;
        let mut cells = Vec::new();
        for y in view.y0..=view.y1 {
            for x in view.x0..=view.x1 {
                if !old.is_some_and(|r| r.contains(x, y)) {
                    cells.push((x, y));
                }
            }
        }
        self.draw_cells(game, &cells, f);
        self.drawn = Some(view);
        self.set_scroll(f);
    }

    /// The block at (x, y): overrides, then the map, its connections and its border.
    pub fn block(&self, game: &impl GameData, x: i16, y: i16) -> u16 {
        match self.overrides.get(&(x, y)) {
            Some(&b) => b,
            None => grid::block_at(game, game.map(self.map), x, y),
        }
    }

    /// Replaces the block of a cell of the current map (Gen3 `setmetatile`) and redraws it if it
    /// is in view (before the first draw after `enter`, it is only recorded). Cells outside the
    /// map are ignored. Overrides last until the next `load` or
    /// `switch_map`.
    pub fn set_block(&mut self, game: &impl GameData, x: i16, y: i16, block: u16, f: &mut Frame) {
        if !grid::in_bounds(game.map(self.map), x, y) {
            return;
        }
        self.overrides.insert((x, y), block);
        if self.drawn.is_some_and(|r| r.contains(x, y)) {
            self.draw_cells(game, &[(x, y)], f);
        }
    }

    /// The connected map and the shift (`new = old + shift`, in cells) for a cell (x, y) just
    /// outside the current map, if a connection covers it. Feed both to [`Field::switch_map`].
    pub fn crossing(&self, game: &impl GameData, x: i16, y: i16) -> Option<(MapId, (i16, i16))> {
        let n = grid::neighbor(game, game.map(self.map), x, y)?;
        Some((n.map, (n.x - x, n.y - y)))
    }

    /// Seamless switch to a connected map: the caller has moved the player across the edge. A
    /// cell at old coordinates (x, y) has new coordinates (x + shift.0, y + shift.1); the camera
    /// and the ring origin move with it, so the cells already drawn stay drawn. Clears overrides
    /// (cells drawn with one keep it until redrawn, as Gen3). Re-uploads a tileset (tiles and
    /// palettes) only if it differs; then the view is invalidated and the next
    /// [`Field::set_camera`] redraws it whole (Gen3 keeps the stale entries). Follow with on-load
    /// `set_block`s, then [`Field::set_camera`].
    ///
    /// Tileset identity is pointer identity (`ptr::eq`), so tilesets must be `static`s shared by
    /// the maps that use them.
    pub fn switch_map_enter(
        &mut self,
        game: &impl GameData,
        to: MapId,
        shift: (i16, i16),
        f: &mut Frame,
    ) {
        let (old, new) = (game.map(self.map), game.map(to));
        let mut same = true;
        if !core::ptr::eq(old.primary, new.primary) {
            upload_primary(new.primary, f);
            same = false;
        }
        if !core::ptr::eq(old.secondary, new.secondary) {
            upload_secondary(new.secondary, f);
            same = false;
        }
        self.map = to;
        self.overrides.clear();
        self.camera_px.0 += shift.0 as i32 * 16;
        self.camera_px.1 += shift.1 as i32 * 16;
        self.ring = (
            (self.ring.0 - shift.0).rem_euclid(RING),
            (self.ring.1 - shift.1).rem_euclid(RING),
        );
        self.drawn = match self.drawn {
            Some(r) if same => Some(Rect {
                x0: r.x0 + shift.0,
                y0: r.y0 + shift.1,
                x1: r.x1 + shift.0,
                y1: r.y1 + shift.1,
            }),
            _ => None,
        };
    }

    /// [`Field::switch_map_enter`] then [`Field::redraw`].
    pub fn switch_map(
        &mut self,
        game: &impl GameData,
        to: MapId,
        shift: (i16, i16),
        f: &mut Frame,
    ) {
        self.switch_map_enter(game, to, shift, f);
        self.redraw(game, f);
    }

    /// Redraws every metatile in view (Gen3 `DrawWholeMapView`) and sets the scroll.
    pub fn redraw(&mut self, game: &impl GameData, f: &mut Frame) {
        self.drawn = None;
        self.set_camera(game, self.camera_px, f);
    }

    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        h.write_u16(self.map.0);
        h.write_u32(self.camera_px.0 as u32);
        h.write_u32(self.camera_px.1 as u32);
        match self.drawn {
            None => h.write(&[0]),
            Some(r) => {
                h.write(&[1]);
                for v in [r.x0, r.y0, r.x1, r.y1] {
                    h.write_u16(v as u16);
                }
            }
        }
        h.write_u16(self.ring.0 as u16);
        h.write_u16(self.ring.1 as u16);
        h.write_u32(self.overrides.len() as u32);
        for (&(x, y), &b) in &self.overrides {
            h.write_u16(x as u16);
            h.write_u16(y as u16);
            h.write_u16(b);
        }
    }

    fn set_scroll(&self, f: &mut Frame) {
        let s = (
            (self.camera_px.0 + self.ring.0 as i32 * 16).rem_euclid(256) as u16,
            (self.camera_px.1 + self.ring.1 as i32 * 16).rem_euclid(256) as u16,
        );
        for bg in FIELD_BGS {
            f.scroll[bg as usize] = s;
        }
    }

    /// Draws `cells` (at most one per ring slot) into the ring: per BG (top, middle, bottom), one
    /// `MapEntries` per contiguous run of entries, in tilemap order.
    fn draw_cells(&self, game: &impl GameData, cells: &[(i16, i16)], f: &mut Frame) {
        if cells.is_empty() {
            return;
        }
        let map = game.map(self.map);
        // (tilemap row, tilemap column, entry per BG)
        let mut entries: Vec<(u8, u8, [u16; 3])> = Vec::with_capacity(cells.len() * 4);
        for &(x, y) in cells {
            let [top, middle, bottom] = metatile_layers(map, self.block(game, x, y));
            let (col, row) = (
                2 * ((x + self.ring.0) & (RING - 1)) as u8,
                2 * ((y + self.ring.1) & (RING - 1)) as u8,
            );
            for (k, ((t, m), b)) in top.into_iter().zip(middle).zip(bottom).enumerate() {
                entries.push((row + k as u8 / 2, col + k as u8 % 2, [t, m, b]));
            }
        }
        entries.sort_unstable_by_key(|&(r, c, _)| (r, c));
        for (i, bg) in FIELD_BGS.into_iter().enumerate() {
            let screen_block = layout::SCREEN_BLOCKS[bg as usize];
            let mut run: Option<(u16, Vec<u16>)> = None;
            let mut next = u16::MAX;
            for &(r, c, e) in &entries {
                let pos = r as u16 * 32 + c as u16;
                match &mut run {
                    // Contiguous in the tilemap (may run on into the next row).
                    Some((_, v)) if pos == next => v.push(e[i]),
                    _ => {
                        if let Some((start, v)) = run.take() {
                            f.cmds.push(Cmd::MapEntries {
                                screen_block,
                                start,
                                entries: v,
                            });
                        }
                        run = Some((pos, alloc::vec![e[i]]));
                    }
                }
                next = pos + 1;
            }
            if let Some((start, v)) = run {
                f.cmds.push(Cmd::MapEntries {
                    screen_block,
                    start,
                    entries: v,
                });
            }
        }
    }
}

/// The metatile and attribute for `block`, drawn with `map`'s tilesets (as Gen3, even for
/// neighbor and border blocks). Missing metatiles draw as all-zero NORMAL.
fn metatile(map: &MapData, block: u16) -> ([u16; 8], u16) {
    let id = block_metatile(block);
    let (set, i) = if id < METATILES_IN_PRIMARY {
        (map.primary, id)
    } else {
        (map.secondary, id - METATILES_IN_PRIMARY)
    };
    let i = i as usize;
    (
        set.metatiles.get(i).copied().unwrap_or([0; 8]),
        set.attributes.get(i).copied().unwrap_or(0),
    )
}

/// What a block puts on each field BG (top, middle, bottom), as Gen3 `DrawMetatile`.
pub fn metatile_layers(map: &MapData, block: u16) -> Layers {
    let (t, attr) = metatile(map, block);
    let bottom = [t[0], t[1], t[2], t[3]];
    let top = [t[4], t[5], t[6], t[7]];
    match attr_layer(attr) {
        LayerType::Normal => [top, bottom, [NORMAL_UNDERLAY; 4]],
        LayerType::Covered => [[0; 4], top, bottom],
        LayerType::Split => [top, [0; 4], bottom],
    }
}

fn upload_primary(ts: &'static Tileset, f: &mut Frame) {
    upload_tiles(ts, 0, f);
    for bank in 0..PALS_IN_PRIMARY {
        if let Some(&p) = ts.palettes.get(bank) {
            let mut colors = p;
            if bank == 0 {
                // `LoadTilesetPalette` forces the backdrop to black.
                colors[0] = 0;
            }
            f.cmds.push(Cmd::BgPalette {
                bank: bank as u8,
                colors,
            });
        }
    }
}

fn upload_secondary(ts: &'static Tileset, f: &mut Frame) {
    upload_tiles(ts, TILES_IN_PRIMARY, f);
    for bank in PALS_IN_PRIMARY..PALS_TOTAL {
        if let Some(&colors) = ts.palettes.get(bank) {
            f.cmds.push(Cmd::BgPalette {
                bank: bank as u8,
                colors,
            });
        }
    }
}

fn upload_tiles(ts: &'static Tileset, first: u16, f: &mut Frame) {
    let n = ts.tiles.len().min(TILES_IN_PRIMARY as usize);
    if n > 0 {
        f.cmds.push(Cmd::BgTiles {
            char_block: layout::FIELD_CHAR_BLOCK,
            first,
            tiles: &ts.tiles[..n],
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tile;
    use crate::data::*;
    use crate::script::{Ctx, Script};

    const N_MT: usize = 512;

    /// Metatile k's entry i is `base + 8k + i`; layer type cycles Normal, Covered, Split.
    const fn metatiles(base: u16) -> [[u16; 8]; N_MT] {
        let mut out = [[0; 8]; N_MT];
        let mut k = 0;
        while k < N_MT {
            let mut i = 0;
            while i < 8 {
                out[k][i] = base + (8 * k + i) as u16;
                i += 1;
            }
            k += 1;
        }
        out
    }

    const fn attributes() -> [u16; N_MT] {
        let mut out = [0; N_MT];
        let mut k = 0;
        while k < N_MT {
            out[k] = ((k % 3) as u16) << 12;
            k += 1;
        }
        out
    }

    const fn palettes(tag: u16) -> [[u16; 16]; 16] {
        let mut out = [[0; 16]; 16];
        let mut b = 0;
        while b < 16 {
            let mut c = 0;
            while c < 16 {
                out[b][c] = tag + (b * 16 + c) as u16 + 1;
                c += 1;
            }
            b += 1;
        }
        out
    }

    static TILES_P: [Tile; 3] = [[1; 8]; 3];
    static TILES_S1: [Tile; 2] = [[2; 8]; 2];
    static TILES_S2: [Tile; 2] = [[3; 8]; 2];
    static MT_P: [[u16; 8]; N_MT] = metatiles(0);
    static MT_S1: [[u16; 8]; N_MT] = metatiles(0x4000);
    static MT_S2: [[u16; 8]; N_MT] = metatiles(0x8000);
    static ATTR: [u16; N_MT] = attributes();
    static PAL_P: [[u16; 16]; 16] = palettes(0x1000);
    static PAL_S1: [[u16; 16]; 16] = palettes(0x2000);
    static PAL_S2: [[u16; 16]; 16] = palettes(0x3000);

    static PRIMARY: Tileset = Tileset {
        tiles: &TILES_P,
        palettes: &PAL_P,
        metatiles: &MT_P,
        attributes: &ATTR,
    };
    static SECONDARY_1: Tileset = Tileset {
        tiles: &TILES_S1,
        palettes: &PAL_S1,
        metatiles: &MT_S1,
        attributes: &ATTR,
    };
    static SECONDARY_2: Tileset = Tileset {
        tiles: &TILES_S2,
        palettes: &PAL_S2,
        metatiles: &MT_S2,
        attributes: &ATTR,
    };

    const fn blocks<const N: usize>(base: u16) -> [u16; N] {
        let mut out = [0; N];
        let mut i = 0;
        while i < N {
            out[i] = base + (i % 500) as u16;
            i += 1;
        }
        out
    }

    // A: 40x30, primary metatiles; B: 30x20 north of A (offset 3), other secondary, secondary
    // metatiles; C: 10x30 east of A, same tilesets as A.
    const A: MapId = MapId(0);
    const B: MapId = MapId(1);
    const C: MapId = MapId(2);
    static BLOCKS_A: [u16; 40 * 30] = blocks(0);
    static BLOCKS_B: [u16; 30 * 20] = blocks(512);
    static BLOCKS_C: [u16; 10 * 30] = blocks(100);

    const fn map(
        width: u16,
        height: u16,
        blocks: &'static [u16],
        secondary: &'static Tileset,
        connections: &'static [Connection],
    ) -> MapData {
        MapData {
            name: "",
            width,
            height,
            blocks,
            border: [400, 401, 402, 403],
            primary: &PRIMARY,
            secondary,
            objects: &[],
            warps: &[],
            coords: &[],
            bgs: &[],
            connections,
            scripts: MapScripts {
                on_load: None,
                on_transition: None,
                on_resume: None,
                on_return_to_field: None,
                on_frame: &[],
                on_warp_into: &[],
            },
            allow_running: true,
        }
    }

    // D: 4x4, a short tileset (4 metatiles, 2 attributes, 3 palettes, no tiles) in both slots.
    const D: MapId = MapId(3);
    static SHORT_PAL: [[u16; 16]; 3] = {
        let p = palettes(0x1000);
        [p[0], p[1], p[2]]
    };
    static SHORT_MT: [[u16; 8]; 4] = {
        let m = metatiles(0);
        [m[0], m[1], m[2], m[3]]
    };
    static SHORT: Tileset = Tileset {
        tiles: &[],
        palettes: &SHORT_PAL,
        metatiles: &SHORT_MT,
        attributes: &[0, 1 << 12],
    };
    static BLOCKS_D: [u16; 16] = [0, 1, 2, 3, 10, 600, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

    static MAPS: [MapData; 4] = [
        map(
            40,
            30,
            &BLOCKS_A,
            &SECONDARY_1,
            &[
                Connection {
                    dir: Dir::Up,
                    offset: 3,
                    map: B,
                },
                Connection {
                    dir: Dir::Right,
                    offset: 0,
                    map: C,
                },
            ],
        ),
        map(
            30,
            20,
            &BLOCKS_B,
            &SECONDARY_2,
            &[Connection {
                dir: Dir::Down,
                offset: -3,
                map: A,
            }],
        ),
        map(
            10,
            30,
            &BLOCKS_C,
            &SECONDARY_1,
            &[Connection {
                dir: Dir::Left,
                offset: 0,
                map: A,
            }],
        ),
        {
            let mut d = map(4, 4, &BLOCKS_D, &SHORT, &[]);
            d.primary = &SHORT;
            d
        },
    ];

    struct Game;

    impl GameData for Game {
        fn map(&self, id: MapId) -> &'static MapData {
            &MAPS[id.0 as usize]
        }
        fn sprite(&self, _: GfxId) -> Option<&'static SpriteData> {
            None
        }
        fn player_sprite(&self) -> &'static SpriteData {
            unimplemented!()
        }
        fn font(&self) -> &'static Font {
            unimplemented!()
        }
        fn window(&self) -> &'static WindowGfx {
            unimplemented!()
        }
        fn text(&self, _: TextId) -> &'static [u8] {
            &[0xFF]
        }
        fn ui_text(&self, _: UiText) -> &'static [u8] {
            &[0xFF]
        }
        fn behavior(&self, _: u8) -> Behavior {
            Behavior::Normal
        }
        fn script(&self, _: ScriptId, _: Ctx) -> Script {
            alloc::boxed::Box::pin(async {})
        }
        fn new_game(&self) -> NewGame {
            NewGame {
                map: A,
                x: 0,
                y: 0,
                facing: Dir::Down,
                script: None,
                player_name: &[0xFF],
            }
        }
    }

    /// The three field tilemaps (top, middle, bottom) after applying a frame's cmds.
    #[derive(Clone)]
    struct Vram([[u16; 1024]; 3]);

    impl Vram {
        fn new() -> Self {
            Vram([[0xDEAD; 1024]; 3])
        }

        fn apply(&mut self, f: &Frame) {
            for c in &f.cmds {
                if let Cmd::MapEntries {
                    screen_block,
                    start,
                    entries,
                } = c
                {
                    let i = FIELD_BGS
                        .iter()
                        .position(|&bg| layout::SCREEN_BLOCKS[bg as usize] == *screen_block)
                        .unwrap();
                    let s = *start as usize;
                    self.0[i][s..s + entries.len()].copy_from_slice(entries);
                }
            }
        }

        /// Asserts every metatile intersecting the view shows the right entries at its ring
        /// slot, and that the scroll matches the camera.
        fn check_view(&self, field: &Field, f: &Frame) {
            let (cx, cy) = field.camera();
            let (ox, oy) = field.ring;
            for bg in FIELD_BGS {
                assert_eq!(
                    f.scroll[bg as usize],
                    (
                        (cx + 16 * ox as i32).rem_euclid(256) as u16,
                        (cy + 16 * oy as i32).rem_euclid(256) as u16
                    )
                );
            }
            let map = Game.map(field.map());
            let v = Rect::view((cx, cy));
            for y in v.y0..=v.y1 {
                for x in v.x0..=v.x1 {
                    let want = metatile_layers(map, field.block(&Game, x, y));
                    let (col, row) = (
                        2 * (x + ox).rem_euclid(16) as usize,
                        2 * (y + oy).rem_euclid(16) as usize,
                    );
                    for (l, layer) in want.iter().enumerate() {
                        for (k, &e) in layer.iter().enumerate() {
                            let at = (row + k / 2) * 32 + col + k % 2;
                            assert_eq!(self.0[l][at], e, "cell ({x}, {y}) layer {l} entry {k}");
                        }
                    }
                }
            }
        }
    }

    fn map_entries(f: &Frame) -> Vec<(u8, u16, Vec<u16>)> {
        f.cmds
            .iter()
            .filter_map(|c| match c {
                Cmd::MapEntries {
                    screen_block,
                    start,
                    entries,
                } => Some((*screen_block, *start, entries.clone())),
                _ => None,
            })
            .collect()
    }

    fn loaded(camera: (i32, i32)) -> (Field, Vram, Frame) {
        let mut field = Field::new(A);
        let mut f = Frame::default();
        field.load(&Game, A, camera, &mut f);
        let mut vram = Vram::new();
        vram.apply(&f);
        vram.check_view(&field, &f);
        (field, vram, f)
    }

    #[test]
    fn layer_routing_per_layer_type() {
        let a = Game.map(A);
        // Metatile 3 is NORMAL (3 % 3 == 0), 4 COVERED, 5 SPLIT; entries are 8k + i.
        let t = |k: u16, i: u16| 8 * k + i;
        assert_eq!(
            metatile_layers(a, 3),
            [
                [t(3, 4), t(3, 5), t(3, 6), t(3, 7)],
                [t(3, 0), t(3, 1), t(3, 2), t(3, 3)],
                [0x3014; 4],
            ]
        );
        assert_eq!(
            metatile_layers(a, 4),
            [
                [0; 4],
                [t(4, 4), t(4, 5), t(4, 6), t(4, 7)],
                [t(4, 0), t(4, 1), t(4, 2), t(4, 3)],
            ]
        );
        assert_eq!(
            metatile_layers(a, 5 | 0xF400),
            [
                [t(5, 4), t(5, 5), t(5, 6), t(5, 7)],
                [0; 4],
                [t(5, 0), t(5, 1), t(5, 2), t(5, 3)],
            ],
            "collision/elevation bits are ignored"
        );
        // Ids >= 512 read the secondary tileset.
        // 513 is secondary metatile 1, COVERED: its top layer goes to the middle BG.
        assert_eq!(metatile_layers(a, 512 + 1)[1][0], 0x4000 + 8 + 4);
    }

    #[test]
    fn load_uploads_tilesets_palettes_and_controls() {
        let (_, _, f) = loaded((0, 0));
        let bgs: Vec<_> = f
            .cmds
            .iter()
            .filter_map(|c| match c {
                Cmd::Bg { layer, control } => Some((*layer, *control)),
                _ => None,
            })
            .collect();
        assert_eq!(bgs.len(), 3);
        for (layer, c) in bgs {
            assert!(c.enabled);
            assert_eq!(c.priority, layer);
            assert_eq!(c.char_block, layout::FIELD_CHAR_BLOCK);
            assert_eq!(c.screen_block, layout::SCREEN_BLOCKS[layer as usize]);
        }
        assert!(f.cmds.contains(&Cmd::BgTiles {
            char_block: 0,
            first: 0,
            tiles: &TILES_P
        }));
        assert!(f.cmds.contains(&Cmd::BgTiles {
            char_block: 0,
            first: 512,
            tiles: &TILES_S1
        }));
        let pals: Vec<_> = f
            .cmds
            .iter()
            .filter_map(|c| match c {
                Cmd::BgPalette { bank, colors } => Some((*bank, *colors)),
                _ => None,
            })
            .collect();
        assert_eq!(pals.len(), 13);
        let mut p0 = PAL_P[0];
        p0[0] = 0;
        assert_eq!(pals[0], (0, p0), "backdrop forced black");
        assert_eq!(pals[5], (5, PAL_P[5]));
        assert_eq!(pals[6], (6, PAL_S1[6]));
        assert_eq!(pals[12], (12, PAL_S1[12]));
    }

    #[test]
    fn ring_position_and_wrap() {
        let (mut field, mut vram, _) = loaded((20 * 16 + 5, 18 * 16 + 9));
        // Cell (20, 18) sits at entries (2*4, 2*2); cell (35, 28) wraps to (2*3, 2*12).
        let top = |vram: &Vram, x: usize, y: usize| vram.0[0][y * 32 + x];
        let l = metatile_layers(Game.map(A), field.block(&Game, 20, 18));
        assert_eq!(top(&vram, 8, 4), l[0][0]);
        assert_eq!(top(&vram, 9, 5), l[0][3]);
        let l = metatile_layers(Game.map(A), field.block(&Game, 35, 28));
        assert_eq!(top(&vram, 6, 24), l[0][0]);
        // Walk a long way in pixel steps, crossing ring wraps and the map edges.
        for step in 0..600 {
            let mut f = Frame::default();
            let (dx, dy) = match step / 100 {
                0 => (1, 0),
                1 => (0, 1),
                2 => (-1, 0),
                3 => (0, -1),
                4 => (-1, -1),
                _ => (3, 2),
            };
            let (cx, cy) = field.camera();
            field.set_camera(&Game, (cx + dx * 4, cy + dy * 4), &mut f);
            vram.apply(&f);
            vram.check_view(&field, &f);
        }
    }

    #[test]
    fn one_metatile_move_draws_only_the_new_column_or_row() {
        let (mut field, _, _) = loaded((0, 0));
        // Right by one metatile: view 16..255 shows columns 1..=15; column 15, rows 0..=9 is new.
        let mut f = Frame::default();
        field.set_camera(&Game, (16, 0), &mut f);
        let mut want = Vec::new();
        for (i, bg) in FIELD_BGS.into_iter().enumerate() {
            for row in 0..20u16 {
                let l = metatile_layers(Game.map(A), field.block(&Game, 15, (row / 2) as i16));
                let k = (row % 2) as usize * 2;
                want.push((
                    layout::SCREEN_BLOCKS[bg as usize],
                    row * 32 + 30,
                    alloc::vec![l[i][k], l[i][k + 1]],
                ));
            }
        }
        assert_eq!(map_entries(&f), want);
        assert_eq!(f.cmds.len(), want.len(), "nothing but the new column");
        assert_eq!(f.scroll[1], (16, 0));

        // Down by one metatile: row 10, columns 1..=15 -> one run of 30 entries per tilemap row.
        let mut f = Frame::default();
        field.set_camera(&Game, (16, 16), &mut f);
        let e = map_entries(&f);
        assert_eq!(e.len(), 6);
        for (n, (sb, start, entries)) in e.iter().enumerate() {
            assert_eq!(*sb, layout::SCREEN_BLOCKS[FIELD_BGS[n / 2] as usize]);
            assert_eq!(*start, (20 + n as u16 % 2) * 32 + 2);
            assert_eq!(entries.len(), 30);
        }

        // Pixel moves within the drawn cells draw nothing but still scroll.
        field.set_camera(&Game, (24, 16), &mut Frame::default());
        let mut f = Frame::default();
        field.set_camera(&Game, (31, 16), &mut f);
        assert!(f.cmds.is_empty());
        assert_eq!(f.scroll[3], (31, 16));
    }

    #[test]
    fn connections_and_border_drawn_with_current_tilesets() {
        // Top-left corner of A: north rows come from B, the corner and west from the border.
        let (field, vram, _) = loaded((-48, -48));
        let a = Game.map(A);
        // (0, -1) is B's (0 - 3, 19): x < 0 there, so outside B -> border.
        assert_eq!(field.block(&Game, 0, -1), 402 | 0xC00);
        // (5, -1) is B's (2, 19), a secondary block drawn with A's secondary tileset.
        let b = field.block(&Game, 5, -1);
        assert_eq!(b, BLOCKS_B[19 * 30 + 2]);
        let l = metatile_layers(a, b);
        assert!(l.iter().flatten().any(|&e| e & 0xC000 == 0x4000));
        let slot =
            |x: i16, y: i16| (2 * y.rem_euclid(16) as usize) * 32 + 2 * x.rem_euclid(16) as usize;
        assert_eq!(vram.0[1][slot(5, -1)], l[1][0]);
        // Border at (-1, -1): border[(x & 1) + 2 (y & 1)] = border[3].
        let l = metatile_layers(a, 403);
        assert_eq!(vram.0[1][slot(-1, -1)], l[1][0]);
        // East edge: C continues A's rows.
        let (field, _, _) = loaded((35 * 16, 10 * 16));
        assert_eq!(field.block(&Game, 40, 12), BLOCKS_C[12 * 10]);
    }

    #[test]
    fn switch_map_shifts_coordinates_and_reuploads_only_what_differs() {
        // Player near A's top edge at (10, 0); camera puts that cell at screen (112, 64).
        let cam = (10 * 16 - 112, -64);
        let (mut field, mut vram, _) = loaded(cam);
        let before: Vec<_> = (-4..6)
            .flat_map(|y| (3..18).map(move |x| (x, y)))
            .map(|(x, y)| field.block(&Game, x, y))
            .collect();
        // The player steps up to (10, -1).
        let (to, shift) = field.crossing(&Game, 10, -1).unwrap();
        assert_eq!((to, shift), (B, (-3, 20)));
        assert_eq!(field.crossing(&Game, 10, 5), None);
        let mut f = Frame::default();
        field.switch_map(&Game, to, shift, &mut f);
        assert_eq!(field.map(), B);
        assert_eq!(field.camera(), (cam.0 - 3 * 16, cam.1 + 20 * 16));
        // B's own cells and A's cells through B's connection are the same blocks as before.
        let after: Vec<_> = (-4..6)
            .flat_map(|y| (3..18).map(move |x| (x, y)))
            .map(|(x, y)| field.block(&Game, x + shift.0, y + shift.1))
            .collect();
        assert_eq!(before, after);
        // Only the secondary tileset differs.
        assert!(f.cmds.contains(&Cmd::BgTiles {
            char_block: 0,
            first: 512,
            tiles: &TILES_S2
        }));
        let banks: Vec<u8> = f
            .cmds
            .iter()
            .filter_map(|c| match c {
                Cmd::BgPalette { bank, .. } => Some(*bank),
                Cmd::BgTiles { first: 0, .. } | Cmd::Bg { .. } => panic!("primary re-uploaded"),
                _ => None,
            })
            .collect();
        assert_eq!(banks, (6..13).collect::<Vec<u8>>());
        vram.apply(&f);
        vram.check_view(&field, &f);

        // A -> C shares both tilesets: no uploads.
        let (mut field, _, _) = loaded((39 * 16 - 112, 10 * 16 - 64));
        let (to, shift) = field.crossing(&Game, 40, 10).unwrap();
        assert_eq!((to, shift), (C, (-40, 0)));
        let mut f = Frame::default();
        field.switch_map(&Game, to, shift, &mut f);
        assert!(f.cmds.iter().all(|c| matches!(c, Cmd::MapEntries { .. })));
    }

    #[test]
    fn set_block_overrides_and_redraws_in_view() {
        let (mut field, mut vram, _) = loaded((0, 0));
        let mut f = Frame::default();
        field.set_block(&Game, 3, 2, 7 | 0x400, &mut f);
        assert_eq!(field.block(&Game, 3, 2), 7 | 0x400);
        // One metatile: 2 runs of 2 entries per BG.
        assert_eq!(map_entries(&f).len(), 6);
        vram.apply(&f);
        vram.check_view(&field, &f);
        assert_eq!(vram.0[1][4 * 32 + 6], 8 * 7 + 4, "metatile 7 is COVERED");

        // Out of view: stored, not drawn; drawn once it scrolls in.
        let mut f = Frame::default();
        field.set_block(&Game, 30, 25, 9, &mut f);
        assert!(f.cmds.is_empty());
        assert_eq!(field.block(&Game, 30, 25), 9);
        field.set_camera(&Game, (25 * 16, 20 * 16), &mut f);
        vram.apply(&f);
        vram.check_view(&field, &f);

        // Outside the map: ignored.
        let mut f = Frame::default();
        field.set_block(&Game, -1, 0, 9, &mut f);
        assert_ne!(field.block(&Game, -1, 0), 9);
        assert!(f.cmds.is_empty());

        // A reload clears overrides.
        field.load(&Game, A, (0, 0), &mut f);
        assert_eq!(field.block(&Game, 3, 2), BLOCKS_A[2 * 40 + 3]);
    }

    #[test]
    fn redraw_draws_the_whole_view_again() {
        let (mut field, _, first) = loaded((8, 8));
        let mut f = Frame::default();
        field.redraw(&Game, &mut f);
        assert_eq!(map_entries(&f), map_entries(&first));
    }

    #[test]
    fn hash_covers_camera_and_overrides() {
        let digest = |field: &Field| {
            let mut h = Fnv64::new();
            field.hash(&mut h);
            h.finish()
        };
        let (mut field, _, _) = loaded((0, 0));
        let h0 = digest(&field);
        let mut f = Frame::default();
        field.set_camera(&Game, (1, 0), &mut f);
        let h1 = digest(&field);
        assert_ne!(h0, h1);
        field.set_block(&Game, 0, 0, 5, &mut f);
        assert_ne!(digest(&field), h1);
    }

    #[test]
    fn enter_then_set_block_is_drawn_by_the_first_redraw() {
        let mut field = Field::new(A);
        let mut f = Frame::default();
        field.enter(&Game, A, (0, 0), &mut f);
        assert!(map_entries(&f).is_empty(), "enter draws nothing");
        field.set_block(&Game, 3, 2, 7, &mut f);
        assert!(
            map_entries(&f).is_empty(),
            "only recorded before the first draw"
        );
        field.redraw(&Game, &mut f);
        let mut vram = Vram::new();
        vram.apply(&f);
        vram.check_view(&field, &f);
        assert_eq!(field.block(&Game, 3, 2), 7);
        assert_eq!(vram.0[1][4 * 32 + 6], 8 * 7 + 4, "metatile 7 is COVERED");
        // Each entry of the 15x10-metatile view is written exactly once per BG.
        let total: usize = map_entries(&f).iter().map(|e| e.2.len()).sum();
        assert_eq!(total, 3 * 15 * 10 * 4);

        // A connection switch keeps the view drawn (A -> C share tilesets: nothing is emitted),
        // so an on-load `set_block` in view is drawn at once.
        let (mut field, mut vram, _) = loaded((39 * 16 - 112, 10 * 16 - 64));
        let (to, shift) = field.crossing(&Game, 40, 10).unwrap();
        let mut f = Frame::default();
        field.switch_map_enter(&Game, to, shift, &mut f);
        assert!(f.cmds.is_empty());
        field.set_block(&Game, 0, 10, 8, &mut f);
        assert_eq!(map_entries(&f).len(), 6);
        let cam = field.camera();
        field.set_camera(&Game, cam, &mut f);
        assert_eq!(map_entries(&f).len(), 6, "nothing else is drawn");
        vram.apply(&f);
        vram.check_view(&field, &f);
        assert_eq!(field.block(&Game, 0, 10), 8);
    }

    #[test]
    fn switch_map_clears_overrides() {
        let (mut field, _, _) = loaded((39 * 16 - 112, 10 * 16 - 64));
        let mut f = Frame::default();
        field.set_block(&Game, 39, 10, 9, &mut f);
        field.set_block(&Game, 0, 0, 9, &mut f);
        let (to, shift) = field.crossing(&Game, 40, 10).unwrap();
        field.switch_map(&Game, to, shift, &mut f);
        // A's (39, 10) is C's (-1, 10), seen through C's west connection: the original block.
        assert_eq!(
            field.block(&Game, 39 + shift.0, 10 + shift.1),
            BLOCKS_A[10 * 40 + 39]
        );
        assert!(field.overrides.is_empty(), "no overrides left");
    }

    #[test]
    fn crossing_keeps_the_view_drawn() {
        // A -> C (shared tilesets): walking on draws only the new column, at the shifted ring.
        let (mut field, mut vram, _) = loaded((39 * 16 - 112, 10 * 16 - 64));
        let (to, shift) = field.crossing(&Game, 40, 10).unwrap();
        let mut f = Frame::default();
        field.switch_map_enter(&Game, to, shift, &mut f);
        let cam = field.camera();
        field.set_camera(&Game, cam, &mut f);
        assert!(map_entries(&f).is_empty(), "no redraw");
        vram.apply(&f);
        vram.check_view(&field, &f);
        let mut f = Frame::default();
        field.set_camera(&Game, (cam.0 + 16, cam.1), &mut f);
        let total: usize = map_entries(&f).iter().map(|e| e.2.len()).sum();
        assert_eq!(total, 3 * 4 * 10, "one new column of 10 metatiles");
        vram.apply(&f);
        vram.check_view(&field, &f);
    }

    #[test]
    fn missing_metatiles_attributes_and_palettes() {
        let d = Game.map(D);
        let bottom = |k: u16| [8 * k, 8 * k + 1, 8 * k + 2, 8 * k + 3];
        let top = |k: u16| [8 * k + 4, 8 * k + 5, 8 * k + 6, 8 * k + 7];
        // Metatile 1 has its attribute (COVERED); 3 has none (NORMAL); 10 and 600 do not exist.
        assert_eq!(metatile_layers(d, 1), [[0; 4], top(1), bottom(1)]);
        assert_eq!(
            metatile_layers(d, 3),
            [top(3), bottom(3), [NORMAL_UNDERLAY; 4]]
        );
        for missing in [10, 600] {
            assert_eq!(
                metatile_layers(d, missing),
                [[0; 4], [0; 4], [NORMAL_UNDERLAY; 4]]
            );
        }

        let mut field = Field::new(D);
        let mut f = Frame::default();
        field.load(&Game, D, (-112, -64), &mut f);
        assert!(!f.cmds.iter().any(|c| matches!(c, Cmd::BgTiles { .. })));
        let banks: Vec<u8> = f
            .cmds
            .iter()
            .filter_map(|c| match c {
                Cmd::BgPalette { bank, .. } => Some(*bank),
                _ => None,
            })
            .collect();
        assert_eq!(banks, [0, 1, 2], "only the palettes the tileset has");
        let mut vram = Vram::new();
        vram.apply(&f);
        vram.check_view(&field, &f);
    }

    #[test]
    fn switch_map_round_trip_with_offsets() {
        // A -> B (north, offset 3) -> A (south, offset -3): shifts cancel, view stays right.
        let cam = (10 * 16 - 112, -64);
        let (mut field, mut vram, _) = loaded(cam);
        let (to, shift) = field.crossing(&Game, 10, -1).unwrap();
        let mut f = Frame::default();
        field.switch_map(&Game, to, shift, &mut f);
        vram.apply(&f);
        vram.check_view(&field, &f);
        // The player is now at B's (7, 19) and steps back down to (7, 20).
        let (back, shift_back) = field.crossing(&Game, 7, 20).unwrap();
        assert_eq!((back, shift_back), (A, (3, -20)));
        let mut f = Frame::default();
        field.switch_map(&Game, back, shift_back, &mut f);
        assert_eq!((field.map(), field.camera()), (A, cam));
        vram.apply(&f);
        vram.check_view(&field, &f);

        // C -> A (west): shift +40.
        let mut field = Field::new(C);
        let mut f = Frame::default();
        field.load(&Game, C, (-112, 10 * 16 - 64), &mut f);
        let (to, shift) = field.crossing(&Game, -1, 10).unwrap();
        assert_eq!((to, shift), (A, (40, 0)));
        field.switch_map(&Game, to, shift, &mut f);
        assert_eq!(field.camera(), (40 * 16 - 112, 10 * 16 - 64));
    }
}
