//! The worked example of an override (Milestone 8 Task D2): Oldale's Pokémon Center nurse, an
//! end-in-callee site. Gen3 (`data/maps/OldaleTown_PokemonCenter_1F/scripts.inc`):
//!
//! ```text
//! setvar VAR_0x800B, LOCALID_OLDALE_NURSE
//! call Common_EventScript_PkmnCenterNurse
//! waitmessage
//! waitbuttonpress
//! release
//! end
//! ```
//!
//! `Common_EventScript_PkmnCenterNurse` returns only on its "goodbye" and "healed" paths, whose
//! last message the caller waits on; every other path (`end`) stops the whole script, the
//! caller's three trailing commands included. The lowering expresses that with a `Flow` check
//! after the call; here it is spelled out.

use crate::data::Flow;
use crate::prelude::*;
use crate::scripts::Common_pkmn_center_nurse::Common_EventScript_PkmnCenterNurse;

/// `LOCALID_OLDALE_NURSE`: the nurse's object (the common script turns it with `VAR_0x800B`).
const NURSE: u16 = 1;

pub async fn OldaleTown_PokemonCenter_1F_EventScript_Nurse(ctx: Ctx) {
    ctx.set_var(VAR_0x800B, NURSE).await;
    match Box::pin(Common_EventScript_PkmnCenterNurse(ctx.clone())).await {
        // Gen3 `end` inside the callee: the script is over.
        Flow::End => {}
        // `return`: the callee left its last message open.
        Flow::Continue => {
            ctx.wait_message().await;
            ctx.wait_button().await;
            ctx.release().await;
        }
    }
}
