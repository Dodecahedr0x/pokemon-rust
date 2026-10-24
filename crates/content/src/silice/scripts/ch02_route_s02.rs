//! Chapter 2, Route S2, the Corridor des Câbles (`RouteS02`):
//! `contenu-jeu/chapitres/02-gris-echec.md` §2 (signs, ambient NPCs), §3 Route_S02_Sc01 to Sc04,
//! §5 (the six trainers) and §7 (the items). `VAR_SIL_STORY` runs 17 (the Bêta barrier open,
//! the end of Chapter 1) to 22 (Gwada fought); Gris-Échec is east.

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["RouteS02"];

// ---- map scripts ----

/// Adrien at the cliff and Gwada by the exit ("SIL2 Ch2–Ch3 maps as built", hide flags): Gwada
/// at `VAR_SIL_STORY` 17–21; Adrien at 17–21, then while his Revive is not given (the source
/// keeps him available when Sc03 ran at 20). Sc01 runs at 17 on this map and sets 20 without a
/// reload, so both must already show at 17.
pub async fn RouteS02_OnTransition(ctx: Ctx) {
    let s = story(&ctx).await;
    let adrien_given = ctx.flag(FLAG_SIL_ITEM_ADRIEN_CH2).await;
    ctx.set_flag(FLAG_SIL_HIDE_ROUTE_S02_GWADA, !(17..=21).contains(&s))
        .await;
    let adrien = s >= 17 && (s <= 21 || !adrien_given);
    ctx.set_flag(FLAG_SIL_HIDE_ROUTE_S02_ADRIEN, !adrien).await;
}

/// The cut cables and the moved server block, redrawn on every load.
pub async fn RouteS02_OnLoad(ctx: Ctx) {
    for o in [&CABLE_1, &CABLE_2, &CABLE_3, &SERVEUR] {
        repaint_obstacle(&ctx, o).await;
    }
}

// ---- scenes ----

/// Route_S02_Sc01, the containment cables: the west entrance (x 1, y 2–5) at `VAR_SIL_STORY`
/// 17. Sets 20.
pub async fn RouteS02_Sc01(ctx: Ctx) {
    ctx.lock().await;
    say(&ctx, Route_S02_Sc01_T1).await;
    set_story(&ctx, 20).await;
    ctx.release().await;
}

/// Route_S02_Sc02, Adrien at the cliff (24, 14): keyed on his Revive not given yet
/// (`FLAG_SIL_ITEM_ADRIEN_CH2`) rather than on `VAR_SIL_STORY` 20, since he stays when Sc03
/// ran first. Sets 21 only from 20. Talked to again, his warning (T5).
pub async fn RouteS02_Sc02(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_ITEM_ADRIEN_CH2).await {
        msgbox(&ctx, Route_S02_Sc02_T5, MsgBox::Npc).await;
        return;
    }
    ctx.lock().await;
    // He looks at the sea first and does not turn round.
    say(&ctx, Route_S02_Sc02_T1).await;
    ctx.face_player().await;
    say(&ctx, Route_S02_Sc02_T2).await;
    say(&ctx, Route_S02_Sc02_T3).await;
    // T4 announces the Revive.
    gift(
        &ctx,
        FLAG_SIL_ITEM_ADRIEN_CH2,
        ITEM_REVIVE,
        1,
        Obtain::Quiet,
    )
    .await;
    say(&ctx, Route_S02_Sc02_T4).await;
    say(&ctx, Route_S02_Sc02_T5).await;
    if story(&ctx).await == 20 {
        set_story(&ctx, 21).await;
    }
    ctx.release().await;
}

/// Gwada's ways to the player, re-derived for the built route: from (50, 6) to the cell beside
/// the player (the source's `Walk(Left) x2` runs into a player on (48–49, 6)).
const GWADA_TO_LEFT: &[Step] = &[walk(Dir::Left), Step::FacePlayer];
const GWADA_TO_RIGHT: &[Step] = &[walk(Dir::Right), Step::FacePlayer];
const GWADA_FACE: &[Step] = &[Step::FacePlayer];
const GWADA_ALERT: &[Step] = &[Step::Emote(Emote::Exclamation)];
/// Gwada off to Gris-Échec: up to row 5, then east along it to the exit (the source's
/// `Walk(Right) x6` crosses the trees at (52–55, 6)), from x 49, 50 or 51.
const GWADA_OUT_49: &[Step] = &[
    walk(Dir::Up),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
];
const GWADA_OUT_50: &[Step] = &[
    walk(Dir::Up),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
];
const GWADA_OUT_ROUND: &[Step] = &[
    walk(Dir::Right),
    walk(Dir::Up),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
];
const GWADA_OUT_51: &[Step] = &[
    walk(Dir::Up),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
];

/// Route_S02_Sc03, Gwada and his overcharged Wattouat: rows 7 (`VAR_SIL_STORY` 20, Adrien
/// skipped) and 6 (21) of x 48–51, or talking to Gwada. The battle, Gwada's heal, then he runs
/// ahead to Gris-Échec. Sets 22.
pub async fn RouteS02_Sc03(ctx: Ctx) {
    if !matches!(story(&ctx).await, 20 | 21) {
        return;
    }
    ctx.lock_all().await;
    // The source's emote(Exclamation) (§9): the engine has emotes.
    move_wait(&ctx, LOCALID_ROUTE_S02_GWADA, GWADA_ALERT).await;
    let (to_player, out) = match ctx.player_pos().await {
        (49, 7) | (48, 6) => (GWADA_TO_LEFT, GWADA_OUT_49),
        (51, 7) => (GWADA_TO_RIGHT, GWADA_OUT_51),
        // Talked to from row 5 (above him): he leaves round the player.
        (_, 5) => (GWADA_FACE, GWADA_OUT_ROUND),
        _ => (GWADA_FACE, GWADA_OUT_50),
    };
    move_wait(&ctx, LOCALID_ROUTE_S02_GWADA, to_player).await;
    say(&ctx, Route_S02_Sc03_T1).await;
    screen_flash(&ctx, YELLOW).await;
    say(&ctx, Route_S02_Sc03_T2).await;
    battle_trainer(&ctx, TRAINER_SIL_GWADA_1, Route_S02_Gwada_Defeat).await;
    say(&ctx, Route_S02_Sc03_T3).await;
    say(&ctx, Route_S02_Sc03_T4).await;
    move_wait(&ctx, LOCALID_ROUTE_S02_GWADA, out).await;
    hide(&ctx, LOCALID_ROUTE_S02_GWADA).await;
    // Gwada shares his kit (no walk back for a heal).
    ctx.heal_party().await;
    say(&ctx, Route_S02_Sc03_T5).await;
    set_story(&ctx, 22).await;
    ctx.release_all().await;
}

/// Gwada talked to before his scene fired (from row 5): the same scene.
pub async fn RouteS02_EventScript_Gwada(ctx: Ctx) {
    RouteS02_Sc03(ctx).await;
}

// ---- trainers (§5; sight from the map's trainer objects) ----

/// CAMPEUR HUGO (12, 2).
pub async fn RouteS02_EventScript_Hugo(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S02_HUGO,
        TRAINER_SIL_CAMPEUR_HUGO,
        Route_S02_Hugo_Intro,
        Route_S02_Hugo_Defeat,
    )
    .await
    {
        post_battle(&ctx, Route_S02_Hugo_Defeat).await;
    }
}

/// PÊCHEUR RENÉ (6, 8).
pub async fn RouteS02_EventScript_Rene(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S02_RENE,
        TRAINER_SIL_PECHEUR_RENE,
        Route_S02_Rene_Intro,
        Route_S02_Rene_Defeat,
    )
    .await
    {
        post_battle(&ctx, Route_S02_Rene_Defeat).await;
    }
}

/// FILLETTE ZOÉ (30, 10).
pub async fn RouteS02_EventScript_Zoe(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S02_ZOE,
        TRAINER_SIL_FILLETTE_ZOE,
        Route_S02_Zoe_Intro,
        Route_S02_Zoe_Defeat,
    )
    .await
    {
        post_battle(&ctx, Route_S02_Zoe_Defeat).await;
    }
}

/// MONTAGNARD GÉRARD (24, 4).
pub async fn RouteS02_EventScript_Gerard(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S02_GERARD,
        TRAINER_SIL_MONTAGNARD_GERARD,
        Route_S02_Gerard_Intro,
        Route_S02_Gerard_Defeat,
    )
    .await
    {
        post_battle(&ctx, Route_S02_Gerard_Defeat).await;
    }
}

/// The SBIRE COCKET in front of a cable (42, 8).
pub async fn RouteS02_EventScript_Sbire(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S02_SBIRE,
        TRAINER_SIL_SBIRE_S02,
        Route_S02_Sbire_Intro,
        Route_S02_Sbire_Defeat,
    )
    .await
    {
        post_battle(&ctx, Route_S02_Sbire_Defeat).await;
    }
}

/// KINÉSISTE ANAÏS (44, 4).
pub async fn RouteS02_EventScript_Anais(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S02_ANAIS,
        TRAINER_SIL_KINESISTE_ANAIS,
        Route_S02_Anais_Intro,
        Route_S02_Anais_Defeat,
    )
    .await
    {
        post_battle(&ctx, Route_S02_Anais_Defeat).await;
    }
}

// ---- ambient NPCs ----

/// The technician (34, 6): the TM Shock Wave, once (`FLAG_SIL_ITEM_ROUTE_S02_TM_SHOCK_WAVE`;
/// his line announces it). Afterwards he only turns to the player: the source has no second
/// line.
pub async fn RouteS02_EventScript_Technicien(ctx: Ctx) {
    gift_npc(
        &ctx,
        FLAG_SIL_ITEM_ROUTE_S02_TM_SHOCK_WAVE,
        ITEM_TM34,
        1,
        Route_S02_Technicien,
    )
    .await;
}

/// The maintenance agent (16, 10): three Paralyze Heals, once
/// (`FLAG_SIL_ITEM_ROUTE_S02_PARALYZE_HEAL`; his line announces them).
pub async fn RouteS02_EventScript_Agent(ctx: Ctx) {
    gift_npc(
        &ctx,
        FLAG_SIL_ITEM_ROUTE_S02_PARALYZE_HEAL,
        ITEM_PARALYZE_HEAL,
        3,
        Route_S02_Agent,
    )
    .await;
}

/// An NPC whose single line gives an item ("{PLAYER} obtient …"): the line and the item once,
/// then only a turn to the player.
async fn gift_npc(ctx: &Ctx, flag: FlagId, item: u16, qty: u16, line: TextId) {
    ctx.lock().await;
    ctx.face_player().await;
    if !ctx.flag(flag).await && gift(ctx, flag, item, qty, Obtain::Quiet).await {
        say(ctx, line).await;
    }
    ctx.release().await;
}

/// The walker on the cliff (4, 13).
pub async fn RouteS02_EventScript_Promeneur(ctx: Ctx) {
    msgbox(&ctx, Route_S02_Promeneur, MsgBox::Npc).await;
}

/// His Skitty (5, 13): the source gives it no line; its master's line about it stands in.
pub async fn RouteS02_EventScript_Skitty(ctx: Ctx) {
    msgbox(&ctx, Route_S02_Promeneur, MsgBox::Npc).await;
}

// ---- items (§7) ----

/// The X Speed in plain sight (11, 12).
pub async fn RouteS02_EventScript_ItemXSpeed(ctx: Ctx) {
    find_item(&ctx, ITEM_X_SPEED, 1).await;
}

/// The Magnet by the transformer (40, 2), behind cable c3.
pub async fn RouteS02_EventScript_ItemMagnet(ctx: Ctx) {
    find_item(&ctx, ITEM_MAGNET, 1).await;
}

/// The Great Balls behind the server block (48, 8): ×2 (§7).
pub async fn RouteS02_EventScript_ItemGreatBall(ctx: Ctx) {
    find_item(&ctx, ITEM_GREAT_BALL, 2).await;
}

// ---- signs ----

pub async fn RouteS02_EventScript_SignEntree(ctx: Ctx) {
    msgbox(&ctx, Route_S02_Sign_Entree, MsgBox::Sign).await;
}

pub async fn RouteS02_EventScript_SignCable(ctx: Ctx) {
    msgbox(&ctx, Route_S02_Sign_Cable, MsgBox::Sign).await;
}

pub async fn RouteS02_EventScript_SignTransfo(ctx: Ctx) {
    msgbox(&ctx, Route_S02_Sign_Transfo, MsgBox::Sign).await;
}

pub async fn RouteS02_EventScript_SignFalaise(ctx: Ctx) {
    msgbox(&ctx, Route_S02_Sign_Falaise, MsgBox::Sign).await;
}

// ---- module obstacles (Route_S02_Sc04 and the server block) ----

/// General's grass (metatile 1), walkable: a cut cable (the security cables are SilRoutesS2S3's
/// `cable_secu`, solid, on grass).
const GRASS: u16 = METATILE_RONCES_COUPEES;

/// A security cable (Route_S02_Sc04, "the same script as Bois_Ether_Sc08" with the cable's
/// texts): Lame-Fréquence. Without the module, Route_S02_Cable_T1 reads as a sign (its
/// question has no box then; the module comes with Chapter 1's badge, before this route).
const fn cable(done: FlagId, paint: &'static [(i16, i16, u16, bool)]) -> Obstacle {
    Obstacle {
        module: FLAG_SIL_MODULE_COUPE,
        done,
        paint,
        blocked: Route_S02_Cable_T1,
        ask: Some(Route_S02_Cable_T1),
        cleared: Route_S02_Cable_T2,
    }
}

/// c1, on cable A (10–11, 5): the shortcut.
const CABLE_1: Obstacle = cable(
    FLAG_SIL_ROUTE_S02_CABLE_1,
    &[(10, 5, GRASS, false), (11, 5, GRASS, false)],
);
/// c2, on cable C (31–32, 9).
const CABLE_2: Obstacle = cable(
    FLAG_SIL_ROUTE_S02_CABLE_2,
    &[(31, 9, GRASS, false), (32, 9, GRASS, false)],
);
/// c3 (38, 2), before the Magnet.
const CABLE_3: Obstacle = cable(FLAG_SIL_ROUTE_S02_CABLE_3, &[(38, 2, GRASS, false)]);

/// The server block (48, 10) in front of the Great Balls' pocket: Champ Magnétique (Chapter 2's
/// badge module, so on a return trip).
// fallback: the Strength push (M10 field moves, the source's §9 fallback "set_metatile to move
// the block one cell + flag"): the block slides one cell west onto cable D (47, 10), and its
// cell becomes grass. The texts are Chapter 3's Depot block lines (Depot_Bloc_T1/_T2), the
// source's only Champ Magnétique obstacle texts.
const SERVEUR: Obstacle = Obstacle {
    module: FLAG_SIL_MODULE_FORCE,
    done: FLAG_SIL_ROUTE_S02_SERVER_MOVED,
    paint: &[(48, 10, GRASS, false), (47, 10, METATILE_SERVEUR, true)],
    blocked: Depot_Bloc_T1,
    ask: None,
    cleared: Depot_Bloc_T2,
};
/// The server block's metatile (SilRoutesS2S3's `serveur`, the map's `Q`).
const METATILE_SERVEUR: u16 = 550;

pub async fn RouteS02_EventScript_Cable1(ctx: Ctx) {
    module_obstacle(&ctx, &CABLE_1).await;
}

pub async fn RouteS02_EventScript_Cable2(ctx: Ctx) {
    module_obstacle(&ctx, &CABLE_2).await;
}

pub async fn RouteS02_EventScript_Cable3(ctx: Ctx) {
    module_obstacle(&ctx, &CABLE_3).await;
}

pub async fn RouteS02_EventScript_Serveur(ctx: Ctx) {
    if module_obstacle(&ctx, &SERVEUR).await {
        fallback(&ctx, "strength").await;
    }
}
