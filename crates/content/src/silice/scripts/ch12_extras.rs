//! Chapter 12, the post-game, optional content on the optional maps and across the region
//! (`contenu-jeu/chapitres/12-post-game.md` §3 `Brico_Bourg_Bar_PG_Sc22`, `Route_S05_PG_Sc27`,
//! `Route_S08_PG_Sc28`, `PG_Fragment`, `Sommet_Apex_PG_Sc31`, `Manga_Square_PG_Sc32a`,
//! `PG_Legendaire`; §4.3 the Antennes de Combat).
//!
//! Crow's Lounge under the Shaker (from 121, a daily rematch), the Debug Island (Deoxys, once the
//! Debug Key is installed), the Stele cave (the starter's « Dominant » form and its tutor move),
//! the three Antennas (a party rule, three trainers, daily), the eight Code Fragments (from 130;
//! the eighth sends the player to Gwada: `ch12_reveil.rs`'s `Sc30`), the Cloud Temple (Jirachi),
//! Tak before the statue, and the legendaries freed by the rematches (`ch12_revanches.rs`) and by
//! Poussin (Lugia, Kyogre).

use super::ch08_evalon::lost;
use super::ch12_reveil::{battle_lost, legendaire};
use crate::prelude::{PartyMode, choose_party_mon};
use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &[
    "BricoBourg_Bar_Lounge",
    "RouteS05_IleDebug",
    "RouteS08_GrotteStele",
    "RouteS02_Antenne",
    "RouteS03_Antenne",
    "RouteS07_Antenne",
    "SommetApex_TempleNuages",
];

// ---- Crow's Lounge (Brico_Bourg_Bar_PG_Sc22) ----

/// The Shaker's bookcase slid open: the stairs (12, 1), the wall top above (12, 0).
const LOUNGE_ESCALIER: (i16, i16, u16) = (12, 1, 592);
const LOUNGE_MUR: (i16, i16, u16) = (12, 0, 530);

/// Draws the bookcase open.
async fn lounge_ouvert(ctx: &Ctx) {
    ctx.set_metatile(LOUNGE_ESCALIER.0, LOUNGE_ESCALIER.1, LOUNGE_ESCALIER.2, false)
        .await;
    ctx.set_metatile(LOUNGE_MUR.0, LOUNGE_MUR.1, LOUNGE_MUR.2, true)
        .await;
}

/// The bookcase stays open once Crow has slid it (`FLAG_SIL_LOUNGE_OUVERT`).
pub async fn BricoBourg_Bar_OnLoad(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_LOUNGE_OUVERT).await {
        lounge_ouvert(&ctx).await;
    }
}

/// `Brico_Bourg_Bar_PG_Sc22` at the Shaker's counter (from 121; `ch00_bar.rs`'s Crow calls it):
/// the first time, the Lounge downstairs and the bookcase sliding open; then the reminder.
pub(super) async fn crow_comptoir(ctx: &Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if !ctx.flag(FLAG_SIL_LOUNGE_OUVERT).await {
        say(ctx, Brico_Bourg_Bar_PG_Sc22_T1).await;
        lounge_ouvert(ctx).await;
        ctx.set_flag(FLAG_SIL_LOUNGE_OUVERT, true).await;
    } else {
        say(ctx, Brico_Bourg_Bar_PG_Sc22_T2).await;
    }
    ctx.release().await;
}

/// `Brico_Bourg_Bar_PG_Sc22` in the Lounge: Crow's daily rematch (`FLAG_SIL_DAILY_CROW`), his
/// reading of the player's starter, the Shell Bell the first win (`FLAG_SIL_CROW_RECOMPENSE`).
///
/// fallback: the item-using AI (§9) is the engine's trainer AI.
pub async fn BricoBourg_Bar_Lounge_PG_Crow(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if ctx.flag(FLAG_SIL_DAILY_CROW).await {
        say(&ctx, Brico_Bourg_Bar_PG_Sc22_T8).await;
        ctx.release().await;
        return;
    }
    say(&ctx, Brico_Bourg_Bar_PG_Sc22_T3).await;
    let lecture = match ctx.var(VAR_SIL_STARTER).await {
        0 => Brico_Bourg_Bar_PG_Sc22_T4a,
        1 => Brico_Bourg_Bar_PG_Sc22_T4b,
        _ => Brico_Bourg_Bar_PG_Sc22_T4c,
    };
    say(&ctx, lecture).await;
    say(&ctx, Brico_Bourg_Bar_PG_Sc22_T5).await;
    if battle_lost(&ctx, TRAINER_SIL_CROW_9, Brico_Bourg_Bar_PG_Sc22_T6).await {
        return;
    }
    if !ctx.flag(FLAG_SIL_CROW_RECOMPENSE).await {
        give_item(&ctx, ITEM_SHELL_BELL, 1).await;
        say(&ctx, Brico_Bourg_Bar_PG_Sc22_T7).await;
        ctx.set_flag(FLAG_SIL_CROW_RECOMPENSE, true).await;
    }
    ctx.set_flag(FLAG_SIL_DAILY_CROW, true).await;
    ctx.release().await;
}

// ---- the Debug Island (Route_S05_PG_Sc27) ----

/// The blinking sea cell east of the wreck (a coord, every step): with the Debug Key installed
/// (`FLAG_SIL_CLE_DEBUG_INSTALLEE`), the sea pixelates and the island appears (the warp). Before,
/// nothing.
///
/// fallback: `set_metatile(ROUTE_S05_TUILE_GLITCH, ILE_DEBUG_ENTREE)`: the warp ends the script
/// at once, so the tile change would never be seen.
pub async fn RouteS05_PG_Sc27(ctx: Ctx) {
    if !ctx.flag(FLAG_SIL_CLE_DEBUG_INSTALLEE).await {
        return;
    }
    ctx.lock_all().await;
    msgbox(&ctx, Route_S05_PG_Sc27_T1, MsgBox::Default).await;
    warp(&ctx, MAP_ROUTE_S05_ILE_DEBUG, 4, 8).await;
}

/// Deoxys on the checkerboard (level 80).
pub async fn RouteS05_IleDebug_EventScript_Deoxys(ctx: Ctx) {
    legendaire(
        &ctx,
        Route_S05_PG_Sc27_T2,
        SPECIES_DEOXYS,
        80,
        LOCALID_ILE_DEBUG_DEOXYS,
    )
    .await;
}

// ---- the Stele cave (Route_S08_PG_Sc28) ----

/// `Route_S08_PG_Sc28`, the original form (reading the stele, from 121, once:
/// `FLAG_SIL_FORME_ORIGINELLE`): the starter's final form as a « Dominant » (level 80, the boss
/// aura, no catching), then its tutor move.
///
/// fallback: `MOVE_SIL_CODE_AQUA` (§4.5) is an invented move; the source's own Gen3 fallback is
/// taught instead (Meganium Ancient Power, Typhlosion Crush Claw, Feraligatr Crunch), through
/// Gen3's move tutor screen (`ChooseMonForMoveTutor`): the player picks the mon (the source's
/// `party_slot_of(species)`, the starter). A loss whites out with the flag unset.
pub async fn RouteS08_GrotteStele_PG_Sc28(ctx: Ctx) {
    if story(&ctx).await < 121 || ctx.flag(FLAG_SIL_FORME_ORIGINELLE).await {
        return;
    }
    ctx.lock().await;
    msgbox(&ctx, Route_S08_PG_Sc28_T1, MsgBox::Default).await;
    let (species, tutor) = match ctx.var(VAR_SIL_STARTER).await {
        0 => (SPECIES_MEGANIUM, MOVE_ANCIENT_POWER),
        1 => (SPECIES_TYPHLOSION, MOVE_CRUSH_CLAW),
        _ => (SPECIES_FERALIGATR, MOVE_CRUNCH),
    };
    msgbox(&ctx, Route_S08_PG_Sc28_T2, MsgBox::Default).await;
    play_cry(&ctx).await;
    if lost(battle_wild_boss(&ctx, species, 80).await) {
        return;
    }
    ctx.set_flag(FLAG_SIL_FORME_ORIGINELLE, true).await;
    msgbox(&ctx, Route_S08_PG_Sc28_T3, MsgBox::Default).await;
    if ask(&ctx, Route_S08_PG_Sc28_T4).await {
        fallback(&ctx, "MOVE_SIL_CODE_AQUA").await;
        let slot = choose_party_mon(&ctx, PartyMode::MoveTutor(tutor)).await;
        if u16::from(slot) != crate::prelude::PARTY_NOTHING_CHOSEN {
            msgbox(&ctx, Route_S08_PG_Sc28_T5, MsgBox::Default).await;
        }
    }
    ctx.release().await;
}

// ---- the Antennes de Combat (§4.3) ----

/// One Antenna: its rule, its three trainers (object, trainer, intro, defeat), its first-win
/// rewards and flags.
struct Antenne {
    regle: PartyRule,
    regle_texte: TextId,
    dresseurs: [(u8, TrainerId, TextId, TextId); 3],
    recompenses: [u16; 2],
    premiere: FlagId,
    daily: FlagId,
}

const ALPHA: Antenne = Antenne {
    regle: PartyRule::NoItems,
    regle_texte: PG_Antenne_T1_SansObjet,
    dresseurs: [
        (
            LOCALID_S02_ANTENNE_HUGO,
            TRAINER_SIL_ANTENNE_ALPHA_1,
            Antenne_Alpha_1_Intro,
            Antenne_Alpha_1_Defeat,
        ),
        (
            LOCALID_S02_ANTENNE_LEA,
            TRAINER_SIL_ANTENNE_ALPHA_2,
            Antenne_Alpha_2_Intro,
            Antenne_Alpha_2_Defeat,
        ),
        (
            LOCALID_S02_ANTENNE_ROLAND,
            TRAINER_SIL_ANTENNE_ALPHA_3,
            Antenne_Alpha_3_Intro,
            Antenne_Alpha_3_Defeat,
        ),
    ],
    recompenses: [ITEM_CHOICE_BAND, ITEM_SIL_PINSIRITE],
    premiere: FLAG_SIL_ANTENNE_ALPHA,
    daily: FLAG_SIL_DAILY_ANTENNE_ALPHA,
};

const GAMMA: Antenne = Antenne {
    regle: PartyRule::MonoSteel,
    regle_texte: PG_Antenne_T1_MonoAcier,
    dresseurs: [
        (
            LOCALID_S03_ANTENNE_FERNAND,
            TRAINER_SIL_ANTENNE_GAMMA_1,
            Antenne_Gamma_1_Intro,
            Antenne_Gamma_1_Defeat,
        ),
        (
            LOCALID_S03_ANTENNE_INES,
            TRAINER_SIL_ANTENNE_GAMMA_2,
            Antenne_Gamma_2_Intro,
            Antenne_Gamma_2_Defeat,
        ),
        (
            LOCALID_S03_ANTENNE_GASTON,
            TRAINER_SIL_ANTENNE_GAMMA_3,
            Antenne_Gamma_3_Intro,
            Antenne_Gamma_3_Defeat,
        ),
    ],
    recompenses: [ITEM_METAL_COAT, ITEM_SIL_SCIZORITE],
    premiere: FLAG_SIL_ANTENNE_GAMMA,
    daily: FLAG_SIL_DAILY_ANTENNE_GAMMA,
};

const ETA: Antenne = Antenne {
    regle: PartyRule::Trio,
    regle_texte: PG_Antenne_T1_Trio,
    dresseurs: [
        (
            LOCALID_S07_ANTENNE_SACHA,
            TRAINER_SIL_ANTENNE_ETA_1,
            Antenne_Eta_1_Intro,
            Antenne_Eta_1_Defeat,
        ),
        (
            LOCALID_S07_ANTENNE_KENJI,
            TRAINER_SIL_ANTENNE_ETA_2,
            Antenne_Eta_2_Intro,
            Antenne_Eta_2_Defeat,
        ),
        (
            LOCALID_S07_ANTENNE_VLAD,
            TRAINER_SIL_ANTENNE_ETA_3,
            Antenne_Eta_3_Intro,
            Antenne_Eta_3_Defeat,
        ),
    ],
    recompenses: [ITEM_SCOPE_LENS, ITEM_SIL_MEDICHAMITE],
    premiere: FLAG_SIL_ANTENNE_ETA,
    daily: FLAG_SIL_DAILY_ANTENNE_ETA,
};

/// `PG_Antenne` at the receptionist: the rule, the daily check, the party checked against the
/// rule (`party_respects`), the question, then up to the top room (4, 6). A map load ends a
/// script, so the climb is marked (`FLAG_SIL_ANTENNE_MONTEE`) and the top room's on-frame script
/// ([`sommet`]) runs the battles.
async fn receptionniste(ctx: &Ctx, a: &Antenne, map: MapId) {
    ctx.lock().await;
    ctx.face_player().await;
    msgbox(ctx, a.regle_texte, MsgBox::Default).await;
    if ctx.flag(a.daily).await {
        msgbox(ctx, PG_Antenne_T5, MsgBox::Default).await;
        ctx.release().await;
        return;
    }
    if !party_respects(ctx, a.regle).await {
        msgbox(ctx, PG_Antenne_T2, MsgBox::Default).await;
        ctx.release().await;
        return;
    }
    if !ask(ctx, PG_Antenne_T3).await {
        ctx.release().await;
        return;
    }
    ctx.set_flag(FLAG_SIL_ANTENNE_MONTEE, true).await;
    warp(ctx, map, 4, 6).await;
}

const ENTRE: &[Step] = &[walk(Dir::Down), walk(Dir::Down)];

/// The top room (on-frame, `VAR_TEMP_1` 0, every load): nothing unless the receptionist sent the
/// player up. The three trainers come in by the north door one after the other; the party is
/// healed after each battle; the first full run gives the rewards. A loss whites out (the climb
/// mark is already cleared).
async fn sommet(ctx: &Ctx, a: &Antenne) {
    ctx.set_var(VAR_TEMP_1, 1).await;
    if !ctx.flag(FLAG_SIL_ANTENNE_MONTEE).await {
        return;
    }
    ctx.set_flag(FLAG_SIL_ANTENNE_MONTEE, false).await;
    ctx.lock_all().await;
    for (obj, trainer, intro, defeat) in a.dresseurs {
        ctx.add_object(obj).await;
        move_wait(ctx, obj, ENTRE).await;
        say(ctx, intro).await;
        if battle_lost(ctx, trainer, defeat).await {
            return;
        }
        ctx.remove_object(obj).await;
        ctx.heal_party().await;
    }
    msgbox(ctx, PG_Antenne_T4, MsgBox::Default).await;
    if !ctx.flag(a.premiere).await {
        for item in a.recompenses {
            give_item(ctx, item, 1).await;
        }
        ctx.set_flag(a.premiere, true).await;
    }
    ctx.set_flag(a.daily, true).await;
    ctx.release_all().await;
}

/// The Antenna's healing machine: the source gives it no script (the receptionist's line says the
/// party is healed between battles); it heals, as a Centre's machine.
async fn soin(ctx: &Ctx) {
    ctx.lock().await;
    play_se(ctx, "SE_USE_ITEM").await;
    ctx.heal_party().await;
    ctx.release().await;
}

pub async fn RouteS02_Antenne_PG_Receptionniste(ctx: Ctx) {
    receptionniste(&ctx, &ALPHA, MAP_ROUTE_S02_ANTENNE).await;
}

pub async fn RouteS02_Antenne_PG_Sommet(ctx: Ctx) {
    sommet(&ctx, &ALPHA).await;
}

pub async fn RouteS02_Antenne_PG_Soin(ctx: Ctx) {
    soin(&ctx).await;
}

pub async fn RouteS03_Antenne_PG_Receptionniste(ctx: Ctx) {
    receptionniste(&ctx, &GAMMA, MAP_ROUTE_S03_ANTENNE).await;
}

pub async fn RouteS03_Antenne_PG_Sommet(ctx: Ctx) {
    sommet(&ctx, &GAMMA).await;
}

pub async fn RouteS03_Antenne_PG_Soin(ctx: Ctx) {
    soin(&ctx).await;
}

pub async fn RouteS07_Antenne_PG_Receptionniste(ctx: Ctx) {
    receptionniste(&ctx, &ETA, MAP_ROUTE_S07_ANTENNE).await;
}

pub async fn RouteS07_Antenne_PG_Sommet(ctx: Ctx) {
    sommet(&ctx, &ETA).await;
}

pub async fn RouteS07_Antenne_PG_Soin(ctx: Ctx) {
    soin(&ctx).await;
}

// ---- the Cloud Temple (Sommet_Apex_PG_Sc31) ----

/// Jirachi asleep on the altar (level 70).
pub async fn SommetApex_TempleNuages_PG_Sc31(ctx: Ctx) {
    legendaire(
        &ctx,
        Sommet_Apex_PG_Sc31_T1,
        SPECIES_JIRACHI,
        70,
        LOCALID_TEMPLE_NUAGES_JIRACHI,
    )
    .await;
}

// ---- the Code Fragments (PG_Fragment) ----

/// The "BIP" two cells before fragment `n` (a coord, every step): from 130
/// (`FLAG_SIL_FRAGMENTS_ACTIFS`) until it is found.
///
/// fallback: `SE_MALLETTE_BIP` (§9) is Gen3's `SE_PIN`.
async fn fragment_bip(ctx: &Ctx, found: FlagId) {
    if !ctx.flag(FLAG_SIL_FRAGMENTS_ACTIFS).await || ctx.flag(found).await {
        return;
    }
    play_se(ctx, "SE_PIN").await;
    msgbox(ctx, PG_Fragment_T1, MsgBox::Autoclose).await;
}

/// Fragment `n` (a sign: the source's « objet caché »): found once from 130; the eighth says
/// Gwada will know.
async fn fragment(ctx: &Ctx, found: FlagId) {
    if !ctx.flag(FLAG_SIL_FRAGMENTS_ACTIFS).await || ctx.flag(found).await {
        return;
    }
    ctx.lock().await;
    msgbox(ctx, PG_Fragment_T2, MsgBox::Default).await;
    ctx.set_flag(found, true).await;
    let n = ctx.var(VAR_SIL_FRAGMENTS).await + 1;
    ctx.set_var(VAR_SIL_FRAGMENTS, n).await;
    if n == 8 {
        msgbox(ctx, PG_Fragment_T3, MsgBox::Default).await;
    }
    ctx.release().await;
}

pub async fn RouteS01_PG_Fragment(ctx: Ctx) {
    fragment(&ctx, FLAG_SIL_FRAGMENT_1).await;
}

pub async fn RouteS01_PG_FragmentBip(ctx: Ctx) {
    fragment_bip(&ctx, FLAG_SIL_FRAGMENT_1).await;
}

pub async fn BoisEther_PG_Fragment(ctx: Ctx) {
    fragment(&ctx, FLAG_SIL_FRAGMENT_2).await;
}

pub async fn BoisEther_PG_FragmentBip(ctx: Ctx) {
    fragment_bip(&ctx, FLAG_SIL_FRAGMENT_2).await;
}

pub async fn GrisEchec_TourObservation_Top_PG_Fragment(ctx: Ctx) {
    fragment(&ctx, FLAG_SIL_FRAGMENT_3).await;
}

pub async fn GrisEchec_TourObservation_Top_PG_FragmentBip(ctx: Ctx) {
    fragment_bip(&ctx, FLAG_SIL_FRAGMENT_3).await;
}

pub async fn ViberCity_PG_Fragment(ctx: Ctx) {
    fragment(&ctx, FLAG_SIL_FRAGMENT_4).await;
}

pub async fn ViberCity_PG_FragmentBip(ctx: Ctx) {
    fragment_bip(&ctx, FLAG_SIL_FRAGMENT_4).await;
}

pub async fn RouteS04_PG_Fragment(ctx: Ctx) {
    fragment(&ctx, FLAG_SIL_FRAGMENT_5).await;
}

pub async fn RouteS04_PG_FragmentBip(ctx: Ctx) {
    fragment_bip(&ctx, FLAG_SIL_FRAGMENT_5).await;
}

pub async fn Owlys_MarcheGris_PG_Fragment(ctx: Ctx) {
    fragment(&ctx, FLAG_SIL_FRAGMENT_6).await;
}

pub async fn Owlys_MarcheGris_PG_FragmentBip(ctx: Ctx) {
    fragment_bip(&ctx, FLAG_SIL_FRAGMENT_6).await;
}

pub async fn ReverseCity_Phare_PG_Fragment(ctx: Ctx) {
    fragment(&ctx, FLAG_SIL_FRAGMENT_7).await;
}

pub async fn ReverseCity_Phare_PG_FragmentBip(ctx: Ctx) {
    fragment_bip(&ctx, FLAG_SIL_FRAGMENT_7).await;
}

pub async fn Evalon_TourScribe_3F_PG_Fragment(ctx: Ctx) {
    fragment(&ctx, FLAG_SIL_FRAGMENT_8).await;
}

pub async fn Evalon_TourScribe_3F_PG_FragmentBip(ctx: Ctx) {
    fragment_bip(&ctx, FLAG_SIL_FRAGMENT_8).await;
}

// ---- the legendaries freed across the region (PG_Legendaire) ----

/// Celebi at the big root (Loïc's dream, level 70).
pub async fn RouteS01_EventScript_Celebi(ctx: Ctx) {
    legendaire(
        &ctx,
        PG_Legendaire_Celebi,
        SPECIES_CELEBI,
        70,
        LOCALID_ROUTE_S01_CELEBI,
    )
    .await;
}

/// Raikou in the Goulet's dead end (Tauber & Hocseat's, level 75).
pub async fn RouteS03_EventScript_Raikou(ctx: Ctx) {
    legendaire(
        &ctx,
        PG_Legendaire_Raikou,
        SPECIES_RAIKOU,
        75,
        LOCALID_ROUTE_S03_RAIKOU,
    )
    .await;
}

/// Lugia in the Coral Grotto's basin (Poussin's, level 78).
pub async fn RouteS05_GrotteCorail_EventScript_Lugia(ctx: Ctx) {
    legendaire(
        &ctx,
        PG_Legendaire_Lugia,
        SPECIES_LUGIA,
        78,
        LOCALID_GROTTE_CORAIL_LUGIA,
    )
    .await;
}

/// Kyogre at the Conduit's end, under water (Poussin's, level 80).
pub async fn RouteS09_Underwater_EventScript_Kyogre(ctx: Ctx) {
    legendaire(
        &ctx,
        PG_Legendaire_Kyogre,
        SPECIES_KYOGRE,
        80,
        LOCALID_ROUTE_S09_UW_KYOGRE,
    )
    .await;
}

/// Registeel in the Citadel's crypt (Loulpix & Doris's, level 75).
pub async fn Evalon_Citadelle_EventScript_Registeel(ctx: Ctx) {
    legendaire(
        &ctx,
        PG_Legendaire_Registeel,
        SPECIES_REGISTEEL,
        75,
        LOCALID_CITADELLE_REGISTEEL,
    )
    .await;
}

/// Latias by the fountain (Loulpix & Doris's, level 78).
pub async fn Evalon_EventScript_Latias(ctx: Ctx) {
    legendaire(
        &ctx,
        PG_Legendaire_Latias,
        SPECIES_LATIAS,
        78,
        LOCALID_EVALON_LATIAS,
    )
    .await;
}

// ---- Tak before the statue (Manga_Square_PG_Sc32a) ----

/// Tak before the lit Rayquaza statue (shown from 130 by `MangaSquare_OnTransition`).
pub async fn MangaSquare_PG_Tak(ctx: Ctx) {
    msgbox(&ctx, Manga_Square_PG_Sc32a_T1, MsgBox::Npc).await;
}
