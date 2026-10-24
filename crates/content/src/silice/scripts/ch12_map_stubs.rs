//! Chapter 12 (the post-game) optional maps: stubs for the names they add.
//!
//! `contenu-jeu/chapitres/12-post-game.md`. The Ch12 optional maps (Crow's Lounge, the Île de
//! Débug, the Grotte de la Stèle, the three Antennes, the Temple des Nuages) and their entrances,
//! the eight Fragments de Code, the roaming legendaries' objects, Tak and Brico-Bourg's final
//! scenes name these scripts. Each stub does nothing until the Ch12 scripts replace it: every
//! actor it would serve starts hidden (`game.ron`), and the coords that fire before the
//! post-game must stay silent. This file claims no map.

use crate::silice::prelude::*;

// ---- BricoBourg_Bar / BricoBourg_Bar_Lounge ----

/// Ch12: redraws the Shaker's bookcase open (the stairs at (12, 1), the wall top at (12, 0))
/// once `Brico_Bourg_Bar_PG_Sc22` has slid it.
pub async fn BricoBourg_Bar_OnLoad(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet: the bookcase stays shut.
}

/// Ch12: Crow behind the Lounge's counter (`PG_Crow`).
pub async fn BricoBourg_Bar_Lounge_PG_Crow(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

// ---- RouteS05 / RouteS05_IleDebug ----

/// Ch12 `Route_S05_PG_Sc27`: the coord on the open water east of the wreck that finds the Île
/// de Débug (`VAR_TEMP_1` 0, so it fires on every step; the scene checks its own conditions).
pub async fn RouteS05_PG_Sc27(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet: the step does nothing.
}

/// Ch12: Deoxys at the heart of the Île de Débug (`PG_Legendaire`).
pub async fn RouteS05_IleDebug_EventScript_Deoxys(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

// ---- RouteS08_GrotteStele ----

/// Ch12 `Route_S08_PG_Sc28`: the stele at the far end of the Grotte de la Stèle.
pub async fn RouteS08_GrotteStele_PG_Sc28(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

// ---- The three Antennes ----

/// Ch12 `PG_Antenne`: the receptionist of the Antenne on Route S02.
pub async fn RouteS02_Antenne_PG_Receptionniste(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12: the Antenne's healing machine on Route S02.
pub async fn RouteS02_Antenne_PG_Soin(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12 `PG_Antenne`: the receptionist of the Antenne on Route S03.
pub async fn RouteS03_Antenne_PG_Receptionniste(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12: the Antenne's healing machine on Route S03.
pub async fn RouteS03_Antenne_PG_Soin(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12 `PG_Antenne`: the receptionist of the Antenne on Route S07.
pub async fn RouteS07_Antenne_PG_Receptionniste(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12: the Antenne's healing machine on Route S07.
pub async fn RouteS07_Antenne_PG_Soin(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

// ---- SommetApex_TempleNuages ----

/// Ch12 `Sommet_Apex_PG_Sc31`: Jirachi on the Temple des Nuages' altar.
pub async fn SommetApex_TempleNuages_PG_Sc31(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

// ---- The eight Fragments de Code (`PG_Fragment`) ----
// Each fragment is a sign; its two "BIP" coords (`VAR_TEMP_1` 0) fire on every step and the
// scene checks its own conditions.

/// Ch12 `PG_Fragment`: Fragment de Code 1, at the big root's foot.
pub async fn RouteS01_PG_Fragment(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12 `PG_Fragment`: the "BIP" two cells before Fragment de Code 1.
pub async fn RouteS01_PG_FragmentBip(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet: the step does nothing.
}

/// Ch12 `PG_Fragment`: Fragment de Code 2, in the planter behind the Herboriste.
pub async fn BoisEther_PG_Fragment(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12 `PG_Fragment`: the "BIP" two cells before Fragment de Code 2.
pub async fn BoisEther_PG_FragmentBip(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet: the step does nothing.
}

/// Ch12 `PG_Fragment`: Fragment de Code 3, at the top of the observation tower.
pub async fn GrisEchec_TourObservation_Top_PG_Fragment(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12 `PG_Fragment`: the "BIP" two cells before Fragment de Code 3.
pub async fn GrisEchec_TourObservation_Top_PG_FragmentBip(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet: the step does nothing.
}

/// Ch12 `PG_Fragment`: Fragment de Code 4, under the Place Stroboscope's neon console.
pub async fn ViberCity_PG_Fragment(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12 `PG_Fragment`: the "BIP" two cells before Fragment de Code 4.
pub async fn ViberCity_PG_FragmentBip(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet: the step does nothing.
}

/// Ch12 `PG_Fragment`: Fragment de Code 5, in a pane of the glass gallery.
pub async fn RouteS04_PG_Fragment(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12 `PG_Fragment`: the "BIP" two cells before Fragment de Code 5.
pub async fn RouteS04_PG_FragmentBip(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet: the step does nothing.
}

/// Ch12 `PG_Fragment`: Fragment de Code 6, under the empty stall of the Marché Gris.
pub async fn Owlys_MarcheGris_PG_Fragment(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12 `PG_Fragment`: the "BIP" two cells before Fragment de Code 6.
pub async fn Owlys_MarcheGris_PG_FragmentBip(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet: the step does nothing.
}

/// Ch12 `PG_Fragment`: Fragment de Code 7, behind the lighthouse's stairs.
pub async fn ReverseCity_Phare_PG_Fragment(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12 `PG_Fragment`: the "BIP" two cells before Fragment de Code 7.
pub async fn ReverseCity_Phare_PG_FragmentBip(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet: the step does nothing.
}

/// Ch12 `PG_Fragment`: Fragment de Code 8, between two grimoires on the Tour du Scribe's top
/// floor.
pub async fn Evalon_TourScribe_3F_PG_Fragment(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12 `PG_Fragment`: the "BIP" two cells before Fragment de Code 8.
pub async fn Evalon_TourScribe_3F_PG_FragmentBip(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet: the step does nothing.
}

// ---- The legendaries (`PG_Legendaire`) ----

/// Ch12: Celebi at the big root's foot on Route S01.
pub async fn RouteS01_EventScript_Celebi(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12: Raikou in Route S03's north-east nook.
pub async fn RouteS03_EventScript_Raikou(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12: Lugia in the Grotte de Corail's inner basin.
pub async fn RouteS05_GrotteCorail_EventScript_Lugia(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12: Kyogre at the far end of the Conduite, under water.
pub async fn RouteS09_Underwater_EventScript_Kyogre(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12: Registeel in the Citadelle's Crypte.
pub async fn Evalon_Citadelle_EventScript_Registeel(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12: Latias by Evalon's fountain.
pub async fn Evalon_EventScript_Latias(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

// ---- MangaSquare ----

/// Ch12 `Manga_Square_PG_Sc32a`: Tak before the Wishing Dragon statue.
pub async fn MangaSquare_PG_Tak(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

// ---- BricoBourg ----

/// Ch12 `Brico_Bourg_PG_Sc01`: the frame script at `VAR_SIL_STORY` 119.
pub async fn BricoBourg_PG_Sc01(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12 `Brico_Bourg_PG_Sc20`: Maman at the house's door (the coords round its front cell at
/// `VAR_SIL_STORY` 130).
pub async fn BricoBourg_PG_Sc20(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet: the step does nothing.
}
