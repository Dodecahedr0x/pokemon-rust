//! Chapter 3, the Goulet de l'Écho (`RouteS03`) and the Cocket depot between its two portals
//! (`RouteS03_Depot`): `contenu-jeu/chapitres/03-viber-city.md` §2.1, §2.2 (signs, ambient
//! NPCs, the server blocks), §3 Route_S03_Sc01, Depot_Sc01 and Depot_Sc02, §5 (the route's
//! trainers) and §7 (the items). `VAR_SIL_STORY` runs 28 (Chapter 2's end, the Gris-Échec north
//! exit) to 31 (Gogo & Romain beaten at the depot). The wild encounters are
//! `battle/encounters.ron`'s.

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

/// The hidden Ether behind the trucker (5, 22), once (`FLAG_SIL_ITEM_S03_ETHER`).
pub async fn RouteS03_EventScript_HiddenEther(ctx: Ctx) {
    // fallback: hidden items (the engine ignores `BgKind::HiddenItem`; the M9 finding of SIL1
    // Task F): the map has a sign here (`05_ch3_scripts.ron`), and this script gives the item
    // with Gen3's obtain messages, once, behind the item's flag.
    hidden_item(&ctx, FLAG_SIL_ITEM_S03_ETHER, ITEM_ETHER).await;
}

/// The hidden Nugget by the echo rock (8, 15), once (`FLAG_SIL_ITEM_S03_NUGGET`).
pub async fn RouteS03_EventScript_HiddenNugget(ctx: Ctx) {
    // fallback: hidden items (as the Ether's).
    hidden_item(&ctx, FLAG_SIL_ITEM_S03_NUGGET, ITEM_NUGGET).await;
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

/// A block: its object, its home cell, its solved cell, its temp var.
type Bloc = (u8, (i16, i16), (i16, i16), VarId);

/// The three blocks: each object, its home cell, its cell once the puzzle is solved (§2.2's
/// solution: A1 pushed up to (7, 6) then right, B1 up into the dock, B2 right), and the temp var
/// that holds its cell on this load (`x + 16 y`; temp vars clear on every load, as the blocks
/// reset).
const BLOCS: [Bloc; 3] = [
    (LOCALID_DEPOT_BLOC_A1, (7, 11), (8, 6), VAR_TEMP_4),
    (LOCALID_DEPOT_BLOC_B1, (7, 4), (7, 3), VAR_TEMP_5),
    (LOCALID_DEPOT_BLOC_B2, (7, 2), (8, 2), VAR_TEMP_6),
];

fn cell_var((x, y): (i16, i16)) -> u16 {
    (x + 16 * y) as u16
}

fn var_cell(v: u16) -> (i16, i16) {
    ((v % 16) as i16, (v / 16) as i16)
}

/// The depot's other objects that a block cannot be pushed onto while they are shown: Gogo,
/// Romain and the sbire before Depot_Sc02, the two item balls until taken.
const DEPOT_OTHERS: [((i16, i16), FlagId); 5] = [
    ((5, 7), FLAG_SIL_HIDE_DEPOT_GOGO_ROMAIN),
    ((9, 7), FLAG_SIL_HIDE_DEPOT_ROMAIN),
    ((4, 8), FLAG_SIL_HIDE_DEPOT_SBIRE),
    ((3, 6), FLAG_SIL_ITEM_DEPOT_METAL_COAT),
    ((11, 8), FLAG_SIL_ITEM_DEPOT_GREAT_BALL),
];

/// The blocks start at home; from `VAR_SIL_STORY` 31 (Gogo & Romain beaten) in the solved
/// layout (Gen3 `setobjectxyperm`), so that a player coming back from Viber-City can cross to
/// the south portal (A1 at home only goes down, onto (7, 12), before the portal). The magnet is
/// off again: `FLAG_SIL_DEPOT_MAGNET_ON` is the source's temporary flag, cleared on every load.
pub async fn RouteS03_Depot_OnTransition(ctx: Ctx) {
    let solved = story(&ctx).await >= 31;
    for (obj, home, done, var) in BLOCS {
        let at = if solved { done } else { home };
        if solved {
            ctx.set_object_template_pos(obj, at.0, at.1).await;
        }
        ctx.set_var(var, cell_var(at)).await;
    }
    ctx.set_flag(FLAG_SIL_DEPOT_MAGNET_ON, false).await;
}

static PUSH_UP: [Step; 1] = [walk(Dir::Up)];
static PUSH_DOWN: [Step; 1] = [walk(Dir::Down)];
static PUSH_LEFT: [Step; 1] = [walk(Dir::Left)];
static PUSH_RIGHT: [Step; 1] = [walk(Dir::Right)];

/// Depot_Sc01, a server block (all three): without `FLAG_SIL_MODULE_FORCE`, `Depot_Bloc_T1`;
/// with it, the first block talked to on this load switches the magnet on (`Depot_Bloc_T2`,
/// `FLAG_SIL_DEPOT_MAGNET_ON`); after that each press pushes the block one cell away from the
/// player, when that cell is free floor.
pub async fn RouteS03_Depot_EventScript_Bloc(ctx: Ctx) {
    if !ctx.flag(FLAG_SIL_MODULE_FORCE).await {
        msgbox(&ctx, Depot_Bloc_T1, MsgBox::Sign).await;
        return;
    }
    ctx.lock().await;
    if !ctx.flag(FLAG_SIL_DEPOT_MAGNET_ON).await {
        say(&ctx, Depot_Bloc_T2).await;
        ctx.set_flag(FLAG_SIL_DEPOT_MAGNET_ON, true).await;
        ctx.release().await;
        return;
    }
    // fallback: field_move Champ Magnétique (the engine pushes no boulder: no Strength before
    // M10): each A press on a block pushes it one cell in the player's facing, with Gen3's
    // boulder rule (free floor, no object, no warp). The blocks still reset on every load.
    fallback(&ctx, "strength").await;
    let talked = ctx.var(VAR_LAST_TALKED).await;
    let dir = ctx.player_facing().await;
    let (dx, dy) = dir.delta();
    let Some(&(obj, _, _, var)) = BLOCS.iter().find(|b| u16::from(b.0) == talked) else {
        ctx.release().await;
        return;
    };
    let (x, y) = var_cell(ctx.var(var).await);
    let to = (x + dx, y + dy);
    if depot_free(&ctx, to).await {
        let path: &'static [Step] = match dir {
            Dir::Up => &PUSH_UP,
            Dir::Down => &PUSH_DOWN,
            Dir::Left => &PUSH_LEFT,
            Dir::Right => &PUSH_RIGHT,
        };
        move_wait(&ctx, obj, path).await;
        ctx.set_var(var, cell_var(to)).await;
    }
    ctx.release().await;
}

/// Whether a block may be pushed onto `(x, y)`: depot floor, and no other block or shown object
/// there.
async fn depot_free(ctx: &Ctx, (x, y): (i16, i16)) -> bool {
    let floor = usize::try_from(y)
        .ok()
        .and_then(|y| DEPOT_FLOOR.get(y))
        .and_then(|row| usize::try_from(x).ok().and_then(|x| row.as_bytes().get(x)))
        == Some(&b'.');
    if !floor {
        return false;
    }
    for (_, _, _, var) in BLOCS {
        if var_cell(ctx.var(var).await) == (x, y) {
            return false;
        }
    }
    for (cell, hidden) in DEPOT_OTHERS {
        if cell == (x, y) && !ctx.flag(hidden).await {
            return false;
        }
    }
    true
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
    // fallback: screen_flash (the smoke bomb's white screen, §9): a fade out and in.
    fallback(&ctx, "screen_flash").await;
    ctx.fade(Fade::ToBlack).await;
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
    ctx.fade(Fade::FromBlack).await;
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
