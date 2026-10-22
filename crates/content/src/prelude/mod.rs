//! What generated scripts (`content/extracted/scripts/<Map>.rs`) may use: the engine's script
//! types, every Gen3 constant (`FLAG_*`, `VAR_*`, `MAP_*`, `SPECIAL_*`, `STD_*`, `ITEM_*`,
//! `OBJ_EVENT_GFX_*`, `SPECIES_*`, `MOVE_*`, `TRAINER_*`, `HEAL_LOCATION_*`), every text id,
//! and the Gen3 std-script helpers, one file per owner (Milestone 8): [`msgbox`], `std`
//! (`callstd`), `special`, `trainer` (Task I), `items`, `text` and `menu` (Task H). The
//! helpers declared in Wave 0 issue their request and return the host's default; the owning task
//! fills the Gen3 behaviour, and the script lowering (Task D2) emits only these names.

pub use crate::consts::*;
pub use alloc::boxed::Box;
pub use core::cmp::Ordering;
pub use engine::battle::BattleOutcome;
pub use engine::data::{
    Dir, FlagId, ItemId, MapId, MoveId, MovementType, PLAYER, ScriptId, SpeciesId, TextId,
    TrainerId, VarId,
};
pub use engine::script::{BufferSrc, Ctx, Emote, Fade, Report, Speed, Step, WarpSlot};

mod items;
mod legacy;
mod menu;
mod msgbox;
mod special;
mod std;
mod text;
mod trainer;

pub use self::std::*;
pub use items::*;
pub use legacy::*;
pub use menu::*;
pub use msgbox::*;
pub use special::*;
pub use text::*;
pub use trainer::*;

/// `checkplayergender`: Gen3 `MALE` (0). The player is SACHA (Brendan) and the rival is May.
pub async fn check_player_gender(_ctx: &Ctx) -> u16 {
    0
}

/// A script command the engine does not implement (`what` is its macro name): logged.
pub async fn unsupported(ctx: &Ctx, what: &'static str) {
    ctx.log("unsupported script command:").await;
    ctx.log(what).await;
}

/// A command or special the script lowering classed `Unsupported` (its decomp name): logged
/// and reported as a `World` event; the script continues.
pub async fn report(ctx: &Ctx, what: &'static str) {
    ctx.report(Report::Unsupported(what)).await;
}

/// Gen3 `setdynamicwarp` / `setescapewarp`: one request; the host stores it (Task F).
pub async fn set_warp(ctx: &Ctx, slot: WarpSlot, map: u16, warp: Option<u8>, x: i16, y: i16) {
    ctx.set_warp(slot, MapId(map), warp, x, y).await;
}

/// The name of `id` in a `(value, name)` table of `crate::data` (for logs).
pub(crate) fn name_of(table: &'static [(u16, &'static str)], id: u16) -> Option<&'static str> {
    table.iter().find(|&&(v, _)| v == id).map(|&(_, n)| n)
}
