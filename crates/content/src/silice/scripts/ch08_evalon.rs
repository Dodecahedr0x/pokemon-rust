//! Chapter 8, the Bourg d'Évalon (`Evalon`): `contenu-jeu/chapitres/08-evalon.md` §2.2 (the
//! signs, the townsfolk, the crackle, the brambles), §3 Evalon_Sc01 (the arrival), Sc04 (the
//! Épreuve du Fer), Sc06 (the fairy's and the pigeon's quests, outside), EpreuvesFinies, Sc07
//! (the bronze bell: Gogo and Romain's invasion) and Sc10 (the departure), and §7 (the town's
//! items). `VAR_SIL_STORY` 81 (the friends' message, Route S8) → 82 (arrived; the three trials)
//! → 83 (trials done, the drawbridge down) → 84 (the invasion repelled) → 85 (the badge,
//! `ch08_gym.rs`) → 86 (gone to Route S9).
//!
//! The buildings are `ch08_buildings.rs`, the Tower `ch08_tour.rs`, the Galeries
//! `ch08_galeries.rs`, the Citadelle `ch08_gym.rs`. The reputation (`VAR_SIL_EVALON_RENOM`,
//! [`add_renom`], [`fiche_lire`]) and the trials' end ([`epreuves_finies`]) are shared from here.
//!
//! **Temp vars on this map:** `VAR_TEMP_3` is the crackle coords' trigger (0: they fire, and
//! no script writes it: "SIL4 Ch8 maps as built"); `VAR_TEMP_2` is 1 once the Mallette has
//! crackled on this visit; `VAR_TEMP_5` holds the brambles cut on this visit.

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["Evalon"];

// ---- the reputation and the trials ----

/// `VAR_SIL_EVALON_RENOM` + `n`, at most 10 (the source's `min(10, …)`; §3 "Sources de RENOM").
pub(super) async fn add_renom(ctx: &Ctx, n: u16) {
    let r = ctx.var(VAR_SIL_EVALON_RENOM).await;
    ctx.set_var(VAR_SIL_EVALON_RENOM, (r + n).min(10)).await;
}

/// `Evalon_Fiche_Lire`, the Fiche de Personnage's rank by `VAR_SIL_EVALON_RENOM` (0–4 Écuyer,
/// 5–7 Chevalier, 8–9 Paladin, 10 Légende), inside a locked scene.
///
/// fallback: a readable key item (the bag's "read", §9): the source's own fallback, the Doyenne
/// reads the sheet aloud when talked to. Reported (`read_item`).
pub(super) async fn fiche_lire(ctx: &Ctx) {
    fallback(ctx, "read_item").await;
    let text = match ctx.var(VAR_SIL_EVALON_RENOM).await {
        0..=4 => Evalon_Fiche_Ecuyer,
        5..=7 => Evalon_Fiche_Chevalier,
        8..=9 => Evalon_Fiche_Paladin,
        _ => Evalon_Fiche_Legende,
    };
    say(ctx, text).await;
}

/// `Evalon_EpreuvesFinies`, called at the end of each trial (inside its locked scene): once the
/// three trials' flags are set at `VAR_SIL_STORY` 82, the drawbridge line and 83. On Évalon
/// itself (the Épreuve du Fer is the last one), the gate opens at once ([`open_gate`]); else
/// `Evalon_OnLoad` and `Evalon_OnTransition` draw it on the next visit.
pub(super) async fn epreuves_finies(ctx: &Ctx) {
    for f in [
        FLAG_SIL_EVALON_EPREUVE_SCRIBE,
        FLAG_SIL_EVALON_EPREUVE_FER,
        FLAG_SIL_EVALON_EPREUVE_ESPRIT,
    ] {
        if !ctx.flag(f).await {
            return;
        }
    }
    if story(ctx).await != 82 {
        return;
    }
    say(ctx, Evalon_EpreuvesFinies_T1).await;
    set_story(ctx, 83).await;
    if ctx.current_map().await == MAP_EVALON {
        open_gate(ctx).await;
    }
}

/// The Porte du Fer's guards aside from 83 (Gen3 `setobjectxyperm`): Bertrand (22, 12),
/// Perceval (27, 12), Iolanda (28, 10) ("SIL4 Ch8 maps as built").
const GUARDS_ASIDE: [(u8, (i16, i16)); 3] = [
    (LOCALID_EVALON_BERTRAND, (22, 12)),
    (LOCALID_EVALON_PERCEVAL, (27, 12)),
    (LOCALID_EVALON_IOLANDA, (28, 10)),
];

/// The friends below the gate at 83, waiting for Sc07 (left to right: Adrien, Gwada, Mathieu,
/// so that the two who hold the line walk left without crossing the partner).
const FRIENDS_AT_GATE: [(u8, FlagId, (i16, i16)); 3] = [
    (LOCALID_EVALON_ADRIEN, FLAG_SIL_HIDE_EVALON_ADRIEN, (24, 16)),
    (LOCALID_EVALON_GWADA, FLAG_SIL_HIDE_EVALON_GWADA, (25, 16)),
    (LOCALID_EVALON_MATHIEU, FLAG_SIL_HIDE_EVALON_AMIS, (26, 16)),
];

/// The raised drawbridge (24–26, 7–8), solid water in the layout, and SilEvalon's `pont`
/// (520, the lowered oak deck), walkable, once lowered.
const DRAWBRIDGE_X: core::ops::RangeInclusive<i16> = 24..=26;
const DRAWBRIDGE_Y: core::ops::RangeInclusive<i16> = 7..=8;
const METATILE_PONT: u16 = 520;

async fn lower_drawbridge(ctx: &Ctx) {
    for y in DRAWBRIDGE_Y {
        for x in DRAWBRIDGE_X {
            ctx.set_metatile(x, y, METATILE_PONT, false).await;
        }
    }
}

/// The trials done while on Évalon: under a fade, the guards step aside, the drawbridge comes
/// down and the friends wait below the gate (what the next load would draw).
async fn open_gate(ctx: &Ctx) {
    ctx.fade(Fade::ToBlack).await;
    for (obj, (x, y)) in GUARDS_ASIDE {
        ctx.set_object_template_pos(obj, x, y).await;
        ctx.set_object_pos(obj, x, y).await;
    }
    for (obj, flag, (x, y)) in FRIENDS_AT_GATE {
        ctx.set_object_template_pos(obj, x, y).await;
        show(ctx, obj, flag).await;
    }
    lower_drawbridge(ctx).await;
    ctx.redraw_map().await;
    ctx.fade(Fade::FromBlack).await;
}

// ---- map scripts ----

/// The brambles (44, 26–29) before the lost pigeon's pocket.
const RONCES: &[(i16, i16)] = &[(44, 26), (44, 27), (44, 28), (44, 29)];

/// The drawbridge down from `VAR_SIL_STORY` 83; the brambles cut for good once the pigeon is
/// home (Q3). The fountain is dry in the layout and stays so.
///
/// fallback: the flowing fountain (Q1): SilEvalon has no running-water metatile for the basin
/// (18–20, 39–41); the fountain's sign says the water is back instead (reported `fountain`).
pub async fn Evalon_OnLoad(ctx: Ctx) {
    if story(&ctx).await >= 83 {
        lower_drawbridge(&ctx).await;
    }
    if ctx.flag(FLAG_SIL_EVALON_Q3).await {
        for &(x, y) in RONCES {
            ctx.set_metatile(x, y, METATILE_RONCES_COUPEES, false).await;
        }
    }
}

/// The story's actors by `VAR_SIL_STORY` ("SIL4 Ch8 maps as built", hide flags):
/// - the friends and the Scribe by the square at 80–81 (Sc01);
/// - the friends below the gate at 83 (Sc07), at [`FRIENDS_AT_GATE`]; else elsewhere (the
///   Taverne at 82–84, Nébula after the badge);
/// - Gogo, Romain and the grunts only inside Sc07 (which shows them);
/// - the guards aside from 83 ([`GUARDS_ASIDE`]);
/// - the south-east path's guard until the badge (85).
pub async fn Evalon_OnTransition(ctx: Ctx) {
    let s = story(&ctx).await;
    let square = (80..=81).contains(&s);
    for (obj, flag, (x, y)) in FRIENDS_AT_GATE {
        if s == 83 {
            ctx.set_object_template_pos(obj, x, y).await;
        }
        ctx.set_flag(flag, !(square || s == 83)).await;
    }
    ctx.set_flag(FLAG_SIL_HIDE_EVALON_SCRIBE, !square).await;
    for f in [
        FLAG_SIL_HIDE_EVALON_GOGO_ROMAIN,
        FLAG_SIL_HIDE_EVALON_ROMAIN,
        FLAG_SIL_HIDE_EVALON_SBIRE_1,
        FLAG_SIL_HIDE_EVALON_SBIRE_2,
        FLAG_SIL_HIDE_EVALON_SBIRE_3,
    ] {
        ctx.set_flag(f, true).await;
    }
    if s >= 83 {
        for (obj, (x, y)) in GUARDS_ASIDE {
            ctx.set_object_template_pos(obj, x, y).await;
        }
    }
    ctx.set_flag(FLAG_SIL_HIDE_EVALON_GARDE_SE, s >= 85).await;
}

// ---- Sc01: the arrival ----

const MATHIEU_IN: &[Step] = &[
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    Step::FacePlayer,
];
const FRIEND_IN: &[Step] = &[
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    Step::FacePlayer,
];
const SCRIBE_IN: &[Step] = &[
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    Step::FacePlayer,
];
const SCRIBE_OUT: &[Step] = &[
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
];

/// Evalon_Sc01, the Mallette crackles and the friends: the row (23–26, 45) past the south
/// portal at `VAR_SIL_STORY` 81. Mathieu comes down from (24, 40) to (24, 44), Adrien and Gwada
/// to (23, 43) and (26, 43), the Scribe from (17, 44) to (22, 44) and back. Sets
/// `VAR_SIL_EVALON_STATE` 1, the reputation 0, the story 82.
pub async fn Evalon_Sc01(ctx: Ctx) {
    if story(&ctx).await != 81 {
        return;
    }
    ctx.lock_all().await;
    // fallback: play_se(SE_GRESILLEMENT), a new sound (§9): the decomp's static zap stands in
    // (no sound yet either way); T1 says "BZZT".
    play_se(&ctx, "SE_M_THUNDER_WAVE").await;
    screen_shake(&ctx, 1, 8).await;
    say(&ctx, Evalon_Sc01_T1).await;
    move_wait(&ctx, LOCALID_EVALON_MATHIEU, MATHIEU_IN).await;
    say(&ctx, Evalon_Sc01_T2).await;
    ctx.move_obj(LOCALID_EVALON_ADRIEN, FRIEND_IN).await;
    move_wait(&ctx, LOCALID_EVALON_GWADA, FRIEND_IN).await;
    ctx.wait_move(LOCALID_EVALON_ADRIEN).await;
    say(&ctx, Evalon_Sc01_T3).await;
    say(&ctx, Evalon_Sc01_T4).await;
    play_se(&ctx, "SE_M_THUNDER_WAVE").await;
    say(&ctx, Evalon_Sc01_T5).await;
    say(&ctx, Evalon_Sc01_T6).await;
    move_wait(&ctx, LOCALID_EVALON_SCRIBE, SCRIBE_IN).await;
    say(&ctx, Evalon_Sc01_T7).await;
    say(&ctx, Evalon_Sc01_T8).await;
    ctx.set_var(VAR_SIL_EVALON_STATE, 1).await;
    ctx.set_var(VAR_SIL_EVALON_RENOM, 0).await;
    set_story(&ctx, 82).await;
    move_wait(&ctx, LOCALID_EVALON_SCRIBE, SCRIBE_OUT).await;
    hide(&ctx, LOCALID_EVALON_SCRIBE).await;
    ctx.release_all().await;
}

// ---- Sc04: the Épreuve du Fer ----

/// Evalon_Sc04, the Porte du Fer: any of the three guards talked to. Perceval's challenge, then
/// each guard not yet beaten in turn (Bertrand, Iolanda, Perceval; a loss whites out and the
/// next talk starts from the next unbeaten guard, as the source's rule). The Lum Berry,
/// `FLAG_SIL_EVALON_EPREUVE_FER`, +2 reputation, the trials' end. Once passed, the talked-to
/// guard's defeat line.
pub async fn Evalon_Sc04(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if ctx.flag(FLAG_SIL_EVALON_EPREUVE_FER).await {
        let text = match ctx.var(VAR_LAST_TALKED).await as u8 {
            LOCALID_EVALON_BERTRAND => Evalon_Bertrand_Defaite,
            LOCALID_EVALON_IOLANDA => Evalon_Iolanda_Defaite,
            _ => Evalon_Perceval_Defaite,
        };
        say(&ctx, text).await;
        ctx.release().await;
        return;
    }
    say(&ctx, Evalon_Sc04_T1).await;
    for (t, intro, defeat, after) in [
        (
            TRAINER_SIL_GARDE_BERTRAND,
            Evalon_Bertrand_Intro,
            Evalon_Bertrand_Defaite,
            Evalon_Sc04_T2,
        ),
        (
            TRAINER_SIL_GARDE_IOLANDA,
            Evalon_Iolanda_Intro,
            Evalon_Iolanda_Defaite,
            Evalon_Sc04_T3,
        ),
    ] {
        if !defeated(&ctx, t).await {
            say(&ctx, intro).await;
            if lost(battle_trainer(&ctx, t, defeat).await) {
                return;
            }
            say(&ctx, after).await;
        }
    }
    say(&ctx, Evalon_Perceval_Intro).await;
    if lost(
        battle_trainer(
            &ctx,
            TRAINER_SIL_CAPITAINE_PERCEVAL,
            Evalon_Perceval_Defaite,
        )
        .await,
    ) {
        return;
    }
    say(&ctx, Evalon_Sc04_T4).await;
    gift(
        &ctx,
        FLAG_SIL_ITEM_EVALON_LUM,
        ITEM_LUM_BERRY,
        1,
        Obtain::Message,
    )
    .await;
    ctx.set_flag(FLAG_SIL_EVALON_EPREUVE_FER, true).await;
    add_renom(&ctx, 2).await;
    epreuves_finies(&ctx).await;
    ctx.release().await;
}

/// Whether a scripted battle ended in a white-out (the script stops there).
pub(super) fn lost(o: BattleOutcome) -> bool {
    matches!(o, BattleOutcome::Lost | BattleOutcome::Drew)
}

// ---- Sc07: the bronze bell ----

/// Where Sc07 stands the player: the drawbridge's foot (25, 10), on the trigger row.
const SC07_AT: i16 = 25;
const STEP_LEFT: &[Step] = &[walk(Dir::Left)];
const STEP_RIGHT: &[Step] = &[walk(Dir::Right)];
const FACE_PLAYER: &[Step] = &[Step::FacePlayer];
const GOGO_IN: &[Step] = &[
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
];
const ROMAIN_IN: &[Step] = &[
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
];
const FRIEND_UP: &[Step] = &[
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    face(Dir::Left),
];
const HOLD_LINE: &[Step] = &[walk(Dir::Left), walk(Dir::Left)];
const SBIRE_STAMP: &[Step] = &[walk_in_place(Dir::Right)];

/// The invaders shown in Sc07, with their hide flags.
const INVADERS: [(u8, FlagId); 5] = [
    (LOCALID_EVALON_GOGO, FLAG_SIL_HIDE_EVALON_GOGO_ROMAIN),
    (LOCALID_EVALON_ROMAIN, FLAG_SIL_HIDE_EVALON_ROMAIN),
    (LOCALID_EVALON_SBIRE_1, FLAG_SIL_HIDE_EVALON_SBIRE_1),
    (LOCALID_EVALON_SBIRE_2, FLAG_SIL_HIDE_EVALON_SBIRE_2),
    (LOCALID_EVALON_SBIRE_3, FLAG_SIL_HIDE_EVALON_SBIRE_3),
];

/// Evalon_Sc07, the bronze bell: the yard's row y 10 (x 11–38) at `VAR_SIL_STORY` 83. The
/// player is walked to the drawbridge's foot (25, 10); Gogo and Romain come from the Galeries'
/// side (west), the friends up from below the gate (24–26, 16) to (24–26, 11). The battle, the
/// Plan de la Prise d'Eau, the Pain de Lembas, `FLAG_SIL_EVALON_INVASION`, +1 reputation.
/// Sets 84.
pub async fn Evalon_Sc07(ctx: Ctx) {
    if story(&ctx).await != 83 {
        return;
    }
    ctx.lock_all().await;
    loop {
        let (x, _) = ctx.player_pos().await;
        if x == SC07_AT {
            break;
        }
        let step = if x < SC07_AT { STEP_RIGHT } else { STEP_LEFT };
        move_wait(&ctx, PLAYER, step).await;
    }
    // fallback: play_sfx(SE_CLOCHE_BRONZE) x3 (§9, a new sound): the decomp's Heal Bell chime
    // once (no sound yet either way); T1 says "DONG! DONG! DONG!".
    play_se(&ctx, "SE_M_HEAL_BELL").await;
    // fallback: play_music(MUS_ENCOUNTER_INTENSE) (audio, M12): the map's music goes on.
    fallback(&ctx, "play_music").await;
    say(&ctx, Evalon_Sc07_T1).await;
    // The source's `Walk(Down) x2` would take Perceval (aside at (27, 12)) into the gate's
    // wall: he turns to the player instead.
    move_wait(&ctx, LOCALID_EVALON_PERCEVAL, FACE_PLAYER).await;
    say(&ctx, Evalon_Sc07_T2).await;
    ctx.turn(PLAYER, Dir::Left).await;
    show(&ctx, INVADERS[0].0, INVADERS[0].1).await;
    show(&ctx, INVADERS[1].0, INVADERS[1].1).await;
    ctx.move_obj(LOCALID_EVALON_ROMAIN, ROMAIN_IN).await;
    move_wait(&ctx, LOCALID_EVALON_GOGO, GOGO_IN).await;
    ctx.wait_move(LOCALID_EVALON_ROMAIN).await;
    for &(obj, flag) in &INVADERS[2..] {
        show(&ctx, obj, flag).await;
    }
    say(&ctx, Evalon_Sc07_T3).await;
    say(&ctx, Evalon_Sc07_T4).await;
    for (obj, _, _) in FRIENDS_AT_GATE {
        ctx.move_obj(obj, FRIEND_UP).await;
    }
    for (obj, _, _) in FRIENDS_AT_GATE {
        ctx.wait_move(obj).await;
    }
    say(&ctx, Evalon_Sc07_T5).await;
    say(&ctx, Evalon_Sc07_T6).await;
    say(&ctx, Evalon_Sc07_T7).await;
    say(&ctx, Evalon_Sc07_T8).await;
    // fallback: multichoice(["ADRIEN", "GWADA", "MATHIEU"]) (§9): the source's own fallback,
    // Mathieu chosen.
    fallback(&ctx, "multichoice").await;
    ctx.set_var(VAR_SIL_EVALON_PARTENAIRE, 3).await;
    say(&ctx, Evalon_Sc07_Partner_T2).await;
    say(&ctx, Evalon_Sc07_Others_T2).await;
    ctx.move_obj(LOCALID_EVALON_ADRIEN, HOLD_LINE).await;
    move_wait(&ctx, LOCALID_EVALON_GWADA, HOLD_LINE).await;
    ctx.wait_move(LOCALID_EVALON_ADRIEN).await;
    for &(obj, _) in &INVADERS[2..] {
        ctx.move_obj(obj, SBIRE_STAMP).await;
    }
    ctx.wait_move(LOCALID_EVALON_SBIRE_3).await;
    // fallback: play_music(MUS_VS_AQUA_MAGMA_LEADER) (audio, M12).
    fallback(&ctx, "play_music").await;
    // fallback: battle_multi(GOGO_4, ROMAIN_4, partner, field = SURDOSE) (§9, no Multi battle
    // and no field effect before M11): the source's own fallback, two single battles in a row
    // with a heal between them (the partner's Berries). A white-out replays the scene; a
    // beaten Gogo is not fought again.
    fallback(&ctx, "battle_multi").await;
    if !defeated(&ctx, TRAINER_SIL_GOGO_4).await
        && lost(battle_trainer(&ctx, TRAINER_SIL_GOGO_4, Evalon_Gogo_Defaite).await)
    {
        return;
    }
    say(&ctx, Evalon_Sc07_Repli_T1).await;
    ctx.heal_party().await;
    if lost(battle_trainer(&ctx, TRAINER_SIL_ROMAIN_4, Evalon_Romain_Defaite).await) {
        return;
    }
    say(&ctx, Evalon_Sc07_T9).await;
    say(&ctx, Evalon_Sc07_T10).await;
    say(&ctx, Evalon_Sc07_T11).await;
    give_item(&ctx, ITEM_SIL_PLAN_PRISE_EAU, 1).await;
    say(&ctx, Evalon_Sc07_T12).await;
    ctx.fade(Fade::ToBlack).await;
    for (obj, _) in INVADERS {
        hide(&ctx, obj).await;
    }
    ctx.fade(Fade::FromBlack).await;
    say(&ctx, Evalon_Sc07_T13).await;
    say(&ctx, Evalon_Sc07_T14).await;
    say(&ctx, Evalon_Sc07_T15).await;
    say(&ctx, Evalon_Sc07_T16).await;
    give_item(&ctx, ITEM_SIL_PAIN_LEMBAS, 1).await;
    ctx.heal_party().await;
    ctx.set_flag(FLAG_SIL_EVALON_INVASION, true).await;
    add_renom(&ctx, 1).await;
    set_story(&ctx, 84).await;
    // fallback: play_music(MUS_FALLARBOR) (audio, M12).
    fallback(&ctx, "play_music").await;
    ctx.release_all().await;
}

// ---- Sc10: the departure ----

/// Evalon_Sc10, the music fades: the south portal's row (23–26, 47) or the south-east path's
/// cells (42–43, 45), (47, 44–45) at `VAR_SIL_STORY` 85. Sets 86.
pub async fn Evalon_Sc10(ctx: Ctx) {
    if story(&ctx).await != 85 {
        return;
    }
    ctx.lock_all().await;
    // fallback: fade_music(Out) (audio, M12): the source's own fallback is MUS_NONE; nothing
    // plays either way.
    fallback(&ctx, "fade_music").await;
    ctx.delay(60).await;
    say(&ctx, Evalon_Sc10_T1).await;
    set_story(&ctx, 86).await;
    ctx.release_all().await;
}

// ---- the crackle ----

/// The Mallette's crackle (§9 "Grésillement aléatoire": every 64 steps, one chance in 4).
///
/// fallback: a step counter (§9): the source's own fallback, three fixed coords (24, 36),
/// (8, 31), (40, 32) on `VAR_TEMP_3` 0. Each fires one time in four, at most once a visit
/// (`VAR_TEMP_2`), after the arrival. `Evalon_Sc01_T1` is the crackle's line (author question:
/// no line of its own).
pub async fn Evalon_EventScript_Gresillement(ctx: Ctx) {
    if story(&ctx).await < 82 || ctx.var(VAR_TEMP_2).await != 0 {
        return;
    }
    if ctx.random(4).await != 0 {
        return;
    }
    ctx.set_var(VAR_TEMP_2, 1).await;
    ctx.lock_all().await;
    play_se(&ctx, "SE_M_THUNDER_WAVE").await;
    say(&ctx, Evalon_Sc01_T1).await;
    ctx.release_all().await;
}

// ---- the quests outside (Q1, Q3) ----

/// Q1, the fairy by the dry fountain: the canal is blocked (`Evalon_Q1_T1`) until the Galeries'
/// rock is broken (`FLAG_SIL_EVALON_Q1_ROCHER`); then the Larme de Fée, `FLAG_SIL_EVALON_Q1`,
/// +1 reputation (once). Afterwards she says her thanks again (author question: no line).
pub async fn Evalon_EventScript_Fee(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if !ctx.flag(FLAG_SIL_EVALON_Q1_ROCHER).await {
        say(&ctx, Evalon_Q1_T1).await;
    } else {
        say(&ctx, Evalon_Q1_T2).await;
        gift(
            &ctx,
            FLAG_SIL_ITEM_EVALON_LARME_FEE,
            ITEM_SIL_LARME_FEE,
            1,
            Obtain::Message,
        )
        .await;
        if !ctx.flag(FLAG_SIL_EVALON_Q1).await {
            ctx.set_flag(FLAG_SIL_EVALON_Q1, true).await;
            add_renom(&ctx, 1).await;
        }
    }
    ctx.release().await;
}

/// Q3, Plume-d'Argent behind the brambles: it flies home to the Pigeonnier (its hide flag,
/// which the keeper reads).
pub async fn Evalon_EventScript_Pigeon(ctx: Ctx) {
    ctx.lock().await;
    say(&ctx, Evalon_Q3_Pigeon).await;
    hide(&ctx, LOCALID_EVALON_PIGEON).await;
    ctx.release().await;
}

/// The brambles (44, 26–29) before the pigeon's pocket (they regrow on reload).
pub async fn Evalon_EventScript_Ronces(ctx: Ctx) {
    super::ch08_route_s08::ronces(&ctx, RONCES, VAR_TEMP_5, METATILE_RONCES_COUPEES).await;
}

/// The fountain (19, 41), read from its south rim: the quest board's line (author question:
/// the source writes no line for the fountain itself).
pub async fn Evalon_EventScript_Fontaine(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_EVALON_Q1_ROCHER).await {
        // fallback: the flowing fountain (`Evalon_OnLoad`'s note): the sign stands in.
        fallback(&ctx, "fountain").await;
    }
    msgbox(&ctx, Evalon_Sign_5, MsgBox::Sign).await;
}

// ---- people ----

/// The child with the wooden sword; after the invasion, `Evalon_Npc_1b`.
pub async fn Evalon_EventScript_Enfant(ctx: Ctx) {
    let text = if ctx.flag(FLAG_SIL_EVALON_INVASION).await {
        Evalon_Npc_1b
    } else {
        Evalon_Npc_1
    };
    msgbox(&ctx, text, MsgBox::Npc).await;
}

pub async fn Evalon_EventScript_Mere(ctx: Ctx) {
    msgbox(&ctx, Evalon_Npc_2, MsgBox::Npc).await;
}

pub async fn Evalon_EventScript_Roliste(ctx: Ctx) {
    msgbox(&ctx, Evalon_Npc_3, MsgBox::Npc).await;
}

pub async fn Evalon_EventScript_Chaudronnier(ctx: Ctx) {
    msgbox(&ctx, Evalon_Npc_4, MsgBox::Npc).await;
}

pub async fn Evalon_EventScript_VieilleDame(ctx: Ctx) {
    msgbox(&ctx, Evalon_Npc_5, MsgBox::Npc).await;
}

/// The keeper's apprentice before the Pigeonnier (the source's colombophile line, `Npc_6`; the
/// keeper himself is in the Taverne's Pigeonnier, "SIL4 Ch8 maps as built").
pub async fn Evalon_EventScript_ApprentiColombophile(ctx: Ctx) {
    msgbox(&ctx, Evalon_Npc_6, MsgBox::Npc).await;
}

pub async fn Evalon_EventScript_GardeBeffroi(ctx: Ctx) {
    msgbox(&ctx, Evalon_Npc_7, MsgBox::Npc).await;
}

pub async fn Evalon_EventScript_Mj(ctx: Ctx) {
    msgbox(&ctx, Evalon_Npc_8, MsgBox::Npc).await;
}

/// The south-east path's guard (44, 45), until the badge (`Evalon_OnTransition` hides him from
/// 85).
pub async fn Evalon_EventScript_GardeSE(ctx: Ctx) {
    msgbox(&ctx, Evalon_Garde_SE, MsgBox::Npc).await;
}

// ---- signs and items ----

pub async fn Evalon_EventScript_Sign1(ctx: Ctx) {
    msgbox(&ctx, Evalon_Sign_1, MsgBox::Sign).await;
}

pub async fn Evalon_EventScript_Sign2(ctx: Ctx) {
    msgbox(&ctx, Evalon_Sign_2, MsgBox::Sign).await;
}

pub async fn Evalon_EventScript_Sign3(ctx: Ctx) {
    msgbox(&ctx, Evalon_Sign_3, MsgBox::Sign).await;
}

pub async fn Evalon_EventScript_Sign4(ctx: Ctx) {
    msgbox(&ctx, Evalon_Sign_4, MsgBox::Sign).await;
}

pub async fn Evalon_EventScript_Sign5(ctx: Ctx) {
    msgbox(&ctx, Evalon_Sign_5, MsgBox::Sign).await;
}

/// The mill pond's islet (Surf).
pub async fn Evalon_EventScript_ItemPpUp(ctx: Ctx) {
    find_item(&ctx, ITEM_PP_UP, 1).await;
}

/// The Revive at the belfry's foot (31, 15).
pub async fn Evalon_EventScript_HiddenRevive(ctx: Ctx) {
    // fallback: hidden items (the engine ignores `BgKind::HiddenItem`; the M9 finding of
    // "SIL1 Task F as built"): the map has a sign here instead (`06_ch8_scripts.ron`).
    hidden_item(&ctx, FLAG_SIL_ITEM_EVALON_CACHE_1, ITEM_REVIVE).await;
}
