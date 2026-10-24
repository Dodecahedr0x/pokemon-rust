//! Chapter 4, the port (`PortRessac`): `contenu-jeu/chapitres/04-port-ressac.md` §2 (signs,
//! ambient NPCs, the rusty distributor), §3 Port_Ressac_Sc01 to Sc06 (the arrival, Crow's
//! lookout, Gogo and Romain, the log shown to Samuel, the quay cleaning, Julien's call), the
//! cargo's guard and §7 (the items). `VAR_SIL_STORY` runs 41 (out of the tunnel) to 49 (the
//! chapter's end, Gwada beaten). The Taverne and the cargo are `ch04_buildings.rs`, the
//! Zenith-Grog `ch04_gym.rs`.
//!
//! Also here: [`push_object`], the Champ Magnétique shared with every push puzzle, and
//! the end of Samuel's lost battle (`PortRessac_EventScript_SamuelPerdu`, R-7).

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["PortRessac"];

/// The open sea (General 368) that replaces the cargo once it has sailed.
const METATILE_MER: u16 = 368;
/// The cargo, its shoal, its stairs and its dock: x 4–11, rows 22–26 ("SIL3 Ch4 maps as built").
const CARGO_X: core::ops::RangeInclusive<i16> = 4..=11;
const CARGO_Y: core::ops::RangeInclusive<i16> = 22..=26;

// ---- the Champ Magnétique (Strength) ----

pub(crate) use crate::silice::push::Puzzle;

/// The source's Champ Magnétique on a pushable object (`FLAG_SIL_MODULE_FORCE`; Emerald's
/// Strength, `EventScript_StrengthBoulder` with the module for the party move): without the
/// module, Ch3's `Depot_Bloc_T1`; with it, the first time on this map load, `Depot_Bloc_T2`
/// and Strength in use (`FLAG_SYS_USE_STRENGTH`, cleared on every map load). The player then
/// pushes the object by walking into it, within its puzzle's rule (`silice::push`).
///
/// `pub(super)`: shared with every puzzle, and not a script (the registry takes the `pub`
/// async fns).
pub(super) async fn push_object(ctx: &Ctx) {
    if !ctx.flag(FLAG_SIL_MODULE_FORCE).await {
        msgbox(ctx, Depot_Bloc_T1, MsgBox::Sign).await;
        return;
    }
    if ctx.flag(FLAG_SYS_USE_STRENGTH).await {
        return;
    }
    ctx.lock().await;
    say(ctx, Depot_Bloc_T2).await;
    ctx.set_flag(FLAG_SYS_USE_STRENGTH, true).await;
    ctx.release().await;
}

/// The blue container (6, 15) and the sand pocket it can move in: pushed north from (6, 16)
/// into (6, 14), it opens the corridor west to the Water Stone (4, 15).
pub(crate) const CONTAINER: Puzzle = Puzzle {
    map: MAP_PORT_RESSAC,
    objects: &[(LOCALID_PORT_CONTAINER, VAR_TEMP_1, (6, 15))],
    free: container_free,
};

fn container_free(_: (i16, i16), c: (i16, i16)) -> bool {
    matches!(c, (6, 14) | (4, 15) | (5, 15) | (6, 15) | (6, 16))
}

pub async fn PortRessac_EventScript_Container(ctx: Ctx) {
    push_object(&ctx).await;
}

// ---- map scripts ----

/// The cargo, gone with the Cocket crew (`FLAG_SIL_HIDE_PORT_CARGO`, Sc05): the open sea on
/// every load. `setmetatile` keeps each cell's elevation, so the dock's cells stay at the land
/// elevation (unreachable on foot either way).
pub async fn PortRessac_OnLoad(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_HIDE_PORT_CARGO).await {
        sail_cargo(&ctx).await;
    }
}

async fn sail_cargo(ctx: &Ctx) {
    for y in CARGO_Y {
        for x in CARGO_X {
            ctx.set_metatile(x, y, METATILE_MER, false).await;
        }
    }
}

/// The scenes' actors by story step: the passers-by and Samuel only inside their scenes; Crow's
/// client until the lookout; the gangway pirate until the cleaning (46); the cleaning's cast
/// (Samuel's pirates, the quay grunts, the foreman, the decor grunts) at 46 only (the source's
/// `FLAG_SIL_HIDE_PORT_QUAIS_SBIRES` window, one hide flag per actor here).
pub async fn PortRessac_OnTransition(ctx: Ctx) {
    let s = story(&ctx).await;
    for f in [
        FLAG_SIL_HIDE_PORT_PASSANT_1,
        FLAG_SIL_HIDE_PORT_PASSANT_2,
        FLAG_SIL_HIDE_PORT_PASSANT_3,
        FLAG_SIL_HIDE_PORT_PASSANT_4,
        FLAG_SIL_HIDE_PORT_SAMUEL,
    ] {
        ctx.set_flag(f, true).await;
    }
    let client_gone = ctx.flag(FLAG_SIL_PORT_CROW_GUET).await;
    ctx.set_flag(FLAG_SIL_HIDE_PORT_MARIN_CLIENT, client_gone)
        .await;
    ctx.set_flag(FLAG_SIL_HIDE_PORT_PIRATE_PASSERELLE, s >= 46)
        .await;
    for (_, f) in QUAY_CAST {
        ctx.set_flag(f, s != 46).await;
    }
}

/// The quay cleaning's cast, in local-id order: Samuel's pirates (16–18), the quay grunts
/// (19–22), the foreman (23), the decor grunts (24–26).
const QUAY_CAST: [(u8, FlagId); 11] = [
    (LOCALID_PORT_PIRATE_1, FLAG_SIL_HIDE_PORT_PIRATE_1),
    (LOCALID_PORT_PIRATE_2, FLAG_SIL_HIDE_PORT_PIRATE_2),
    (LOCALID_PORT_PIRATE_3, FLAG_SIL_HIDE_PORT_PIRATE_3),
    (LOCALID_PORT_SBIRE_1, FLAG_SIL_HIDE_PORT_SBIRE_1),
    (LOCALID_PORT_SBIRE_2, FLAG_SIL_HIDE_PORT_SBIRE_2),
    (LOCALID_PORT_SBIRE_3, FLAG_SIL_HIDE_PORT_SBIRE_3),
    (LOCALID_PORT_SBIRE_4, FLAG_SIL_HIDE_PORT_SBIRE_4),
    (LOCALID_PORT_CONTREMAITRE, FLAG_SIL_HIDE_PORT_CONTREMAITRE),
    (LOCALID_PORT_SBIRE_DECOR_1, FLAG_SIL_HIDE_PORT_DECOR_1),
    (LOCALID_PORT_SBIRE_DECOR_2, FLAG_SIL_HIDE_PORT_DECOR_2),
    (LOCALID_PORT_SBIRE_DECOR_3, FLAG_SIL_HIDE_PORT_DECOR_3),
];

// ---- scenes ----

/// Port_Ressac_Sc01, the arrival: the map's on-frame entry at `VAR_SIL_STORY` 41. Sets 42.
pub async fn PortRessac_Sc01(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Port_Ressac_Sc01_T1).await;
    set_story(&ctx, 42).await;
    ctx.release_all().await;
}

/// The source's `fade(Out) ; warp(MAP_PORT_RESSAC, None, x, y) ; fade(In)` inside a scene: a
/// warp would end the script (the map load), so the player is moved on the map instead, behind
/// the fade, facing `dir`.
async fn reposition(ctx: &Ctx, (x, y): (i16, i16), dir: Dir) {
    ctx.fade(Fade::ToBlack).await;
    ctx.set_object_pos(PLAYER, x, y).await;
    ctx.turn(PLAYER, dir).await;
    ctx.redraw_map().await;
    ctx.fade(Fade::FromBlack).await;
}

/// The alley's corner on the quay, facing the passers-by (east), and the cell in front of Crow.
const GUET: (i16, i16) = (41, 18);
const DEVANT_CROW: (i16, i16) = (44, 16);

const LEFT_4: &[Step] = &[
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
];
const LEFT_3: &[Step] = &[walk(Dir::Left), walk(Dir::Left), walk(Dir::Left)];
const STAGGER: &[Step] = &[
    walk(Dir::Left),
    Step::Delay(8),
    walk(Dir::Left),
    walk(Dir::Left),
];
const MIAOUSS: &[Step] = &[Step::Jump(Dir::Left, 0), walk_in_place(Dir::Left)];
const FACE_PLAYER: &[Step] = &[Step::FacePlayer];

/// One passer-by of the lookout: shown, walked in, described, then "whistle?". A right call
/// scores (`VAR_SIL_PORT_RESSAC_STATE` + 1, `_Bon`); a whistle for nothing costs Crow time
/// (`_Faux`); a missed patrol sees the crate and fights (`seen`). The passer-by leaves.
async fn passant(
    ctx: &Ctx,
    (obj, flag): (u8, FlagId),
    walk_in: &'static [Step],
    text: TextId,
    patrol: Option<(TrainerId, TextId, TextId)>,
) {
    show(ctx, obj, flag).await;
    move_wait(ctx, obj, walk_in).await;
    say(ctx, text).await;
    let whistle = ask(ctx, Port_Ressac_Sc02_Siffler).await;
    if whistle {
        // fallback: play_se (audio, M12). SE_SIFFLET is the decomp's SE_M_GRASSWHISTLE.
        play_se(ctx, "SE_M_GRASSWHISTLE").await;
    }
    match (patrol, whistle) {
        (Some(_), true) | (None, false) => {
            say(ctx, Port_Ressac_Sc02_Bon).await;
            let n = ctx.var(VAR_SIL_PORT_RESSAC_STATE).await;
            ctx.set_var(VAR_SIL_PORT_RESSAC_STATE, n + 1).await;
        }
        (None, true) => say(ctx, Port_Ressac_Sc02_Faux).await,
        (Some((trainer, seen, defeat)), false) => {
            move_wait(ctx, obj, FACE_PLAYER).await;
            say(ctx, seen).await;
            battle_trainer(ctx, trainer, defeat).await;
        }
    }
    hide(ctx, obj).await;
}

/// Port_Ressac_Sc02, Crow's lookout: Crow (obj 5) at `VAR_SIL_STORY` 42. The player keeps watch
/// at the alley's corner while four passers-by come along the quay; whistle for the Cocket
/// patrols only (`VAR_SIL_PORT_RESSAC_STATE` counts the right calls, reset at the end). Then
/// TM31, the Star Piece for 4 out of 4, the cargo's guard asleep (`FLAG_SIL_PORT_GARDE_DORT`)
/// and the client gone. Sets 43. Outside 42: his tip about the cargo, `_T6` (the source has no
/// other line for him).
pub async fn PortRessac_Sc02(ctx: Ctx) {
    if story(&ctx).await != 42 {
        msgbox(&ctx, Port_Ressac_Sc02_T6, MsgBox::Npc).await;
        return;
    }
    ctx.lock_all().await;
    ctx.face_player().await;
    if !ask(&ctx, Port_Ressac_Sc02_T1).await {
        say(&ctx, Port_Ressac_Sc02_T2).await;
        ctx.release_all().await;
        return;
    }
    say(&ctx, Port_Ressac_Sc02_T3).await;
    ctx.set_var(VAR_SIL_PORT_RESSAC_STATE, 0).await;
    reposition(&ctx, GUET, Dir::Right).await;
    passant(
        &ctx,
        (LOCALID_PORT_PASSANT_1, FLAG_SIL_HIDE_PORT_PASSANT_1),
        LEFT_4,
        Port_Ressac_Sc02_P1,
        Some((
            TRAINER_SIL_PORT_GUET_1,
            Port_Ressac_Sc02_P1_Vu,
            Port_Ressac_Sc02_P1_Defeat,
        )),
    )
    .await;
    passant(
        &ctx,
        (LOCALID_PORT_PASSANT_2, FLAG_SIL_HIDE_PORT_PASSANT_2),
        STAGGER,
        Port_Ressac_Sc02_P2,
        None,
    )
    .await;
    passant(
        &ctx,
        (LOCALID_PORT_PASSANT_3, FLAG_SIL_HIDE_PORT_PASSANT_3),
        MIAOUSS,
        Port_Ressac_Sc02_P3,
        None,
    )
    .await;
    passant(
        &ctx,
        (LOCALID_PORT_PASSANT_4, FLAG_SIL_HIDE_PORT_PASSANT_4),
        LEFT_3,
        Port_Ressac_Sc02_P4,
        Some((
            TRAINER_SIL_PORT_GUET_2,
            Port_Ressac_Sc02_P4_Vu,
            Port_Ressac_Sc02_P4_Defeat,
        )),
    )
    .await;
    reposition(&ctx, DEVANT_CROW, Dir::Up).await;
    move_wait(&ctx, LOCALID_PORT_CROW, FACE_PLAYER).await;
    say(&ctx, Port_Ressac_Sc02_T4).await;
    // "Une CT qui n'existe pas. Garde-la." does not say it is obtained: Gen3's messages.
    give_item(&ctx, ITEM_TM31, 1).await;
    if ctx.var(VAR_SIL_PORT_RESSAC_STATE).await == 4 {
        say(&ctx, Port_Ressac_Sc02_T5).await;
        give_item(&ctx, ITEM_STAR_PIECE, 1).await;
    }
    say(&ctx, Port_Ressac_Sc02_T6).await;
    ctx.set_var(VAR_SIL_PORT_RESSAC_STATE, 0).await;
    ctx.set_flag(FLAG_SIL_PORT_CROW_GUET, true).await;
    // The source's sleeping guard "changes sprite": he has none yet; his line follows the flag.
    ctx.set_flag(FLAG_SIL_PORT_GARDE_DORT, true).await;
    hide(&ctx, LOCALID_PORT_MARIN_CLIENT).await;
    set_story(&ctx, 43).await;
    ctx.release_all().await;
}

const ROMAIN_SWAY: &[Step] = &[
    walk_in_place(Dir::Left),
    walk_in_place(Dir::Left),
    face(Dir::Down),
];
const FACE_UP: &[Step] = &[face(Dir::Up)];
const FACE_RIGHT: &[Step] = &[face(Dir::Right)];
const ROMAIN_IN: &[Step] = &[walk(Dir::Down), Step::FacePlayer];

/// Port_Ressac_Sc03, Gogo and Romain drunk on the jetty: the jetty's coords at `VAR_SIL_STORY`
/// 43 (or talking to either of them). The Vieux Patch (`FLAG_SIL_PORT_VIEUX_PATCH`, its §7
/// flag). Sets 44.
pub async fn PortRessac_Sc03(ctx: Ctx) {
    if story(&ctx).await != 43 {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Port_Ressac_Sc03_T1).await;
    move_wait(&ctx, LOCALID_PORT_ROMAIN, ROMAIN_SWAY).await;
    say(&ctx, Port_Ressac_Sc03_T2).await;
    move_wait(&ctx, LOCALID_PORT_GOGO, FACE_UP).await;
    say(&ctx, Port_Ressac_Sc03_T3).await;
    move_wait(&ctx, LOCALID_PORT_ROMAIN, ROMAIN_IN).await;
    say(&ctx, Port_Ressac_Sc03_T4).await;
    // T5 names the item, it does not say it is obtained: Gen3's messages first.
    gift(
        &ctx,
        FLAG_SIL_PORT_VIEUX_PATCH,
        ITEM_SIL_VIEUX_PATCH,
        1,
        Obtain::Message,
    )
    .await;
    say(&ctx, Port_Ressac_Sc03_T5).await;
    move_wait(&ctx, LOCALID_PORT_GOGO, FACE_RIGHT).await;
    say(&ctx, Port_Ressac_Sc03_T6).await;
    set_story(&ctx, 44).await;
    ctx.release_all().await;
}

/// Gogo, sitting over the water: the scene at 43; else his last line, `Sc03_T6` (the source has
/// none of its own for him).
pub async fn PortRessac_EventScript_Gogo(ctx: Ctx) {
    if story(&ctx).await == 43 {
        PortRessac_Sc03(ctx).await;
        return;
    }
    msgbox(&ctx, Port_Ressac_Sc03_T6, MsgBox::Npc).await;
}

/// Romain, swaying: the scene at 43; else their song, `Sc03_T1`.
pub async fn PortRessac_EventScript_Romain(ctx: Ctx) {
    if story(&ctx).await == 43 {
        PortRessac_Sc03(ctx).await;
        return;
    }
    msgbox(&ctx, Port_Ressac_Sc03_T1, MsgBox::Npc).await;
}

const UP_2: &[Step] = &[walk(Dir::Up), walk(Dir::Up)];
const SAMUEL_DOWN: &[Step] = &[
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    Step::FacePlayer,
];
const SAMUEL_READS: &[Step] = &[walk_in_place(Dir::Down), walk_in_place(Dir::Down)];

/// Port_Ressac_Sc04, the log shown to Samuel: the gangway pirate (obj 14) at `VAR_SIL_STORY`
/// 45. Samuel comes down to the dock (32, 26) and stays there until the map reloads: the player
/// ends the scene on the gate's coord cell (31, 26), so with the dock empty a step right and up
/// would board the Zenith-Grog during the cleaning. His crew and the Cocket quay cast appear
/// (the source's `FLAG_SIL_HIDE_PORT_QUAIS_SBIRES` cleared). Sets 46.
///
/// Outside 45: the pirate's line; after a loss to Samuel without the badge, the R-7 reminder.
pub async fn PortRessac_Sc04(ctx: Ctx) {
    if story(&ctx).await != 45 {
        let reminder = ctx.flag(FLAG_SIL_SAMUEL_PERDU).await && !ctx.flag(FLAG_SIL_BADGE_04).await;
        let t = if reminder {
            Port_Ressac_Gym_Rappel_Badge
        } else {
            Port_Ressac_Npc_Pirate_Passerelle
        };
        msgbox(&ctx, t, MsgBox::Npc).await;
        return;
    }
    ctx.lock_all().await;
    ctx.face_player().await;
    say(&ctx, Port_Ressac_Sc04_T1).await;
    move_wait(&ctx, LOCALID_PORT_PIRATE_PASSERELLE, UP_2).await;
    hide(&ctx, LOCALID_PORT_PIRATE_PASSERELLE).await;
    ctx.delay(30).await;
    show(&ctx, LOCALID_PORT_SAMUEL, FLAG_SIL_HIDE_PORT_SAMUEL).await;
    move_wait(&ctx, LOCALID_PORT_SAMUEL, SAMUEL_DOWN).await;
    say(&ctx, Port_Ressac_Sc04_T2).await;
    say(&ctx, Port_Ressac_Sc04_T3).await;
    move_wait(&ctx, LOCALID_PORT_SAMUEL, SAMUEL_READS).await;
    say(&ctx, Port_Ressac_Sc04_T4).await;
    // Samuel strikes the rail (no duration in the source: the prelude's).
    screen_shake(&ctx, 4, 0).await;
    say(&ctx, Port_Ressac_Sc04_T5).await;
    // fallback: play_se (audio, M12). SE_SIFFLET is the decomp's SE_M_GRASSWHISTLE.
    play_se(&ctx, "SE_M_GRASSWHISTLE").await;
    // The crew comes on the quays: the pirates appear at their duel cells, facing the decor
    // grunts (the source's `Walk(Down) x2` from the deck has no room on the built quays), with
    // the whole cast of the cleaning.
    ctx.set_flag(FLAG_SIL_HIDE_PORT_QUAIS_SBIRES, false).await;
    for (obj, flag) in QUAY_CAST {
        show(&ctx, obj, flag).await;
    }
    say(&ctx, Port_Ressac_Sc04_T6).await;
    ctx.set_flag(FLAG_SIL_PORT_SAMUEL_ALLIE, true).await;
    set_story(&ctx, 46).await;
    ctx.release_all().await;
}

const SAMUEL_IN: &[Step] = &[
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    Step::FacePlayer,
];
const SAMUEL_OUT: &[Step] = &[walk(Dir::Right), walk(Dir::Right), walk(Dir::Right)];

/// Port_Ressac_Sc05, the foreman and the cargo's departure: the coord (5, 20), two tiles before
/// him, at `VAR_SIL_STORY` 46 (or talking to him). The battle, the Cocket crew gone, the cargo
/// sailed (`FLAG_SIL_HIDE_PORT_CARGO`, the sea painted over it), Samuel's thanks. Sets 47
/// (`FLAG_SIL_PORT_LIBERE`). A loss whites out (the script ends there; 46 stays).
pub async fn PortRessac_Sc05(ctx: Ctx) {
    if story(&ctx).await != 46 {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Port_Ressac_Sc05_T1).await;
    battle_trainer(&ctx, TRAINER_SIL_PORT_CONTREMAITRE, Port_Ressac_Sc05_T2).await;
    say(&ctx, Port_Ressac_Sc05_T3).await;
    ctx.fade(Fade::ToBlack).await;
    for (obj, _) in &QUAY_CAST[3..] {
        hide(&ctx, *obj).await;
    }
    ctx.set_flag(FLAG_SIL_HIDE_PORT_QUAIS_SBIRES, true).await;
    hide(&ctx, LOCALID_PORT_GARDE_CARGO).await;
    ctx.set_flag(FLAG_SIL_HIDE_PORT_CARGO, true).await;
    sail_cargo(&ctx).await;
    ctx.fade(Fade::FromBlack).await;
    // Samuel's template is on the gym's deck (and Sc04 left him on the dock): he comes along
    // the corridor from 4 cells east of the player.
    let (x, y) = ctx.player_pos().await;
    show(&ctx, LOCALID_PORT_SAMUEL, FLAG_SIL_HIDE_PORT_SAMUEL).await;
    ctx.set_object_pos(LOCALID_PORT_SAMUEL, x + 4, y).await;
    move_wait(&ctx, LOCALID_PORT_SAMUEL, SAMUEL_IN).await;
    say(&ctx, Port_Ressac_Sc05_T4).await;
    move_wait(&ctx, LOCALID_PORT_SAMUEL, SAMUEL_OUT).await;
    hide(&ctx, LOCALID_PORT_SAMUEL).await;
    ctx.set_flag(FLAG_SIL_PORT_LIBERE, true).await;
    set_story(&ctx, 47).await;
    ctx.release_all().await;
}

/// The foreman (obj 23): only reachable past Sc05's coord; talked to at 46, the scene.
pub async fn PortRessac_EventScript_Contremaitre(ctx: Ctx) {
    PortRessac_Sc05(ctx).await;
}

/// Port_Ressac_Sc06, Julien's call on the east jetty: the coords at `VAR_SIL_STORY` 49, once
/// (`FLAG_SIL_PORT_APPEL_JULIEN`; a coord event fires on every step while its var matches).
pub async fn PortRessac_Sc06(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_PORT_APPEL_JULIEN).await {
        return;
    }
    ctx.lock_all().await;
    // fallback: play_se, mallette_call, end_call (audio, the call window): as Ch1's calls.
    play_se(&ctx, "SE_POKENAV_CALL").await;
    mallette_call(&ctx, "JULIEN").await;
    say(&ctx, Port_Ressac_Sc06_T1).await;
    end_call(&ctx).await;
    ctx.set_flag(FLAG_SIL_PORT_APPEL_JULIEN, true).await;
    ctx.release_all().await;
}

/// The end of a lost battle against Samuel (Gym_Sc02, R-7): the port's on-frame entry on
/// `VAR_SIL_PORT_RESSAC_STATE` 9 (the old fallback's "Samuel's battle running"). Samuel's toll, the Hydro-Propulseur
/// after the first loss, `FLAG_SIL_SAMUEL_PERDU`, and `VAR_SIL_STORY` 48 so that Gwada waits on
/// the coupée.
///
/// Only a save made under the old fallback (a loss whited out to the Taverne's door) reaches
/// it now: Samuel's battle goes on after a loss and Gym_Sc02 plays [`samuel_toll`] itself.
/// `take_money_percent(20)`: the source's fixed 2000; `take_heal_items(3)`: none.
pub async fn PortRessac_EventScript_SamuelPerdu(ctx: Ctx) {
    ctx.set_var(VAR_SIL_PORT_RESSAC_STATE, 0).await;
    // The source's `clear_flag(FLAG_SIL_PORT_GYM_CONFUS)` after the battle, which the whiteout
    // skipped in Gym_Sc02: a rematch starts without the confusion (§3 Gym_Sc01).
    ctx.set_flag(FLAG_SIL_PORT_GYM_CONFUS, false).await;
    ctx.lock_all().await;
    samuel_toll(&ctx).await;
    ctx.release_all().await;
}

/// Samuel's toll after a lost battle (Gym_Sc02's defeat branch, R-7): his line, 20 % of the
/// money (the source's fixed 2000), the heal, the Hydro-Propulseur after the first loss,
/// `FLAG_SIL_SAMUEL_PERDU`, `VAR_SIL_STORY` 48. The caller holds the lock.
pub(super) async fn samuel_toll(ctx: &Ctx) {
    let ctx = ctx.clone();
    say(&ctx, Port_Ressac_Gym_Sc02_Perdu).await;
    fallback(&ctx, "take_money_percent").await;
    money_remove(&ctx, 2000).await;
    fallback(&ctx, "take_heal_items").await;
    ctx.heal_party().await;
    if !ctx.flag(FLAG_SIL_MODULE_SURF).await {
        say(&ctx, Port_Ressac_Gym_Sc02_Perdu_Surf).await;
        ctx.set_flag(FLAG_SIL_MODULE_SURF, true).await;
        say(&ctx, Port_Ressac_Gym_Sc02_Module).await;
        say(&ctx, Port_Ressac_Gym_Sc02_Perdu_Badge).await;
    } else {
        say(&ctx, Port_Ressac_Gym_Sc02_Perdu2).await;
    }
    ctx.set_flag(FLAG_SIL_SAMUEL_PERDU, true).await;
    if story(&ctx).await < 48 {
        set_story(&ctx, 48).await;
    }
}

// ---- gates ----

const STEP_LEFT: &[Step] = &[walk(Dir::Left)];

/// The cargo's dock (6, 26) at `VAR_SIL_STORY` 42: the guard's line, and the player steps back
/// the way they came (the only way onto the dock is from the pier, west).
pub async fn PortRessac_EventScript_CargoBloque(ctx: Ctx) {
    ctx.lock_all().await;
    ctx.turn(LOCALID_PORT_GARDE_CARGO, Dir::Left).await;
    say(&ctx, Port_Ressac_Garde_Cargo_T1).await;
    move_wait(&ctx, PLAYER, STEP_LEFT).await;
    ctx.release_all().await;
}

/// The Zenith-Grog's dock (31, 26) during the cleaning (`VAR_SIL_STORY` 46): Samuel's order from
/// the gangway, `Sc04_T6` (the source has no line for this gate), and the player steps back.
pub async fn PortRessac_EventScript_PasserelleQuais(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Port_Ressac_Sc04_T6).await;
    move_wait(&ctx, PLAYER, STEP_LEFT).await;
    ctx.release_all().await;
}

/// The cargo's guard: awake, then asleep after Crow's crate (`FLAG_SIL_PORT_GARDE_DORT`).
pub async fn PortRessac_EventScript_GardeCargo(ctx: Ctx) {
    let t = if ctx.flag(FLAG_SIL_PORT_GARDE_DORT).await {
        Port_Ressac_Garde_Cargo_T2
    } else {
        Port_Ressac_Garde_Cargo_T1
    };
    msgbox(&ctx, t, MsgBox::Npc).await;
}

// ---- the quay grunts (§5, Sc05; sight 1 over the corridor) ----

pub async fn PortRessac_EventScript_Sbire1(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_PORT_SBIRE_1,
        TRAINER_SIL_PORT_SBIRE_1,
        Port_Ressac_Sc05_S1_Intro,
        Port_Ressac_Sc05_S1_Defeat,
    )
    .await
    {
        post_battle(&ctx, Port_Ressac_Sc05_S1_Defeat).await;
    }
}

pub async fn PortRessac_EventScript_Sbire2(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_PORT_SBIRE_2,
        TRAINER_SIL_PORT_SBIRE_2,
        Port_Ressac_Sc05_S2_Intro,
        Port_Ressac_Sc05_S2_Defeat,
    )
    .await
    {
        post_battle(&ctx, Port_Ressac_Sc05_S2_Defeat).await;
    }
}

pub async fn PortRessac_EventScript_Sbire3(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_PORT_SBIRE_3,
        TRAINER_SIL_PORT_SBIRE_3,
        Port_Ressac_Sc05_S3_Intro,
        Port_Ressac_Sc05_S3_Defeat,
    )
    .await
    {
        post_battle(&ctx, Port_Ressac_Sc05_S3_Defeat).await;
    }
}

pub async fn PortRessac_EventScript_Sbire4(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_PORT_SBIRE_4,
        TRAINER_SIL_PORT_SBIRE_4,
        Port_Ressac_Sc05_S4_Intro,
        Port_Ressac_Sc05_S4_Defeat,
    )
    .await
    {
        post_battle(&ctx, Port_Ressac_Sc05_S4_Defeat).await;
    }
}

// ---- the distributor, the angler, the items ----

/// The rusty distributor (8, 7): a kick and a free Lemonade once (`FLAG_SIL_PORT_LIMONADE`),
/// then it purrs.
pub async fn PortRessac_EventScript_Distributeur(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_PORT_LIMONADE).await {
        msgbox(&ctx, Port_Ressac_Distrib_T2, MsgBox::Sign).await;
        // fallback: the vending menu (Emerald's `special DrawVendingMachine…` multichoice, §7:
        // Fresh Water 200, Soda Pop 300, Lemonade 350): nothing sold yet.
        fallback(&ctx, "vending_machine").await;
        return;
    }
    ctx.lock().await;
    say(&ctx, Port_Ressac_Distrib_T1).await;
    gift(
        &ctx,
        FLAG_SIL_PORT_LIMONADE,
        ITEM_LEMONADE,
        1,
        Obtain::Message,
    )
    .await;
    ctx.release().await;
}

/// The angler (obj 28): the Good Rod once (`FLAG_SIL_PORT_CANNE`; his `_Canne` line offers it),
/// then his line about the nervous Pokémon.
pub async fn PortRessac_EventScript_Pecheur(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_PORT_CANNE).await {
        msgbox(&ctx, Port_Ressac_Npc_Pecheur, MsgBox::Npc).await;
        return;
    }
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Port_Ressac_Npc_Pecheur_Canne).await;
    gift(&ctx, FLAG_SIL_PORT_CANNE, ITEM_GOOD_ROD, 1, Obtain::Message).await;
    ctx.release().await;
}

pub async fn PortRessac_EventScript_SuperBall(ctx: Ctx) {
    find_item(&ctx, ITEM_GREAT_BALL, 1).await;
}

// ---- signs ----

pub async fn PortRessac_EventScript_SignVille(ctx: Ctx) {
    msgbox(&ctx, Port_Ressac_Sign_Ville, MsgBox::Sign).await;
}

pub async fn PortRessac_EventScript_SignNeon(ctx: Ctx) {
    msgbox(&ctx, Port_Ressac_Sign_Neon, MsgBox::Sign).await;
}

pub async fn PortRessac_EventScript_SignCargo(ctx: Ctx) {
    msgbox(&ctx, Port_Ressac_Sign_Cargo, MsgBox::Sign).await;
}

pub async fn PortRessac_EventScript_SignGym(ctx: Ctx) {
    msgbox(&ctx, Port_Ressac_Sign_Gym, MsgBox::Sign).await;
}

// ---- ambient NPCs (the `_Apres` lines from VAR_SIL_STORY 47, the port freed) ----

pub async fn PortRessac_EventScript_Docker(ctx: Ctx) {
    let t = if story(&ctx).await >= 47 {
        Port_Ressac_Npc_Docker_Apres
    } else {
        Port_Ressac_Npc_Docker
    };
    msgbox(&ctx, t, MsgBox::Npc).await;
}

pub async fn PortRessac_EventScript_VieuxMarin(ctx: Ctx) {
    msgbox(&ctx, Port_Ressac_Npc_VieuxMarin, MsgBox::Npc).await;
}

pub async fn PortRessac_EventScript_Fetarde(ctx: Ctx) {
    msgbox(&ctx, Port_Ressac_Npc_Fetarde, MsgBox::Npc).await;
}

pub async fn PortRessac_EventScript_Enfant(ctx: Ctx) {
    msgbox(&ctx, Port_Ressac_Npc_Enfant, MsgBox::Npc).await;
}

/// The grunt by the blue container: `Npc_Sbire`, then from 47 `Npc_Sbire_Apres` (he quit).
pub async fn PortRessac_EventScript_Sbire(ctx: Ctx) {
    let t = if story(&ctx).await >= 47 {
        Port_Ressac_Npc_Sbire_Apres
    } else {
        Port_Ressac_Npc_Sbire
    };
    msgbox(&ctx, t, MsgBox::Npc).await;
}

/// Crow's client, waiting for his crate: the docker's rum line (the source has none for him).
pub async fn PortRessac_EventScript_MarinClient(ctx: Ctx) {
    msgbox(&ctx, Port_Ressac_Npc_Docker, MsgBox::Npc).await;
}

/// Samuel's pirates brawling on the quays (46 only). The source has no line for them, and
/// every Ch4 line that fits (`Sc04_T5`, `Sc04_T6`) is Samuel's own, with his name: they only
/// turn to the player (as Ch2's Technicien after his gift).
pub async fn PortRessac_EventScript_Pirate(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    ctx.release().await;
}

/// The decor grunts brawling with them: the Cocket "move along", `Npc_Sbire`.
pub async fn PortRessac_EventScript_SbireDecor(ctx: Ctx) {
    msgbox(&ctx, Port_Ressac_Npc_Sbire, MsgBox::Npc).await;
}
