//! Chapter 12 (the post-game) names on maps an earlier chapter's scripts claim: stubs.
//!
//! `contenu-jeu/chapitres/12-post-game.md`. The Ch12 maps put scenes, actors and events on
//! earlier chapters' maps (the plan's "SIL6 Ch12 maps as built"). Once a chapter's file claims
//! such a map (`pub const MAPS`), every script the map names must exist, so the Ch12 names live
//! here as stubs until the Ch12 scripts replace them. This file claims no map. Each stub does
//! nothing: every actor it would serve starts hidden (`game.ron`), and the coords and objects
//! that do fire before the post-game must stay silent. The plan's "SIL6 Ch12 maps: review" lists
//! which names each later chapter adds here when it claims its maps.

use crate::silice::prelude::*;

// ---- RouteS05_Epave (claimed by ch05_route_s05.rs) ----

/// Ch12: shows Arthur (`FLAG_SIL_HIDE_ARTHUR_EPAVE`) from `VAR_SIL_STORY` 123 until his Vestige
/// scene (Sc06) is done. (The stubs name no item flag: `silice check`'s `placement-map` reads
/// a flag named in a script as given.)
pub async fn RouteS05_Epave_OnTransition(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet: Arthur stays hidden.
}

/// Ch12 `Route_S05_PG_Sc06`, Arthur on the wreck (his object is hidden until then).
pub async fn RouteS05_Epave_PG_Sc06(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

// ---- Owlys_GrotteEst (claimed by ch05_grotte_est.rs) ----
// Ch12 also extends ch05's `Owlys_GrotteEst_OnTransition`: Silas (`FLAG_SIL_HIDE_SILAS_GROTTE`)
// shown from 123 until his Vestige scene (Sc07) is done.

/// Ch12 `Owlys_PG_Sc07`, Silas at the Point Zéro: the coord (33, 9) (`VAR_TEMP_1` 0, so it
/// fires on every step; the scene checks `VAR_SIL_STORY >= 123` and that Sc07 is not done).
pub async fn Owlys_GrotteEst_PG_Sc07(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet: the step does nothing.
}

/// Ch12: Silas's machine at the Point Zéro (Sc07's Clé de Débug, `PG_Lib_Silas`). Its object is
/// shown from the start, so talking to it before the post-game does nothing.
pub async fn Owlys_GrotteEst_EventScript_Machine(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12: Mew (`PG_Legendaire`; hidden until `SommetApex_PG_Sc19` clears `FLAG_SIL_HIDE_MEW`).
pub async fn Owlys_GrotteEst_EventScript_Mew(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

// ---- MangaSquare_Bibliotheque (claimed by ch06_dojo.rs) ----

/// Ch12: shows the Council (Adrien, Gwada, Kimiko, Mathieu, the two citizens) from
/// `VAR_SIL_STORY` 120, Gwada not at 127–129. Ch6 needs no transition script of its own here.
pub async fn MangaSquare_Bibliotheque_OnTransition(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet: the Council stays hidden.
}

/// Ch12 `Manga_Square_PG_Sc02`, the Council's call: row 22 at `VAR_SIL_STORY` 120.
pub async fn MangaSquare_Bibliotheque_PG_Sc02(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12 `Manga_Square_PG_Sc21`, the Council's Adrien (his object is hidden until then).
pub async fn MangaSquare_Bibliotheque_PG_Sc21(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12 `Manga_Square_PG_Sc30`, the Council's Gwada (hidden until then).
pub async fn MangaSquare_Bibliotheque_PG_Sc30(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12: the Council's Kimiko (hidden until then).
pub async fn MangaSquare_Bibliotheque_PG_Kimiko(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12: Mathieu at the Council's door (hidden until then).
pub async fn MangaSquare_Bibliotheque_PG_Mathieu(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12: the two citizens queuing for the Council (hidden until then).
pub async fn MangaSquare_Bibliotheque_PG_Citoyen(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

// ---- RouteS06_BoisSoupirs (claimed by ch06_route_s06.rs) ----

/// Ch12: shows Kimiko and her drone in the clearing from `VAR_SIL_STORY` 123 until
/// `FLAG_SIL_VESTIGE_KIMIKO`. Ch6 needs no transition script of its own here.
pub async fn RouteS06_BoisSoupirs_OnTransition(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet: the clearing's actors stay hidden.
}

/// Ch12 `Route_S06_PG_Sc05`, Kimiko's vestige: the coord (12, 19) at the secret path's end
/// (`VAR_TEMP_1` 0, so it fires on every step; the scene checks its own condition).
pub async fn RouteS06_BoisSoupirs_PG_Sc05(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet: the step does nothing.
}

/// Ch12 `Route_S06_PG_Lib`, the feather's stump (15, 21).
pub async fn RouteS06_BoisSoupirs_PG_Lib(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12: Kimiko's drone in the clearing (hidden until then).
pub async fn RouteS06_BoisSoupirs_EventScript_Drone(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12: Ho-Oh (`PG_Legendaire`; hidden until its scene clears `FLAG_SIL_HIDE_HO_OH`).
pub async fn RouteS06_BoisSoupirs_EventScript_HoOh(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

// ---- ReverseCity_Phare (claimed by ch07_phare.rs) ----
// Ch7's `ReverseCity_Phare_OnTransition` already shows Thomas (`FLAG_SIL_HIDE_THOMAS_PHARE`) at
// `VAR_SIL_STORY` 122 only.

/// Ch12 `Reverse_City_PG_Sc04`, Thomas's revenge at the top of the stairs: the coord (12, 8)
/// at `VAR_SIL_STORY` 122.
pub async fn ReverseCity_Phare_PG_Sc04(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet: the step does nothing.
}

/// Ch12: Thomas at the top (hidden except at `VAR_SIL_STORY` 122).
pub async fn ReverseCity_Phare_PG_Thomas(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

// ---- TourNebula_2F (claimed by ch10_studio.rs) ----

/// Ch12: Gwada at the Studio's post-game console (hidden until then).
pub async fn TourNebula_2F_PG_Gwada(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12: the Studio's post-game PC (the bg (PG)): silent before.
pub async fn TourNebula_2F_PG_PC(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

// ---- SommetApex (claimed by ch11_apex.rs) ----
// Ch11's `SommetApex_OnTransition` already clears `FLAG_SIL_HIDE_APEX_SOCLE` from 120.

/// Ch12 `Sommet_Apex_PG_Sc16c`, Poussin's daily rematch on his stage (hidden until then).
pub async fn SommetApex_PG_Sc16c(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12 `Sommet_Apex_PG_Sc19`, the Purity Beacon: row 16 at `VAR_SIL_STORY` 129.
pub async fn SommetApex_PG_Sc19(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet: the step does nothing.
}

/// Ch12 `Sommet_Apex_PG_Sc32`, Rayquaza: the socle's four neighbours (`VAR_TEMP_1` 0, so it
/// fires on every step; the scene checks `VAR_SIL_STORY >= 130` and the capture flag).
pub async fn SommetApex_PG_Sc32(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet: the step does nothing.
}

/// Ch12: the empty socle (hidden until 120).
pub async fn SommetApex_EventScript_Socle(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}

/// Ch12: Rayquaza on the stage (hidden outside `SommetApex_PG_Sc32`).
pub async fn SommetApex_EventScript_Rayquaza(_ctx: Ctx) {
    // fallback: Ch12 post-game scripts not written yet.
}
