//! Chapter 8, the Chutes du Paradoxe (`RouteS08`): `contenu-jeu/chapitres/08-evalon.md` §2.1
//! (the signs, the friends' clues, the townsfolk, the brambles and the cracked rock), §3
//! Route_S08_Sc01 (the entry) and Route_S08_Sc02 (the messenger pigeon), §5 (the six route
//! trainers) and §7 (the items). `VAR_SIL_STORY` 76 (Chapter 7's end) → 80 (on the route) → 81
//! (the friends' message). Évalon is `ch08_evalon.rs`.
//!
//! The paliers are joined by waterfalls and a river ("SIL4 Ch8 maps as built"): without Surf
//! and Waterfall in the engine, palier 1 and palier 3 are only joined by the modules' moves.

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["RouteS08"];

// ---- the brambles (shared with Évalon's) ----

/// Synthetic brambles that regrow on every load (Gen3's cut trees; "SIL4 Ch8 maps as built":
/// metatile 565 and a bg script on each cell, no flag). The cell is the one the player faces;
/// `cells` lists the map's bramble cells, and bit `i` of `var` (a temp var, cleared on every
/// load) marks `cells[i]` cut during this visit, so a cut cell's sign stays quiet. Without
/// `FLAG_SIL_MODULE_COUPE`, `Bois_Ether_Sc08_T1` (the generic brambles, Chapter 1's
/// `Bois_Ether_Sc08`); with it, `_T2` (yes/no), on yes `_T3` and the cell repainted as `cut`.
pub(super) async fn ronces(ctx: &Ctx, cells: &[(i16, i16)], var: VarId, cut: u16) {
    let (x, y) = ctx.player_pos().await;
    let (dx, dy) = ctx.player_facing().await.delta();
    let cell = (x + dx, y + dy);
    let Some(i) = cells.iter().position(|&c| c == cell) else {
        return;
    };
    let done = ctx.var(var).await;
    if done & (1 << i) != 0 {
        return;
    }
    if !ctx.flag(FLAG_SIL_MODULE_COUPE).await {
        msgbox(ctx, Bois_Ether_Sc08_T1, MsgBox::Sign).await;
        return;
    }
    ctx.lock().await;
    if ask(ctx, Bois_Ether_Sc08_T2).await {
        say(ctx, Bois_Ether_Sc08_T3).await;
        ctx.set_metatile(cell.0, cell.1, cut, false).await;
        ctx.redraw_map().await;
        ctx.set_var(var, done | (1 << i)).await;
    }
    ctx.release().await;
}

/// The palier 3 brambles (5–6, 13), over the shortcut to the Rare Candy.
const RONCES: &[(i16, i16)] = &[(5, 13), (6, 13)];
/// Fallarbor's ash ground, the palier's floor around them.
const METATILE_CENDRE: u16 = 633;

pub async fn RouteS08_EventScript_Ronces(ctx: Ctx) {
    ronces(&ctx, RONCES, VAR_TEMP_5, METATILE_CENDRE).await;
}

/// The cracked rock (31, 10) before the braises nook: Chapter 7's silica block (the Scanner de
/// Faille, `ch07_route_s07::bloc`). Its flag is a temp flag (`FLAG_TEMP_11`, the phase file
/// `06_ch8_scripts.ron`), as Gen3's rocks: gone while the player is on the map, back on the next
/// load.
pub async fn RouteS08_EventScript_Rocher(ctx: Ctx) {
    super::ch07_route_s07::bloc(&ctx).await;
}

// ---- map scripts ----

/// The messenger pigeon is only on the map inside Route_S08_Sc02 (which shows it).
pub async fn RouteS08_OnTransition(ctx: Ctx) {
    ctx.set_flag(FLAG_SIL_HIDE_S08_PIGEON, true).await;
}

// ---- scenes ----

/// Route_S08_Sc01, the entry: the coord row (18–21, 77) at `VAR_SIL_STORY` 76 (Chapter 7's
/// end, Reverse_City_Sc07), two cells past the mats from Reverse-City. Sets 80.
pub async fn RouteS08_Sc01(ctx: Ctx) {
    if story(&ctx).await != 76 {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Route_S08_Sc01_T1).await;
    set_story(&ctx, 80).await;
    ctx.release_all().await;
}

const PIGEON_IN: &[Step] = &[walk(Dir::Down), walk(Dir::Down), walk(Dir::Down)];
const PIGEON_OUT: &[Step] = &[walk(Dir::Up), walk(Dir::Up), walk(Dir::Up), walk(Dir::Up)];

/// Route_S08_Sc02, the messenger pigeon: palier 3's entry row (30–33, 21), above the pool at
/// the top of waterfall 3, at `VAR_SIL_STORY` 80. The pigeon comes down from (31, 16) to
/// (31, 19) and flies off north. Sets 81.
pub async fn RouteS08_Sc02(ctx: Ctx) {
    if story(&ctx).await != 80 {
        return;
    }
    ctx.lock_all().await;
    show(&ctx, LOCALID_S08_PIGEON, FLAG_SIL_HIDE_S08_PIGEON).await;
    move_wait(&ctx, LOCALID_S08_PIGEON, PIGEON_IN).await;
    say(&ctx, Route_S08_Sc02_T1).await;
    say(&ctx, Route_S08_Sc02_T2).await;
    move_wait(&ctx, LOCALID_S08_PIGEON, PIGEON_OUT).await;
    hide(&ctx, LOCALID_S08_PIGEON).await;
    set_story(&ctx, 81).await;
    ctx.release_all().await;
}

// ---- trainers ----

/// Palier 1, the east bank (35, 64), sight 3 down.
pub async fn RouteS08_EventScript_Gaston(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_S08_GASTON,
        TRAINER_SIL_S08_GASTON,
        Route_S08_Gaston_Intro,
        Route_S08_Gaston_Defaite,
    )
    .await
    {
        post_battle(&ctx, Route_S08_Gaston_Defaite).await;
    }
}

/// Palier 1, in the lake (14, 68), sight 4 right.
pub async fn RouteS08_EventScript_Nais(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_S08_NAIS,
        TRAINER_SIL_S08_NAIS,
        Route_S08_Nais_Intro,
        Route_S08_Nais_Defaite,
    )
    .await
    {
        post_battle(&ctx, Route_S08_Nais_Defaite).await;
    }
}

/// Palier 2, the east bank (32, 49), sight 3 left.
pub async fn RouteS08_EventScript_Theo(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_S08_THEO,
        TRAINER_SIL_S08_THEO,
        Route_S08_Theo_Intro,
        Route_S08_Theo_Defaite,
    )
    .await
    {
        post_battle(&ctx, Route_S08_Theo_Defaite).await;
    }
}

/// The twins on their pontoon (11–12, 36): one `double: true` entry, Léa's sight (2 left).
/// Both twins run this script: one double battle against the entry's party.
pub async fn RouteS08_EventScript_LeaZoe(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_S08_LEA,
        TRAINER_SIL_S08_LEA_ZOE,
        Route_S08_LeaZoe_Intro,
        Route_S08_LeaZoe_Defaite,
    )
    .await
    {
        post_battle(&ctx, Route_S08_LeaZoe_Defaite).await;
    }
}

/// Palier 3, the knight in training (13, 12), sight 3 down.
pub async fn RouteS08_EventScript_Galahad(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_S08_GALAHAD,
        TRAINER_SIL_S08_GALAHAD,
        Route_S08_Galahad_Intro,
        Route_S08_Galahad_Defaite,
    )
    .await
    {
        post_battle(&ctx, Route_S08_Galahad_Defaite).await;
    }
}

/// Palier 3 (25, 18), sight 4 left, over the way from the pool to the portal.
pub async fn RouteS08_EventScript_Ambre(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_S08_AMBRE,
        TRAINER_SIL_S08_AMBRE,
        Route_S08_Ambre_Intro,
        Route_S08_Ambre_Defaite,
    )
    .await
    {
        post_battle(&ctx, Route_S08_Ambre_Defaite).await;
    }
}

// ---- people ----

/// The hiker by the entry (palier 1).
pub async fn RouteS08_EventScript_Npc1(ctx: Ctx) {
    msgbox(&ctx, Route_S08_Npc_1, MsgBox::Npc).await;
}

/// The fisherwoman on the west islet (palier 2).
pub async fn RouteS08_EventScript_Npc2(ctx: Ctx) {
    msgbox(&ctx, Route_S08_Npc_2, MsgBox::Npc).await;
}

/// The old role-player on his rock (palier 3).
pub async fn RouteS08_EventScript_Npc3(ctx: Ctx) {
    msgbox(&ctx, Route_S08_Npc_3, MsgBox::Npc).await;
}

// ---- signs and the friends' clues ----

pub async fn RouteS08_EventScript_Sign1(ctx: Ctx) {
    msgbox(&ctx, Route_S08_Sign_1, MsgBox::Sign).await;
}

pub async fn RouteS08_EventScript_Sign2(ctx: Ctx) {
    msgbox(&ctx, Route_S08_Sign_2, MsgBox::Sign).await;
}

/// Mathieu's scarf-cape on a branch (palier 1).
pub async fn RouteS08_EventScript_Cape(ctx: Ctx) {
    msgbox(&ctx, Route_S08_Cape, MsgBox::Sign).await;
}

/// Adrien's verses on the Rocher du Poème (palier 2).
pub async fn RouteS08_EventScript_Poeme(ctx: Ctx) {
    msgbox(&ctx, Route_S08_Poeme, MsgBox::Sign).await;
}

/// Gwada's barbecue circle (32, 6), behind the cracked rock.
pub async fn RouteS08_EventScript_Braises(ctx: Ctx) {
    msgbox(&ctx, Route_S08_Braises, MsgBox::Sign).await;
}

// ---- items ----

/// Palier 1, the east bank.
pub async fn RouteS08_EventScript_ItemUltraBall(ctx: Ctx) {
    find_item(&ctx, ITEM_ULTRA_BALL, 1).await;
}

/// Palier 2, the west islet (Surf).
pub async fn RouteS08_EventScript_ItemMaxEther(ctx: Ctx) {
    find_item(&ctx, ITEM_MAX_ETHER, 1).await;
}

/// The grotte-poche under the paradox waterfall (Waterfall down).
pub async fn RouteS08_EventScript_ItemTmLaserGlace(ctx: Ctx) {
    find_item(&ctx, ITEM_TM13, 1).await;
}

/// Palier 3, behind the brambles.
pub async fn RouteS08_EventScript_ItemRareCandy(ctx: Ctx) {
    find_item(&ctx, ITEM_RARE_CANDY, 1).await;
}

