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

mod snapshot;

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

/// Cells left drawn from the map before a connection crossing ([`Field::stale`]), inclusive.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StaleRect {
    pub x0: i16,
    pub y0: i16,
    pub x1: i16,
    pub y1: i16,
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

    /// Whether the view still shows cells drawn from the map before a connection crossing.
    /// Never since Milestone 8 Task F: a crossing redraws the kept cells the new map draws
    /// differently ([`Field::switch_map_enter`]), so the view is always the current map's.
    pub fn view_stale(&self) -> bool {
        false
    }

    /// The cells the view still shows from the map before a connection crossing: always `None`
    /// (see [`Field::view_stale`]). Kept for `World::hash` and the snapshot's `stale`.
    pub fn stale(&self) -> Option<StaleRect> {
        None
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
        // More than a new row and a new column (a scrolling step): a whole-view redraw.
        let edge = (view.x1 - view.x0 + 1) + (view.y1 - view.y0 + 1);
        if cells.len() > edge as usize {
            f.bulk = true;
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
    /// and the ring origin move with it, so the cells already drawn stay drawn. Clears overrides.
    /// Re-uploads a tileset (tiles and palettes) only if it differs; then the view is
    /// invalidated and the next [`Field::set_camera`] redraws it whole. Follow with on-load
    /// `set_block`s, then [`Field::set_camera`].
    ///
    /// Gen3 keeps every drawn entry, so the old map's border and `setmetatile` overrides stay on
    /// screen until they scroll out. Here the kept cells whose metatile differs from what the
    /// new map draws there are redrawn at once (Milestone 8 Task F): the view is then always the
    /// current map's, so a crossing frame is capturable and a restore redraws the same picture.
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
        // The kept cells (new coordinates) the new map draws differently.
        let mut differ = Vec::new();
        if same && let Some(r) = self.drawn {
            for y in r.y0..=r.y1 {
                for x in r.x0..=r.x1 {
                    let (nx, ny) = (x + shift.0, y + shift.1);
                    let drawn = block_metatile(self.block(game, x, y));
                    if drawn != block_metatile(grid::block_at(game, new, nx, ny)) {
                        differ.push((nx, ny));
                    }
                }
            }
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
        self.draw_cells(game, &differ, f);
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
mod tests;
