//! The read-only map view core (M6 Task E): whole-map pictures drawn exactly as the game draws
//! them, per-cell data and event overlays.
//!
//! [`render_map`] does not reimplement metatile drawing: it drives the engine's
//! [`Field`] over a grid of 240x160 camera windows on a fresh native [`Ppu`] and stitches the
//! windows, so tileset uploads, layer routing, the NORMAL underlay, the border and connections
//! are the game's own (decision 9). `tests/map_pixels.rs` checks it against live sessions.

use engine::data::{
    BLOCK_METATILE_MASK, Behavior, BgKind, Dir, FlagId, GameData, Gfx, METATILES_IN_PRIMARY,
    MapData, MapId, MovementType, ScriptId, VarId, attr_behavior, block_collision, block_elevation,
    block_metatile,
};
use engine::field::Field;
use engine::video::{Frame, SCREEN_H, SCREEN_W};
use game_native::Ppu;

/// A whole-map picture: the map's cells plus `margin` cells of border/connections on each side.
/// Map cell (x, y) has its top-left pixel at `((x + margin) * 16, (y + margin) * 16)`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MapImage {
    pub map: MapId,
    /// Cells drawn around the map on each side.
    pub margin: u16,
    /// Pixels: `(width + 2 * margin) * 16`.
    pub width: usize,
    /// Pixels: `(height + 2 * margin) * 16`.
    pub height: usize,
    /// Row-major RGB555, `width * height`.
    pub rgb555: Vec<u16>,
}

impl MapImage {
    /// Image pixel of the map pixel `(x, y)` (cell `(x / 16, y / 16)`).
    pub fn to_image(&self, x: i32, y: i32) -> (i32, i32) {
        let m = self.margin as i32 * 16;
        (x + m, y + m)
    }

    /// The 240x160 window whose top-left map pixel is `camera` (what the screen shows with
    /// that camera), or `None` if it does not fit inside the image.
    pub fn crop(&self, camera: (i32, i32)) -> Option<Vec<u16>> {
        let (x0, y0) = self.to_image(camera.0, camera.1);
        if x0 < 0
            || y0 < 0
            || x0 as usize + SCREEN_W > self.width
            || y0 as usize + SCREEN_H > self.height
        {
            return None;
        }
        let (x0, y0) = (x0 as usize, y0 as usize);
        let mut out = Vec::with_capacity(SCREEN_W * SCREEN_H);
        for y in y0..y0 + SCREEN_H {
            out.extend_from_slice(&self.rgb555[y * self.width + x0..][..SCREEN_W]);
        }
        Some(out)
    }
}

/// The map's BG layers as the game draws them: `Field::enter`, the `overrides` as
/// `Field::set_block` (recorded before the first draw, as on-load `setmetatile`s), then
/// `Field::set_camera` over a grid of 240x160 windows covering the image, each applied to one
/// fresh `Ppu` and rendered (no OBJs, brightness 0, BG0 off), stitched. Margin cells come from
/// connections and the border (`Field::block`). `overrides` are `setmetatile` blocks (a
/// snapshot's `field.overrides`); cells outside the map are ignored, as the game does.
///
/// A map the game does not have (0x0) gives a `2 * margin` square of its border.
pub fn render_map(
    game: &impl GameData,
    map: MapId,
    margin: u16,
    overrides: &[((i16, i16), u16)],
) -> MapImage {
    let m = game.map(map);
    let (cw, ch) = (
        m.width as usize + 2 * margin as usize,
        m.height as usize + 2 * margin as usize,
    );
    let (width, height) = (cw * 16, ch * 16);
    let mut img = MapImage {
        map,
        margin,
        width,
        height,
        rgb555: vec![0; width * height],
    };
    if width == 0 || height == 0 {
        return img;
    }
    let origin = -(margin as i32) * 16;
    let mut field = Field::new(map);
    let mut ppu = Ppu::new();
    let mut f = Frame::default();
    field.enter(game, map, (origin, origin), &mut f);
    for &((x, y), block) in overrides {
        field.set_block(game, x, y, block, &mut f);
    }
    ppu.apply(&f);
    let mut fb = vec![0u16; SCREEN_W * SCREEN_H];
    for wy in (0..height).step_by(SCREEN_H) {
        for wx in (0..width).step_by(SCREEN_W) {
            let mut f = Frame::default();
            field.set_camera(game, (origin + wx as i32, origin + wy as i32), &mut f);
            ppu.apply(&f);
            ppu.render(&f, &mut fb);
            let (w, h) = (SCREEN_W.min(width - wx), SCREEN_H.min(height - wy));
            for row in 0..h {
                let dst = (wy + row) * width + wx;
                img.rgb555[dst..dst + w].copy_from_slice(&fb[row * SCREEN_W..][..w]);
            }
        }
    }
    img
}

/// One map cell for the hover panel and the collision/elevation/behavior overlays.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CellInfo {
    pub x: i16,
    pub y: i16,
    /// The raw block (metatile, collision and elevation bits).
    pub block: u16,
    pub metatile: u16,
    pub collision: u8,
    pub elevation: u8,
    /// `GameData::behavior(raw_behavior)`.
    pub behavior: Behavior,
    /// The metatile attribute's behavior byte (Gen3 `MB_*`); `0xFF` (`MB_INVALID`) for a
    /// metatile the tileset does not have.
    pub raw_behavior: u8,
}

/// Raw behavior byte of a metatile the tileset lacks (Gen3 `MB_INVALID`).
const MB_INVALID: u8 = 0xFF;

/// The behavior byte of `block` drawn with `map`'s tilesets (engine `block_behavior`'s lookup).
fn raw_behavior(map: &MapData, block: u16) -> u8 {
    let mt = block_metatile(block);
    let attr = if mt < METATILES_IN_PRIMARY {
        map.primary.attributes.get(mt as usize)
    } else {
        map.secondary
            .attributes
            .get((mt - METATILES_IN_PRIMARY) as usize)
    };
    attr.map_or(MB_INVALID, |&a| attr_behavior(a))
}

/// Every cell of the map's own blocks (no margin, no overrides), row-major, `width * height`.
pub fn cells(game: &impl GameData, map: MapId) -> Vec<CellInfo> {
    let m = game.map(map);
    let w = m.width as usize;
    m.blocks
        .iter()
        .enumerate()
        .map(|(i, &block)| {
            let raw = raw_behavior(m, block);
            CellInfo {
                x: (i % w) as i16,
                y: (i / w) as i16,
                block,
                metatile: block & BLOCK_METATILE_MASK,
                collision: block_collision(block),
                elevation: block_elevation(block),
                behavior: game.behavior(raw),
                raw_behavior: raw,
            }
        })
        .collect()
}

/// One event or connection drawn over the map. Coordinates are the map's cells.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Overlay {
    Object {
        local_id: u8,
        x: i16,
        y: i16,
        gfx: Gfx,
        movement: MovementType,
        range: (u8, u8),
        script: Option<&'static str>,
        hide_flag: Option<FlagId>,
    },
    Warp {
        /// Index in the map's warps (what other maps' `dest_warp` names).
        index: u8,
        x: i16,
        y: i16,
        dest: MapId,
        /// The destination's decomp name, `"MISSING"` when it is not exported.
        dest_name: &'static str,
        dest_warp: u8,
    },
    Coord {
        x: i16,
        y: i16,
        var: VarId,
        value: u16,
        script: &'static str,
    },
    Bg {
        x: i16,
        y: i16,
        kind: BgKind,
        /// The script of a `BgKind::Script`.
        script: Option<&'static str>,
    },
    Connection {
        dir: Dir,
        offset: i16,
        map: MapId,
        /// The connected map's decomp name, `"MISSING"` when it is not exported.
        name: &'static str,
    },
}

/// Script name (`content::data::SCRIPT_NAMES`), `"?"` for an id outside the table.
pub fn script_name(id: ScriptId) -> &'static str {
    content::data::SCRIPT_NAMES
        .get(id.0 as usize)
        .copied()
        .unwrap_or("?")
}

/// The decomp name of `map`, `"MISSING"` when the game does not have it.
pub fn map_name(game: &impl GameData, map: MapId) -> &'static str {
    if game.has_map(map) {
        game.map(map).name
    } else {
        "MISSING"
    }
}

/// Objects, warps, coord events, bg events and connections, in that order, each in `MapData`
/// order.
pub fn overlays(game: &impl GameData, map: MapId) -> Vec<Overlay> {
    let m = game.map(map);
    let objects = m.objects.iter().map(|o| Overlay::Object {
        local_id: o.local_id,
        x: o.x,
        y: o.y,
        gfx: o.gfx,
        movement: o.movement,
        range: o.range,
        script: o.script.map(script_name),
        hide_flag: o.hide_flag,
    });
    let warps = m.warps.iter().enumerate().map(|(i, w)| Overlay::Warp {
        index: i as u8,
        x: w.x,
        y: w.y,
        dest: w.dest_map,
        dest_name: map_name(game, w.dest_map),
        dest_warp: w.dest_warp,
    });
    let coords = m.coords.iter().map(|c| Overlay::Coord {
        x: c.x,
        y: c.y,
        var: c.var,
        value: c.value,
        script: script_name(c.script),
    });
    let bgs = m.bgs.iter().map(|b| Overlay::Bg {
        x: b.x,
        y: b.y,
        kind: b.kind,
        script: match b.kind {
            BgKind::Script { script, .. } => Some(script_name(script)),
            BgKind::HiddenItem { .. } => None,
        },
    });
    let connections = m.connections.iter().map(|c| Overlay::Connection {
        dir: c.dir,
        offset: c.offset,
        map: c.map,
        name: map_name(game, c.map),
    });
    objects
        .chain(warps)
        .chain(coords)
        .chain(bgs)
        .chain(connections)
        .collect()
}

/// Every exported map with its decomp name, in `content::data::MAP_IDS` order.
pub fn map_list() -> Vec<(MapId, &'static str)> {
    content::data::MAP_IDS
        .iter()
        .map(|&id| (id, content::data::map(id.0).map_or("MISSING", |m| m.name)))
        .collect()
}
