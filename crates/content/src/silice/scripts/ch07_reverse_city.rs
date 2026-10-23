//! Chapter 7, Reverse-City, the mirror city (`ReverseCity`), its Centre, the Supermarché
//! Cocket and the "villain's" house: `contenu-jeu/chapitres/07-reverse-city.md` §2 (the mirror
//! signs and their plaques, the townsfolk, the Centre, the Mart, the house), §3
//! Reverse_City_Sc01 (the welcome), Sc02 (Gogo & Romain), Sc03 (the confusion emitters), Sc05
//! (the Poudre Silice) and Sc07 (Benou opens the Porte Êta-Nord), §5 (the city's trainers) and
//! §7 (the items, the Mart). `VAR_SIL_STORY` 71 (the call heard) → 72 (in the city) → 73 (Gogo &
//! Romain beaten, the Palais open; the gym is `ch07_gym.rs`) → 75 (the badge) → 76 (the north
//! gate open, the chapter's end). The lighthouse and the vault are `ch07_phare.rs`.
//!
//! **Inverted controls** (R-2, §9 `set_input_inverted`): outside, from the welcome until Reverse
//! is beaten (`FLAG_SIL_REVERSE_BATTU`), and off in every interior. The engine has no inverted
//! walking yet: [`set_input_inverted`] reports the fallback and the player walks normally.

use crate::silice::prelude::*;

use super::ch07_route_s07::bloc;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &[
    "ReverseCity",
    "ReverseCity_PokemonCenter",
    "ReverseCity_Mart",
    "ReverseCity_House1",
];

/// The Porte Êta-Nord (27, 0): Sootopolis 545 (`MB_NON_ANIMATED_DOOR`), solid in the layout
/// until Sc07 opens it; the same metatile, walkable, once open ("SIL4 Ch7 maps as built", the
/// Ch8 link).
const PORTE_NORD: (i16, i16) = (27, 0);
const METATILE_PORTE_NORD: u16 = 545;

/// The source's `set_input_inverted(on)` (R-2, §9): up/down and left/right swapped while
/// walking, on Reverse-City's outside and in the Palais until Reverse is beaten, off in the
/// interiors. The engine has no inverted walking: nothing changes. Reported when the source
/// wants the controls inverted (off is the engine's normal walking).
pub(super) async fn set_input_inverted(ctx: &Ctx, on: bool) {
    if on {
        // fallback: set_input_inverted(true) (§9, R-2): the player walks normally.
        fallback(ctx, "set_input_inverted").await;
    }
}

// ---- map scripts ----

/// The Porte Êta-Nord open once `FLAG_SIL_REVERSE_PORTE_NORD_OUVERTE` is set (Sc07).
pub async fn ReverseCity_OnLoad(ctx: Ctx) {
    repaint_if(
        &ctx,
        FLAG_SIL_REVERSE_PORTE_NORD_OUVERTE,
        PORTE_NORD,
        METATILE_PORTE_NORD,
    )
    .await;
}

/// The inverted controls until Reverse is beaten (from the welcome, Sc01: before it the
/// player has not met the distortion yet), and the hide flags ("SIL4 Ch7 maps as built"):
/// - Gogo, Romain and the Palais guard until Sc02 (`VAR_SIL_STORY` 73);
/// - the welcoming resident and Benou at the north gate only in their scenes (Sc01, Sc07).
pub async fn ReverseCity_OnTransition(ctx: Ctx) {
    let s = story(&ctx).await;
    let battu = ctx.flag(FLAG_SIL_REVERSE_BATTU).await;
    set_input_inverted(&ctx, s >= 72 && !battu).await;
    ctx.set_flag(FLAG_SIL_HIDE_REVERSE_GOGO_ROMAIN, s >= 73)
        .await;
    ctx.set_flag(FLAG_SIL_HIDE_REVERSE_ROMAIN, s >= 73).await;
    ctx.set_flag(FLAG_SIL_HIDE_REVERSE_GARDE_PALAIS, s >= 73)
        .await;
    ctx.set_flag(FLAG_SIL_HIDE_REVERSE_HABITANT_ACCUEIL, true)
        .await;
    ctx.set_flag(FLAG_SIL_HIDE_REVERSE_BENOU_PORTE, true).await;
    // fallback: WalkBackwards (§9): the residents walk backwards; no movement type wanders
    // backwards (a scene's walk does: `LockFacing`), so they wander facing their way.
    fallback(&ctx, "walk_backwards").await;
}

// ---- scenes ----

/// The welcoming resident comes down to the player walking backwards (the source's
/// `WalkBackwards(Down) x3`: facing up, `LockFacing`), then faces the player.
const ACCUEIL_IN: &[Step] = &[
    face(Dir::Up),
    Step::LockFacing,
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    Step::UnlockFacing,
    Step::FacePlayer,
];
/// …and goes back up the same way (`WalkBackwards(Up) x3`: facing down).
const ACCUEIL_OUT: &[Step] = &[
    face(Dir::Down),
    Step::LockFacing,
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    Step::UnlockFacing,
];

/// Reverse_City_Sc01, "Bienvenue… ou plutôt, au revoir": the coord row (30–32, 34) in front of
/// the south gate, `VAR_SIL_STORY` 71. The negative flash, the distortion, the inverted
/// controls, the welcoming resident (31, 30) walks down to (31, 33) and back. Sets 72.
pub async fn ReverseCity_Sc01(ctx: Ctx) {
    if story(&ctx).await != 71 {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Reverse_City_Sc01_T1).await;
    // fallback: screen_flash(NEGATIF) (§9): a fade out and in (prelude).
    screen_flash(&ctx).await;
    say(&ctx, Reverse_City_Sc01_T2).await;
    set_input_inverted(&ctx, true).await;
    say(&ctx, Reverse_City_Sc01_T3).await;
    show(
        &ctx,
        LOCALID_REVERSE_HABITANT_ACCUEIL,
        FLAG_SIL_HIDE_REVERSE_HABITANT_ACCUEIL,
    )
    .await;
    move_wait(&ctx, LOCALID_REVERSE_HABITANT_ACCUEIL, ACCUEIL_IN).await;
    say(&ctx, Reverse_City_Sc01_T4).await;
    move_wait(&ctx, LOCALID_REVERSE_HABITANT_ACCUEIL, ACCUEIL_OUT).await;
    hide(&ctx, LOCALID_REVERSE_HABITANT_ACCUEIL).await;
    set_story(&ctx, 72).await;
    ctx.release_all().await;
}

/// Gogo and Romain come down two cells, to (18, 14) and (21, 14), above the fence's gap.
const DOWN_2: &[Step] = &[walk(Dir::Down), walk(Dir::Down)];

/// Reverse_City_Sc02, the duel of the boosted: the fence's gap (17–21, 16), `VAR_SIL_STORY`
/// 72. Gogo then Romain (the two-trainer fallback), Romain's Multi Exp V2
/// (`FLAG_SIL_ITEM_REVERSE_MULTI_EXP_V2`), both leave, the Palais guard leaves. Sets 73.
pub async fn ReverseCity_Sc02(ctx: Ctx) {
    if story(&ctx).await != 72 {
        return;
    }
    ctx.lock_all().await;
    // fallback: play_music(MUS_ENCOUNTER_MAGMA) (audio, M12): the map's own music.
    fallback(&ctx, "play_music").await;
    ctx.move_obj(LOCALID_REVERSE_GOGO, DOWN_2).await;
    ctx.move_obj(LOCALID_REVERSE_ROMAIN, DOWN_2).await;
    ctx.wait_move(LOCALID_REVERSE_ROMAIN).await;
    ctx.wait_move(LOCALID_REVERSE_GOGO).await;
    say(&ctx, Reverse_City_Sc02_T1).await;
    say(&ctx, Reverse_City_Sc02_T2).await;
    say(&ctx, Reverse_City_Sc02_T3).await;
    say(&ctx, Reverse_City_Sc02_T4).await;
    // fallback: the Data Injectors' mutation (§3, §9: the last Pokémon's stat stages, glitched
    // palette and 1/8 HP a turn): the source's own fallback, already in `trainers.ron` (Donphan's
    // Liechi Berry and Double-Edge, Kadabra's Petaya Berry); `Injecteur_*` are not shown.
    fallback(&ctx, "injector_mutation").await;
    // fallback: battle_two_trainers(GOGO_3, ROMAIN_3, double) (prelude): two single battles in
    // a row, no heal between (the source's own fallback). A loss whites out: the scene replays.
    battle_two_trainers(
        &ctx,
        (TRAINER_SIL_GOGO_3, Reverse_City_Sc02_D1),
        (TRAINER_SIL_ROMAIN_3, Reverse_City_Sc02_D2),
    )
    .await;
    say(&ctx, Reverse_City_Sc02_T5).await;
    say(&ctx, Reverse_City_Sc02_T6).await;
    gift(
        &ctx,
        FLAG_SIL_ITEM_REVERSE_MULTI_EXP_V2,
        ITEM_SIL_MULTI_EXP_V2,
        1,
        Obtain::Message,
    )
    .await;
    say(&ctx, Reverse_City_Sc02_T7).await;
    say(&ctx, Reverse_City_Sc02_T8).await;
    ctx.fade(Fade::ToBlack).await;
    // `hide` sets each object's hide flag (GOGO_ROMAIN, ROMAIN, GARDE_PALAIS).
    hide(&ctx, LOCALID_REVERSE_GOGO).await;
    hide(&ctx, LOCALID_REVERSE_ROMAIN).await;
    hide(&ctx, LOCALID_REVERSE_GARDE_PALAIS).await;
    ctx.fade(Fade::FromBlack).await;
    say(&ctx, Reverse_City_Sc02_T9).await;
    set_story(&ctx, 73).await;
    ctx.release_all().await;
}

/// Benou leaves the gate box (29, 3): one row down first, so that he goes round a player on
/// the coord row (26–28, 3), then the source's `Walk(Left) x4`.
const BENOU_OUT: &[Step] = &[
    walk(Dir::Down),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
];

/// Reverse_City_Sc07, Benou hacks the Porte Êta-Nord: the coord row (26–28, 3), two cells in
/// front of the gate, `VAR_SIL_STORY` 75. The gate opens (`FLAG_SIL_REVERSE_PORTE_NORD_OUVERTE`,
/// repainted by `ReverseCity_OnLoad`), the meeting point moves to Évalon. Sets 76.
pub async fn ReverseCity_Sc07(ctx: Ctx) {
    if story(&ctx).await != 75 {
        return;
    }
    ctx.lock_all().await;
    show(
        &ctx,
        LOCALID_REVERSE_BENOU_PORTE,
        FLAG_SIL_HIDE_REVERSE_BENOU_PORTE,
    )
    .await;
    say(&ctx, Reverse_City_Sc07_T1).await;
    // fallback: play_se(SE_HACK) (no sound yet; the decomp's SE_PC_LOGIN stands for it).
    play_se(&ctx, "SE_PC_LOGIN").await;
    let (x, y) = PORTE_NORD;
    ctx.door(x, y, true).await;
    ctx.wait_door().await;
    ctx.set_metatile(x, y, METATILE_PORTE_NORD, false).await;
    ctx.redraw_map().await;
    ctx.set_flag(FLAG_SIL_REVERSE_PORTE_NORD_OUVERTE, true).await;
    say(&ctx, Reverse_City_Sc07_T2).await;
    say(&ctx, Reverse_City_Sc07_T3).await;
    say(&ctx, Reverse_City_Sc07_T4).await;
    move_wait(&ctx, LOCALID_REVERSE_BENOU_PORTE, BENOU_OUT).await;
    hide(&ctx, LOCALID_REVERSE_BENOU_PORTE).await;
    set_story(&ctx, 76).await;
    ctx.release_all().await;
}

// ---- the confusion emitters (Sc03, the source's trainer-object fallback) ----

/// An emitter after its battle: just beaten (its defeat text, `Emetteur_T3`, says the light
/// goes out), `flag` (`FLAG_SIL_REVERSE_EMETTEUR_<n>`) set; talked to again, `Emetteur_T4`.
async fn emetteur(ctx: &Ctx, beaten_before: bool, flag: FlagId) {
    // fallback: the emitters' idle timer (§3 Sc03, §9: 300 still frames in a zone confuse the
    // lead): the source's own fallback, trainer objects (sight 2) against Voltorb and
    // Electrode. Talking to one fights it first, so the Scanner's `Emetteur_T2` is not offered.
    fallback(ctx, "idle_timer").await;
    if beaten_before {
        post_battle(ctx, Emetteur_T4).await;
    }
    ctx.set_flag(flag, true).await;
}

/// ÉMETTEUR 1, west (3, 12), sight 2 right.
pub async fn ReverseCity_EventScript_Emetteur1(ctx: Ctx) {
    let beaten = trainer(
        &ctx,
        LOCALID_REVERSE_EMETTEUR1,
        TRAINER_SIL_EMETTEUR_1,
        Emetteur_T1,
        Emetteur_T3,
    )
    .await;
    emetteur(&ctx, beaten, FLAG_SIL_REVERSE_EMETTEUR_1).await;
}

/// ÉMETTEUR 2, east (36, 12), sight 2 left.
pub async fn ReverseCity_EventScript_Emetteur2(ctx: Ctx) {
    let beaten = trainer(
        &ctx,
        LOCALID_REVERSE_EMETTEUR2,
        TRAINER_SIL_EMETTEUR_2,
        Emetteur_T1,
        Emetteur_T3,
    )
    .await;
    emetteur(&ctx, beaten, FLAG_SIL_REVERSE_EMETTEUR_2).await;
}

/// ÉMETTEUR 3, the south quays (12, 34), sight 2 up.
pub async fn ReverseCity_EventScript_Emetteur3(ctx: Ctx) {
    let beaten = trainer(
        &ctx,
        LOCALID_REVERSE_EMETTEUR3,
        TRAINER_SIL_EMETTEUR_3,
        Emetteur_T1,
        Emetteur_T3,
    )
    .await;
    emetteur(&ctx, beaten, FLAG_SIL_REVERSE_EMETTEUR_3).await;
}

// ---- trainers (no lines in the source: the Cocket promoter's) ----

/// SBIRE COCKET (35, 22), sight 3 left: the promoter's line (`Reverse_Npc_Sbire_Promo`).
pub async fn ReverseCity_EventScript_Sbire1(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_REVERSE_SBIRE1,
        TRAINER_SIL_SBIRE_REVERSE_1,
        Reverse_Npc_Sbire_Promo,
        Reverse_Npc_Sbire_Promo,
    )
    .await
    {
        post_battle(&ctx, Reverse_Npc_Sbire_Promo).await;
    }
}

/// SBIRE COCKET (36, 30), sight 3 up: the promoter's line.
pub async fn ReverseCity_EventScript_Sbire2(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_REVERSE_SBIRE2,
        TRAINER_SIL_SBIRE_REVERSE_2,
        Reverse_Npc_Sbire_Promo,
        Reverse_Npc_Sbire_Promo,
    )
    .await
    {
        post_battle(&ctx, Reverse_Npc_Sbire_Promo).await;
    }
}

// ---- the townsfolk ----

pub async fn ReverseCity_EventScript_Habitant1(ctx: Ctx) {
    msgbox(&ctx, Reverse_Npc_Habitant1, MsgBox::Npc).await;
}

pub async fn ReverseCity_EventScript_Habitant2(ctx: Ctx) {
    msgbox(&ctx, Reverse_Npc_Habitant2, MsgBox::Npc).await;
}

pub async fn ReverseCity_EventScript_Enfant(ctx: Ctx) {
    msgbox(&ctx, Reverse_Npc_Enfant, MsgBox::Npc).await;
}

pub async fn ReverseCity_EventScript_Vieille(ctx: Ctx) {
    msgbox(&ctx, Reverse_Npc_Vieille, MsgBox::Npc).await;
}

pub async fn ReverseCity_EventScript_SbirePromo(ctx: Ctx) {
    msgbox(&ctx, Reverse_Npc_Sbire_Promo, MsgBox::Npc).await;
}

pub async fn ReverseCity_EventScript_Docker(ctx: Ctx) {
    msgbox(&ctx, Reverse_Npc_Docker, MsgBox::Npc).await;
}

/// The Palais guard (5, 9), on the door until Sc02 (the north half is behind Sc02's fence).
pub async fn ReverseCity_EventScript_GardePalais(ctx: Ctx) {
    msgbox(&ctx, Reverse_Npc_Garde_Palais, MsgBox::Npc).await;
}

pub async fn ReverseCity_EventScript_SbireRuelle(ctx: Ctx) {
    msgbox(&ctx, Reverse_Npc_Ruelle, MsgBox::Npc).await;
}

/// The alley's silica block (10, 6), before the vault: Route S7's block script.
pub async fn ReverseCity_EventScript_BlocRuelle(ctx: Ctx) {
    bloc(&ctx).await;
}

// ---- the mirror signs and their plaques (the vault's code, Sc04) ----

/// A mirror sign with a numbered plaque: its text, then the plaque noted
/// (`FLAG_SIL_REVERSE_PLAQUE_<n>`, the vault's multichoice fallback in `ch07_phare.rs`).
async fn plaque(ctx: &Ctx, text: TextId, plaque: FlagId) {
    msgbox(ctx, text, MsgBox::Sign).await;
    ctx.set_flag(plaque, true).await;
}

/// The south gate, plaque 1: 3.
pub async fn ReverseCity_EventScript_SignVille(ctx: Ctx) {
    plaque(&ctx, Reverse_Sign_Ville, FLAG_SIL_REVERSE_PLAQUE_1).await;
}

/// The Supermarché, plaque 2: 8.
pub async fn ReverseCity_EventScript_SignMart(ctx: Ctx) {
    plaque(&ctx, Reverse_Sign_Mart, FLAG_SIL_REVERSE_PLAQUE_2).await;
}

/// The lighthouse, plaque 3: 1.
pub async fn ReverseCity_EventScript_SignPhare(ctx: Ctx) {
    plaque(&ctx, Reverse_Sign_Phare, FLAG_SIL_REVERSE_PLAQUE_3).await;
}

/// The Palais, plaque 4: 6.
pub async fn ReverseCity_EventScript_SignGym(ctx: Ctx) {
    plaque(&ctx, Reverse_Sign_Gym, FLAG_SIL_REVERSE_PLAQUE_4).await;
}

/// The Centre, no plaque.
pub async fn ReverseCity_EventScript_SignPC(ctx: Ctx) {
    msgbox(&ctx, Reverse_Sign_PC, MsgBox::Sign).await;
}

/// The two empty upside-down bins (14, 18) and (33, 19). The source gives them no text (and the
/// build has no Gen3 `Text_EmptyTrashCan`): nothing is found, nothing is said (author question).
pub async fn ReverseCity_EventScript_Poubelle(_ctx: Ctx) {}

// ---- items ----

/// Reverse_City_Sc05, the Poudre Silice by the docker (21, 26): the standard pickup, then the
/// sachet's warning label.
pub async fn ReverseCity_EventScript_ItemPoudreSilice(ctx: Ctx) {
    if find_item(&ctx, ITEM_SIL_POUDRE_SILICE, 1).await {
        // `find_item` has released: the label is read under its own lock (a box shown after
        // the release would stay open).
        ctx.lock().await;
        say(&ctx, Poudre_Silice_Ramasse).await;
        ctx.release().await;
    }
}

/// Bin 2 in the west alley (2, 22).
pub async fn ReverseCity_EventScript_HiddenPpUp(ctx: Ctx) {
    // fallback: hidden items (the engine ignores `BgKind::HiddenItem`; the M9 finding of
    // "SIL1 Task F as built"): the map has a sign here instead.
    hidden_item(&ctx, FLAG_SIL_ITEM_REVERSE_PP_UP, ITEM_PP_UP).await;
}

/// Bin 4 on the quays (24, 25).
pub async fn ReverseCity_EventScript_HiddenHpUp(ctx: Ctx) {
    // fallback: hidden items (as above).
    hidden_item(&ctx, FLAG_SIL_ITEM_REVERSE_HP_UP, ITEM_HP_UP).await;
}

/// Under the east pontoon (26, 30).
pub async fn ReverseCity_EventScript_HiddenPoudreSilice(ctx: Ctx) {
    // fallback: hidden items (as above).
    hidden_item(
        &ctx,
        FLAG_SIL_ITEM_REVERSE_POUDRE_SILICE_2,
        ITEM_SIL_POUDRE_SILICE,
    )
    .await;
}

// ---- the Centre ----

/// The only room "the right way round": the respawn (the new `HEAL_LOCATION_SIL_REVERSE_CITY`,
/// `game.ron`) and normal controls.
pub async fn ReverseCity_PokemonCenter_OnTransition(ctx: Ctx) {
    ctx.set_respawn(HEAL_LOCATION_SIL_REVERSE_CITY).await;
    set_input_inverted(&ctx, false).await;
}

/// The nurse: her special welcome, then Gen3's nurse.
pub async fn ReverseCity_PokemonCenter_EventScript_Nurse(ctx: Ctx) {
    msgbox(&ctx, Reverse_PC_Infirmiere_Bonus, MsgBox::Npc).await;
    pokecenter_nurse(&ctx).await;
}

pub async fn ReverseCity_PokemonCenter_EventScript_Dresseuse(ctx: Ctx) {
    msgbox(&ctx, Reverse_PC_Npc, MsgBox::Npc).await;
}

// ---- the Supermarché Cocket ----

/// Normal controls (an interior).
pub async fn ReverseCity_Mart_OnTransition(ctx: Ctx) {
    set_input_inverted(&ctx, false).await;
}

/// The Supermarché's stock (§7, "Boosts en gros, prix en petit").
const MART_STOCK: &[ItemId] = &[
    ItemId(ITEM_X_ATTACK),
    ItemId(ITEM_X_SPEED),
    ItemId(ITEM_X_DEFEND),
    ItemId(ITEM_X_SPECIAL),
    ItemId(ITEM_X_ACCURACY),
    ItemId(ITEM_DIRE_HIT),
    ItemId(ITEM_GUARD_SPEC),
    ItemId(ITEM_ULTRA_BALL),
    ItemId(ITEM_HYPER_POTION),
    ItemId(ITEM_MAX_REPEL),
    ItemId(ITEM_FULL_HEAL),
];

/// The cashier. The bulk bonus (§7: an eleventh "+" for ten, `Reverse_Mart_Caissier_Bonus`)
/// waits for the shop screen.
pub async fn ReverseCity_Mart_EventScript_Clerk(ctx: Ctx) {
    // fallback: mart (no shop screen before M9): greeting and goodbye, nothing sold.
    mart(&ctx, MART_STOCK).await;
}

pub async fn ReverseCity_Mart_EventScript_Client(ctx: Ctx) {
    msgbox(&ctx, Reverse_Mart_Npc, MsgBox::Npc).await;
}

// ---- the "villain's" house ----

/// Normal controls (an interior).
pub async fn ReverseCity_House1_OnTransition(ctx: Ctx) {
    set_input_inverted(&ctx, false).await;
}

/// The "villain" old man: his insults and the Revival Herb once
/// (`FLAG_SIL_ITEM_REVERSE_REVIVAL_HERB`), then T2.
pub async fn ReverseCity_House1_EventScript_VieilHomme(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_ITEM_REVERSE_REVIVAL_HERB).await {
        msgbox(&ctx, Reverse_House1_T2, MsgBox::Npc).await;
        return;
    }
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Reverse_House1_T1).await;
    gift(
        &ctx,
        FLAG_SIL_ITEM_REVERSE_REVIVAL_HERB,
        ITEM_REVIVAL_HERB,
        1,
        Obtain::Message,
    )
    .await;
    ctx.release().await;
}
