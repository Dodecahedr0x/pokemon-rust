//! Chapter 9, the Prise d'Eau: `contenu-jeu/chapitres/09-nebula.md` §2.1–2.3 (Route S9, the
//! Station de Pompage, the conduit), §3 Route_S09_Sc01 (the signal comes back), Sc02 (Julien's
//! call), Sc03 (the pumps) and Sc04 (the dive), §5 (the six route trainers) and §7 (the items).
//! `VAR_SIL_STORY` 86 (Chapter 8's end) → 90 (Benou) → 91 (Julien) → 92 (pumps A and B off).
//! Nébula is `ch09_nebula.rs`, the Tower's B1F and hall `ch09_tour.rs`.
//!
//! The lake is crossed with the Hydro-Propulseur and the conduit reached with the Sonde de
//! Pression: the engine's Surf and Dive, gated by the module flags (`crate::silice::Game::
//! field_move_gate`, `scripts/mallette.rs`). P2 dives through the map's Dive connection into the
//! conduit (`RouteS09_Underwater`), whose south exit is a south arrow warp onto Nébula's quai.

use super::ch04_port::{Puzzle, push_object};
use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["RouteS09", "RouteS09_Station", "RouteS09_Underwater"];

// ---- Route S9: the security cables ----

/// Slateport's concrete, the station pontoon's floor (the cables' cells once cut).
const METATILE_BETON: u16 = 514;

/// The security cables (47, 18–19) before the station pontoon (Lame-Fréquence, the module
/// obstacles of §2.1): Chapter 2's cable (`ch02_route_s02::cable`), its texts and module.
const CABLES: Obstacle = Obstacle {
    module: FLAG_SIL_MODULE_COUPE,
    done: FLAG_SIL_S09_CABLES,
    paint: &[
        (47, 18, METATILE_BETON, false),
        (47, 19, METATILE_BETON, false),
    ],
    blocked: Route_S02_Cable_T1,
    ask: Some(Route_S02_Cable_T1),
    cleared: Route_S02_Cable_T2,
};

pub async fn RouteS09_EventScript_Cables(ctx: Ctx) {
    module_obstacle(&ctx, &CABLES).await;
}

// ---- Route S9: the containers (Champ Magnétique) ----

/// The two containers in the gap (4–5, 35) above the nook (2–6, 36–41) that hides the Max
/// Repel: Chapter 4's push (`ch04_port::push_object`, `FLAG_SIL_MODULE_FORCE`). Their cells are
/// temp vars, so they are home again on every load.
pub(crate) const CONTENEURS: Puzzle = Puzzle {
    map: MAP_ROUTE_S09,
    objects: &[
    (LOCALID_ROUTE_S09_CONTENEUR_1, VAR_TEMP_1, (4, 35)),
    (LOCALID_ROUTE_S09_CONTENEUR_2, VAR_TEMP_2, (5, 35)),
],
    free: conteneur_free,
};

/// Where a container may be pushed: any walkable cell of the berge (x 2–13, y 21–41), the gap
/// and the nook included. Softlock review: with the berge open, two containers can never seal
/// the player in the nook (a container in a nook exit can always be pushed up through the gap
/// or aside), which a nook-only rule allowed (both at (4–5, 36)).
fn conteneur_free(_: (i16, i16), (x, y): (i16, i16)) -> bool {
    let solid = matches!(
        (x, y),
        (2 | 3 | 6 | 7, 35) | (7, 36..=41) | (10, 34) | (6, 24)
    );
    (2..=13).contains(&x) && (21..=41).contains(&y) && !solid
}

pub async fn RouteS09_EventScript_Conteneur(ctx: Ctx) {
    push_object(&ctx).await;
}

// ---- Route S9: map scripts and scenes ----

/// The cables stay cut.
pub async fn RouteS09_OnLoad(ctx: Ctx) {
    repaint_obstacle(&ctx, &CABLES).await;
}

/// Route_S09_Sc01, the signal comes back: the coords (4, 4–5), four cells past Évalon's exit,
/// at `VAR_SIL_STORY` 86 (Chapter 8's end). Benou's call; sets 90.
pub async fn RouteS09_Sc01(ctx: Ctx) {
    if story(&ctx).await != 86 {
        return;
    }
    ctx.lock_all().await;
    play_music(&ctx, "MUS_ABNORMAL_WEATHER").await;
    // `screen_flash()`: the source's white flash.
    screen_flash(&ctx, WHITE).await;
    say(&ctx, Route_S09_Sc01_T1).await;
    say(&ctx, Route_S09_Sc01_T2).await;
    mallette_call(&ctx, "BENOU").await;
    say(&ctx, Route_S09_Sc01_T3).await;
    say(&ctx, Route_S09_Sc01_T4).await;
    say(&ctx, Route_S09_Sc01_T5).await;
    set_story(&ctx, 90).await;
    ctx.release_all().await;
}

/// Route_S09_Sc02, Julien's call: the coord row (2–15, 14) across the berge, before the
/// pontoon, at `VAR_SIL_STORY` 90. Sets 91.
pub async fn RouteS09_Sc02(ctx: Ctx) {
    if story(&ctx).await != 90 {
        return;
    }
    ctx.lock_all().await;
    mallette_call(&ctx, "JULIEN").await;
    say(&ctx, Route_S09_Sc02_T1).await;
    say(&ctx, Route_S09_Sc02_T2).await;
    say(&ctx, Route_S09_Sc02_T3).await;
    end_call(&ctx).await;
    say(&ctx, Route_S09_Sc02_T4).await;
    set_story(&ctx, 91).await;
    ctx.release_all().await;
}

// ---- Route S9: trainers ----

/// The fisherman on the pontoon (19, 22), sight 3 left. The pontoon is two rows deep (y 22–23):
/// his approach never closes it. After the battle, his story of the lake (`Route_S09_Npc_2`).
pub async fn RouteS09_EventScript_Anselme(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S09_ANSELME,
        TRAINER_SIL_S09_ANSELME,
        Route_S09_Anselme_Intro,
        Route_S09_Anselme_Defaite,
    )
    .await
    {
        post_battle(&ctx, Route_S09_Npc_2).await;
    }
}

/// On the lake (27, 17), sight 4 down.
pub async fn RouteS09_EventScript_Marin(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S09_MARIN,
        TRAINER_SIL_S09_MARIN,
        Route_S09_Marin_Intro,
        Route_S09_Marin_Defaite,
    )
    .await
    {
        post_battle(&ctx, Route_S09_Marin_Defaite).await;
    }
}

/// On the lake (41, 13), sight 4 left.
pub async fn RouteS09_EventScript_Oceane(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S09_OCEANE,
        TRAINER_SIL_S09_OCEANE,
        Route_S09_Oceane_Intro,
        Route_S09_Oceane_Defaite,
    )
    .await
    {
        post_battle(&ctx, Route_S09_Oceane_Defaite).await;
    }
}

/// The Cocket diver N°7 at his buoy (26, 31), sight 4 right.
pub async fn RouteS09_EventScript_Plongeur7(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S09_PLONGEUR_7,
        TRAINER_SIL_S09_PLONGEUR_7,
        Route_S09_Plongeur7_Intro,
        Route_S09_Plongeur7_Defaite,
    )
    .await
    {
        post_battle(&ctx, Route_S09_Plongeur7_Defaite).await;
    }
}

/// The Cocket diver N°12 (44, 33), sight 4 down, over P2's east side.
pub async fn RouteS09_EventScript_Plongeuse12(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S09_PLONGEUSE_12,
        TRAINER_SIL_S09_PLONGEUSE_12,
        Route_S09_Plongeuse12_Intro,
        Route_S09_Plongeuse12_Defaite,
    )
    .await
    {
        post_battle(&ctx, Route_S09_Plongeuse12_Defaite).await;
    }
}

// ---- Route S9: people, signs, items ----

/// The refugee on the shepherd's path.
pub async fn RouteS09_EventScript_Refugie(ctx: Ctx) {
    msgbox(&ctx, Route_S09_Npc_1, MsgBox::Npc).await;
}

/// The technician on her break, before the station.
pub async fn RouteS09_EventScript_Technicienne(ctx: Ctx) {
    msgbox(&ctx, Route_S09_Npc_3, MsgBox::Npc).await;
}

pub async fn RouteS09_EventScript_Sign1(ctx: Ctx) {
    msgbox(&ctx, Route_S09_Sign_1, MsgBox::Sign).await;
}

pub async fn RouteS09_EventScript_Sign2(ctx: Ctx) {
    msgbox(&ctx, Route_S09_Sign_2, MsgBox::Sign).await;
}

pub async fn RouteS09_EventScript_Sign3(ctx: Ctx) {
    msgbox(&ctx, Route_S09_Sign_3, MsgBox::Sign).await;
}

/// The shepherd's path.
pub async fn RouteS09_EventScript_ItemUltraBall(ctx: Ctx) {
    find_item(&ctx, ITEM_ULTRA_BALL, 1).await;
}

/// The nook behind the containers.
pub async fn RouteS09_EventScript_ItemConteneurs(ctx: Ctx) {
    find_item(&ctx, ITEM_MAX_REPEL, 1).await;
}

// ---- the Station de Pompage ----

/// The breaker's cabinet (2, 2), in the glass office: Chapter 4's push, along the office's back
/// row (y 2, x 1–8) only, so it can never close the office door (4, 6). Pushed east from (1, 2),
/// it uncovers the breaker (2, 1), read from (2, 2).
pub(crate) const ARMOIRE: Puzzle = Puzzle {
    map: MAP_ROUTE_S09_STATION,
    objects: &[(LOCALID_S09_STATION_ARMOIRE, VAR_TEMP_1, (2, 2))],
    free: armoire_free,
};

fn armoire_free(_: (i16, i16), (x, y): (i16, i16)) -> bool {
    y == 2 && (1..=8).contains(&x)
}

pub async fn RouteS09_Station_EventScript_Armoire(ctx: Ctx) {
    push_object(&ctx).await;
}

/// The breaker B behind the cabinet: `Route_S09_Sc03_T5` and `FLAG_SIL_S09_DISJONCTEUR`, once.
pub async fn RouteS09_Station_EventScript_Disjoncteur(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_S09_DISJONCTEUR).await {
        return;
    }
    ctx.lock().await;
    play_se(&ctx, "SE_SWITCH").await;
    say(&ctx, Route_S09_Sc03_T5).await;
    ctx.set_flag(FLAG_SIL_S09_DISJONCTEUR, true).await;
    ctx.release().await;
}

/// A pump turned off: `Route_S09_Sc03_T2`, and once both are off `_T3` and `VAR_SIL_STORY` 92.
///
/// fallback: the source's red light (`set_metatile(PUPITRE, VOYANT_ROUGE)`): the Facility
/// tileset has no lit-console metatile, so the console is left as drawn.
async fn pompe_off(ctx: &Ctx, flag: FlagId) {
    ctx.set_flag(flag, true).await;
    play_se(ctx, "SE_PIN").await;
    say(ctx, Route_S09_Sc03_T2).await;
    if ctx.flag(FLAG_SIL_S09_POMPE_A).await && ctx.flag(FLAG_SIL_S09_POMPE_B).await {
        say(ctx, Route_S09_Sc03_T3).await;
        if story(ctx).await < 92 {
            set_story(ctx, 92).await;
        }
    }
}

/// Pump A's console (3, 10): free; the question, then off.
pub async fn RouteS09_Station_EventScript_PupitreA(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_S09_POMPE_A).await {
        return;
    }
    ctx.lock().await;
    if ask(&ctx, Route_S09_Sc03_T1).await {
        pompe_off(&ctx, FLAG_SIL_S09_POMPE_A).await;
    }
    ctx.release().await;
}

/// Pump B's console (11, 10), which Gérard guards: locked (`Route_S09_Sc03_T4`) until the
/// breaker is reset, then off at once. The source writes no question for B (its `Sc03_T1`
/// names pump A): an author question; no question is asked.
pub async fn RouteS09_Station_EventScript_PupitreB(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_S09_POMPE_B).await {
        return;
    }
    if !ctx.flag(FLAG_SIL_S09_DISJONCTEUR).await {
        msgbox(&ctx, Route_S09_Sc03_T4, MsgBox::Sign).await;
        return;
    }
    ctx.lock().await;
    pompe_off(&ctx, FLAG_SIL_S09_POMPE_B).await;
    ctx.release().await;
}

/// Pump C's console (17, 10): the warning; it never stops.
pub async fn RouteS09_Station_EventScript_PompeC(ctx: Ctx) {
    msgbox(&ctx, Route_S09_Station_PompeC, MsgBox::Sign).await;
}

const ECART_1: &[Step] = &[walk(Dir::Right), face(Dir::Left)];
const ECART_2: &[Step] = &[walk(Dir::Right), walk(Dir::Right), face(Dir::Left)];

/// Gérard (13, 11), sight 3 left over pump B's console row; after the battle, Berthe's story.
/// "Après le combat, il s'écarte": his approach along row 11 can end on (11, 11), the only cell
/// pump B's console (11, 10) is read from (a softlock until the map is left), so after the
/// battle he walks back to his post (13, 11). He stands next to the player, who is on row 11.
pub async fn RouteS09_Station_EventScript_Technicien(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_S09_STATION_TECHNICIEN,
        TRAINER_SIL_S09_TECHNICIEN,
        Route_S09_Technicien_Intro,
        Route_S09_Technicien_Defaite,
    )
    .await
    {
        post_battle(&ctx, Route_S09_Technicien_Apres).await;
        return;
    }
    if !defeated(&ctx, TRAINER_SIL_S09_TECHNICIEN).await {
        return;
    }
    let steps = match ctx.player_pos().await {
        (10, 11) => ECART_2,
        (11, 11) => ECART_1,
        _ => return,
    };
    move_wait(&ctx, LOCALID_S09_STATION_TECHNICIEN, steps).await;
    // His approach's cell is kept for his respawns (out of view and back): his post's instead.
    ctx.copy_object_pos_to_template(LOCALID_S09_STATION_TECHNICIEN)
        .await;
}

/// The cleaner.
pub async fn RouteS09_Station_EventScript_Agent(ctx: Ctx) {
    msgbox(&ctx, Route_S09_Station_Npc_1, MsgBox::Npc).await;
}

/// The glass office's drawer.
pub async fn RouteS09_Station_EventScript_ItemTM24(ctx: Ctx) {
    find_item(&ctx, ITEM_TM24, 1).await;
}

/// Pump C's catwalk.
pub async fn RouteS09_Station_EventScript_ItemElixir(ctx: Ctx) {
    find_item(&ctx, ITEM_ELIXIR, 1).await;
}

/// Nothing to set up: the consoles read their flags.
pub async fn RouteS09_Station_OnLoad(_ctx: Ctx) {}

// ---- the conduit (underwater) ----

/// The conduit's floor (`MB_NO_SURFACING`), the currents' cells once their pump is off.
const METATILE_FOND: u16 = 681;
/// Current A (32–37, 48–50), pump A's; its coord row is (32–37, 47), on `VAR_TEMP_2` 0.
const COURANT_A: (core::ops::RangeInclusive<i16>, core::ops::RangeInclusive<i16>) = (32..=37, 48..=50);
/// Current B (42–47, 61–63), pump B's; its coord row is (42–47, 60), on `VAR_TEMP_3` 0.
const COURANT_B: (core::ops::RangeInclusive<i16>, core::ops::RangeInclusive<i16>) = (42..=47, 61..=63);

async fn calm(ctx: &Ctx, (xs, ys): (core::ops::RangeInclusive<i16>, core::ops::RangeInclusive<i16>)) {
    for y in ys {
        for x in xs.clone() {
            ctx.set_metatile(x, y, METATILE_FOND, false).await;
        }
    }
}

/// §9 "Courants sous-marins conditionnels": a pump turned off calms its current on the next
/// load (its cells become the conduit's floor) and silences its coord row (`VAR_TEMP_2`/`_3`).
pub async fn RouteS09_Underwater_OnLoad(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_S09_POMPE_A).await {
        calm(&ctx, COURANT_A).await;
        ctx.set_var(VAR_TEMP_2, 1).await;
    }
    if ctx.flag(FLAG_SIL_S09_POMPE_B).await {
        calm(&ctx, COURANT_B).await;
        ctx.set_var(VAR_TEMP_3, 1).await;
    }
}

const BACK_UP: &[Step] = &[walk(Dir::Up), walk(Dir::Up)];

/// A live current (its pump still on): `Route_S09_UW_Courant`, and the suction carries the player
/// two cells back north (the current's own cells are solid until the pump is off).
pub async fn RouteS09_Underwater_EventScript_Courant(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Route_S09_UW_Courant).await;
    move_wait(&ctx, PLAYER, BACK_UP).await;
    ctx.release_all().await;
}

/// Route_S09_Sc04's text, the first time the player is in the conduit (the on-frame script on
/// `VAR_TEMP_1` 0, after the Dive's own warp): `FLAG_SIL_S09_PREMIERE_PLONGEE`.
pub async fn RouteS09_Underwater_Sc04(ctx: Ctx) {
    ctx.set_var(VAR_TEMP_1, 1).await;
    if ctx.flag(FLAG_SIL_S09_PREMIERE_PLONGEE).await {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Route_S09_Sc04_T1).await;
    ctx.set_flag(FLAG_SIL_S09_PREMIERE_PLONGEE, true).await;
    ctx.release_all().await;
}

/// A porthole on an empty room.
pub async fn RouteS09_Underwater_EventScript_Hublot(ctx: Ctx) {
    msgbox(&ctx, Route_S09_UW_Hublot, MsgBox::Sign).await;
}

/// The Coude des Courants, past pump B's current: the puzzle's reward.
pub async fn RouteS09_Underwater_EventScript_ItemMaxRevive(ctx: Ctx) {
    find_item(&ctx, ITEM_MAX_REVIVE, 1).await;
}

