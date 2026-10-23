//! The map tour's pure logic (`--features tour`; no agb imports). The GBA shell and its
//! native mirror (`crates/emu/tests/tour.rs`, which includes this file through `#[path]`) run
//! the same list and the same warps, so the ROM and native stay in step.
//!
//! The tour boots a new game, then each L+R warps to the next map of [`tour`]: the Act 1
//! maps, then one map per tileset family (primary and secondary pair) not seen yet (the
//! gated part), then every other map (the stretch).

use alloc::vec::Vec;
use engine::Buttons;
use engine::data::{GameData, MapId, block_collision};

/// The Act 1 maps (new game to the first badge), as `content/build/support/act1_maps.ron`
/// lists them (decomp folder names; the tour test checks the two agree).
pub const ACT1_MAPS: [&str; 45] = [
    "LittlerootTown",
    "LittlerootTown_BrendansHouse_1F",
    "LittlerootTown_BrendansHouse_2F",
    "LittlerootTown_MaysHouse_1F",
    "LittlerootTown_MaysHouse_2F",
    "LittlerootTown_ProfessorBirchsLab",
    "Route101",
    "OldaleTown",
    "OldaleTown_House1",
    "OldaleTown_House2",
    "OldaleTown_Mart",
    "OldaleTown_PokemonCenter_1F",
    "OldaleTown_PokemonCenter_2F",
    "Route103",
    "Route102",
    "PetalburgCity",
    "PetalburgCity_Gym",
    "PetalburgCity_House1",
    "PetalburgCity_House2",
    "PetalburgCity_Mart",
    "PetalburgCity_PokemonCenter_1F",
    "PetalburgCity_PokemonCenter_2F",
    "PetalburgCity_WallysHouse",
    "Route104",
    "Route104_MrBrineysHouse",
    "Route104_PrettyPetalFlowerShop",
    "PetalburgWoods",
    "RustboroCity",
    "RustboroCity_CuttersHouse",
    "RustboroCity_DevonCorp_1F",
    "RustboroCity_DevonCorp_2F",
    "RustboroCity_DevonCorp_3F",
    "RustboroCity_Flat1_1F",
    "RustboroCity_Flat1_2F",
    "RustboroCity_Flat2_1F",
    "RustboroCity_Flat2_2F",
    "RustboroCity_Flat2_3F",
    "RustboroCity_Gym",
    "RustboroCity_House1",
    "RustboroCity_House2",
    "RustboroCity_House3",
    "RustboroCity_Mart",
    "RustboroCity_PokemonCenter_1F",
    "RustboroCity_PokemonCenter_2F",
    "RustboroCity_PokemonSchool",
];

/// L and R newly held together. Not START (the tour's combo until M9 B): since the content has
/// a start menu, the field opens it on START before the shell's warp, which then refuses.
pub fn combo_pressed(prev: Buttons, cur: Buttons) -> bool {
    const LR: Buttons = Buttons(Buttons::L.0 | Buttons::R.0);
    cur.contains(LR) && !prev.contains(LR)
}

/// The tour: `maps` (the game's maps, in content order) reordered as the module docs say, and
/// how many of them are gated.
pub fn tour(game: &impl GameData, maps: &[MapId]) -> (Vec<MapId>, usize) {
    let mut out: Vec<MapId> = Vec::new();
    for name in ACT1_MAPS {
        if let Some(&m) = maps.iter().find(|&&m| game.map(m).name == name) {
            out.push(m);
        }
    }
    let family = |m: MapId| {
        let d = game.map(m);
        (d.primary as *const _, d.secondary as *const _)
    };
    let mut families: Vec<_> = out.iter().map(|&m| family(m)).collect();
    for &m in maps {
        let f = family(m);
        if !out.contains(&m) && !families.contains(&f) {
            families.push(f);
            out.push(m);
        }
    }
    let gated = out.len();
    for &m in maps {
        if !out.contains(&m) {
            out.push(m);
        }
    }
    (out, gated)
}

/// Where the tour lands on `map`: its first warp, else the walkable cell (collision 0, no
/// object template) nearest its centre (ties in row order), as `cargo xtask sweep` starts.
/// Returns `World::debug_warp`'s (warp, x, y). The search widens from the centre one distance
/// at a time, so it stays short on the GBA (it runs in the frame of the combo).
pub fn landing(game: &impl GameData, map: MapId) -> (Option<u8>, i16, i16) {
    let m = game.map(map);
    if !m.warps.is_empty() {
        return (Some(0), -1, -1);
    }
    let (w, h) = (i32::from(m.width), i32::from(m.height));
    let (cx, cy) = (w / 2, h / 2);
    let free = |x: i32, y: i32| {
        (0..w).contains(&x)
            && (0..h).contains(&y)
            && block_collision(m.blocks[(y * w + x) as usize]) == 0
            && !(m.objects.iter()).any(|o| (i32::from(o.x), i32::from(o.y)) == (x, y))
    };
    for d in 0..w + h {
        // Distance `d` in row order: `y` ascending, then `x` ascending.
        for y in cy - d..=cy + d {
            let r = d - (y - cy).abs();
            let xs = if r == 0 { [cx, cx] } else { [cx - r, cx + r] };
            for x in xs {
                if free(x, y) {
                    return (None, x as i16, y as i16);
                }
            }
        }
    }
    (None, cx as i16, cy as i16)
}

/// The shell's tour state: the next stop.
pub struct Tour {
    pub maps: Vec<MapId>,
    pub next: usize,
}

impl Tour {
    pub fn new(game: &impl GameData, maps: &[MapId]) -> Tour {
        Tour {
            maps: tour(game, maps).0,
            next: 0,
        }
    }

    /// On the combo, warps `world` to the next stop (wrapping after the last). Returns whether
    /// it warped (refused in a battle, the starter screen or a transition: the stop stays
    /// next).
    pub fn on_keys(
        &mut self,
        game: &impl GameData,
        world: &mut engine::World,
        prev: Buttons,
        keys: Buttons,
    ) -> bool {
        if !combo_pressed(prev, keys) || self.maps.is_empty() {
            return false;
        }
        let map = self.maps[self.next % self.maps.len()];
        let (warp, x, y) = landing(game, map);
        if !world.debug_warp(game, map, warp, x, y) {
            return false;
        }
        self.next += 1;
        true
    }
}
