//! Chapter 12, the post-game, Act III outside the Dome and on the Apex
//! (`contenu-jeu/chapitres/12-post-game.md` §3 `No_Mans_Land_PG_Sc16` to `Sommet_Apex_PG_Sc19`,
//! `PG_Lib_Poussin`, `Sommet_Apex_PG_Sc32`, `PG_Legendaire`'s Centrale puzzle; §5 the No Man's
//! Land's trainers; §7 its items).
//!
//! The way (`VAR_SIL_STORY` 128 → 130): the outside (`Sc16`, the first steps out of the Breach),
//! Poussin at the Relay (`Sc16b`: the last directive cut, the Blue Orb, Lugia and Kyogre freed),
//! the three soil samples (`Sc17`, back to Gwada: `ch12_breche.rs`'s `Sc18`, 129), the Purity
//! Beacon on the Apex (`Sc19`, 130). Then Poussin's daily duel on his stage (`Sc16c`) and
//! Rayquaza (`Sc32`).
//!
//! The legendaries of the outside (the plan's "clear points"): Moltres and Latios appear with
//! `Tour_Nebula_2F_PG_Sc15` (128), Suicune with `Sc19` (130), Zapdos with the Centrale's third
//! switch, Regice with the Ice Vault's door (the three samples), Groudon and Regirock with Julien's
//! liberation (`Nebula_PG_Sc03c`, which also lifts the Heart's boulder and opens the Relay's
//! trapdoor).

use super::ch04_port::{Puzzle, push_object};
use super::ch12_breche::gaz;
use super::ch12_reveil::{battle_lost, legendaire};
use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &[
    "NoMansLand",
    "NoMansLand_Centrale",
    "NoMansLand_CaveauGlace",
    "NoMansLand_Cratere",
    "NoMansLand_Relais_B1F",
];

/// A cracked rock or mirror (an object, `BREAKABLE_ROCK`), Gen3's `EventScript_RockSmash` with
/// the Mallette's Scanner de Faille (`FLAG_SIL_MODULE_ECLATE_ROC`): without it, the cracked
/// rock's line; with it, the scanner's question and the object removed (until the next load, as
/// Gen3's rocks). The source gives the post-game's rocks no line of their own: Chapter 6's
/// (`Route_S06_Sign_Rocher`) and Chapter 7's (`Route_S07_Bloc_Scan`, `_Casse`) are reused.
///
/// fallback: the Rock Smash encounter table (no wild Pokémon jumps out).
pub(super) async fn rock_smash(ctx: &Ctx) {
    if !ctx.flag(FLAG_SIL_MODULE_ECLATE_ROC).await {
        msgbox(ctx, Route_S06_Sign_Rocher, MsgBox::Sign).await;
        return;
    }
    ctx.lock().await;
    if ask(ctx, Route_S07_Bloc_Scan).await {
        play_se(ctx, "SE_M_ROCK_THROW").await;
        say(ctx, Route_S07_Bloc_Casse).await;
        let obj = ctx.var(VAR_LAST_TALKED).await;
        ctx.remove_object(obj as u8).await;
    }
    ctx.release().await;
}

// ---- the No Man's Land: map scripts ----

/// The Relay's trapdoor (31, 27): Pacifidlog's open trapdoor... SilNebula's metatile 609, walkable,
/// once Julien has freed his legendaries (Regirock waits in the bunker below).
const TRAPPE: (i16, i16, u16) = (31, 27, 609);

/// The Relay's trapdoor opens from `FLAG_SIL_LIBERATION_JULIEN`.
pub async fn NoMansLand_OnLoad(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_LIBERATION_JULIEN).await {
        ctx.set_metatile(TRAPPE.0, TRAPPE.1, TRAPPE.2, false).await;
    }
}

/// The exiled Vestiges' traces ("Après la Traque"): Kimiko's repaired drone over the Oasis from
/// 130, Arthur's paper boat on the Wind Cliff from 128, each only if exiled.
pub async fn NoMansLand_OnTransition(ctx: Ctx) {
    let s = story(&ctx).await;
    let drone = s >= 130
        && ctx.flag(FLAG_SIL_VESTIGE_KIMIKO).await
        && !ctx.flag(FLAG_SIL_PARDON_KIMIKO).await;
    ctx.set_flag(FLAG_SIL_HIDE_NML_DRONE, !drone).await;
    let origami = s >= 128
        && ctx.flag(FLAG_SIL_VESTIGE_ARTHUR).await
        && !ctx.flag(FLAG_SIL_PARDON_ARTHUR).await;
    ctx.set_flag(FLAG_SIL_HIDE_NML_ORIGAMI, !origami).await;
}

// ---- the No Man's Land: scenes ----

const OUT_STEPS: &[Step] = &[walk(Dir::Up), walk(Dir::Up), Step::Delay(30)];
const LOOK_BACK: &[Step] = &[face(Dir::Down), Step::Delay(40), face(Dir::Up)];

/// `No_Mans_Land_PG_Sc16`, the outside (row 56, three cells past the Breach, at 128, once:
/// `FLAG_SIL_PG_DEHORS_VU`): the wind, the Dome seen from outside, traces of life.
pub async fn NoMansLand_PG_Sc16(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_PG_DEHORS_VU).await {
        return;
    }
    ctx.lock_all().await;
    // fallback: MUS_SIL_NO_MANS_LAND (play_music).
    fallback(&ctx, "play_music").await;
    move_wait(&ctx, PLAYER, OUT_STEPS).await;
    say(&ctx, No_Mans_Land_PG_Sc16_T1).await;
    move_wait(&ctx, PLAYER, LOOK_BACK).await;
    say(&ctx, No_Mans_Land_PG_Sc16_T2).await;
    say(&ctx, No_Mans_Land_PG_Sc16_T3).await;
    ctx.set_flag(FLAG_SIL_PG_DEHORS_VU, true).await;
    ctx.release_all().await;
}

/// `No_Mans_Land_PG_Sc16b`, the Relay (row 37 at 128, and Poussin himself (29, 29)), until
/// `FLAG_SIL_RELAIS_ETEINT` (Poussin's map flag): the echo he obeyed, the cable cut, TM18 (Rain
/// Dance), Kyogre and Lugia freed (`PG_Lib_Poussin`: their clear point), the Blue Orb; Poussin back
/// on his Apex stage (`FLAG_SIL_HIDE_APEX_POUSSIN_PG` cleared).
///
/// fallback: the Relay's light switched off (`set_metatile(RELAIS_VOYANT, ETEINT)`): the source
/// names no cell or tile; the text says it.
pub async fn NoMansLand_PG_Sc16b(ctx: Ctx) {
    if story(&ctx).await < 128 || ctx.flag(FLAG_SIL_RELAIS_ETEINT).await {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, No_Mans_Land_PG_Sc16b_T1).await;
    ctx.turn(LOCALID_NML_POUSSIN, Dir::Down).await;
    say(&ctx, No_Mans_Land_PG_Sc16b_T2).await;
    say(&ctx, No_Mans_Land_PG_Sc16b_T3).await;
    if !ask(&ctx, No_Mans_Land_PG_Sc16b_T4).await {
        say(&ctx, No_Mans_Land_PG_Sc16b_T6).await;
    }
    say(&ctx, No_Mans_Land_PG_Sc16b_T5).await;
    fallback(&ctx, "relais_voyant").await;
    say(&ctx, No_Mans_Land_PG_Sc16b_T7).await;
    give_item(&ctx, ITEM_TM18, 1).await;
    say(&ctx, No_Mans_Land_PG_Sc16b_T8).await;
    add_item(&ctx, ITEM_SIL_ORBE_BLEU, 1).await;
    say(&ctx, No_Mans_Land_PG_Sc16b_T9).await;
    say(&ctx, No_Mans_Land_PG_Sc16b_T10).await;
    ctx.fade(Fade::ToBlack).await;
    hide(&ctx, LOCALID_NML_POUSSIN).await;
    ctx.fade(Fade::FromBlack).await;
    ctx.set_flag(FLAG_SIL_RELAIS_ETEINT, true).await;
    ctx.set_flag(FLAG_SIL_HIDE_APEX_POUSSIN_PG, false).await;
    ctx.set_flag(FLAG_SIL_HIDE_LUGIA, false).await;
    ctx.set_flag(FLAG_SIL_HIDE_KYOGRE, false).await;
    ctx.release_all().await;
}

/// `No_Mans_Land_PG_Sc17`, a soil sample (`n` 1–3: the Oasis, the Centrale's grille, the Crater's
/// foot), from 128: the line, the sample (`FLAG_SIL_ECHANTILLON_<n>`, the patch's map flag).
async fn echantillon(ctx: &Ctx, text: TextId, obj: u8) {
    if story(ctx).await < 128 {
        return;
    }
    ctx.lock().await;
    say(ctx, text).await;
    add_item(ctx, ITEM_SIL_ECHANTILLON_SOL, 1).await;
    hide(ctx, obj).await;
    ctx.release().await;
}

pub async fn NoMansLand_PG_Sc17_1(ctx: Ctx) {
    echantillon(&ctx, No_Mans_Land_PG_Sc17_T1, LOCALID_NML_TERRE_1).await;
}

pub async fn NoMansLand_PG_Sc17_2(ctx: Ctx) {
    echantillon(&ctx, No_Mans_Land_PG_Sc17_T2, LOCALID_NML_TERRE_2).await;
}

pub async fn NoMansLand_PG_Sc17_3(ctx: Ctx) {
    echantillon(&ctx, No_Mans_Land_PG_Sc17_T3, LOCALID_NML_TERRE_3).await;
}

/// Kimiko's drone over the Oasis (exiled).
pub async fn NoMansLand_PG_Drone(ctx: Ctx) {
    msgbox(&ctx, No_Mans_Land_PG_Drone, MsgBox::Sign).await;
}

/// Arthur's paper boat on the Wind Cliff (exiled).
pub async fn NoMansLand_PG_Origami(ctx: Ctx) {
    msgbox(&ctx, No_Mans_Land_PG_Origami, MsgBox::Sign).await;
}

/// Gas 04, the way to the Centrale: the Filter v2.
pub async fn NoMansLand_EventScript_Gaz04(ctx: Ctx) {
    gaz(&ctx, LOCALID_NML_GAZ_04, true).await;
}

/// Gas 05, the way to the Crater: the Filter v2.
pub async fn NoMansLand_EventScript_Gaz05(ctx: Ctx) {
    gaz(&ctx, LOCALID_NML_GAZ_05, true).await;
}

/// The Relay's plaque (29, 27): the source writes its words only inside `Sc16b`'s first line,
/// which is read again.
pub async fn NoMansLand_EventScript_Plaque(ctx: Ctx) {
    msgbox(&ctx, No_Mans_Land_PG_Sc16b_T1, MsgBox::Sign).await;
}

/// The Centrale's sign (12, 8): its entrance panel (`PG_Centrale_T1`).
pub async fn NoMansLand_EventScript_PanneauCentrale(ctx: Ctx) {
    msgbox(&ctx, PG_Centrale_T1, MsgBox::Sign).await;
}

/// The steel beam on the broken bridge (37, 30) (`VAR_TEMP_2`).
pub(crate) const POUTRE: Puzzle = Puzzle {
    map: MAP_NO_MANS_LAND,
    objects: &[(LOCALID_NML_POUTRE, VAR_TEMP_2, (37, 30))],
    free: poutre_free,
};

/// East along the bridge's row, up to Latios's cell (the plan's review: the beam opens the bridge
/// and strands nothing).
fn poutre_free(_: (i16, i16), (x, y): (i16, i16)) -> bool {
    y == 30 && (38..=54).contains(&x)
}

/// The steel beam: the Champ Magnétique (`FLAG_SIL_MODULE_FORCE`), as Chapter 3's blocks.
pub async fn NoMansLand_EventScript_Poutre(ctx: Ctx) {
    push_object(&ctx).await;
}

/// A cracked rock (the Scanner de Faille).
pub async fn NoMansLand_EventScript_Rocher(ctx: Ctx) {
    rock_smash(&ctx).await;
}

/// RANDONNEUR MARCEL, on the way to the Centrale.
pub async fn NoMansLand_EventScript_Marcel(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_NML_MARCEL,
        TRAINER_SIL_NML_RANDONNEUR_1,
        No_Mans_Land_PG_Marcel_Intro,
        No_Mans_Land_PG_Marcel_Defeat,
    )
    .await
    {
        post_battle(&ctx, No_Mans_Land_PG_Marcel_Defeat).await;
    }
}

/// TOP DRESSEUSE AÏCHA, by the Oasis.
pub async fn NoMansLand_EventScript_Aicha(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_NML_AICHA,
        TRAINER_SIL_NML_TOPDRESSEUSE_1,
        No_Mans_Land_PG_Aicha_Intro,
        No_Mans_Land_PG_Aicha_Defeat,
    )
    .await
    {
        post_battle(&ctx, No_Mans_Land_PG_Aicha_Defeat).await;
    }
}

/// EX-COCKET KEVIN, before the Relay.
pub async fn NoMansLand_EventScript_Kevin(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_NML_KEVIN,
        TRAINER_SIL_NML_EXCOCKET_1,
        No_Mans_Land_PG_Kevin_Intro,
        No_Mans_Land_PG_Kevin_Defeat,
    )
    .await
    {
        post_battle(&ctx, No_Mans_Land_PG_Kevin_Defeat).await;
    }
}

/// CHERCHEUR ÉLIAS, by the broken bridge.
pub async fn NoMansLand_EventScript_Elias(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_NML_ELIAS,
        TRAINER_SIL_NML_CHERCHEUR_1,
        No_Mans_Land_PG_Elias_Intro,
        No_Mans_Land_PG_Elias_Defeat,
    )
    .await
    {
        post_battle(&ctx, No_Mans_Land_PG_Elias_Defeat).await;
    }
}

/// GARDE SALOMÉ, at the Crater's foot.
pub async fn NoMansLand_EventScript_Salome(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_NML_SALOME,
        TRAINER_SIL_NML_RANGER_1,
        No_Mans_Land_PG_Salome_Intro,
        No_Mans_Land_PG_Salome_Defeat,
    )
    .await
    {
        post_battle(&ctx, No_Mans_Land_PG_Salome_Defeat).await;
    }
}

/// The Tyranitarite in the Relay's carcass.
pub async fn NoMansLand_EventScript_ItemTyranitarite(ctx: Ctx) {
    find_item(&ctx, ITEM_SIL_TYRANITARITE, 1).await;
}

/// The Max Revive on the Oasis's north shore.
pub async fn NoMansLand_EventScript_ItemMaxRevive(ctx: Ctx) {
    find_item(&ctx, ITEM_MAX_REVIVE, 1).await;
}

/// The Latiosite on the Wind Cliff: its ball appears once Latios is caught (its flag starts set,
/// Latios's capture clears it).
pub async fn NoMansLand_EventScript_ItemLatiosite(ctx: Ctx) {
    find_item(&ctx, ITEM_SIL_LATIOSITE, 1).await;
}

/// Suicune on the Oasis's shore (`PG_Legendaire`, level 78; shown by `Sommet_Apex_PG_Sc19`).
pub async fn NoMansLand_EventScript_Suicune(ctx: Ctx) {
    legendaire(
        &ctx,
        PG_Legendaire_Suicune,
        SPECIES_SUICUNE,
        78,
        LOCALID_NML_SUICUNE,
    )
    .await;
}

/// Latios on the Wind Cliff (`PG_Legendaire`, level 78; shown by `Tour_Nebula_2F_PG_Sc15`).
/// Caught, it leaves the Latiosite's ball behind (§7: the ball appears once
/// `FLAG_SIL_HIDE_LATIOS` is set).
pub async fn NoMansLand_EventScript_Latios(ctx: Ctx) {
    ctx.lock().await;
    msgbox(&ctx, PG_Legendaire_Latios, MsgBox::Default).await;
    play_cry(&ctx).await;
    if battle_wild(&ctx, SPECIES_LATIOS, 78).await == BattleOutcome::Caught {
        hide(&ctx, LOCALID_NML_LATIOS).await;
        show(
            &ctx,
            LOCALID_NML_ITEM_LATIOSITE,
            FLAG_SIL_ITEM_PG_NML_LATIOSITE,
        )
        .await;
    }
    ctx.release().await;
}

// ---- the Dead Power Plant (Centrale) ----

/// The three switches in order (the entrance panel: "du plus petit au plus grand"; A 12 V,
/// B 220 V, C 380 V).
const INTERRUPTEURS: [FlagId; 3] = [FLAG_SIL_CENTRALE_A, FLAG_SIL_CENTRALE_B, FLAG_SIL_CENTRALE_C];

/// A switch (`i`: 0 A, 1 B, 2 C): the next in order lights; the third opens the control room's
/// blast door and wakes Zapdos (its clear point); a wrong order resets them all
/// (`PG_Centrale_T2`). Nothing once the door is open. The switches have no line of their own.
async fn interrupteur(ctx: &Ctx, i: usize) {
    if ctx.flag(FLAG_SIL_HIDE_CENTRALE_PORTE).await {
        return;
    }
    let mut next = 0;
    while next < 3 && ctx.flag(INTERRUPTEURS[next]).await {
        next += 1;
    }
    if i < next {
        return;
    }
    ctx.lock().await;
    play_se(ctx, "SE_SWITCH").await;
    if i != next {
        say(ctx, PG_Centrale_T2).await;
        for f in INTERRUPTEURS {
            ctx.set_flag(f, false).await;
        }
    } else {
        ctx.set_flag(INTERRUPTEURS[i], true).await;
        if i == 2 {
            ctx.delay(30).await;
            hide(ctx, LOCALID_CENTRALE_PORTE).await;
            ctx.set_flag(FLAG_SIL_HIDE_ZAPDOS, false).await;
            ctx.add_object(LOCALID_CENTRALE_ELECTHOR).await;
        }
    }
    ctx.release().await;
}

pub async fn NoMansLand_Centrale_EventScript_InterrupteurA(ctx: Ctx) {
    interrupteur(&ctx, 0).await;
}

pub async fn NoMansLand_Centrale_EventScript_InterrupteurB(ctx: Ctx) {
    interrupteur(&ctx, 1).await;
}

pub async fn NoMansLand_Centrale_EventScript_InterrupteurC(ctx: Ctx) {
    interrupteur(&ctx, 2).await;
}

/// The entrance panel (11, 5).
pub async fn NoMansLand_Centrale_EventScript_Panneau(ctx: Ctx) {
    msgbox(&ctx, PG_Centrale_T1, MsgBox::Sign).await;
}

/// The control room's blast door, shut until the third switch: the panel's line.
pub async fn NoMansLand_Centrale_EventScript_Porte(ctx: Ctx) {
    msgbox(&ctx, PG_Centrale_T1, MsgBox::Sign).await;
}

/// Zapdos in the control room (`PG_Legendaire`, level 75).
pub async fn NoMansLand_Centrale_EventScript_Electhor(ctx: Ctx) {
    legendaire(
        &ctx,
        PG_Legendaire_Electhor,
        SPECIES_ZAPDOS,
        75,
        LOCALID_CENTRALE_ELECTHOR,
    )
    .await;
}

/// The Zinc in a locker.
pub async fn NoMansLand_Centrale_EventScript_ItemZinc(ctx: Ctx) {
    find_item(&ctx, ITEM_ZINC, 1).await;
}

// ---- the Ice Vault (Caveau Glacé) ----

/// The stone door with three dots: it answers the living soil (the three samples). Opening it
/// is Regice's clear point; nothing once open. Without the samples it does nothing (the source
/// gives the door no line).
pub async fn NoMansLand_CaveauGlace_EventScript_Porte(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_HIDE_CAVEAU_PORTE).await {
        return;
    }
    for f in [
        FLAG_SIL_ECHANTILLON_1,
        FLAG_SIL_ECHANTILLON_2,
        FLAG_SIL_ECHANTILLON_3,
    ] {
        if !ctx.flag(f).await {
            return;
        }
    }
    ctx.lock().await;
    play_se(&ctx, "SE_DOOR").await;
    hide(&ctx, LOCALID_CAVEAU_PORTE).await;
    show(&ctx, LOCALID_CAVEAU_REGICE, FLAG_SIL_HIDE_REGICE).await;
    ctx.release().await;
}

/// Regice behind the door (`PG_Legendaire`, level 75).
pub async fn NoMansLand_CaveauGlace_EventScript_Regice(ctx: Ctx) {
    legendaire(
        &ctx,
        PG_Legendaire_Regice,
        SPECIES_REGICE,
        75,
        LOCALID_CAVEAU_REGICE,
    )
    .await;
}

/// The Glalitite at the ice's end (room 2).
pub async fn NoMansLand_CaveauGlace_EventScript_ItemGlalitite(ctx: Ctx) {
    find_item(&ctx, ITEM_SIL_GLALITITE, 1).await;
}

// ---- the Crater ----

/// The Heart of the Crater's boulder (2, 12): there until Julien frees Groudon
/// (`FLAG_SIL_LIBERATION_JULIEN`). Moltres is not read here (its clear point is `Sc15`).
pub async fn NoMansLand_Cratere_OnTransition(ctx: Ctx) {
    let ouvert = ctx.flag(FLAG_SIL_LIBERATION_JULIEN).await;
    ctx.set_flag(FLAG_SIL_HIDE_CRATERE_ROCHER, ouvert).await;
}

/// The Heart's boulder: it never moves (the plan's review). The source gives it no line.
pub async fn NoMansLand_Cratere_EventScript_Rocher(_ctx: Ctx) {}

/// The cracked rock (10, 15) over the Aerodactylite (the Scanner de Faille).
pub async fn NoMansLand_Cratere_EventScript_RocherFissure(ctx: Ctx) {
    rock_smash(&ctx).await;
}

/// The Houndoominite on an ash ledge.
pub async fn NoMansLand_Cratere_EventScript_ItemHoundoominite(ctx: Ctx) {
    find_item(&ctx, ITEM_SIL_HOUNDOOMINITE, 1).await;
}

/// Moltres on the ridge (`PG_Legendaire`, level 75).
pub async fn NoMansLand_Cratere_EventScript_Sulfura(ctx: Ctx) {
    legendaire(
        &ctx,
        PG_Legendaire_Sulfura,
        SPECIES_MOLTRES,
        75,
        LOCALID_CRATERE_SULFURA,
    )
    .await;
}

/// Groudon asleep in the Heart (`PG_Legendaire`, level 80).
pub async fn NoMansLand_Cratere_EventScript_Groudon(ctx: Ctx) {
    legendaire(
        &ctx,
        PG_Legendaire_Groudon,
        SPECIES_GROUDON,
        80,
        LOCALID_CRATERE_GROUDON,
    )
    .await;
}

// ---- the Relay's bunker ----

/// The braille wall: the source gives it no reading (no braille puzzle, §2).
pub async fn NoMansLand_Relais_B1F_EventScript_Braille(_ctx: Ctx) {}

/// Regirock in the bunker (`PG_Legendaire`, level 75).
pub async fn NoMansLand_Relais_B1F_EventScript_Regirock(ctx: Ctx) {
    legendaire(
        &ctx,
        PG_Legendaire_Regirock,
        SPECIES_REGIROCK,
        75,
        LOCALID_RELAIS_REGIROCK,
    )
    .await;
}

// ---- the Apex (Chapter 11's map) ----

/// `Sommet_Apex_PG_Sc16c`, the recall: Poussin's daily duel on his stage
/// (`FLAG_SIL_DAILY_POUSSIN`), `TRAINER_SIL_POUSSIN_9` (the rain without legendaries); the first
/// win gives the Swampertite (`FLAG_SIL_ITEM_PG_SWAMPERTITE`).
pub async fn SommetApex_PG_Sc16c(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if ctx.flag(FLAG_SIL_DAILY_POUSSIN).await {
        say(&ctx, Sommet_Apex_PG_Sc16c_T4).await;
        ctx.release().await;
        return;
    }
    let premier = !ctx.flag(FLAG_SIL_ITEM_PG_SWAMPERTITE).await;
    if premier {
        say(&ctx, Sommet_Apex_PG_Sc16c_T1).await;
    }
    if !ask(&ctx, Sommet_Apex_PG_Sc16c_T2).await {
        say(&ctx, Sommet_Apex_PG_Sc16c_T3).await;
        ctx.release().await;
        return;
    }
    // fallback: MUS_SIL_VS_POUSSIN (play_music).
    fallback(&ctx, "play_music").await;
    if battle_lost(&ctx, TRAINER_SIL_POUSSIN_9, Sommet_Apex_PG_Sc16c_T5).await {
        return;
    }
    if premier {
        say(&ctx, Sommet_Apex_PG_Sc16c_T6).await;
        add_item(&ctx, ITEM_SIL_SWAMPERTITE, 1).await;
        say(&ctx, Sommet_Apex_PG_Sc16c_T7).await;
        ctx.set_flag(FLAG_SIL_ITEM_PG_SWAMPERTITE, true).await;
    }
    ctx.set_flag(FLAG_SIL_DAILY_POUSSIN, true).await;
    ctx.release().await;
}

/// `Sommet_Apex_PG_Sc19`, the Purity Beacon (row 16 at 129): the Mallette on the socle, the slow
/// white, the true dawn, the sync at 0 (`VAR_SIL_SYNC`), the friends' words, the eight Code
/// Fragments (`FLAG_SIL_FRAGMENTS_ACTIFS`), 130. The clear point of Suicune, of Mew if Silas's
/// Vestige is done and of Ho-Oh if Kimiko's is (the plan's table).
///
/// Deviations: the Apex has no objects for Gwada, Mathieu, Adrien, Julien and Thomas (the maps'
/// review), so their lines are said without them on the roof; the player does not walk the three
/// cells to the socle (the row's cells are not all open).
pub async fn SommetApex_PG_Sc19(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Sommet_Apex_PG_Sc19_T1).await;
    say(&ctx, Sommet_Apex_PG_Sc19_T2).await;
    say(&ctx, Sommet_Apex_PG_Sc19_T3).await;
    fade_white(&ctx, true).await;
    // set_palette_set(ORIGINALE), set_weather(NONE); fallback: MUS_SIL_AUBE_VRAIE (§9).
    clear_palette(&ctx, 0).await;
    set_weather(&ctx, engine::weather::NONE).await;
    fallback(&ctx, "play_music").await;
    fade_white(&ctx, false).await;
    say(&ctx, Sommet_Apex_PG_Sc19_T4).await;
    ctx.set_var(VAR_SIL_SYNC, 0).await;
    say(&ctx, Sommet_Apex_PG_Sc19_T5).await;
    say(&ctx, Sommet_Apex_PG_Sc19_T6).await;
    say(&ctx, Sommet_Apex_PG_Sc19_T7).await;
    say(&ctx, Sommet_Apex_PG_Sc19_T8).await;
    say(&ctx, Sommet_Apex_PG_Sc19_T9).await;
    ctx.set_flag(FLAG_SIL_FRAGMENTS_ACTIFS, true).await;
    set_story(&ctx, 130).await;
    ctx.set_flag(FLAG_SIL_HIDE_SUICUNE, false).await;
    if ctx.flag(FLAG_SIL_VESTIGE_SILAS).await {
        ctx.set_flag(FLAG_SIL_HIDE_MEW, false).await;
    }
    if ctx.flag(FLAG_SIL_VESTIGE_KIMIKO).await {
        ctx.set_flag(FLAG_SIL_HIDE_HO_OH, false).await;
    }
    ctx.release_all().await;
}

const RAYQUAZA_DESCEND: &[Step] = &[walk(Dir::Down), walk(Dir::Down), walk(Dir::Down)];

/// `Sommet_Apex_PG_Sc32`, Rayquaza (the socle's four neighbours, from 130 until
/// `FLAG_SIL_RAYQUAZA_CAPTURE`; once a visit, `VAR_TEMP_1`): the roof shakes, Rayquaza comes down,
/// the battle (level 80). It never stays on the stage: its hide flag is set again before the
/// battle (a whiteout cannot leave it shown), and it is removed after; a capture sets
/// `FLAG_SIL_RAYQUAZA_CAPTURE` (the plan's clear points: Rayquaza is a scene prop).
pub async fn SommetApex_PG_Sc32(ctx: Ctx) {
    if story(&ctx).await < 130 || ctx.flag(FLAG_SIL_RAYQUAZA_CAPTURE).await {
        return;
    }
    ctx.set_var(VAR_TEMP_1, 1).await;
    ctx.lock_all().await;
    screen_shake(&ctx, 2, 40).await;
    show(&ctx, LOCALID_APEX_RAYQUAZA, FLAG_SIL_HIDE_RAYQUAZA).await;
    move_wait(&ctx, LOCALID_APEX_RAYQUAZA, RAYQUAZA_DESCEND).await;
    say(&ctx, Sommet_Apex_PG_Sc32_T1).await;
    ctx.set_flag(FLAG_SIL_HIDE_RAYQUAZA, true).await;
    let outcome = battle_wild(&ctx, SPECIES_RAYQUAZA, 80).await;
    if outcome == BattleOutcome::Caught {
        ctx.set_flag(FLAG_SIL_RAYQUAZA_CAPTURE, true).await;
    }
    hide(&ctx, LOCALID_APEX_RAYQUAZA).await;
    ctx.release_all().await;
}

/// The empty socle (shown from 120): the source gives it no line.
pub async fn SommetApex_EventScript_Socle(_ctx: Ctx) {}

/// Rayquaza is shown only inside `Sc32`, which locks the player: never talked to.
pub async fn SommetApex_EventScript_Rayquaza(_ctx: Ctx) {}
