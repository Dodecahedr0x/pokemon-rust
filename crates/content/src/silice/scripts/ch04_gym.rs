//! Chapter 4, the Zenith-Grog gym (`PortRessac_Gym`, the deck and the quarterdeck, and
//! `PortRessac_Gym_Cale`, the hold): `contenu-jeu/chapitres/04-port-ressac.md` §3
//! Port_Ressac_Gym_Sc01 to Sc03 (the breath test, Samuel, Gwada l'Enjaillé) and §4 (the guide,
//! the crew, the barrels' trial). `VAR_SIL_STORY` runs 47 (the gym open) to 49 (Gwada beaten).

use super::ch04_port::{Puzzle, push_object};
use crate::silice::prelude::*;
use crate::silice::rules::{WITH_ROLL, WITH_ROLL_CONFUSED};

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["PortRessac_Gym", "PortRessac_Gym_Cale"];

/// The quarterdeck's grille (9–10, 10), opened as the deck (SilPortRessacGym's planks, 512).
const GRILLE: [(i16, i16); 2] = [(9, 10), (10, 10)];
const METATILE_PONT: u16 = 512;

async fn open_grille(ctx: &Ctx) {
    for (x, y) in GRILLE {
        ctx.set_metatile(x, y, METATILE_PONT, false).await;
    }
}

// ---- the deck ----

/// From `VAR_SIL_STORY` 48 (Samuel faced, won or lost) the grille is open on every load, so
/// the player leaves the quarterdeck by its stairs (the hold's barrels are back in place).
pub async fn PortRessac_Gym_OnLoad(ctx: Ctx) {
    if story(&ctx).await >= 48 {
        open_grille(&ctx).await;
    }
}

/// Gwada is hidden until Sc03 shows him on the coupée.
pub async fn PortRessac_Gym_OnTransition(ctx: Ctx) {
    ctx.set_flag(FLAG_SIL_HIDE_PORT_GYM_GWADA, true).await;
}

/// The guide at the mast's foot: his advice; after a loss to Samuel without the badge, the R-7
/// reminder (`Rappel_Badge`).
pub async fn PortRessac_Gym_EventScript_Guide(ctx: Ctx) {
    let reminder = ctx.flag(FLAG_SIL_SAMUEL_PERDU).await && !ctx.flag(FLAG_SIL_BADGE_04).await;
    let t = if reminder {
        Port_Ressac_Gym_Rappel_Badge
    } else {
        Port_Ressac_Gym_Guide
    };
    msgbox(&ctx, t, MsgBox::Npc).await;
}

/// The grille (9–10, 10), closed until Samuel has been faced: the guide's "go through the hold"
/// (the source has no text for the grille itself). Nothing once open.
pub async fn PortRessac_Gym_EventScript_Grille(ctx: Ctx) {
    if story(&ctx).await >= 48 {
        return;
    }
    msgbox(&ctx, Port_Ressac_Gym_Guide, MsgBox::Sign).await;
}

/// The roll notice (12, 13). The roll itself is a fallback (Samuel's battle).
pub async fn PortRessac_Gym_EventScript_SignRoulis(ctx: Ctx) {
    msgbox(&ctx, Port_Ressac_Gym_Sign_Roulis, MsgBox::Sign).await;
}

/// MARIN PATRICE, the bosun (4, 17), facing east, sight 6.
pub async fn PortRessac_Gym_EventScript_Patrice(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GYM4_PATRICE,
        TRAINER_SIL_PORT_GYM_PATRICE,
        Port_Ressac_Gym_Patrice_Intro,
        Port_Ressac_Gym_Patrice_Defeat,
    )
    .await
    {
        post_battle(&ctx, Port_Ressac_Gym_Patrice_Defeat).await;
    }
}

/// COMBATTANTE JESSY (16, 14), facing west, sight 3.
pub async fn PortRessac_Gym_EventScript_Jessy(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GYM4_JESSY,
        TRAINER_SIL_PORT_GYM_JESSY,
        Port_Ressac_Gym_Jessy_Intro,
        Port_Ressac_Gym_Jessy_Defeat,
    )
    .await
    {
        post_battle(&ctx, Port_Ressac_Gym_Jessy_Defeat).await;
    }
}

/// KARATEKA GUS (7, 5), over the ladder's top, facing west, sight 2.
pub async fn PortRessac_Gym_EventScript_Gus(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GYM4_GUS,
        TRAINER_SIL_PORT_GYM_GUS,
        Port_Ressac_Gym_Gus_Intro,
        Port_Ressac_Gym_Gus_Defeat,
    )
    .await
    {
        post_battle(&ctx, Port_Ressac_Gym_Gus_Defeat).await;
    }
}

// ---- Samuel ----

/// One shot of the breath test: the drum (`shot`), the beat's pause, then "drink?". `drink`:
/// the right answer. Returns whether it was right.
async fn shot(ctx: &Ctx, text: TextId, pause: u16, drink: bool) -> bool {
    say(ctx, text).await;
    ctx.delay(pause).await;
    ask(ctx, Port_Ressac_Gym_Boire).await == drink
}

/// Port_Ressac_Gym_Sc01, the breath test: Samuel at the helm (obj 1) from `VAR_SIL_STORY` 47
/// until the badge. After a loss, the rematch prompt first. Once (`FLAG_SIL_PORT_GYM_ALCOOTEST`):
/// four shots, drunk on the BOOM only; the first wrong one ends the test, and the player's lead
/// would start the battle confused (`FLAG_SIL_PORT_GYM_CONFUS`); four right ones give a Persim
/// Berry. Then the battle (Sc02). With the badge, his thanks (`Sc02_T3b`; no line of its own).
pub async fn PortRessac_Gym_Sc01(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_BADGE_04).await {
        if super::ch12_revanches::ouvertes(&ctx).await {
            super::ch12_revanches::revanche(&ctx, &super::ch12_revanches::SAMUEL).await;
            return;
        }
        msgbox(&ctx, Port_Ressac_Gym_Sc02_T3b, MsgBox::Npc).await;
        return;
    }
    ctx.lock_all().await;
    ctx.face_player().await;
    if ctx.flag(FLAG_SIL_SAMUEL_PERDU).await && !ask(&ctx, Port_Ressac_Gym_Revanche).await {
        say(&ctx, Port_Ressac_Gym_Revanche_Non).await;
        ctx.release_all().await;
        return;
    }
    if !ctx.flag(FLAG_SIL_PORT_GYM_ALCOOTEST).await {
        say(&ctx, Port_Ressac_Gym_Sc01_T1).await;
        ctx.set_flag(FLAG_SIL_PORT_GYM_ALCOOTEST, true).await;
        let mut ok = shot(&ctx, Port_Ressac_Gym_Shot1, 20, true).await;
        ok = ok && shot(&ctx, Port_Ressac_Gym_Shot2, 40, false).await;
        ok = ok && shot(&ctx, Port_Ressac_Gym_Shot3, 10, true).await;
        if ok {
            // The ship rolls (the source gives no duration: the prelude's).
            screen_shake(&ctx, 2, 0).await;
            ok = shot(&ctx, Port_Ressac_Gym_Shot4, 30, true).await;
        }
        if ok {
            say(&ctx, Port_Ressac_Gym_Sc01_Reussi).await;
            give_item(&ctx, ITEM_PERSIM_BERRY, 1).await;
        } else {
            say(&ctx, Port_Ressac_Gym_Sc01_Rate).await;
            ctx.set_flag(FLAG_SIL_PORT_GYM_CONFUS, true).await;
        }
        // The source's `VAR_SIL_PORT_RESSAC_STATE` (0 so far, 1 failed) is the `ok` above.
        ctx.set_var(VAR_SIL_PORT_RESSAC_STATE, 0).await;
    }
    say(&ctx, Port_Ressac_Gym_Sc01_T2).await;
    PortRessac_Gym_Sc02(ctx.clone()).await;
}

/// Port_Ressac_Gym_Sc02, Samuel's battle (chained from Sc01): the Badge Grog, TM01, the
/// Hydro-Propulseur (`FLAG_SIL_MODULE_SURF`, unless a loss gave it), the Millésime Sam, the
/// synchronisation at 40 %. Sets 48 (never back). The grille opens: the player leaves by the
/// quarterdeck's stairs.
///
/// A loss (R-7) goes on without a whiteout: Samuel's toll (`ch04_port::samuel_toll`), and the
/// player stays on the quarterdeck for a rematch.
pub async fn PortRessac_Gym_Sc02(ctx: Ctx) {
    // The roll (accuracy −1 every second turn for the non-Water, non-Fighting) is the ship's
    // field rule; a failed sobriety test adds the lead's confusion (`silice::rules::ROLL*`).
    let rules = if ctx.flag(FLAG_SIL_PORT_GYM_CONFUS).await {
        &WITH_ROLL_CONFUSED
    } else {
        &WITH_ROLL
    };
    // R-7: a loss goes on without a whiteout, into Samuel's toll on the quarterdeck.
    let outcome = battle_trainer_with(
        &ctx,
        TRAINER_SIL_SAMUEL_1,
        Port_Ressac_Gym_Sc02_Defeat,
        true,
        rules,
    )
    .await;
    ctx.set_flag(FLAG_SIL_PORT_GYM_CONFUS, false).await;
    if matches!(outcome, BattleOutcome::Lost | BattleOutcome::Drew) {
        super::ch04_port::samuel_toll(&ctx).await;
        ctx.release_all().await;
        return;
    }
    say(&ctx, Port_Ressac_Gym_Sc02_T1).await;
    // fallback: play_fanfare(MUS_OBTAIN_BADGE) (audio, M12): nothing.
    fallback(&ctx, "play_fanfare").await;
    say(&ctx, Port_Ressac_Gym_Sc02_Badge).await;
    ctx.set_flag(FLAG_SIL_BADGE_04, true).await;
    say(&ctx, Port_Ressac_Gym_Sc02_T2).await;
    give_item(&ctx, ITEM_TM01, 1).await;
    if ctx.flag(FLAG_SIL_MODULE_SURF).await {
        say(&ctx, Port_Ressac_Gym_Sc02_T3b).await;
    } else {
        say(&ctx, Port_Ressac_Gym_Sc02_T3).await;
        ctx.set_flag(FLAG_SIL_MODULE_SURF, true).await;
        say(&ctx, Port_Ressac_Gym_Sc02_Module).await;
    }
    give_item(&ctx, ITEM_SIL_MILLESIME_SAM, 1).await;
    say(&ctx, Port_Ressac_Gym_Sc02_T4).await;
    ctx.set_var(VAR_SIL_SYNC, 40).await;
    // fallback: show_sync (the Mallette's gauge screen): `_Sync` gives the value.
    show_sync(&ctx, 40).await;
    say(&ctx, Port_Ressac_Gym_Sc02_Sync).await;
    if story(&ctx).await < 48 {
        set_story(&ctx, 48).await;
    }
    open_grille(&ctx).await;
    ctx.release_all().await;
}

const GWADA_IN: &[Step] = &[walk(Dir::Down), walk(Dir::Down), Step::FacePlayer];
const GWADA_OUT: &[Step] = &[walk(Dir::Left), walk(Dir::Left), face(Dir::Right)];

/// Port_Ressac_Gym_Sc03, Gwada l'Enjaillé: the coupée's top (9–10, 18) at `VAR_SIL_STORY` 48,
/// on the way out (after a win or a loss). A battle at once, without a heal: only the
/// Millésime Sam, if the player still has it, heals first. The Dragon Scale. Sets 49
/// (`FLAG_SIL_PORT_GWADA_BATTU`). A loss whites out (48 stays: he waits again).
pub async fn PortRessac_Gym_Sc03(ctx: Ctx) {
    if story(&ctx).await != 48 {
        return;
    }
    ctx.lock_all().await;
    show(&ctx, LOCALID_GYM4_GWADA, FLAG_SIL_HIDE_PORT_GYM_GWADA).await;
    move_wait(&ctx, LOCALID_GYM4_GWADA, GWADA_IN).await;
    say(&ctx, Port_Ressac_Gym_Sc03_T1).await;
    if check_item(&ctx, ITEM_SIL_MILLESIME_SAM, 1).await && ask(&ctx, Port_Ressac_Gym_Sc03_T2).await
    {
        remove_item(&ctx, ITEM_SIL_MILLESIME_SAM, 1).await;
        ctx.heal_party().await;
        say(&ctx, Port_Ressac_Gym_Sc03_T3).await;
    }
    say(&ctx, Port_Ressac_Gym_Sc03_T4).await;
    // The "rum" (evasion +1, accuracy −1) is Gwada's own field rule (`silice::rules::RUM`);
    // the source's fallback (Bright Powder, Reflect) stays in the trainer data.
    battle_trainer(&ctx, TRAINER_SIL_GWADA_2, Port_Ressac_Gym_Sc03_Defeat).await;
    say(&ctx, Port_Ressac_Gym_Sc03_T5).await;
    give_item(&ctx, ITEM_DRAGON_SCALE, 1).await;
    say(&ctx, Port_Ressac_Gym_Sc03_T6).await;
    move_wait(&ctx, LOCALID_GYM4_GWADA, GWADA_OUT).await;
    hide(&ctx, LOCALID_GYM4_GWADA).await;
    ctx.set_flag(FLAG_SIL_PORT_GWADA_BATTU, true).await;
    set_story(&ctx, 49).await;
    ctx.release_all().await;
}

// ---- the hold: the barrels' trial (§4) ----

/// The hold's planks (`.`), as the source's grid; `!` the explosive barrels n°2 (8, 3) and
/// n°1 (7, 6), floor once blown. The lashed barrels (`#`, `$`), the hatch (S) and the ladder (E)
/// are not floor.
const CALE: [&str; 9] = [
    "########E##",
    "#.........#",
    "#.........#",
    "####.###!##",
    "#.........#",
    "#$.......$#",
    "##.####!###",
    "#.........#",
    "#####S#####",
];
const PLANCHER: u16 = 525;
const FUT_1: (i16, i16) = (7, 6);
const FUT_2: (i16, i16) = (8, 3);
/// Timothé's barrel (6, 1): he stands visible on it (the source's fallback for a hidden trainer).
const TIMOTHE: (i16, i16) = (6, 1);

/// The three movable barrels A (4, 3), D (4, 4) and B (2, 6).
pub(crate) const FUTS: Puzzle = Puzzle {
    map: MAP_PORT_RESSAC_GYM_CALE,
    objects: &[
    (LOCALID_CALE_FUT_A, VAR_TEMP_1, (4, 3)),
    (LOCALID_CALE_FUT_D, VAR_TEMP_2, (4, 4)),
    (LOCALID_CALE_FUT_B, VAR_TEMP_3, (2, 6)),
],
    free: fut_free,
};

/// Whether a barrel can be pushed onto `(x, y)`: a plank, not Timothé's cell. A blown barrel's
/// cell is floor only once blown, which [`fut_free`] cannot read (a `fn`): the explosive cells
/// count as solid, as the barrels they were (a barrel never needs to go there).
fn fut_free(_: (i16, i16), (x, y): (i16, i16)) -> bool {
    let row = usize::try_from(y).ok().and_then(|y| CALE.get(y));
    let cell = row.and_then(|r| usize::try_from(x).ok().and_then(|x| r.as_bytes().get(x)));
    cell == Some(&b'.') && (x, y) != TIMOTHE
}

/// The blown barrels are floor on every load (their flags are permanent).
pub async fn PortRessac_Gym_Cale_OnLoad(ctx: Ctx) {
    repaint_if(&ctx, FLAG_SIL_PORT_CALE_FUT_1, FUT_1, PLANCHER).await;
    repaint_if(&ctx, FLAG_SIL_PORT_CALE_FUT_2, FUT_2, PLANCHER).await;
}

/// A movable barrel (A, D or B): Champ Magnétique pushes it one cell (`push_object`).
pub async fn PortRessac_Gym_Cale_EventScript_Fut(ctx: Ctx) {
    push_object(&ctx).await;
}

/// An explosive barrel: it blows (the source's `screen_flash`), a wild Pokémon jumps out, and
/// the cell is floor for good (`flag`).
async fn explosif(ctx: &Ctx, species: SpeciesId, level: u8, cell: (i16, i16), flag: FlagId) {
    if ctx.flag(flag).await {
        return;
    }
    ctx.lock().await;
    say(ctx, Port_Ressac_Cale_Explosif_T1).await;
    screen_flash(ctx, WHITE).await;
    battle_wild(ctx, species, level).await;
    ctx.set_metatile(cell.0, cell.1, PLANCHER, false).await;
    ctx.set_flag(flag, true).await;
    ctx.release().await;
}

/// Explosive barrel n°1 (7, 6), in the first row: a Voltorb, level 33.
pub async fn PortRessac_Gym_Cale_EventScript_Explosif1(ctx: Ctx) {
    explosif(&ctx, SPECIES_VOLTORB, 33, FUT_1, FLAG_SIL_PORT_CALE_FUT_1).await;
}

/// Explosive barrel n°2 (8, 3), under the ladder: an Electrode, level 34.
pub async fn PortRessac_Gym_Cale_EventScript_Explosif2(ctx: Ctx) {
    explosif(&ctx, SPECIES_ELECTRODE, 34, FUT_2, FLAG_SIL_PORT_CALE_FUT_2).await;
}

/// A loot barrel: searched once (`flag`, its §7 flag), then empty.
async fn butin(ctx: &Ctx, flag: FlagId, item: u16, qty: u16) {
    if ctx.flag(flag).await {
        msgbox(ctx, Port_Ressac_Cale_Vide, MsgBox::Sign).await;
        return;
    }
    // A sign on the barrel (`06_ch4_scripts.ron`), not a Gen3 hidden item: the source gives
    // the barrel its texts and a quantity.
    ctx.lock().await;
    say(ctx, Port_Ressac_Cale_Butin).await;
    gift(ctx, flag, item, qty, Obtain::Message).await;
    ctx.release().await;
}

/// The loot barrel (1, 5): a Hyper Potion.
pub async fn PortRessac_Gym_Cale_EventScript_ButinHyperPotion(ctx: Ctx) {
    butin(
        &ctx,
        FLAG_SIL_ITEM_PORT_CALE_HYPER_POTION,
        ITEM_HYPER_POTION,
        1,
    )
    .await;
}

/// The loot barrel (9, 5): Rhum Pirate ×2.
pub async fn PortRessac_Gym_Cale_EventScript_ButinRhum(ctx: Ctx) {
    butin(&ctx, FLAG_SIL_ITEM_PORT_CALE_RHUM, ITEM_SIL_RHUM_PIRATE, 2).await;
}

/// MARIN TIMOTHÉ, the cabin boy, by his barrel (6, 1), sight 1.
pub async fn PortRessac_Gym_Cale_EventScript_Timothe(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_CALE_TIMOTHE,
        TRAINER_SIL_PORT_GYM_TIMOTHE,
        Port_Ressac_Gym_Timothe_Intro,
        Port_Ressac_Gym_Timothe_Defeat,
    )
    .await
    {
        post_battle(&ctx, Port_Ressac_Gym_Timothe_Defeat).await;
    }
}
