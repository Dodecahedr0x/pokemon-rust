//! What a metatile is for: its behaviour (from the tileset's attributes, as the game reads
//! them) and the collision it usually has. Both are heuristics for authoring, derived at run
//! time from the build's tilesets and the extract's maps; nothing here is committed.
//!
//! **Behaviour codes** (palette labels, `render --collision`): one letter per engine
//! [`Behavior`] class of the metatile's `MB_*` byte:
//!
//! | code | class | e.g. |
//! |---|---|---|
//! | `D` | door (animated; entering warps) | `MB_ANIMATED_DOOR` |
//! | `W` | warp cell (entering warps) | `MB_NON_ANIMATED_DOOR`, `MB_DEEP_SOUTH_WARP`, ladders, escalators, holes |
//! | `A` | arrow warp (pushing on it warps) | `MB_*_ARROW_WARP` (warp mats, exits) |
//! | `G` | tall grass (encounters) | `MB_TALL_GRASS` |
//! | `L` | ledge (solid; jumped from the uphill side) | `MB_JUMP_*` |
//! | `C` | counter (talk across) | `MB_COUNTER` |
//! | `S` | water (blocked without Surf) | `MB_POND_WATER`, `MB_OCEAN_WATER`… (`*WATER*`) |
//! | `I` | directional wall (walkable; some sides cannot be crossed) | `MB_IMPASSABLE_*` |
//! | `B` | other blocked behaviour | `MB_*_CURRENT`… |
//! | `F` | forced movement (walkable; pushes the player on) | `MB_WALK_*`, `MB_SLIDE_*`, `MB_MUDDY_SLOPE` |
//! | `K` | ice (walkable; slides, or cracks under a map's ice puzzle) | `MB_ICE`, `MB_THIN_ICE`, `MB_CRACKED_ICE` |
//! | `H` | cracked floor or hole (walkable; a hole drops to the map's hole warp) | `MB_CRACKED_FLOOR`, `MB_CRACKED_FLOOR_HOLE` |
//!
//! Normal metatiles (and the other behaviours the engine treats as normal) have no code.
//!
//! **Usual collision** of a cell ([`usual_collision`], [`cell_collision`]), first rule that
//! applies:
//! 1. the extract: when at least [`MIN_USES`] cells of the extracted maps use the metatile
//!    (with the same tileset; maps sharing a layout count it once), their verdict: at least
//!    [`MIN_AGREEMENT`] percent walkable (collision 0) gives 0, as many solid (1..=3) gives 1,
//!    and anything less clear gives none (the metatile is used both ways, e.g. water at a
//!    map's edge);
//! 2. the behaviour: tall grass, water, forced-movement cells (arrows, slides, muddy slopes),
//!    ice and cracked floors and holes are walkable (the game steps on them); ledges and
//!    counters are solid (Gen3 jumps a ledge before it checks collision, and every Emerald
//!    ledge cell is solid); warp cells and arrow warps are walkable when a warp event is on the
//!    cell, and have no usual collision without one; doors have none (Emerald's are walkable
//!    or solid: the door opens when the player pushes against it);
//! 3. otherwise none: no warning, and `AutoCollision` leaves the cell alone.
//!
//! The checks ([`issues`]) are warnings, never errors: a cell walkable against solid (or the
//! reverse), a walkable door, warp or arrow-warp cell without a warp event, and a warp event on
//! a metatile that triggers no warp. An `AllowCollision` op (`model::Allowance`, kept in the
//! map's edits) silences them for listed cells or metatiles, with a reason; an allowance that
//! silences nothing is itself a warning.
//!
//! The extract has the build's maps only (the M7 slice: 15 maps), so most metatiles fall
//! through to rule 2 or 3. On the stub world there is no extract and the behaviours are the
//! stub tilesets'.

use super::catalog::Catalog;
use super::model::{Allowance, Doc};
use super::validate::{Issue, Out};
use content::Emerald;
use engine::data::{
    Behavior, GameData, METATILES_IN_PRIMARY, attr_behavior, block_collision, block_metatile,
};
use std::collections::BTreeSet;

/// Extracted cells a metatile needs before its use counts as its usual collision.
pub const MIN_USES: u32 = 5;
/// Percent of those cells that must agree.
pub const MIN_AGREEMENT: u32 = 95;

/// A metatile's behaviour: its raw `MB_*` byte, the name and the engine's class.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetatileBehavior {
    pub raw: u8,
    /// `MB_*` (`MB_0x..` when unnamed).
    pub name: String,
    pub class: Behavior,
}

impl MetatileBehavior {
    pub fn from_raw(raw: u8) -> MetatileBehavior {
        MetatileBehavior {
            raw,
            name: extract::names::behavior_name(raw),
            class: Emerald::for_shells().behavior(raw),
        }
    }

    /// The one-letter code (module docs), `None` for a normal metatile.
    pub fn code(&self) -> Option<char> {
        Some(match self.class {
            Behavior::Door => 'D',
            Behavior::Warp
            | Behavior::NonAnimDoor(_)
            | Behavior::Ladder
            | Behavior::Escalator(_) => 'W',
            Behavior::ArrowWarp(_) => 'A',
            Behavior::TallGrass => 'G',
            Behavior::Ledge(_) => 'L',
            Behavior::Counter => 'C',
            Behavior::Blocked if self.is_water() => 'S',
            Behavior::Blocked => 'B',
            Behavior::Impassable(_) => 'I',
            Behavior::ForcedWalk(_) | Behavior::Slide(_) | Behavior::MuddySlope => 'F',
            Behavior::Ice | Behavior::ThinIce | Behavior::CrackedIce => 'K',
            Behavior::CrackedFloor | Behavior::CrackedFloorHole => 'H',
            Behavior::Normal => return None,
        })
    }

    fn is_water(&self) -> bool {
        self.name.contains("WATER")
    }

    /// Whether stepping onto a cell of it triggers the cell's warp.
    pub fn triggers_warp(&self) -> bool {
        self.class.enters_warp() || matches!(self.class, Behavior::ArrowWarp(_))
    }

    /// The class in words.
    pub fn what(&self) -> &'static str {
        match self.class {
            Behavior::Door => "door",
            Behavior::Warp => "warp",
            Behavior::NonAnimDoor(_) => "non-animated door",
            Behavior::Ladder => "ladder",
            Behavior::Escalator(_) => "escalator",
            Behavior::ArrowWarp(_) => "arrow warp",
            Behavior::TallGrass => "tall grass",
            Behavior::Ledge(_) => "ledge",
            Behavior::Counter => "counter",
            Behavior::Blocked if self.is_water() => "water",
            Behavior::Blocked => "blocked",
            Behavior::Impassable(_) => "directional wall",
            Behavior::ForcedWalk(_) => "walk arrow",
            Behavior::Slide(_) => "slide arrow",
            Behavior::MuddySlope => "muddy slope",
            Behavior::Ice => "ice",
            Behavior::ThinIce => "thin ice",
            Behavior::CrackedIce => "cracked ice",
            Behavior::CrackedFloor => "cracked floor",
            Behavior::CrackedFloorHole => "hole",
            Behavior::Normal => "normal",
        }
    }

    /// The collision the behaviour asks for (rule 2), if any; `warp`: a warp event is on the
    /// cell.
    pub fn collision(&self, warp: bool) -> Option<u8> {
        match self.class {
            // Emerald has doors with warps both walkable and solid (the door opens when the
            // player pushes against it).
            Behavior::Door => None,
            Behavior::Warp
            | Behavior::NonAnimDoor(_)
            | Behavior::Ladder
            | Behavior::Escalator(_)
            | Behavior::ArrowWarp(_) => warp.then_some(0),
            Behavior::TallGrass => Some(0),
            // The game moves the player across them.
            Behavior::ForcedWalk(_)
            | Behavior::Slide(_)
            | Behavior::MuddySlope
            | Behavior::Ice
            | Behavior::ThinIce
            | Behavior::CrackedIce
            | Behavior::CrackedFloor
            | Behavior::CrackedFloorHole => Some(0),
            Behavior::Ledge(_) => Some(1),
            Behavior::Blocked if self.is_water() => Some(0),
            Behavior::Counter => Some(1),
            // Walkable cells whose sides block: either collision can be right.
            Behavior::Blocked | Behavior::Impassable(_) | Behavior::Normal => None,
        }
    }
}

/// The legend of the codes, one line each.
pub const LEGEND: [(char, &str); 12] = [
    (
        'D',
        "door: entering warps (needs a warp event; a decorative door is solid)",
    ),
    (
        'W',
        "warp cell (stairs, ladder, plain door): entering warps",
    ),
    ('A', "arrow warp (warp mat, exit): pushing on it warps"),
    ('G', "tall grass"),
    ('L', "ledge (solid; jumped from the uphill side)"),
    ('C', "counter (solid; talk across)"),
    ('S', "water (needs Surf)"),
    ('I', "directional wall (walkable; some sides blocked)"),
    ('B', "blocked behaviour"),
    ('F', "forced movement (walk or slide arrow, muddy slope)"),
    (
        'K',
        "ice (slides; thin and cracked ice crack under an ice puzzle)",
    ),
    ('H', "cracked floor or hole (drops to the map's hole warp)"),
];

/// The tileset owning `metatile` (by name) and its index in it.
fn owner<'a>(primary: &'a str, secondary: &'a str, metatile: u16) -> (&'a str, usize) {
    if metatile < METATILES_IN_PRIMARY {
        (primary, metatile as usize)
    } else {
        (secondary, (metatile - METATILES_IN_PRIMARY) as usize)
    }
}

/// The behaviour of `metatile` with these tilesets (`None`: unknown tileset or metatile, or
/// a tileset without attributes).
pub fn behavior(
    c: &Catalog,
    primary: &str,
    secondary: &str,
    metatile: u16,
) -> Option<MetatileBehavior> {
    let (ts, i) = owner(primary, secondary, metatile);
    let t = c.tilesets.get(ts)?;
    if i >= t.metatiles() || i >= t.tileset.attributes.len() {
        return None;
    }
    let attr = t.tileset.attributes[i];
    Some(MetatileBehavior::from_raw(attr_behavior(attr)))
}

/// Why a cell has its usual collision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Usual {
    /// `agree` of the `n` cells of the extract that use the metatile agree.
    Extract { agree: u32, n: u32 },
    /// Its behaviour (`warp`: with a warp event on the cell).
    Behavior { b: MetatileBehavior, warp: bool },
}

impl std::fmt::Display for Usual {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Usual::Extract { agree, n } => {
                write!(f, "in {agree} of {n} extracted cells using it")
            }
            Usual::Behavior { b, warp } => {
                write!(f, "as a {} metatile ({})", b.what(), b.name)?;
                if *warp {
                    f.write_str(" with a warp")?;
                }
                Ok(())
            }
        }
    }
}

/// The extract's verdict on `metatile`: `Some(Some(c))` clear, `Some(None)` used both ways
/// (ambiguous), `None` fewer than [`MIN_USES`] cells.
fn extract_verdict(
    c: &Catalog,
    primary: &str,
    secondary: &str,
    metatile: u16,
) -> Option<Option<(u8, Usual)>> {
    let (ts, _) = owner(primary, secondary, metatile);
    let &[walk, solid] = c.collision_uses.get(&(ts.to_string(), metatile))?;
    let n = walk + solid;
    if n < MIN_USES {
        return None;
    }
    let (agree, usual) = if walk >= solid { (walk, 0) } else { (solid, 1) };
    Some((agree * 100 >= MIN_AGREEMENT * n).then_some((usual, Usual::Extract { agree, n })))
}

/// The usual collision of `metatile` (0 walkable, 1 solid) and why, for a cell with or without
/// a warp event (module docs).
pub fn usual_collision(
    c: &Catalog,
    primary: &str,
    secondary: &str,
    metatile: u16,
    warp: bool,
) -> Option<(u8, Usual)> {
    if let Some(v) = extract_verdict(c, primary, secondary, metatile) {
        return v;
    }
    let b = behavior(c, primary, secondary, metatile)?;
    Some((b.collision(warp)?, Usual::Behavior { b, warp }))
}

/// The cells of `d` with a warp event.
pub fn warp_cells(d: &Doc) -> BTreeSet<(i32, i32)> {
    (d.map.warps.iter())
        .map(|w| (w.x as i32, w.y as i32))
        .collect()
}

/// The usual collision of every cell of `d`, row-major.
pub fn usual_collisions(c: &Catalog, d: &Doc) -> Vec<Option<(u8, Usual)>> {
    let warps = warp_cells(d);
    let w = d.width.max(1) as usize;
    (d.blocks.iter().enumerate())
        .map(|(i, &b)| {
            let warp = warps.contains(&((i % w) as i32, (i / w) as i32));
            usual_collision(c, &d.primary, &d.secondary, block_metatile(b), warp)
        })
        .collect()
}

/// Cells whose collision disagrees with their usual one (walkable against solid).
pub fn collision_mismatches(d: &Doc, c: &Catalog) -> Vec<(i32, i32, u8, Usual)> {
    let w = d.width.max(1) as usize;
    (usual_collisions(c, d).into_iter().enumerate())
        .filter_map(|(i, u)| {
            let (usual, why) = u?;
            let b = d.blocks[i];
            ((block_collision(b) == 0) != (usual == 0))
                .then(|| ((i % w) as i32, (i / w) as i32, usual, why))
        })
        .collect()
}

/// Which allowance (index) covers cell (x, y) holding `metatile`.
pub fn allowed(allow: &[Allowance], x: i32, y: i32, metatile: u16) -> Option<usize> {
    allow
        .iter()
        .position(|a| a.cells.contains(&(x, y)) || a.metatiles.contains(&metatile))
}

/// The behaviour warnings of `d` (heuristics, so never errors), but those its allowances
/// silence:
/// - a cell whose collision disagrees with its usual collision;
/// - a walkable door, warp or arrow-warp cell without a warp event (a decorative door is
///   solid);
/// - a warp event on a cell whose metatile triggers no warp;
/// - an allowance that silences nothing.
pub fn issues(d: &Doc, c: &Catalog) -> Vec<Issue> {
    // (cell, metatile, at, message)
    let mut found: Vec<(i32, i32, u16, String, String)> = Vec::new();
    for (x, y, usual, why) in collision_mismatches(d, c) {
        let b = d.block(x, y).unwrap_or_default();
        let (now, want) = match usual {
            0 => (
                format!("solid (collision {})", block_collision(b)),
                "walkable",
            ),
            _ => ("walkable".to_string(), "solid"),
        };
        found.push((
            x,
            y,
            block_metatile(b),
            format!("cell ({x}, {y})"),
            format!(
                "{now}, but metatile {} is {want} {why} (AutoCollision sets it)",
                block_metatile(b)
            ),
        ));
    }
    let warps = warp_cells(d);
    for y in 0..d.height as i32 {
        for x in 0..d.width as i32 {
            let b = d.block(x, y).unwrap_or_default();
            if block_collision(b) == 0
                && !warps.contains(&(x, y))
                && let Some(mb) = behavior(c, &d.primary, &d.secondary, block_metatile(b))
                && mb.triggers_warp()
            {
                found.push((
                    x,
                    y,
                    block_metatile(b),
                    format!("cell ({x}, {y})"),
                    format!(
                        "walkable {} metatile {} ({}) has no warp event (a decorative door is \
                         solid)",
                        mb.what(),
                        block_metatile(b),
                        mb.name
                    ),
                ));
            }
        }
    }
    for (i, wp) in d.map.warps.iter().enumerate() {
        let (x, y) = (wp.x as i32, wp.y as i32);
        let Some(b) = d.block(x, y) else {
            continue; // out of bounds: an error already
        };
        if let Some(mb) = behavior(c, &d.primary, &d.secondary, block_metatile(b))
            && !mb.triggers_warp()
        {
            found.push((
                x,
                y,
                block_metatile(b),
                format!("warp {i}"),
                format!(
                    "on metatile {} ({}), which triggers no warp (a door, warp or arrow-warp \
                     metatile does; fine if it is only an arrival point)",
                    block_metatile(b),
                    mb.name
                ),
            ));
        }
    }
    let mut o = Out(Vec::new());
    let mut used = vec![false; d.allow.len()];
    for (x, y, mt, at, msg) in found {
        match allowed(&d.allow, x, y, mt) {
            Some(a) => used[a] = true,
            None => o.warn(at, msg),
        }
    }
    for (i, a) in d.allow.iter().enumerate() {
        if !used[i] {
            o.warn(
                format!("allowance {i}"),
                format!("{:?} silences no warning (remove it)", a.reason),
            );
        }
    }
    o.0
}
