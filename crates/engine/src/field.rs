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
        self.x0 <= x && x <= self.x1 && self.y0 <= y && y <= self.y1
    }
}

/// Cells left drawn from the map before a connection crossing ([`Field::stale`]), inclusive.
/// Never set since Milestone 8 Task F (a crossing redraws what differs); kept for the hash and
/// the snapshot format.
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
    /// An upload spread over frames, in progress ([`Field::enter_spread`], a crossing into
    /// other tilesets).
    spread: Option<Spread>,
    /// The current map's layout override (Gen3 `setmaplayoutindex`; `None`: its own).
    layout: Option<u16>,
    /// The layout the next map entered takes ([`Field::set_next_layout`]): Gen3's
    /// `mapLayoutId`, reset to the header's by `LoadCurrentMapData` and set by the new map's
    /// `on_transition` script before `InitMap` loads it.
    next_layout: Option<u16>,
}

/// Tiles a spread upload sends per frame while the screen is black (a warp load: one whole
/// tileset, 16 KiB) and while it shows (a crossing into other tilesets: 4 KiB, a few lines of
/// vblank).
pub const SPREAD_TILES_BLACK: u16 = TILES_IN_PRIMARY;
pub const SPREAD_TILES_VISIBLE: u16 = 128;
/// Metatile rows of the view a spread upload redraws per frame (a whole view is over a
/// vblank's budget).
pub const SPREAD_ROWS: i16 = 3;

/// A field upload spread over frames (Milestone 8 Task F; Gen3 `LoadMapInStepsLocal` for warp
/// loads). Each [`Field::set_camera`] sends the next part: the BG controls and at most
/// `budget` tiles, primary first; once a call has no tiles left to send, the palettes and the
/// view's top [`SPREAD_ROWS`] rows; then the next rows each frame, until the whole view is
/// drawn with them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Spread {
    /// The next view row (map y) to redraw, once the tiles and palettes are in.
    rows: Option<i16>,
    /// BG1–BG3 controls still to send (a warp load).
    controls: bool,
    /// The next tile to send of the primary and the secondary tileset; `None` when sent (or
    /// unchanged).
    primary: Option<u16>,
    secondary: Option<u16>,
    /// Palettes to send at the end: the primary banks, the secondary banks.
    palettes: (bool, bool),
    /// Tiles per frame.
    budget: u16,
}

impl Spread {
    /// A warp load's, on a black screen (as opposed to a crossing's, while the screen shows).
    fn black(&self) -> bool {
        self.budget == SPREAD_TILES_BLACK
    }

    fn hash(&self, h: &mut Fnv64) {
        let next = |n: Option<u16>| n.map_or(0xFFFF, |n| n);
        h.write(&[
            0x5D,
            self.controls as u8,
            self.palettes.0 as u8,
            self.palettes.1 as u8,
            self.rows.is_some() as u8,
        ]);
        h.write_u16(next(self.primary));
        h.write_u16(next(self.secondary));
        h.write_u16(self.budget);
        h.write_u16(self.rows.unwrap_or(0) as u16);
    }
}

impl Field {
    pub fn new(map: MapId) -> Self {
        Field {
            map,
            camera_px: (0, 0),
            drawn: None,
            overrides: BTreeMap::new(),
            ring: (0, 0),
            spread: None,
            layout: None,
            next_layout: None,
        }
    }

    /// The current map's data, with its layout override if any.
    pub fn data(&self, game: &impl GameData) -> &'static MapData {
        Self::data_of(game, self.map, self.layout)
    }

    fn data_of(game: &impl GameData, map: MapId, layout: Option<u16>) -> &'static MapData {
        (layout.and_then(|l| game.map_layout(map, l))).unwrap_or_else(|| game.map(map))
    }

    /// Sets the layout of the next map entered (`None`: its own; Gen3 `setmaplayoutindex`).
    pub fn set_next_layout(&mut self, layout: Option<u16>) {
        self.next_layout = layout;
    }

    /// Drops a next layout that map `to` does not have as a variant
    /// ([`GameData::map_layout`]: one equal to its own layout, Route 130's `LAYOUT_ROUTE130`
    /// without Mirage Island, is not one): the map is drawn with its own either way, and
    /// the override, which a checkpoint must name a variant for, would not survive a restore.
    pub fn settle_next_layout(&mut self, game: &impl GameData, to: MapId) {
        if self
            .next_layout
            .is_some_and(|l| game.map_layout(to, l).is_none())
        {
            self.next_layout = None;
        }
    }

    /// The current map's layout override.
    pub fn layout(&self) -> Option<u16> {
        self.layout
    }

    /// Whether an upload spread over frames is still in progress: the view is not all drawn
    /// with the current map's tilesets yet.
    pub fn uploading(&self) -> bool {
        self.spread.is_some()
    }

    pub fn map(&self) -> MapId {
        self.map
    }

    /// The cells the view still shows from the map before a connection crossing: always `None`
    /// since Milestone 8 Task F, since a crossing redraws the kept cells the new map draws
    /// differently ([`Field::switch_map_enter`]). Kept for `World::hash` and the snapshot's
    /// `stale`.
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
        self.enter_spread(map, camera_px);
        self.spread = None;
        field_controls(f);
        let m = self.data(game);
        upload_primary(m.primary, f);
        upload_secondary(m.secondary, f);
    }

    /// [`Field::enter`] with the uploads spread over the next [`Field::set_camera`]s, for a
    /// warp load while the screen is black (Gen3 `LoadMapInStepsLocal`): the controls and the
    /// primary tiles, then the secondary tiles, then the palettes and the view, [`SPREAD_ROWS`]
    /// rows a frame. Nothing is drawn before. [`Field::uploading`] until done.
    pub fn enter_spread(&mut self, map: MapId, camera_px: (i32, i32)) {
        self.map = map;
        self.layout = self.next_layout;
        self.overrides.clear();
        self.camera_px = camera_px;
        self.drawn = None;
        self.ring = (0, 0);
        self.spread = Some(Spread {
            controls: true,
            primary: Some(0),
            secondary: Some(0),
            palettes: (true, true),
            budget: SPREAD_TILES_BLACK,
            rows: None,
        });
    }

    /// Sends the next uploads of the spread `s` (see [`Spread`]): the controls and tiles, or once
    /// they are all in, the palettes, starting the redraw at the top row of `view`.
    fn send_part(&self, game: &impl GameData, mut s: Spread, view: Rect, f: &mut Frame) -> Spread {
        if s.rows.is_some() {
            return s;
        }
        let m = self.data(game);
        if s.controls {
            field_controls(f);
            s.controls = false;
        }
        let mut left = s.budget;
        let mut sent = false;
        for (next, ts, first) in [
            (&mut s.primary, m.primary, 0),
            (&mut s.secondary, m.secondary, TILES_IN_PRIMARY),
        ] {
            let Some(at) = *next else { continue };
            if left == 0 {
                break;
            }
            let end = tile_count(ts).min(at.saturating_add(left));
            if end > at {
                f.cmds.push(Cmd::BgTiles {
                    char_block: layout::FIELD_CHAR_BLOCK,
                    first: first + at,
                    tiles: &ts.tiles[at as usize..end as usize],
                });
                left -= end - at;
                sent = true;
            }
            *next = (end < tile_count(ts)).then_some(end);
        }
        if sent || s.primary.is_some() || s.secondary.is_some() {
            return s;
        }
        if s.palettes.0 {
            primary_palettes(m.primary, f);
        }
        if s.palettes.1 {
            secondary_palettes(m.secondary, f);
        }
        s.palettes = (false, false);
        s.rows = Some(view.y0);
        s
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
        // The map keeps its layout; a pending one still waits for the next map.
        let next = core::mem::replace(&mut self.next_layout, self.layout);
        self.enter(game, self.map, camera, f);
        self.next_layout = next;
        self.ring = ring;
        self.overrides = overrides;
    }

    /// [`Field::restore`] with the uploads spread over the next [`Field::set_camera`]s, as
    /// [`Field::enter_spread`]: for a caller that keeps the screen black while
    /// [`Field::uploading`] (a battle's return to the field; Milestone 8 Task F).
    pub fn restore_spread(&mut self) {
        let (camera, ring, overrides) = (
            self.camera_px,
            self.ring,
            core::mem::take(&mut self.overrides),
        );
        let next = core::mem::replace(&mut self.next_layout, self.layout);
        self.enter_spread(self.map, camera);
        self.next_layout = next;
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
        if let Some(s) = self.spread {
            let s = self.send_part(game, s, view, f);
            self.spread = Some(s);
            if self.drawn.is_none() && s.black() {
                if s.rows.is_none() {
                    // A warp load: nothing is drawn before its tiles are in.
                    f.bulk = true;
                    self.set_scroll(f);
                    return;
                }
                // Its rows are drawn below, a few per frame.
                self.drawn = Some(view);
            }
        }
        let old = self.drawn;
        let mut cells = Vec::new();
        // A camera still in the drawn view (most frames) brings no cell into view.
        if old != Some(view) {
            for y in view.y0..=view.y1 {
                for x in view.x0..=view.x1 {
                    if !old.is_some_and(|r| r.contains(x, y)) {
                        cells.push((x, y));
                    }
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
        if let Some(s) = self.spread
            && let Some(y) = s.rows
        {
            // The view's next rows, drawn with the new tiles and palettes.
            let end = y.saturating_add(SPREAD_ROWS).min(view.y1 + 1);
            let cells: Vec<(i16, i16)> = (y.max(view.y0)..end)
                .flat_map(|y| (view.x0..=view.x1).map(move |x| (x, y)))
                .collect();
            self.draw_cells(game, &cells, f);
            f.bulk |= s.black();
            self.spread = (end <= view.y1).then_some(Spread {
                rows: Some(end),
                ..s
            });
        }
        self.set_scroll(f);
    }

    /// The block at (x, y): overrides, then the map, its connections and its border.
    pub fn block(&self, game: &impl GameData, x: i16, y: i16) -> u16 {
        match self.overrides.get(&(x, y)) {
            Some(&b) => b,
            None => grid::block_at(game, self.data(game), x, y),
        }
    }

    /// Replaces the block of a cell of the current map (Gen3 `setmetatile`) and redraws it if it
    /// is in view (before the first draw after `enter`, it is only recorded). Cells outside the
    /// map are ignored. Overrides last until the next `load` or
    /// `switch_map`.
    pub fn set_block(&mut self, game: &impl GameData, x: i16, y: i16, block: u16, f: &mut Frame) {
        if !grid::in_bounds(self.data(game), x, y) {
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
        let n = grid::neighbor(game, self.data(game), x, y)?;
        Some((n.map, (n.x - x, n.y - y)))
    }

    /// Seamless switch to a connected map: the caller has moved the player across the edge. A
    /// cell at old coordinates (x, y) has new coordinates (x + shift.0, y + shift.1); the camera
    /// and the ring origin move with it, so the cells already drawn stay drawn. Clears overrides.
    /// Follow with on-load `set_block`s, then [`Field::set_camera`].
    ///
    /// Gen3 keeps every drawn entry, so the old map's border and `setmetatile` overrides stay on
    /// screen until they scroll out. Here the kept cells whose metatile differs from what the
    /// new map draws there are redrawn at once (Milestone 8 Task F): the view is then always the
    /// current map's, so a crossing frame is capturable and a restore redraws the same picture.
    ///
    /// Into other tilesets, the tilesets that differ are uploaded over the next frames,
    /// [`SPREAD_TILES_VISIBLE`] tiles per frame, then their palettes, and the view is redrawn
    /// with them, [`SPREAD_ROWS`] rows per frame ([`Field::uploading`] until then; cells coming
    /// into view meanwhile are drawn as usual, and the rows not redrawn yet show the old map's
    /// entries over the new tiles for those few frames). Gen3 uploads the secondary tileset at
    /// once (`CopySecondaryTilesetToVramUsingHeap`), over a vblank's budget here, and keeps
    /// the old entries until they scroll out.
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
        let (old, new) = (self.data(game), Self::data_of(game, to, self.next_layout));
        let primary = !core::ptr::eq(old.primary, new.primary);
        let secondary = !core::ptr::eq(old.secondary, new.secondary);
        // Without a spread (which redraws the whole view), the kept cells that differ.
        let differ = match self.drawn {
            Some(r) if !primary && !secondary && self.spread.is_none() => {
                self.kept_cells_to_redraw(game, to, shift, r)
            }
            _ => Vec::new(),
        };
        // A tileset still uploading restarts from its first tile.
        let pending = self.spread.map_or((false, false), |s| {
            (
                s.primary.is_some() || s.palettes.0,
                s.secondary.is_some() || s.palettes.1,
            )
        });
        let (primary, secondary) = (primary || pending.0, secondary || pending.1);
        if primary || secondary || self.spread.is_some() {
            self.spread = Some(Spread {
                controls: self.spread.is_some_and(|s| s.controls),
                primary: primary.then_some(0),
                secondary: secondary.then_some(0),
                palettes: (primary, secondary),
                budget: SPREAD_TILES_VISIBLE,
                rows: None,
            });
        }
        self.map = to;
        self.layout = self.next_layout;
        self.overrides.clear();
        self.camera_px.0 += shift.0 as i32 * 16;
        self.camera_px.1 += shift.1 as i32 * 16;
        self.ring = (
            (self.ring.0 - shift.0).rem_euclid(RING),
            (self.ring.1 - shift.1).rem_euclid(RING),
        );
        self.drawn = self.drawn.map(|r| Rect {
            x0: r.x0 + shift.0,
            y0: r.y0 + shift.1,
            x1: r.x1 + shift.0,
            y1: r.y1 + shift.1,
        });
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
        // Only while an upload is spread, so the hash is unchanged otherwise.
        if let Some(s) = &self.spread {
            s.hash(h);
        }
        // The layout override (checkpoint schema 6), only when there is one, so the hash of
        // a map on its own layout is unchanged.
        if let Some(l) = self.layout {
            h.write(&[0x4C]);
            h.write_u16(l);
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

    /// The cells of `kept` (drawn from the current map, old coordinates) that map `to` draws
    /// with another metatile, in new coordinates. Before the switch: the current map is still
    /// the old one, its overrides included. Only cells off the [`grid::Seam`] are compared, plus
    /// the overrides on it, so a usual crossing compares a handful of cells.
    fn kept_cells_to_redraw(
        &self,
        game: &impl GameData,
        to: MapId,
        shift: (i16, i16),
        kept: Rect,
    ) -> Vec<(i16, i16)> {
        let new = Self::data_of(game, to, self.next_layout);
        let mut differ = Vec::new();
        let mut check = |x: i16, y: i16| {
            let (nx, ny) = (x + shift.0, y + shift.1);
            let drawn = block_metatile(self.block(game, x, y));
            if drawn != block_metatile(grid::block_at(game, new, nx, ny)) {
                differ.push((nx, ny));
            }
        };
        let Some(seam) = grid::seam(game, self.map, to, shift) else {
            for y in kept.y0..=kept.y1 {
                for x in kept.x0..=kept.x1 {
                    check(x, y);
                }
            }
            return differ;
        };
        let on_seam = |x: i16, y: i16| {
            let (a, b) = if seam.vertical { (x, y) } else { (y, x) };
            (seam.along.0..seam.along.1).contains(&a) && (seam.across.0..seam.across.1).contains(&b)
        };
        for y in kept.y0..=kept.y1 {
            // This row's cells on the seam (half-open, maybe empty), skipped.
            let (s0, s1) = match seam.vertical {
                true if (seam.across.0..seam.across.1).contains(&y) => seam.along,
                false if (seam.along.0..seam.along.1).contains(&y) => seam.across,
                _ => (0, 0),
            };
            let (s0, s1) = if s0 < s1 { (s0, s1) } else { (0, 0) };
            for x in (kept.x0..s0.min(kept.x1 + 1)).chain(s1.max(kept.x0)..=kept.x1) {
                check(x, y);
            }
        }
        for &(x, y) in self.overrides.keys() {
            if kept.contains(x, y) && on_seam(x, y) {
                check(x, y);
            }
        }
        differ
    }

    /// Draws `cells` (at most one per ring slot) into the ring: per BG (top, middle, bottom), one
    /// `MapEntries` per contiguous run of entries, in tilemap order.
    fn draw_cells(&self, game: &impl GameData, cells: &[(i16, i16)], f: &mut Frame) {
        if cells.is_empty() {
            return;
        }
        let map = self.data(game);
        // (tilemap position, entry per BG), in tilemap order within each run of cells on one
        // row (the callers give rows left to right): the cells' top tilemap row, then their
        // bottom one. No sort: on the GBA, sorting and growing vectors cost most of a draw.
        let mut entries: Vec<(u16, [u16; 3])> = alloc::vec![(0, [0; 3]); cells.len() * 4];
        let mut base = 0;
        for group in cells.chunk_by(|a, b| a.1 == b.1) {
            let n = group.len();
            for (i, &(x, y)) in group.iter().enumerate() {
                let [top, middle, bottom] = metatile_layers(map, self.block(game, x, y));
                let col = 2 * ((x + self.ring.0) & (RING - 1)) as u16;
                let row = 2 * ((y + self.ring.1) & (RING - 1)) as u16;
                for k in 0..4 {
                    let at = base + (k / 2) * 2 * n + 2 * i + k % 2;
                    let pos = (row + k as u16 / 2) * 32 + col + k as u16 % 2;
                    entries[at] = (pos, [top[k], middle[k], bottom[k]]);
                }
            }
            base += 4 * n;
        }
        // Runs contiguous in the tilemap, the same on each BG; each run's entries are allocated
        // once, at their final size.
        let mut runs: Vec<(usize, usize)> = Vec::with_capacity(entries.len());
        let mut start = 0;
        for k in 1..=entries.len() {
            if k == entries.len() || entries[k].0 != entries[k - 1].0 + 1 {
                runs.push((start, k));
                start = k;
            }
        }
        f.cmds.reserve(3 * runs.len());
        for (i, bg) in FIELD_BGS.into_iter().enumerate() {
            let screen_block = layout::SCREEN_BLOCKS[bg as usize];
            for &(a, b) in &runs {
                f.cmds.push(Cmd::MapEntries {
                    screen_block,
                    start: entries[a].0,
                    entries: entries[a..b].iter().map(|e| e.1[i]).collect(),
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
    primary_palettes(ts, f);
}

fn primary_palettes(ts: &'static Tileset, f: &mut Frame) {
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
    secondary_palettes(ts, f);
}

fn secondary_palettes(ts: &'static Tileset, f: &mut Frame) {
    for bank in PALS_IN_PRIMARY..PALS_TOTAL {
        if let Some(&colors) = ts.palettes.get(bank) {
            f.cmds.push(Cmd::BgPalette {
                bank: bank as u8,
                colors,
            });
        }
    }
}

/// The BG1–BG3 controls of the field.
fn field_controls(f: &mut Frame) {
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
}

/// Tiles of `ts` that are uploaded (at most a tileset slot).
fn tile_count(ts: &'static Tileset) -> u16 {
    ts.tiles.len().min(TILES_IN_PRIMARY as usize) as u16
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
