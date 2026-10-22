//! `special` / `specialvar` (Gen3 `gSpecials`).

use super::*;

/// `special <id>` / `specialvar`: logs the special's name, runs the ones below and returns 0.
/// Every value-returning special the slice uses (`ShouldTryRematchBattle`,
/// `IsTrainerRegistered`, `HasAllHoennMons`) means "no" at 0.
///
/// - `ChooseStarter`: Gen3 `ChooseStarter` + `CB2_GiveStarter` + `CB2_StartFirstBattle`: the
///   starter screen, `VAR_STARTER_MON` = the choice, the starter at level 5, then the first
///   battle (`battle_controllers.c`: Zigzagoon, level 2).
/// - `HealPlayerParty`: heals the party.
/// - `ChangePokemonNickname`: no naming screen, the name is kept; the field fades back in from
///   the script's `fadescreen FADE_TO_BLACK` as `CB2_ReturnToFieldContinueScriptPlayMapMusic`
///   does.
/// - `PlayerFaceTrainerAfterBattle`: cosmetic (the player turns to the trainer), only logged.
pub async fn special(ctx: &Ctx, id: u16) -> u16 {
    ctx.log(name_of(&crate::data::SPECIAL_NAMES, id).unwrap_or("SPECIAL_?"))
        .await;
    // Guards, not patterns: a mistyped constant would silently become a catch-all binding.
    match id {
        id if id == SPECIAL_ChooseStarter => {
            let starters = crate::battle::STARTERS;
            let i = ctx.choose_starter(starters).await;
            debug_assert!(i < 3, "starter index {i}");
            // `CB2_GiveStarter` stores the raw choice; `GetStarterPokemon` maps an out-of-range
            // one to the first starter.
            ctx.set_var(VAR_STARTER_MON, i.into()).await;
            let species = starters.get(usize::from(i)).unwrap_or(&starters[0]);
            ctx.give_mon(*species, 5).await;
            let (foe, level) = crate::battle::FIRST_BATTLE_FOE;
            ctx.battle_wild(foe, level, true).await;
        }
        id if id == SPECIAL_HealPlayerParty => ctx.heal_party().await,
        id if id == SPECIAL_ChangePokemonNickname => ctx.fade(Fade::FromBlack).await,
        _ => {}
    }
    0
}
