//! `$OUT_DIR/ui.rs`: the Milestone 9 UI tables (`content::data::{START_MENU, ITEM_USE, …}`),
//! answered by `game_data_tables!` for every flavour. Task C owns the file: it fills the tables
//! from the extracted `ui/`, `icons/` and `tables/` files (blobs for pictures and icons).
//!
//! Wave 0: every table is empty, so each `GameData` UI method answers `None` (or its default)
//! and the screens draw frames and text only. Tasks K/PT fill `TM_MOVES` and `TM_COMPAT` from
//! `tables/tmhm.ron` (the teach flow).

use crate::input::World;

/// The Rust source of `ui.rs`, `include!`d into `content::data`.
pub fn generate(world: &World) -> String {
    let (tm_moves, tm_compat) = tmhm(world);
    let mut s = String::from(
        "\
/// The START menu (Gen3 `sStartMenuItems`; empty until Milestone 9 Task C).
pub static START_MENU: &[::engine::data::StartItem] = &[];
/// `ItemUse` by item id.
pub static ITEM_USE: &[::engine::data::ItemUse] = &[];
/// Item descriptions by item id (`gItems[].description`).
pub static ITEM_DESCRIPTIONS: &[Option<&[u8]>] = &[];
/// Move descriptions by move id (`gMoveDescriptionPointers`).
pub static MOVE_DESCRIPTIONS: &[Option<&[u8]>] = &[];
/// `(UiGfx tag, picture)`.
pub static UI_GFX: &[(::engine::data::UiGfx, ::engine::data::UiPicture)] = &[];
/// The user window frames (`gUserFrames`).
pub static WINDOW_FRAMES: &[::engine::data::WindowGfx] = &[];
/// `(item id, icon)`, sorted by item id.
pub static ITEM_ICONS: &[(u16, ::engine::data::Icon)] = &[];
/// `(species id, icon)`, sorted by species id.
pub static MON_ICONS: &[(u16, ::engine::data::Icon)] = &[];
/// `sIngameTrades`.
pub static IN_GAME_TRADES: &[::engine::data::InGameTrade] = &[];
/// Nature names by nature (`gNatureNamePointers`).
pub static NATURE_NAMES: &[&[u8]] = &[];
",
    );
    s.push_str(&format!(
        "/// `(TM/HM item, move)`, sorted by item id (`sTMHMMoves`).\n\
         pub static TM_MOVES: &[(u16, u16)] = &[{tm_moves}];\n\
         /// `(species, bitset over TM_MOVES)`, sorted by species id (`gTMHMLearnsets`).\n\
         pub static TM_COMPAT: &[(u16, u64)] = &[{tm_compat}];\n"
    ));
    s
}

/// `TM_MOVES` and `TM_COMPAT`'s entries from `tables/tmhm.ron` (decomp names to ids); a name
/// without an id drops its entry (and, for an item, every later bit: the bitsets index
/// `TM_MOVES`).
fn tmhm(world: &World) -> (String, String) {
    use extract::names;
    let t = &world.tables.tmhm;
    let mut moves = Vec::new();
    for (item, mv) in &t.moves {
        match (names::ITEMS.value(item), names::MOVES.value(mv)) {
            (Some(i), Some(m)) => moves.push((i, m)),
            _ => break,
        }
    }
    let sorted = moves.windows(2).all(|w| w[0].0 < w[1].0);
    if !sorted || moves.len() > 64 {
        moves.clear();
    }
    let mut compat: Vec<(u32, u64)> = t
        .learnsets
        .iter()
        .filter_map(|(s, &bits)| Some((names::SPECIES.value(s)?, bits)))
        .collect();
    compat.sort_unstable();
    if moves.is_empty() {
        compat.clear();
    }
    let mask = if moves.len() == 64 {
        u64::MAX
    } else {
        (1u64 << moves.len()) - 1
    };
    (
        moves
            .iter()
            .map(|(i, m)| format!("({i}, {m})"))
            .collect::<Vec<_>>()
            .join(", "),
        compat
            .iter()
            .map(|(s, b)| format!("({s}, {:#x})", b & mask))
            .collect::<Vec<_>>()
            .join(", "),
    )
}
