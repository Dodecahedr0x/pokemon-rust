//! Party, save and PC specials that open a screen (Milestone 9 Task J owns the file):
//! `ChoosePartyMon`, `SaveGame`, the player's PCs.
//!
//! Wave 0: each issues its request and stores the host's answer as Gen3 does; the host answers
//! the defaults (nothing chosen, not saved).

use super::*;
pub use engine::script::{PARTY_NOTHING_CHOSEN, PartyMode, PcMode};

/// Gen3 `special ChoosePartyMon` (and `ChooseMonForMoveTutor` with `PartyMode::MoveTutor`): the
/// party slot chosen, or `PARTY_NOTHING_CHOSEN`, also written to `VAR_0x8004` as Gen3 does.
pub async fn choose_party_mon(ctx: &Ctx, mode: PartyMode) -> u8 {
    let slot = ctx.choose_party_mon(mode).await;
    ctx.set_var(VAR_0x8004, slot).await;
    slot as u8
}

/// Gen3 `special SaveGame`: the save dialog; whether the game was saved, also written to
/// `VAR_RESULT` as Gen3 does.
pub async fn save_game(ctx: &Ctx) -> bool {
    let saved = ctx.save_dialog().await;
    ctx.set_var(VAR_RESULT, saved as u16).await;
    saved
}

/// Gen3 `special PlayerPC` / `BedroomPC` / `ShowPokemonStorageSystemPC`.
pub async fn player_pc(ctx: &Ctx, mode: PcMode) {
    ctx.player_pc(mode).await;
}
