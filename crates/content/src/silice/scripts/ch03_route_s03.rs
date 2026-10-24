//! Chapter 3, the Goulet de l'Écho (`RouteS03`) and the Cocket depot between its two portals
//! (`RouteS03_Depot`): `contenu-jeu/chapitres/03-viber-city.md` §2.1, §2.2 (signs, ambient
//! NPCs, the server blocks), §3 Route_S03_Sc01, Depot_Sc01 and Depot_Sc02, §5 (the route's
//! trainers) and §7 (the items). `VAR_SIL_STORY` runs 28 (Chapter 2's end, the Gris-Échec north
//! exit) to 31 (Gogo & Romain beaten at the depot). The wild encounters are
//! `battle/encounters.ron`'s.

use super::ch04_port::Puzzle;
use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["RouteS03", "RouteS03_Depot"];

// ---- Route S3 ----

/// Route_S03_Sc01, the Goulet's entry: the row three cells after the south edge (x 5–14,
/// y 46) at `VAR_SIL_STORY` 28. Sets 30.
pub async fn RouteS03_Sc01(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Route_S03_Sc01_T1).await;
    set_story(&ctx, 30).await;
    ctx.release_all().await;
}

// ---- Route S3: trainers (§5; sight from the map's trainer objects, then the `_Apres` line) ----

/// The SBIRE COCKET guarding the depot's south portal (12, 21), facing left.
pub async fn RouteS03_EventScript_Sbire(ctx: Ctx) {
    if trainer(&ctx, LOCALID_ROUTE_S03_SBIRE, TRAINER_SIL_GRUNT_S03_1, Route_S03_Sbire_Intro, Route_S03_Sbire_Defaite).await {
        post_battle(&ctx, Route_S03_Sbire_Apres).await;
    }
}

/// MONTAGNARD ROGER on his ledge (17, 35), facing west.
pub async fn RouteS03_EventScript_Roger(ctx: Ctx) {
    if trainer(&ctx, LOCALID_ROUTE_S03_ROGER, TRAINER_SIL_S03_ROGER_1, Route_S03_Roger_Intro, Route_S03_Roger_Defaite).await {
        post_battle(&ctx, Route_S03_Roger_Apres).await;
    }
}

/// GUITARISTE SACHA (8, 31), facing right.
pub async fn RouteS03_EventScript_Sacha(ctx: Ctx) {
    if trainer(&ctx, LOCALID_ROUTE_S03_SACHA, TRAINER_SIL_S03_SACHA_1, Route_S03_Sacha_Intro, Route_S03_Sacha_Defaite).await {
        post_battle(&ctx, Route_S03_Sacha_Apres).await;
    }
}

/// DOCKER MARCO (13, 27), facing left.
pub async fn RouteS03_EventScript_Marco(ctx: Ctx) {
    if trainer(&ctx, LOCALID_ROUTE_S03_MARCO, TRAINER_SIL_S03_MARCO_1, Route_S03_Marco_Intro, Route_S03_Marco_Defaite).await {
        post_battle(&ctx, Route_S03_Marco_Apres).await;
    }
}

/// {PKMN} RANGER INÈS (5, 24), facing right.
pub async fn RouteS03_EventScript_Ines(ctx: Ctx) {
    if trainer(&ctx, LOCALID_ROUTE_S03_INES, TRAINER_SIL_S03_INES_1, Route_S03_Ines_Intro, Route_S03_Ines_Defaite).await {
        post_battle(&ctx, Route_S03_Ines_Apres).await;
    }
}

/// KINÉSISTE NOÉ in the north part (8, 10), facing down.
pub async fn RouteS03_EventScript_Noe(ctx: Ctx) {
    if trainer(&ctx, LOCALID_ROUTE_S03_NOE, TRAINER_SIL_S03_NOE_1, Route_S03_Noe_Intro, Route_S03_Noe_Defaite).await {
        post_battle(&ctx, Route_S03_Noe_Apres).await;
    }
}

// ---- Route S3: items (§7) ----

/// The Super Potion on Roger's ledge (17, 34).
pub async fn RouteS03_EventScript_ItemSuperPotion(ctx: Ctx) {
    find_item(&ctx, ITEM_SUPER_POTION, 1).await;
}

/// The Hard Stone at the cliff's foot (5, 36).
pub async fn RouteS03_EventScript_ItemHardStone(ctx: Ctx) {
    find_item(&ctx, ITEM_HARD_STONE, 1).await;
}

/// The Super Repel in the north grass (13, 13).
pub async fn RouteS03_EventScript_ItemSuperRepel(ctx: Ctx) {
    find_item(&ctx, ITEM_SUPER_REPEL, 1).await;
}

// ---- Route S3: signs and ambient NPCs (§2.1) ----

pub async fn RouteS03_EventScript_Panneau1(ctx: Ctx) {
    msgbox(&ctx, Route_S03_Panneau_T1, MsgBox::Sign).await;
}

pub async fn RouteS03_EventScript_Panneau2(ctx: Ctx) {
    msgbox(&ctx, Route_S03_Panneau_T2, MsgBox::Sign).await;
}

/// The hiker by the south entry.
pub async fn RouteS03_EventScript_Pnj01(ctx: Ctx) {
    msgbox(&ctx, Route_S03_Pnj01_T1, MsgBox::Npc).await;
}

/// The trucker in front of the containers.
pub async fn RouteS03_EventScript_Pnj02(ctx: Ctx) {
    msgbox(&ctx, Route_S03_Pnj02_T1, MsgBox::Npc).await;
}

/// The little girl in the north part, past the depot.
pub async fn RouteS03_EventScript_Pnj03(ctx: Ctx) {
    msgbox(&ctx, Route_S03_Pnj03_T1, MsgBox::Npc).await;
}

// ---- the depot: the server blocks (Depot_Sc01) ----

/// The depot's floor (§2.2's grid, as built): the cells a block may be pushed onto. The two
/// portals (7, 0) and (7, 13) are warps, never a block's cell.
const DEPOT_FLOOR: [&str; 14] = [
    "###############",
    "#######.#######",
    "#####.....#####",
    "#####.#.#.#####",
    "#####.....#####",
    "#######.#######",
    "###.........###",
    "###.........###",
    "###.........###",
    "###.........###",
    "#######.#######",
    "#######.#######",
    "#######.#######",
    "###############",
];

/// The first row of the south corridor (7, 10–12), between the rest area and the south portal.
const SOUTH_CORRIDOR: i16 = 10;

/// The three blocks: each object, its home cell and its cell once the puzzle is solved (§2.2's
/// solution: A1 pushed up to (7, 6) then right, B1 up into the dock, B2 right).
const BLOCS: [(u8, (i16, i16), (i16, i16)); 3] = [
    (LOCALID_DEPOT_BLOC_A1, (7, 11), (8, 6)),
    (LOCALID_DEPOT_BLOC_B1, (7, 4), (7, 3)),
    (LOCALID_DEPOT_BLOC_B2, (7, 2), (8, 2)),
];

/// The blocks' push puzzle (`silice::push`): their cells in `VAR_TEMP_4`–`_6` once pushed.
pub(crate) const DEPOT: Puzzle = Puzzle {
    map: MAP_ROUTE_S03_DEPOT,
    objects: &[
        (LOCALID_DEPOT_BLOC_A1, VAR_TEMP_4, (7, 11)),
        (LOCALID_DEPOT_BLOC_B1, VAR_TEMP_5, (7, 4)),
        (LOCALID_DEPOT_BLOC_B2, VAR_TEMP_6, (7, 2)),
    ],
    free: depot_free,
};

/// The blocks start at home; from `VAR_SIL_STORY` 31 (Gogo & Romain beaten) in the solved
/// layout (Gen3 `setobjectxyperm`), so that a player coming back from Viber-City can cross to
/// the south portal (A1 at home only goes down, onto (7, 12), before the portal). The magnet is
/// off again: `FLAG_SIL_DEPOT_MAGNET_ON` is the source's temporary flag, cleared on every load.
pub async fn RouteS03_Depot_OnTransition(ctx: Ctx) {
    if story(&ctx).await >= 31 {
        for (obj, _, done) in BLOCS {
            ctx.set_object_template_pos(obj, done.0, done.1).await;
        }
    }
    ctx.set_flag(FLAG_SIL_DEPOT_MAGNET_ON, false).await;
}

/// Depot_Sc01, a server block (all three): without `FLAG_SIL_MODULE_FORCE`, `Depot_Bloc_T1`;
/// with it, the first block talked to on this load switches the magnet on (`Depot_Bloc_T2`,
/// `FLAG_SIL_DEPOT_MAGNET_ON`) and with it Strength (`FLAG_SYS_USE_STRENGTH`): the player then
/// pushes a block by walking into it, onto free depot floor ([`depot_free`]).
pub async fn RouteS03_Depot_EventScript_Bloc(ctx: Ctx) {
    if !ctx.flag(FLAG_SIL_MODULE_FORCE).await {
        msgbox(&ctx, Depot_Bloc_T1, MsgBox::Sign).await;
        return;
    }
    if ctx.flag(FLAG_SIL_DEPOT_MAGNET_ON).await {
        return;
    }
    ctx.lock().await;
    say(&ctx, Depot_Bloc_T2).await;
    ctx.set_flag(FLAG_SIL_DEPOT_MAGNET_ON, true).await;
    ctx.set_flag(FLAG_SYS_USE_STRENGTH, true).await;
    ctx.release().await;
}

/// Whether a block may be pushed from `from` onto `to`: depot floor (the other blocks and the
/// shown objects block it as any object does), and never down into the south corridor (rows
/// 10–12): a block there walls off the south portal, the only way out of a botched layout
/// (Depot_Panneau_T1's promise; the source's grid allows it, a softlock within one visit).
fn depot_free(from: (i16, i16), (x, y): (i16, i16)) -> bool {
    let floor = usize::try_from(y)
        .ok()
        .and_then(|y| DEPOT_FLOOR.get(y))
        .and_then(|row| usize::try_from(x).ok().and_then(|x| row.as_bytes().get(x)))
        == Some(&b'.');
    floor && !(y > from.1 && y >= SOUTH_CORRIDOR)
}

// ---- the depot: Gogo & Romain (Depot_Sc02) ----

const FACE_DOWN: &[Step] = &[face(Dir::Down)];
const GOGO_IN: &[Step] = &[walk(Dir::Right), walk(Dir::Down)];
const ROMAIN_IN: &[Step] = &[walk(Dir::Left), walk(Dir::Down)];

/// Depot_Sc02, Gogo & Romain's patrol: (7, 10), the only way into the rest area, at
/// `VAR_SIL_STORY` 30. Two battles, the smoke bomb, the Metal Coat on the table. Sets 31.
pub async fn RouteS03_Depot_Sc02(ctx: Ctx) {
    ctx.lock_all().await;
    move_wait(&ctx, LOCALID_DEPOT_ROMAIN, FACE_DOWN).await;
    say(&ctx, Depot_Sc02_T1).await;
    move_wait(&ctx, LOCALID_DEPOT_GOGO, FACE_DOWN).await;
    say(&ctx, Depot_Sc02_T2).await;
    ctx.move_obj(LOCALID_DEPOT_GOGO, GOGO_IN).await;
    move_wait(&ctx, LOCALID_DEPOT_ROMAIN, ROMAIN_IN).await;
    ctx.wait_move(LOCALID_DEPOT_GOGO).await;
    say(&ctx, Depot_Sc02_T3).await;
    // fallback: battle_two_trainers (a double battle against two trainers, Milestone 11): the
    // source's own fallback, Gogo then Romain, without a heal between them.
    battle_two_trainers(
        &ctx,
        (TRAINER_SIL_GOGO_2, Depot_Sc02_Defaite_Gogo),
        (TRAINER_SIL_ROMAIN_2, Depot_Sc02_Defaite_Romain),
    )
    .await;
    say(&ctx, Depot_Sc02_T4).await;
    say(&ctx, Depot_Sc02_T5).await;
    // The smoke bomb's white screen (§9): a white fade while they flee.
    fade_white(&ctx, true).await;
    hide(&ctx, LOCALID_DEPOT_GOGO).await;
    hide(&ctx, LOCALID_DEPOT_ROMAIN).await;
    hide(&ctx, LOCALID_DEPOT_SBIRE).await;
    // §7: the Metal Coat shows on the table once the scene is over.
    show(
        &ctx,
        LOCALID_DEPOT_ITEM_METAL_COAT,
        FLAG_SIL_ITEM_DEPOT_METAL_COAT,
    )
    .await;
    fade_white(&ctx, false).await;
    say(&ctx, Depot_Sc02_T6).await;
    set_story(&ctx, 31).await;
    ctx.release_all().await;
}

// ---- the depot: items and signs ----

/// The Metal Coat on the rest area's table (3, 6), shown by Depot_Sc02.
pub async fn RouteS03_Depot_EventScript_ItemMetalCoat(ctx: Ctx) {
    find_item(&ctx, ITEM_METAL_COAT, 1).await;
}

/// The Great Ball in the rest area's corner (11, 8).
pub async fn RouteS03_Depot_EventScript_ItemGreatBall(ctx: Ctx) {
    find_item(&ctx, ITEM_GREAT_BALL, 1).await;
}

pub async fn RouteS03_Depot_EventScript_Panneau1(ctx: Ctx) {
    msgbox(&ctx, Depot_Panneau_T1, MsgBox::Sign).await;
}

pub async fn RouteS03_Depot_EventScript_Panneau2(ctx: Ctx) {
    msgbox(&ctx, Depot_Panneau_T2, MsgBox::Sign).await;
}
