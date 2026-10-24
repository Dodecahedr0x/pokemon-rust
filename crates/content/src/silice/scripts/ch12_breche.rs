//! Chapter 12, the post-game, Act III inside the Dome: the Breach (`contenu-jeu/chapitres/
//! 12-post-game.md` §3 `Nebula_PG_Sc10` to `Tour_Nebula_2F_PG_Sc18`, `Tour_Nebula_PG_Ascenseur`,
//! `PG_Gaz`, `PG_Lib_Thomas`; §5 the Breach's and the ruins' trainers; §7 their items).
//!
//! The way (`VAR_SIL_STORY` 126 → 129): the sky splits over Nébula (`Sc10`, on-frame on 126: the
//! Horde, Thomas, 127), Thomas's camp in the Breach (`Sc10b`), Gwada's camp at the Studio
//! (`Sc11`: the Filter v1), the three ruined floors and their Components (`Sc12`–`Sc14`, through
//! the elevator hub), the Filter v2 (`Sc15`, 128), then the outside (`ch12_dehors.rs`) and the
//! soil samples back to Gwada (`Sc18`, 129).
//!
//! The ruined floors have no walked way in: the elevator hub (`Tour_Nebula_PG_Ascenseur`, every
//! cabin arrival from 119, the hall's door, the ruins' south doors) warps by coordinates.
//!
//! Trainers' lines: §5's short intro and defeat lines are `text/fr.ron`'s
//! `<Map>_PG_<Name>_Intro` / `_Defeat` (the source's own naming rule). The beaten trainers repeat
//! their defeat line (the chapters write no post-battle lines).

use super::ch04_port::{Pushable, push_object};
use super::ch10_ascenseur::{LANDING_2F, LANDING_APEX};
use super::ch12_reveil::legendaire;
use crate::silice::mallette::{LIST_ASCENSEUR, LIST_ASCENSEUR_FERME, LIST_ASCENSEUR_TOIT};
use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &[
    "Breche",
    "TourNebula_1F_Ruine",
    "TourNebula_3F_Ruine",
    "TourNebula_4F_Ruine",
];

// ---- the elevator hub ----

/// Each ruined floor's landing (its warp 0, by its south door).
pub const LANDING_1F_RUINE: (i16, i16) = (7, 13);
pub const LANDING_3F_RUINE: (i16, i16) = (8, 11);
pub const LANDING_4F_RUINE: (i16, i16) = (7, 13);
/// The Cloud Temple's arrival (§3 `Sommet_Apex_PG_Sc31`, the map's south steps).
pub const LANDING_TEMPLE: (i16, i16) = (5, 18);
/// The hall's cell before the elevator door (`Nebula_TourHall` (12, 1)): where a cancelled ride
/// from the cabin lets the player out.
const HALL_DOOR: (i16, i16) = (12, 2);

/// A floor of the hub.
#[derive(Clone, Copy)]
enum Etage {
    Ruine1,
    Studio,
    Ruine3,
    Ruine4,
    Sommet,
    Toit,
}

/// `Tour_Nebula_PG_Ascenseur`, the post-game elevator (§3): before 127 only the Studio (2F) and the
/// Apex, the closed floors' notice first; from 127 the ruined 1F, 3F and 4F too, and the roof
/// (the Cloud Temple) with the Space Ticket (`FLAG_SIL_TICKET_ESPACE`). A real `multichoice`
/// (`silice::mallette`'s lists). `from_cabin`: run on the cabin's arrival (the cabin is never
/// walked), so a cancel lets the player out into the hall; elsewhere a cancel just closes.
///
/// Deviation: the 2F and the Apex land on their as-built landings (Chapter 10's
/// [`LANDING_2F`], [`LANDING_APEX`]) rather than the source's (5, 10) and (10, 14).
pub(super) async fn tour_pg_ascenseur(ctx: &Ctx, from_cabin: bool) {
    ctx.lock_all().await;
    let s = story(ctx).await;
    let floors: &[Etage] = if s < 127 {
        say(ctx, Tour_Nebula_PG_Ascenseur_T1).await;
        &[Etage::Studio, Etage::Sommet]
    } else if ctx.flag(FLAG_SIL_TICKET_ESPACE).await {
        &[
            Etage::Ruine1,
            Etage::Studio,
            Etage::Ruine3,
            Etage::Ruine4,
            Etage::Sommet,
            Etage::Toit,
        ]
    } else {
        &[
            Etage::Ruine1,
            Etage::Studio,
            Etage::Ruine3,
            Etage::Ruine4,
            Etage::Sommet,
        ]
    };
    let list = match floors.len() {
        2 => LIST_ASCENSEUR_FERME,
        5 => LIST_ASCENSEUR,
        _ => LIST_ASCENSEUR_TOIT,
    };
    let choice = multichoice(ctx, list, 0, 0, 0, true).await;
    ctx.close_message().await;
    let Some(&etage) = floors.get(usize::from(choice)) else {
        if from_cabin {
            warp(ctx, MAP_NEBULA_TOUR_HALL, HALL_DOOR.0, HALL_DOOR.1).await;
        } else {
            ctx.release_all().await;
        }
        return;
    };
    play_se(ctx, "SE_ELEVATOR").await;
    let (map, (x, y)) = match etage {
        Etage::Ruine1 => (MAP_TOUR_NEBULA_1F_RUINE, LANDING_1F_RUINE),
        Etage::Studio => (MAP_TOUR_NEBULA_2F, LANDING_2F),
        Etage::Ruine3 => (MAP_TOUR_NEBULA_3F_RUINE, LANDING_3F_RUINE),
        Etage::Ruine4 => (MAP_TOUR_NEBULA_4F_RUINE, LANDING_4F_RUINE),
        Etage::Sommet => (MAP_SOMMET_APEX, LANDING_APEX),
        Etage::Toit => (MAP_SOMMET_APEX_TEMPLE_NUAGES, LANDING_TEMPLE),
    };
    warp(ctx, map, x, y).await;
}

/// The 1F ruin's south door: the hub.
pub async fn TourNebula_1F_Ruine_EventScript_Ascenseur(ctx: Ctx) {
    tour_pg_ascenseur(&ctx, false).await;
}

/// The 3F ruin's south door: the hub.
pub async fn TourNebula_3F_Ruine_EventScript_Ascenseur(ctx: Ctx) {
    tour_pg_ascenseur(&ctx, false).await;
}

/// The 4F ruin's south door: the hub.
pub async fn TourNebula_4F_Ruine_EventScript_Ascenseur(ctx: Ctx) {
    tour_pg_ascenseur(&ctx, false).await;
}

// ---- PG_Gaz: the gas sheets ----

/// `PG_Gaz` (§3), the gas sheet `obj` (an invisible NPC on a one-cell neck): without the Filter
/// (`FLAG_SIL_MODULE_FILTRE`), or a v2 sheet before 128, the notice; else the question, and the
/// sheet goes (its hide flag `FLAG_SIL_HIDE_GAZ_<n>`, the map object's).
pub(super) async fn gaz(ctx: &Ctx, obj: u8, v2: bool) {
    ctx.lock().await;
    if !ctx.flag(FLAG_SIL_MODULE_FILTRE).await || (v2 && story(ctx).await < 128) {
        say(ctx, PG_Gaz_T1).await;
        ctx.release().await;
        return;
    }
    if ask(ctx, PG_Gaz_T2).await {
        play_se(ctx, "SE_FILTRE").await;
        ctx.fade(Fade::ToBlack).await;
        hide(ctx, obj).await;
        ctx.fade(Fade::FromBlack).await;
        say(ctx, PG_Gaz_T3).await;
    }
    ctx.release().await;
}

// ---- Nébula: the sky splits (Sc10) ----

/// `set_palette_set(DETRUITE)` and `set_weather(FOG_LIGHT)` from `Nebula_PG_Sc10` (127) to
/// `Sommet_Apex_PG_Sc19` (130), on every load of Nébula, the Tower's ruins and the Breach
/// (`FOG_LIGHT` is Gen3's horizontal fog). Their on-transition scripts call it.
pub(crate) async fn destroyed_sky(ctx: &Ctx) {
    if (127..130).contains(&story(ctx).await) {
        ctx.set_weather(u16::from(engine::weather::FOG_HORIZONTAL))
            .await;
        set_palette(ctx, DETRUITE, 0, false).await;
    }
}

/// The Breach's sky ([`destroyed_sky`]).
pub async fn Breche_OnTransition(ctx: Ctx) {
    destroyed_sky(&ctx).await;
}

/// The Tower's 1F ruins' sky ([`destroyed_sky`]).
pub async fn TourNebula_1F_Ruine_OnTransition(ctx: Ctx) {
    destroyed_sky(&ctx).await;
}

/// The Tower's 3F ruins' sky ([`destroyed_sky`]).
pub async fn TourNebula_3F_Ruine_OnTransition(ctx: Ctx) {
    destroyed_sky(&ctx).await;
}

/// The Tower's 4F ruins' sky ([`destroyed_sky`]).
pub async fn TourNebula_4F_Ruine_OnTransition(ctx: Ctx) {
    destroyed_sky(&ctx).await;
}

const MATHIEU_COMES: &[Step] = &[walk(Dir::Down), walk(Dir::Down), walk(Dir::Down), Step::FacePlayer];
const THOMAS_COMES: &[Step] = &[
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    face(Dir::Up),
];

/// `Nebula_PG_Sc10`, the sky splits (on-frame at 126): Mathieu, the roar, Thomas, the Horde (the
/// source's fallback: Tyranitar then Crobat at 72, Thomas heals between them), 127.
///
/// The "destroyed" palette and the fog (§9, `FOG_LIGHT`: Gen3's horizontal fog) from here to
/// 129 ([`destroyed_sky`]). Fallbacks: the alarm music (§9); `battle_wild_double`
/// with Thomas as the partner (no multi battle against two wild Pokémon): two wild battles in a
/// row. The crack itself is `Nebula_OnLoad`'s (from 126). Thomas leaves for the Breach's camp at
/// the end (hidden; the source leaves him standing, with no script on him).
pub async fn Nebula_PG_Sc10(ctx: Ctx) {
    ctx.lock_all().await;
    set_palette(&ctx, DETRUITE, PALETTE_FRAMES, false).await;
    set_weather(&ctx, engine::weather::FOG_HORIZONTAL).await;
    fallback(&ctx, "play_music").await;
    move_wait(&ctx, LOCALID_NEBULA_PG_MATHIEU, MATHIEU_COMES).await;
    say(&ctx, Nebula_PG_Sc10_T1).await;
    play_cry(&ctx).await;
    say(&ctx, Nebula_PG_Sc10_T2).await;
    show(&ctx, LOCALID_NEBULA_PG_THOMAS, FLAG_SIL_HIDE_NEBULA_PG_THOMAS).await;
    move_wait(&ctx, LOCALID_NEBULA_PG_THOMAS, THOMAS_COMES).await;
    say(&ctx, Nebula_PG_Sc10_T3).await;
    fallback(&ctx, "battle_wild_double").await;
    battle_wild(&ctx, SPECIES_TYRANITAR, 72).await;
    ctx.heal_party().await;
    say(&ctx, Nebula_PG_Sc10_T4).await;
    battle_wild(&ctx, SPECIES_CROBAT, 72).await;
    hide(&ctx, LOCALID_NEBULA_PG_TYRANITAR).await;
    hide(&ctx, LOCALID_NEBULA_PG_CROBAT).await;
    say(&ctx, Nebula_PG_Sc10_T5).await;
    say(&ctx, Nebula_PG_Sc10_T6).await;
    ctx.heal_party().await;
    hide(&ctx, LOCALID_NEBULA_PG_THOMAS).await;
    set_story(&ctx, 127).await;
    ctx.release_all().await;
}

// ---- the Breach ----

/// `Breche_PG_Sc10b`, Thomas's camp (from 127): he heals the team; the Heracronite once
/// (`FLAG_SIL_ITEM_PG_HERACRONITE`); from 130, `PG_Lib_Thomas` once: Articuno back in the sky
/// (`FLAG_SIL_LIBERATION_ARTIKODIN`, Articuno's clear point: it lands on the overhang).
pub async fn Breche_PG_Sc10b(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Breche_PG_Sc10b_T1).await;
    ctx.fade(Fade::ToBlack).await;
    ctx.heal_party().await;
    ctx.fade(Fade::FromBlack).await;
    say(&ctx, Breche_PG_Sc10b_T2).await;
    if gift(
        &ctx,
        FLAG_SIL_ITEM_PG_HERACRONITE,
        ITEM_SIL_HERACRONITE,
        1,
        Obtain::Message,
    )
    .await
    {
        say(&ctx, Breche_PG_Sc10b_T3).await;
    }
    if story(&ctx).await >= 130 && !ctx.flag(FLAG_SIL_LIBERATION_ARTIKODIN).await {
        say(&ctx, Breche_PG_Lib_T1).await;
        ctx.set_flag(FLAG_SIL_LIBERATION_ARTIKODIN, true).await;
        ctx.set_flag(FLAG_SIL_HIDE_ARTICUNO, false).await;
    }
    ctx.release().await;
}

/// The Council's supplies at the camp (§7 "Boutiques", from 127): the guard's line, then the
/// mart.
const BRECHE_SHOP: &[ItemId] = &[
    ItemId(ITEM_ANTIDOTE),
    ItemId(ITEM_FULL_HEAL),
    ItemId(ITEM_HYPER_POTION),
    ItemId(ITEM_ULTRA_BALL),
    ItemId(ITEM_MAX_REPEL),
    ItemId(ITEM_SIL_PIERRE_LIEN),
];

/// Mathieu's supply tent (the guard, (12, 19)).
pub async fn Breche_PG_Shop(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Breche_PG_Shop_T1).await;
    crate::prelude::mart(&ctx, BRECHE_SHOP).await;
    ctx.release().await;
}

/// Gas 01 (the first third): the Filter v1.
pub async fn Breche_EventScript_Gaz01(ctx: Ctx) {
    gaz(&ctx, LOCALID_BRECHE_GAZ_01, false).await;
}

/// Gas 02 (after the camp): the Filter v2.
pub async fn Breche_EventScript_Gaz02(ctx: Ctx) {
    gaz(&ctx, LOCALID_BRECHE_GAZ_02, true).await;
}

/// Gas 03 (before the exit): the Filter v2.
pub async fn Breche_EventScript_Gaz03(ctx: Ctx) {
    gaz(&ctx, LOCALID_BRECHE_GAZ_03, true).await;
}

/// EX-COCKET DAVY, the corridor's first third.
pub async fn Breche_EventScript_Davy(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_BRECHE_DAVY,
        TRAINER_SIL_BRECHE_EXCOCKET_1,
        Breche_PG_Davy_Intro,
        Breche_PG_Davy_Defeat,
    )
    .await
    {
        post_battle(&ctx, Breche_PG_Davy_Defeat).await;
    }
}

/// EX-COCKET NORA, just before gas 01.
pub async fn Breche_EventScript_Nora(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_BRECHE_NORA,
        TRAINER_SIL_BRECHE_EXCOCKET_2,
        Breche_PG_Nora_Intro,
        Breche_PG_Nora_Defeat,
    )
    .await
    {
        post_battle(&ctx, Breche_PG_Nora_Defeat).await;
    }
}

/// GARDE BASTIEN, at the camp's entrance.
pub async fn Breche_EventScript_Bastien(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_BRECHE_BASTIEN,
        TRAINER_SIL_BRECHE_GARDE_1,
        Breche_PG_Bastien_Intro,
        Breche_PG_Bastien_Defeat,
    )
    .await
    {
        post_battle(&ctx, Breche_PG_Bastien_Defeat).await;
    }
}

/// CHERCHEUSE OLGA, after the camp.
pub async fn Breche_EventScript_Olga(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_BRECHE_OLGA,
        TRAINER_SIL_BRECHE_CHERCHEUSE_1,
        Breche_PG_Olga_Intro,
        Breche_PG_Olga_Defeat,
    )
    .await
    {
        post_battle(&ctx, Breche_PG_Olga_Defeat).await;
    }
}

/// Three Ultra Balls under a glass shard (§7: "Hyper Ball ×3").
pub async fn Breche_EventScript_ItemUltraBall(ctx: Ctx) {
    find_item(&ctx, ITEM_ULTRA_BALL, 3).await;
}

/// The Max Elixir behind gas 02.
pub async fn Breche_EventScript_ItemMaxElixir(ctx: Ctx) {
    find_item(&ctx, ITEM_MAX_ELIXIR, 1).await;
}

/// The Rare Candy against the west wall (4, 28), hidden.
pub async fn Breche_EventScript_HiddenRareCandy(ctx: Ctx) {
    // fallback: hidden items (the engine ignores `BgKind::HiddenItem`): a sign here instead.
    hidden_item(&ctx, FLAG_SIL_ITEM_PG_BRECHE_3, ITEM_RARE_CANDY).await;
}

/// Articuno on the glass overhang (`PG_Legendaire`, level 75; shown by `PG_Lib_Thomas`).
pub async fn Breche_EventScript_Artikodin(ctx: Ctx) {
    legendaire(
        &ctx,
        PG_Legendaire_Artikodin,
        SPECIES_ARTICUNO,
        75,
        LOCALID_BRECHE_ARTIKODIN,
    )
    .await;
}

// ---- the Studio, Gwada's camp (2F) ----

/// Gwada at the Studio from 127 to 129 (`Chapter 10`'s `TourNebula_2F_OnTransition` calls this).
pub(super) async fn tour_2f_gwada_here(ctx: &Ctx) {
    let here = (127..=129).contains(&story(ctx).await);
    ctx.set_flag(FLAG_SIL_HIDE_TOUR_2F_GWADA, !here).await;
}

/// Whether all three flags are set.
async fn all(ctx: &Ctx, flags: [FlagId; 3]) -> bool {
    for f in flags {
        if !ctx.flag(f).await {
            return false;
        }
    }
    true
}

/// Gwada at his camp (local 10): `Sc11` (the Filter v1, 127), his reminder until the three
/// Components are in, `Sc15` (the Filter v2, 128), the samples' reminder, `Sc18` (129), then
/// the Beacon's reminder. The reminders at 128 and 129 repeat his last instruction
/// (`Sc15_T3`, `Sc18_T3`): the source gives him no other line.
pub async fn TourNebula_2F_PG_Gwada(ctx: Ctx) {
    let s = story(&ctx).await;
    ctx.lock().await;
    ctx.face_player().await;
    match s {
        127 if !ctx.flag(FLAG_SIL_MODULE_FILTRE).await => tour_2f_pg_sc11(&ctx).await,
        127 if all(
            &ctx,
            [FLAG_SIL_COMPOSANT_1, FLAG_SIL_COMPOSANT_2, FLAG_SIL_COMPOSANT_3],
        )
        .await =>
        {
            tour_2f_pg_sc15(&ctx).await
        }
        127 => say(&ctx, Tour_Nebula_2F_PG_Sc11_T6).await,
        128 if all(
            &ctx,
            [FLAG_SIL_ECHANTILLON_1, FLAG_SIL_ECHANTILLON_2, FLAG_SIL_ECHANTILLON_3],
        )
        .await =>
        {
            tour_2f_pg_sc18(&ctx).await
        }
        128 => say(&ctx, Tour_Nebula_2F_PG_Sc15_T3).await,
        _ => say(&ctx, Tour_Nebula_2F_PG_Sc18_T3).await,
    }
    ctx.release().await;
}

/// `Tour_Nebula_2F_PG_Sc11`, Gwada's HQ: the Breach, the dust, the Mallette reprogrammed as a
/// lung (`FLAG_SIL_MODULE_FILTRE`), the three Components in the ruined floors.
async fn tour_2f_pg_sc11(ctx: &Ctx) {
    say(ctx, Tour_Nebula_2F_PG_Sc11_T1).await;
    say(ctx, Tour_Nebula_2F_PG_Sc11_T2).await;
    say(ctx, Tour_Nebula_2F_PG_Sc11_T3).await;
    say(ctx, Tour_Nebula_2F_PG_Sc11_T4).await;
    ctx.set_flag(FLAG_SIL_MODULE_FILTRE, true).await;
    say(ctx, Tour_Nebula_2F_PG_Sc11_T5).await;
}

/// `Tour_Nebula_2F_PG_Sc15`, the Filter v2 (the three Components): 128. This is the clear point
/// of Moltres and Latios (the plan's table): they appear outside.
async fn tour_2f_pg_sc15(ctx: &Ctx) {
    say(ctx, Tour_Nebula_2F_PG_Sc15_T1).await;
    ctx.fade(Fade::ToBlack).await;
    ctx.delay(60).await;
    play_se(ctx, "SE_FILTRE").await;
    ctx.fade(Fade::FromBlack).await;
    say(ctx, Tour_Nebula_2F_PG_Sc15_T2).await;
    say(ctx, Tour_Nebula_2F_PG_Sc15_T3).await;
    set_story(ctx, 128).await;
    ctx.set_flag(FLAG_SIL_HIDE_MOLTRES, false).await;
    ctx.set_flag(FLAG_SIL_HIDE_LATIOS, false).await;
}

/// `Tour_Nebula_2F_PG_Sc18`, life can start again (the three samples): the Beacon's idea, the
/// Sceptilite (`FLAG_SIL_ITEM_PG_SCEPTILITE`), 129.
async fn tour_2f_pg_sc18(ctx: &Ctx) {
    say(ctx, Tour_Nebula_2F_PG_Sc18_T1).await;
    ctx.delay(40).await;
    say(ctx, Tour_Nebula_2F_PG_Sc18_T2).await;
    say(ctx, Tour_Nebula_2F_PG_Sc18_T3).await;
    gift(
        ctx,
        FLAG_SIL_ITEM_PG_SCEPTILITE,
        ITEM_SIL_SCEPTILITE,
        1,
        Obtain::Message,
    )
    .await;
    say(ctx, Tour_Nebula_2F_PG_Sc18_T4).await;
    set_story(ctx, 129).await;
}

/// The camp's PC (21, 5): the Pokémon storage, as a Centre's (§2), while Gwada camps there
/// (127–129); silent otherwise.
pub async fn TourNebula_2F_PG_PC(ctx: Ctx) {
    if !(127..=129).contains(&story(&ctx).await) {
        return;
    }
    ctx.lock().await;
    player_pc(&ctx, PcMode::Storage).await;
    ctx.release().await;
}

// ---- the ruined 1F: Mathias's cellar (Composant A) ----

/// `Tour_Nebula_1F_Ruine_PG_Sc12`, Mathias among his barrels.
pub async fn TourNebula_1F_Ruine_PG_Sc12(ctx: Ctx) {
    msgbox(&ctx, Tour_Nebula_1F_Ruine_PG_Sc12_T1, MsgBox::Npc).await;
}

/// `Sc12`'s Component A on the still (behind gas 06): `FLAG_SIL_COMPOSANT_1`, Mathias's comment.
pub async fn TourNebula_1F_Ruine_PG_Sc12_Composant(ctx: Ctx) {
    ctx.lock().await;
    say(&ctx, Tour_Nebula_1F_Ruine_PG_Sc12_T2).await;
    add_item(&ctx, ITEM_SIL_COMPOSANT_FILTRE, 1).await;
    ctx.set_flag(FLAG_SIL_COMPOSANT_1, true).await;
    hide(&ctx, LOCALID_RUINE1_COMPOSANT).await;
    say(&ctx, Tour_Nebula_1F_Ruine_PG_Sc12_T3).await;
    ctx.release().await;
}

/// Gas 06, before the still: the Filter v1.
pub async fn TourNebula_1F_Ruine_EventScript_Gaz06(ctx: Ctx) {
    gaz(&ctx, LOCALID_RUINE1_GAZ, false).await;
}

/// EX-COCKET GUS, between the barrels.
pub async fn TourNebula_1F_Ruine_EventScript_Gus(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_RUINE1_GUS,
        TRAINER_SIL_TOUR_RUINE_EXCOCKET_1,
        Tour_Nebula_1F_Ruine_PG_Gus_Intro,
        Tour_Nebula_1F_Ruine_PG_Gus_Defeat,
    )
    .await
    {
        post_battle(&ctx, Tour_Nebula_1F_Ruine_PG_Gus_Defeat).await;
    }
}

/// The two overturned barrels (§2: the Champ Magnétique opens the middle aisle) and their cells
/// once pushed (`VAR_TEMP_2`, `VAR_TEMP_3`).
const FUTS: &[Pushable] = &[
    (LOCALID_RUINE1_FUT_1, VAR_TEMP_2, (10, 8)),
    (LOCALID_RUINE1_FUT_2, VAR_TEMP_3, (7, 10)),
];

/// East only (the plan's review): the second barrel (7, 10) into (8, 10), the first (10, 8) into
/// (11, 8); no push strands the player.
fn fut_free(c: (i16, i16)) -> bool {
    matches!(c, (8, 10) | (11, 8))
}

/// A barrel: the Champ Magnétique (`FLAG_SIL_MODULE_FORCE`), as Chapter 3's server blocks.
pub async fn TourNebula_1F_Ruine_EventScript_Fut(ctx: Ctx) {
    push_object(&ctx, FUTS, fut_free).await;
}

/// The Rum in an intact barrel (12, 9), hidden (§7).
pub async fn TourNebula_1F_Ruine_EventScript_HiddenRhum(ctx: Ctx) {
    // fallback: hidden items (the engine ignores `BgKind::HiddenItem`): a sign here instead.
    hidden_item(&ctx, FLAG_SIL_ITEM_PG_RUINE1_RHUM, ITEM_SIL_RHUM_PIRATE).await;
}

/// Entei by the still (`PG_Legendaire`, level 75; shown by `Nebula_PG_Sc03c`).
pub async fn TourNebula_1F_Ruine_EventScript_Entei(ctx: Ctx) {
    legendaire(
        &ctx,
        PG_Legendaire_Entei,
        SPECIES_ENTEI,
        75,
        LOCALID_RUINE1_ENTEI,
    )
    .await;
}

// ---- the ruined 3F: the burst Horizon (Composant B) ----

/// `Tour_Nebula_3F_Ruine_PG_Sc13`, the lectern where Arthur's compass lay: Component B
/// (`FLAG_SIL_COMPOSANT_2`), its line by Arthur's pardon. Once.
pub async fn TourNebula_3F_Ruine_PG_Sc13(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_COMPOSANT_2).await {
        return;
    }
    ctx.lock().await;
    say(&ctx, Tour_Nebula_3F_Ruine_PG_Sc13_T1).await;
    if ctx.flag(FLAG_SIL_PARDON_ARTHUR).await {
        say(&ctx, Tour_Nebula_3F_Ruine_PG_Sc13_T2).await;
    } else {
        say(&ctx, Tour_Nebula_3F_Ruine_PG_Sc13_T3).await;
    }
    add_item(&ctx, ITEM_SIL_COMPOSANT_FILTRE, 1).await;
    ctx.set_flag(FLAG_SIL_COMPOSANT_2, true).await;
    say(&ctx, Tour_Nebula_3F_Ruine_PG_Sc13_T4).await;
    ctx.release().await;
}

/// Gas 07, on the ledge: the Filter v1.
pub async fn TourNebula_3F_Ruine_EventScript_Gaz07(ctx: Ctx) {
    gaz(&ctx, LOCALID_RUINE3_GAZ, false).await;
}

/// EX-COCKET LUDO, on the ledge.
pub async fn TourNebula_3F_Ruine_EventScript_Ludo(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_RUINE3_LUDO,
        TRAINER_SIL_TOUR_RUINE_EXCOCKET_2,
        Tour_Nebula_3F_Ruine_PG_Ludo_Intro,
        Tour_Nebula_3F_Ruine_PG_Ludo_Defeat,
    )
    .await
    {
        post_battle(&ctx, Tour_Nebula_3F_Ruine_PG_Ludo_Defeat).await;
    }
}

/// The PP Up at a broken walkway's end.
pub async fn TourNebula_3F_Ruine_EventScript_ItemPpUp(ctx: Ctx) {
    find_item(&ctx, ITEM_PP_UP, 1).await;
}

// ---- the ruined 4F: the broken Reflections (Composant C) ----

/// `Tour_Nebula_4F_Ruine_PG_Sc14`, the Shadow Prism's socle (behind gas 08): Component C
/// (`FLAG_SIL_COMPOSANT_3`, the prism's map flag).
pub async fn TourNebula_4F_Ruine_PG_Sc14(ctx: Ctx) {
    ctx.lock().await;
    say(&ctx, Tour_Nebula_4F_Ruine_PG_Sc14_T1).await;
    add_item(&ctx, ITEM_SIL_COMPOSANT_FILTRE, 1).await;
    ctx.set_flag(FLAG_SIL_COMPOSANT_3, true).await;
    hide(&ctx, LOCALID_RUINE4_PRISME).await;
    say(&ctx, Tour_Nebula_4F_Ruine_PG_Sc14_T2).await;
    ctx.release().await;
}

/// Gas 08, in the middle: the Filter v1.
pub async fn TourNebula_4F_Ruine_EventScript_Gaz08(ctx: Ctx) {
    gaz(&ctx, LOCALID_RUINE4_GAZ, false).await;
}

/// EX-COCKET MIRA, in the mirrors.
pub async fn TourNebula_4F_Ruine_EventScript_Mira(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_RUINE4_MIRA,
        TRAINER_SIL_TOUR_RUINE_EXCOCKET_3,
        Tour_Nebula_4F_Ruine_PG_Mira_Intro,
        Tour_Nebula_4F_Ruine_PG_Mira_Defeat,
    )
    .await
    {
        post_battle(&ctx, Tour_Nebula_4F_Ruine_PG_Mira_Defeat).await;
    }
}

/// Three Timer Balls behind a cracked mirror (§7: "Chrono Ball ×3").
pub async fn TourNebula_4F_Ruine_EventScript_ItemTimerBall(ctx: Ctx) {
    find_item(&ctx, ITEM_TIMER_BALL, 3).await;
}

/// A cracked mirror (Rock Smash; the Scanner de Faille module, `FLAG_SIL_MODULE_ECLATE_ROC`):
/// the module's question, then the mirror shatters (Gen3's `EventScript_RockSmash` on the
/// object, as Chapter 4's rocks).
pub async fn TourNebula_4F_Ruine_EventScript_Miroir(ctx: Ctx) {
    super::ch12_dehors::rock_smash(&ctx).await;
}
