//! `$OUT_DIR/ui.rs`: the Milestone 9 UI tables (`content::data::{START_MENU, ITEM_USE, …}`),
//! answered by `game_data_tables!` for every flavour. Task C owns the file: it fills the tables
//! from the extracted `ui/`, `icons/` and `tables/` files (blobs for pictures and icons).
//!
//! Wave 0: every table is empty, so each `GameData` UI method answers `None` (or its default)
//! and the screens draw frames and text only.

use crate::input::World;

/// The Rust source of `ui.rs`, `include!`d into `content::data`.
pub fn generate(world: &World) -> String {
    let _ = world;
    String::from(
        "\
/// The START menu (Gen3 `sStartMenuItems`; empty until Milestone 9 Task C).
pub static START_MENU: &[::engine::data::StartItem] = &[];
/// `ItemUse` by item id.
pub static ITEM_USE: &[::engine::data::ItemUse] = &[];
/// Item descriptions by item id (`gItems[].description`).
pub static ITEM_DESCRIPTIONS: &[Option<&[u8]>] = &[];
/// Move descriptions by move id (`gMoveDescriptionPointers`).
pub static MOVE_DESCRIPTIONS: &[Option<&[u8]>] = &[];
/// `(TM/HM item, move)`, sorted by item id.
pub static TM_MOVES: &[(u16, u16)] = &[];
/// `(species, bitset over TM_MOVES)`, sorted by species id (`sTMHMLearnsets`).
pub static TM_COMPAT: &[(u16, u64)] = &[];
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
    )
}
