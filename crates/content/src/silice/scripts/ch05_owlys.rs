//! Chapter 5, Owlys, the city under the abyss (`Owlys`), its Centre, the Gray Market and the
//! orphanage: `contenu-jeu/chapitres/05-owlys.md` §2 (the signs, the townsfolk, the Centre's
//! remark, the market, the orphanage), §3 Owlys_Sc01 to Sc05 (the arrival, Mathieu's plan, Milo
//! and the alley grunt, the feather lock, the elevator opening) and §7 (the items and the two
//! shops). `VAR_SIL_STORY` 50 (from the rift) → 51 (arrived) → 52 (Mathieu's plan; the East
//! Cave is `ch05_grotte_est.rs`) → 53 (the feather) → 54 (the badge, `ch05_gym.rs`) → 55 (the
//! elevator open, the chapter's end).

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &[
    "Owlys",
    "Owlys_PokemonCenter",
    "Owlys_MarcheGris",
    "Owlys_Orphelinat",
];

/// The sanctuary's door (30, 12): a cave hole (774), solid in the layout until the feather lock
/// opens it ("SIL3 Ch5 maps as built").
const PORTE_GYM: (i16, i16) = (30, 12);
const METATILE_PORTE_GYM: u16 = 774;

// ---- map scripts ----

/// The sanctuary door open once `FLAG_SIL_OWLYS_PORTE_GYM` is set.
pub async fn Owlys_OnLoad(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_OWLYS_PORTE_GYM).await {
        let (x, y) = PORTE_GYM;
        ctx.set_metatile(x, y, METATILE_PORTE_GYM, false).await;
    }
}

/// Object visibility ("SIL3 Ch5 maps as built", the six Owlys hide flags):
/// - Mathieu on the square until his plan (`VAR_SIL_STORY` 52);
/// - the eastern grunt only in Owlys_Sc02;
/// - Milo and the alley grunt until Owlys_Sc03 (`FLAG_SIL_OWLYS_MILO`);
/// - Mathieu after the badge once he stays (`FLAG_SIL_OWLYS_MATHIEU_RESTE`);
/// - both elevator Watchers until the badge (`FLAG_SIL_BADGE_05`).
pub async fn Owlys_OnTransition(ctx: Ctx) {
    let s = story(&ctx).await;
    let milo = ctx.flag(FLAG_SIL_OWLYS_MILO).await;
    let reste = ctx.flag(FLAG_SIL_OWLYS_MATHIEU_RESTE).await;
    let badge = ctx.flag(FLAG_SIL_BADGE_05).await;
    ctx.set_flag(FLAG_SIL_HIDE_OWLYS_MATHIEU_PLACE, s >= 52)
        .await;
    ctx.set_flag(FLAG_SIL_HIDE_OWLYS_SBIRE_EST, true).await;
    ctx.set_flag(FLAG_SIL_HIDE_OWLYS_MILO, milo).await;
    ctx.set_flag(FLAG_SIL_HIDE_OWLYS_SBIRE_RUELLE, milo).await;
    ctx.set_flag(FLAG_SIL_HIDE_OWLYS_MATHIEU_APRES, !reste).await;
    ctx.set_flag(FLAG_SIL_HIDE_OWLYS_GARDES_ASCENSEUR, badge)
        .await;
    ctx.set_flag(FLAG_SIL_HIDE_OWLYS_GARDE_ASCENSEUR_B, badge)
        .await;
}

// ---- scenes ----

/// Owlys_Sc01, the arrival through the iron door: the first frame at `VAR_SIL_STORY` 50.
/// Sets 51.
pub async fn Owlys_Sc01(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Owlys_Sc01_T1).await;
    set_story(&ctx, 51).await;
    ctx.release_all().await;
}

/// The eastern grunt crosses the square (the source's `Walk(Right) x8`).
const SBIRE_EST_CROSS: &[Step] = &[
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
];

/// Mathieu off east to the East Cave (`Walk(Right) x6`), from (31, 25).
const MATHIEU_OUT: &[Step] = &[
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
];
/// The same when the player talks to him from his right, (32, 25): one row down first.
const MATHIEU_OUT_AROUND: &[Step] = &[
    walk(Dir::Down),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
];

/// Owlys_Sc02, Mathieu wants to see Justice (the square, `VAR_SIL_STORY` 51): the eastern
/// grunt crosses the square, the skewers (3 Super Potions, once:
/// `FLAG_SIL_OWLYS_MATHIEU_BROCHETTES`), then his plan (yes/no). On no he waits; talking again
/// asks again (without the grunt and the gift). On yes he leaves for the East Cave: story 52.
pub async fn Owlys_Sc02(ctx: Ctx) {
    ctx.lock_all().await;
    ctx.face_player().await;
    if !ctx.flag(FLAG_SIL_OWLYS_MATHIEU_BROCHETTES).await {
        say(&ctx, Owlys_Sc02_T1).await;
        show(&ctx, LOCALID_OWLYS_SBIRE_EST, FLAG_SIL_HIDE_OWLYS_SBIRE_EST).await;
        move_wait(&ctx, LOCALID_OWLYS_SBIRE_EST, SBIRE_EST_CROSS).await;
        hide(&ctx, LOCALID_OWLYS_SBIRE_EST).await;
        ctx.turn(LOCALID_OWLYS_MATHIEU, Dir::Right).await;
        say(&ctx, Owlys_Sc02_T2).await;
        gift(
            &ctx,
            FLAG_SIL_OWLYS_MATHIEU_BROCHETTES,
            ITEM_SUPER_POTION,
            3,
            Obtain::Message,
        )
        .await;
    }
    if !ask(&ctx, Owlys_Sc02_T3).await {
        say(&ctx, Owlys_Sc02_T4).await;
        ctx.release_all().await;
        return;
    }
    say(&ctx, Owlys_Sc02_T5).await;
    let path = if ctx.player_pos().await == (32, 25) {
        MATHIEU_OUT_AROUND
    } else {
        MATHIEU_OUT
    };
    move_wait(&ctx, LOCALID_OWLYS_MATHIEU, path).await;
    hide(&ctx, LOCALID_OWLYS_MATHIEU).await;
    ctx.set_flag(FLAG_SIL_HIDE_OWLYS_MATHIEU_PLACE, true).await;
    set_story(&ctx, 52).await;
    ctx.release_all().await;
}

/// Milo goes home (the source's `Walk(Up) x4`), along the alley's column x 6, from (5, 33):
/// the grunt's sight is column x 5, where the player stands. From x 6, column x 4.
const MILO_HOME: &[Step] = &[
    walk(Dir::Right),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
];
const MILO_HOME_LEFT: &[Step] = &[
    walk(Dir::Left),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
];

/// Owlys_Sc03, Milo and TM46 Thief (optional): the alley grunt (5, 31), a sight trainer, has
/// Milo cornered. Beaten, he goes; Milo gives the TM (`FLAG_SIL_OWLYS_MILO`, once) and runs
/// home. Talked to again before the scene ends (a whiteout), his defeat line.
pub async fn Owlys_Sc03(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_OWLYS_SBIRE_RUELLE,
        TRAINER_SIL_OWLYS_SBIRE_RUELLE,
        Owlys_Sc03_T1,
        Owlys_Sc03_Defeat,
    )
    .await
    {
        post_battle(&ctx, Owlys_Sc03_Defeat).await;
    }
    if ctx.flag(FLAG_SIL_OWLYS_MILO).await {
        return;
    }
    ctx.lock_all().await;
    ctx.fade(Fade::ToBlack).await;
    hide(&ctx, LOCALID_OWLYS_SBIRE_RUELLE).await;
    ctx.fade(Fade::FromBlack).await;
    move_wait(&ctx, LOCALID_OWLYS_MILO, &[Step::FacePlayer]).await;
    say(&ctx, Owlys_Sc03_T2).await;
    gift(&ctx, FLAG_SIL_OWLYS_MILO, ITEM_TM46, 1, Obtain::Message).await;
    say(&ctx, Owlys_Sc03_T3).await;
    let path = if ctx.player_pos().await.0 == 6 {
        MILO_HOME_LEFT
    } else {
        MILO_HOME
    };
    move_wait(&ctx, LOCALID_OWLYS_MILO, path).await;
    hide(&ctx, LOCALID_OWLYS_MILO).await;
    ctx.release_all().await;
}

/// Owlys_Sc04, the feather lock on the sanctuary door (a sign on the door's cell): shut before
/// the feather (`VAR_SIL_STORY` < 53); with it, the door opens once (`FLAG_SIL_OWLYS_PORTE_GYM`).
/// The source's door animation (`door` / `wait_door`) is a cave hole without frames here: the
/// cell's collision is opened and the map redrawn.
pub async fn Owlys_Sc04(ctx: Ctx) {
    if story(&ctx).await < 53 || !ctx.flag(FLAG_SIL_OWLYS_PLUME).await {
        msgbox(&ctx, Owlys_Sc04_Ferme, MsgBox::Sign).await;
        return;
    }
    if ctx.flag(FLAG_SIL_OWLYS_PORTE_GYM).await {
        return;
    }
    ctx.lock().await;
    say(&ctx, Owlys_Sc04_T1).await;
    let (x, y) = PORTE_GYM;
    ctx.set_metatile(x, y, METATILE_PORTE_GYM, false).await;
    ctx.redraw_map().await;
    ctx.set_flag(FLAG_SIL_OWLYS_PORTE_GYM, true).await;
    ctx.release().await;
}

/// Owlys_Sc05, after the badge: the first frame back in town at `VAR_SIL_STORY` 54 (the
/// Watchers are gone, the Mallette's signal moves). Sets 55.
pub async fn Owlys_Sc05(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Owlys_Sc05_T1).await;
    set_story(&ctx, 55).await;
    ctx.release_all().await;
}

// ---- trainers ----

/// The warehouse's guard by its door (47, 8): seen from the door's front cell.
pub async fn Owlys_EventScript_SiloGarde(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_OWLYS_SILO_GARDE,
        TRAINER_SIL_OWLYS_SILO_GARDE,
        Owlys_Silo_Garde_Intro,
        Owlys_Silo_Garde_Defeat,
    )
    .await
    {
        post_battle(&ctx, Owlys_Silo_Garde_Defeat).await;
    }
}

// ---- the townsfolk ----

pub async fn Owlys_EventScript_Lanterne(ctx: Ctx) {
    msgbox(&ctx, Owlys_Npc_Lanterne, MsgBox::Npc).await;
}

pub async fn Owlys_EventScript_Vendeur(ctx: Ctx) {
    msgbox(&ctx, Owlys_Npc_Vendeur, MsgBox::Npc).await;
}

pub async fn Owlys_EventScript_Gamin(ctx: Ctx) {
    msgbox(&ctx, Owlys_Npc_Gamin, MsgBox::Npc).await;
}

pub async fn Owlys_EventScript_Garde(ctx: Ctx) {
    msgbox(&ctx, Owlys_Npc_Garde, MsgBox::Npc).await;
}

pub async fn Owlys_EventScript_Docker(ctx: Ctx) {
    msgbox(&ctx, Owlys_Npc_Docker, MsgBox::Npc).await;
}

pub async fn Owlys_EventScript_Poete(ctx: Ctx) {
    msgbox(&ctx, Owlys_Npc_Poete, MsgBox::Npc).await;
}

pub async fn Owlys_EventScript_Rumeur(ctx: Ctx) {
    msgbox(&ctx, Owlys_Npc_Rumeur, MsgBox::Npc).await;
}

/// Mathieu by the fountain after the badge, without his scarf.
pub async fn Owlys_EventScript_MathieuApres(ctx: Ctx) {
    msgbox(&ctx, Owlys_Npc_Mathieu_Apres, MsgBox::Npc).await;
}

/// The elevator's two Watchers, until the badge. The source gives them no line: the elevator
/// sign's ("on the Governor's authorisation") stands in.
pub async fn Owlys_EventScript_Veilleur(ctx: Ctx) {
    msgbox(&ctx, Owlys_Sign_Ascenseur, MsgBox::Npc).await;
}

// ---- signs and items ----

pub async fn Owlys_EventScript_SignVille(ctx: Ctx) {
    msgbox(&ctx, Owlys_Sign_Ville, MsgBox::Sign).await;
}

pub async fn Owlys_EventScript_SignGym(ctx: Ctx) {
    msgbox(&ctx, Owlys_Sign_Gym, MsgBox::Sign).await;
}

pub async fn Owlys_EventScript_SignAscenseur(ctx: Ctx) {
    msgbox(&ctx, Owlys_Sign_Ascenseur, MsgBox::Sign).await;
}

pub async fn Owlys_EventScript_SignEntrepot(ctx: Ctx) {
    msgbox(&ctx, Owlys_Sign_Entrepot, MsgBox::Sign).await;
}

/// The Hoothoot gargoyle on the square (its left eye askew).
pub async fn Owlys_EventScript_Gargouille(ctx: Ctx) {
    msgbox(&ctx, Owlys_Npc_Gargouille, MsgBox::Sign).await;
}

/// The Escape Rope on the east walkway.
pub async fn Owlys_EventScript_ItemCorde(ctx: Ctx) {
    find_item(&ctx, ITEM_ESCAPE_ROPE, 1).await;
}

/// The Black Glasses hidden behind the gargoyle (34, 25).
pub async fn Owlys_EventScript_HiddenBlackGlasses(ctx: Ctx) {
    // fallback: hidden items (the engine ignores `BgKind::HiddenItem`; the M9 finding of
    // "SIL1 Task F as built"): the map has a sign here instead.
    hidden_item(&ctx, FLAG_SIL_ITEM_OWLYS_LUNETTES_NOIRES, ITEM_BLACK_GLASSES).await;
}

// ---- the Centre ----

/// The Centre is Owlys's heal point and the Sustentation destination
/// (`HEAL_LOCATION_SIL_OWLYS`, `game.ron`).
pub async fn Owlys_PokemonCenter_OnTransition(ctx: Ctx) {
    ctx.set_respawn(HEAL_LOCATION_SIL_OWLYS).await;
}

/// The nurse: her remark about the lamps, then Gen3's nurse.
pub async fn Owlys_PokemonCenter_EventScript_Nurse(ctx: Ctx) {
    msgbox(&ctx, Owlys_Centre_Infirmiere_Remarque, MsgBox::Npc).await;
    pokecenter_nurse(&ctx).await;
}

/// The Centre's counter (§7).
const CENTRE_STOCK: &[ItemId] = &[
    ItemId(ITEM_ULTRA_BALL),
    ItemId(ITEM_GREAT_BALL),
    ItemId(ITEM_HYPER_POTION),
    ItemId(ITEM_FULL_HEAL),
    ItemId(ITEM_REVIVE),
    ItemId(ITEM_MAX_REPEL),
    ItemId(ITEM_ESCAPE_ROPE),
];

pub async fn Owlys_PokemonCenter_EventScript_Clerk(ctx: Ctx) {
    // fallback: mart (no shop screen before M9): greeting and goodbye, nothing sold.
    mart(&ctx, CENTRE_STOCK).await;
}

/// A pilgrim waiting for the elevator. The source gives him no line: Lanterne's ("here we
/// ask whether you have a lamp") stands in.
pub async fn Owlys_PokemonCenter_EventScript_Pelerin(ctx: Ctx) {
    msgbox(&ctx, Owlys_Npc_Lanterne, MsgBox::Npc).await;
}

// ---- the Gray Market ----

/// The corrupt grunt's contraband (§7).
const MARCHE_STOCK: &[ItemId] = &[
    ItemId(ITEM_LUM_BERRY),
    ItemId(ITEM_FOCUS_BAND),
    ItemId(ITEM_QUICK_CLAW),
    ItemId(ITEM_SCOPE_LENS),
    ItemId(ITEM_PP_UP),
    ItemId(ITEM_RARE_CANDY),
    ItemId(ITEM_SIL_PIERRE_LIEN),
    ItemId(ITEM_X_ATTACK),
    ItemId(ITEM_X_DEFEND),
];

/// The corrupt grunt: his welcome, then his shop.
pub async fn Owlys_MarcheGris_EventScript_Vendeur(ctx: Ctx) {
    msgbox(&ctx, Owlys_Marche_Vendeur_Intro, MsgBox::Npc).await;
    // fallback: mart (no shop screen before M9): greeting and goodbye, nothing sold.
    mart(&ctx, MARCHE_STOCK).await;
}

pub async fn Owlys_MarcheGris_EventScript_Guetteur(ctx: Ctx) {
    msgbox(&ctx, Owlys_Marche_Guetteur, MsgBox::Npc).await;
}

pub async fn Owlys_MarcheGris_EventScript_Cliente(ctx: Ctx) {
    msgbox(&ctx, Owlys_Marche_Cliente, MsgBox::Npc).await;
}

// ---- the orphanage ----

pub async fn Owlys_Orphelinat_EventScript_Directrice(ctx: Ctx) {
    msgbox(&ctx, Owlys_Orphelinat_Directrice, MsgBox::Npc).await;
}

pub async fn Owlys_Orphelinat_EventScript_Enfant1(ctx: Ctx) {
    msgbox(&ctx, Owlys_Orphelinat_Enfant1, MsgBox::Npc).await;
}

pub async fn Owlys_Orphelinat_EventScript_Enfant2(ctx: Ctx) {
    msgbox(&ctx, Owlys_Orphelinat_Enfant2, MsgBox::Npc).await;
}

pub async fn Owlys_Orphelinat_EventScript_Enfant3(ctx: Ctx) {
    msgbox(&ctx, Owlys_Orphelinat_Enfant3, MsgBox::Npc).await;
}

/// The children's owl drawings. The source gives them no text: the first child's ("I'll be
/// the Iron Owl!") stands in, as a sign.
pub async fn Owlys_Orphelinat_EventScript_Dessins(ctx: Ctx) {
    msgbox(&ctx, Owlys_Orphelinat_Enfant1, MsgBox::Sign).await;
}
