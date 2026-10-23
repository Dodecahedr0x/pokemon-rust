//! Chapter 2, Gris-Échec (`GrisEchec`), its Centre and Mart, and the disused factory
//! (`GrisEchec_Usine`): `contenu-jeu/chapitres/02-gris-echec.md` §2 (the embargo's NPCs, signs,
//! the factory), §3 Gris_Echec_Sc01 to Sc06 (Sc04 is Tour_Sc01, in `ch02_tour.rs`), §5 (the
//! jammers' sbires) and §7 (the items). `VAR_SIL_STORY` runs 22 (in town) to 28 (the Gamma
//! barrier open): 23 the embargo, 24 the three jammers cut, 25 the Carte Perforée, 26–27 the
//! arena (`ch02_gym.rs`), 28 Kimiko's stream. `VAR_SIL_GRIS_ECHEC_STATE` counts the jammers.

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &[
    "GrisEchec",
    "GrisEchec_PokemonCenter",
    "GrisEchec_Mart",
    "GrisEchec_Usine",
];

// ---- the town's cells ----

/// The Tour d'Observation's door (21–22, 4): Rustboro's door metatile (the stamp's), and the
/// wall above it standing in for the source's "crackling", shut door.
const TOUR_DOOR: [(i16, i16); 2] = [(21, 4), (22, 4)];
const METATILE_TOUR_DOOR: u16 = 693;
const METATILE_TOUR_SHUT: u16 = 685;
/// The Sector Gamma barrier: the source's (37, 1) is (39, 1) on the built town, a fence cell
/// (764); open, the street's paving (699).
const BARRIER: (i16, i16) = (39, 1);
const METATILE_BARRIERE_OUVERTE: u16 = 699;

// ---- map scripts ----

/// Object visibility by story step ("SIL2 Ch2–Ch3 maps as built"): Gwada waits at the arena at
/// `VAR_SIL_STORY` 22–27, the Gamma barrier's guard stands until 28, Lainergie shows at 27. The
/// arena guard and his stepped-aside copy are Gris_Echec_Sc05's. The source's white noise on
/// the music during the embargo has no engine (no music selection by script yet).
pub async fn GrisEchec_OnTransition(ctx: Ctx) {
    let s = story(&ctx).await;
    ctx.set_flag(FLAG_SIL_HIDE_GRIS_GWADA, !(22..=27).contains(&s))
        .await;
    ctx.set_flag(FLAG_SIL_HIDE_GRIS_GARDE_S3, s >= 28).await;
    ctx.set_flag(FLAG_SIL_HIDE_GRIS_LAINERGIE, s != 27).await;
}

/// The Tour door shut until `FLAG_SIL_TOUR_OPEN`; the Gamma barrier open from
/// `VAR_SIL_STORY` 28. The jammers keep their graphic once cut (see [`jammer`]).
pub async fn GrisEchec_OnLoad(ctx: Ctx) {
    if !ctx.flag(FLAG_SIL_TOUR_OPEN).await {
        for (x, y) in TOUR_DOOR {
            ctx.set_metatile(x, y, METATILE_TOUR_SHUT, true).await;
        }
    }
    if story(&ctx).await >= 28 {
        let (x, y) = BARRIER;
        ctx.set_metatile(x, y, METATILE_BARRIERE_OUVERTE, false)
            .await;
    }
}

// ---- scenes ----

/// Gris_Echec_Sc01, the white-noise embargo: the west entrance (x 1, y 10–12) at
/// `VAR_SIL_STORY` 22. The Mallette finds the three jammers. Sets 23 and
/// `VAR_SIL_GRIS_ECHEC_STATE` 0.
pub async fn GrisEchec_Sc01(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Gris_Echec_Sc01_T1).await;
    // fallback: play_se (audio, M12): a no-op that reports; SE_RING is the decomp's
    // SE_POKENAV_CALL.
    play_se(&ctx, "SE_POKENAV_CALL").await;
    say(&ctx, Gris_Echec_Sc01_T2).await;
    set_story(&ctx, 23).await;
    ctx.set_var(VAR_SIL_GRIS_ECHEC_STATE, 0).await;
    ctx.release_all().await;
}

/// Gris_Echec_Sc02, a jammer (J1 the roof, J2 the plaza bin, J3 the factory): Lame-Fréquence
/// on yes; `n`'s flag, one more in `VAR_SIL_GRIS_ECHEC_STATE`; the third one sets
/// `VAR_SIL_STORY` 24. Nothing once cut.
async fn jammer(ctx: &Ctx, cut: FlagId) {
    if ctx.flag(cut).await {
        return;
    }
    ctx.lock().await;
    if !ask(ctx, Gris_Echec_Sc02_T1).await {
        ctx.release().await;
        return;
    }
    say(ctx, Gris_Echec_Sc02_T2).await;
    ctx.set_flag(cut, true).await;
    // fallback: METATILE_BROUILLEUR_HS (no switched-off jammer graphic in General + Rustboro
    // or BikeShop, "SIL2 Ch2–Ch3 maps as built"): the jammer cell keeps its graphic.
    fallback(ctx, "jammer graphic").await;
    let n = ctx.var(VAR_SIL_GRIS_ECHEC_STATE).await + 1;
    ctx.set_var(VAR_SIL_GRIS_ECHEC_STATE, n).await;
    if n < 3 {
        say(ctx, Gris_Echec_Sc02_T3).await;
    } else {
        say(ctx, Gris_Echec_Sc02_T4).await;
        set_story(ctx, 24).await;
    }
    ctx.release().await;
}

/// J1, on the Mart annex roof (17, 4).
pub async fn GrisEchec_EventScript_Brouilleur1(ctx: Ctx) {
    jammer(&ctx, FLAG_SIL_GRIS_JAMMER_1).await;
}

/// J2, in the plaza bin (20, 14): the bin's line, the disguised sweeper's battle first while
/// he is undefeated (the bin can be reached from every side, out of his sight), then the
/// jammer.
pub async fn GrisEchec_EventScript_Brouilleur2(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_GRIS_JAMMER_2).await {
        return;
    }
    msgbox(&ctx, Gris_Echec_Poubelle, MsgBox::Sign).await;
    if !defeated(&ctx, TRAINER_SIL_SBIRE_BROUILLEUR_2).await {
        GrisEchec_EventScript_Balayeur(ctx.clone()).await;
        if !defeated(&ctx, TRAINER_SIL_SBIRE_BROUILLEUR_2).await {
            return;
        }
    }
    jammer(&ctx, FLAG_SIL_GRIS_JAMMER_2).await;
}

/// The cards the Tour door opens with ([`GrisEchec_Sc03`]).
async fn open_tour(ctx: &Ctx) {
    ctx.set_flag(FLAG_SIL_TOUR_OPEN, true).await;
    for (x, y) in TOUR_DOOR {
        ctx.set_metatile(x, y, METATILE_TOUR_DOOR, false).await;
    }
    ctx.redraw_map().await;
}

/// Gris_Echec_Sc03, the arena guard (34, 10) (and his stepped-aside copy (35, 10)): white noise
/// during the embargo; without the Carte Perforée, where it is and Gwada's debate (the Tour
/// door stops crackling, `FLAG_SIL_TOUR_OPEN`); with it, Gris_Echec_Sc05.
pub async fn GrisEchec_Sc03(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if story(&ctx).await <= 23 {
        say(&ctx, Gris_Echec_Npc_Bruit).await;
        ctx.release().await;
        return;
    }
    if !ctx.flag(FLAG_SIL_CARTE_PERFOREE).await {
        say(&ctx, Gris_Echec_Sc03_T1).await;
        say(&ctx, Gris_Echec_Sc03_T2).await;
        if !ctx.flag(FLAG_SIL_TOUR_OPEN).await {
            open_tour(&ctx).await;
        }
    } else if ctx.flag(FLAG_SIL_HIDE_GRIS_GARDE_ECARTE).await {
        GrisEchec_Sc05(ctx.clone()).await;
    } else {
        // Already aside: his authentication line again.
        say(&ctx, Gris_Echec_Sc05_T1).await;
    }
    ctx.release().await;
}

/// The guard steps right and faces left; the player first leaves (35, 10) if standing there.
const GARDE_ASIDE: &[Step] = &[walk(Dir::Right), face(Dir::Left)];
const PLAYER_OFF_ASIDE: &[Step] = &[walk(Dir::Down), face(Dir::Up)];

/// Gris_Echec_Sc05, the arena opens (called by Sc03 with the card, inside its `lock`): the
/// guard steps aside, then the stepped-aside copy replaces him ("maps as built": keyed on a
/// flag, so a reload keeps the door shut until the card is shown); Gwada waits outside.
async fn GrisEchec_Sc05(ctx: Ctx) {
    say(&ctx, Gris_Echec_Sc05_T1).await;
    if ctx.player_pos().await == (35, 10) {
        move_wait(&ctx, PLAYER, PLAYER_OFF_ASIDE).await;
    }
    move_wait(&ctx, LOCALID_GRIS_GARDE, GARDE_ASIDE).await;
    hide(&ctx, LOCALID_GRIS_GARDE).await;
    show(&ctx, LOCALID_GRIS_GARDE_ECARTE, FLAG_SIL_HIDE_GRIS_GARDE_ECARTE).await;
    say(&ctx, Gris_Echec_Sc05_T2).await;
}

/// Gwada and Lainergie off west (the source's `Walk(Left) x8`, both together: the follower
/// fallback).
const DUO_OUT: &[Step] = &[
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
];

/// Gris_Echec_Sc06, Kimiko's stream: the first step out of the arena, (34, 11), at
/// `VAR_SIL_STORY` 27. Benou opens the Gamma barrier; Gwada and Lainergie leave. Sets 28.
pub async fn GrisEchec_Sc06(ctx: Ctx) {
    ctx.lock_all().await;
    // The source's play_music(MUS_KIMIKO_STREAM): no music selection by script yet.
    say(&ctx, Gris_Echec_Sc06_T1).await;
    say(&ctx, Gris_Echec_Sc06_T2).await;
    say(&ctx, Gris_Echec_Sc06_T3).await;
    say(&ctx, Gris_Echec_Sc06_T4).await;
    say(&ctx, Gris_Echec_Sc06_T5).await;
    // fallback: mallette_call / end_call (the call window, plan §1.1 P1): plain messages.
    mallette_call(&ctx, "BENOU").await;
    say(&ctx, Gris_Echec_Sc06_T6).await;
    end_call(&ctx).await;
    hide(&ctx, LOCALID_GRIS_GARDE_S3).await;
    let (x, y) = BARRIER;
    ctx.set_metatile(x, y, METATILE_BARRIERE_OUVERTE, false)
        .await;
    ctx.redraw_map().await;
    // fallback: the follower Pokémon (Lainergie behind Gwada, §9): two objects moved together.
    fallback(&ctx, "follower").await;
    ctx.move_obj(LOCALID_GRIS_GWADA, DUO_OUT).await;
    move_wait(&ctx, LOCALID_GRIS_LAINERGIE, DUO_OUT).await;
    ctx.wait_move(LOCALID_GRIS_GWADA).await;
    hide(&ctx, LOCALID_GRIS_GWADA).await;
    hide(&ctx, LOCALID_GRIS_LAINERGIE).await;
    set_story(&ctx, 28).await;
    ctx.release_all().await;
}

// ---- trainers (§5; sight from the map's trainer objects) ----

/// The disguised sweeper by the plaza bin (J2's guard).
pub async fn GrisEchec_EventScript_Balayeur(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GRIS_BALAYEUR,
        TRAINER_SIL_SBIRE_BROUILLEUR_2,
        Gris_Echec_Balayeur_Intro,
        Gris_Echec_Balayeur_Defeat,
    )
    .await
    {
        after_battle(&ctx, Gris_Echec_Balayeur_Defeat).await;
    }
}

/// The roof sbire (J1's guard), who sees the ladder top.
pub async fn GrisEchec_EventScript_SbireToit(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GRIS_SBIRE_TOIT,
        TRAINER_SIL_SBIRE_BROUILLEUR_1,
        Gris_Echec_Toit_Intro,
        Gris_Echec_Toit_Defeat,
    )
    .await
    {
        after_battle(&ctx, Gris_Echec_Toit_Defeat).await;
    }
}

/// A beaten trainer talked to again: the defeat text (the source has no post-battle lines).
async fn after_battle(ctx: &Ctx, text: TextId) {
    msgbox(ctx, text, MsgBox::Autoclose).await;
}

// ---- the townsfolk (the embargo) ----

/// A townsperson: white noise until the jammer that gives them their voice back is cut (§2:
/// one third of the town per jammer, read from `VAR_SIL_GRIS_ECHEC_STATE`: the n-th cut jammer
/// frees the n-th third), then their line.
async fn townsperson(ctx: &Ctx, third: u16, line: TextId) {
    let free = story(ctx).await >= 24 || ctx.var(VAR_SIL_GRIS_ECHEC_STATE).await >= third;
    let text = if free { line } else { Gris_Echec_Npc_Bruit };
    msgbox(ctx, text, MsgBox::Npc).await;
}

pub async fn GrisEchec_EventScript_Echecs(ctx: Ctx) {
    townsperson(&ctx, 1, Gris_Echec_Npc_Echecs).await;
}

pub async fn GrisEchec_EventScript_Pressee(ctx: Ctx) {
    townsperson(&ctx, 1, Gris_Echec_Npc_Pressee).await;
}

pub async fn GrisEchec_EventScript_Reparateur(ctx: Ctx) {
    townsperson(&ctx, 2, Gris_Echec_Npc_Reparateur).await;
}

pub async fn GrisEchec_EventScript_Etudiante(ctx: Ctx) {
    townsperson(&ctx, 2, Gris_Echec_Npc_Etudiante).await;
}

pub async fn GrisEchec_EventScript_Ouvrier(ctx: Ctx) {
    townsperson(&ctx, 3, Gris_Echec_Npc_Ouvrier).await;
}

pub async fn GrisEchec_EventScript_Enfant(ctx: Ctx) {
    townsperson(&ctx, 3, Gris_Echec_Npc_Enfant).await;
}

/// Gwada at the arena door (22–27): his debate with the guard before the card, "I wait here"
/// with it, and before he leaves after the badge, Gris_Echec_Sc06_T7.
pub async fn GrisEchec_EventScript_Gwada(ctx: Ctx) {
    let text = match story(&ctx).await {
        ..=24 => Gris_Echec_Sc03_T2,
        25 | 26 => Gris_Echec_Sc05_T2,
        _ => Gris_Echec_Sc06_T7,
    };
    msgbox(&ctx, text, MsgBox::Npc).await;
}

/// Lainergie beside Gwada (27): the source gives it no line; Gwada's Sc06_T7 stands in.
pub async fn GrisEchec_EventScript_Lainergie(ctx: Ctx) {
    msgbox(&ctx, Gris_Echec_Sc06_T7, MsgBox::Npc).await;
}

/// The Gamma barrier's guard (38, 1), until Sc06.
pub async fn GrisEchec_EventScript_GardeS3(ctx: Ctx) {
    msgbox(&ctx, Gris_Echec_Npc_Garde_S3, MsgBox::Npc).await;
}

// ---- signs and items ----

pub async fn GrisEchec_EventScript_SignVille(ctx: Ctx) {
    msgbox(&ctx, Gris_Echec_Sign_Ville, MsgBox::Sign).await;
}

pub async fn GrisEchec_EventScript_SignArene(ctx: Ctx) {
    msgbox(&ctx, Gris_Echec_Sign_Arene, MsgBox::Sign).await;
}

pub async fn GrisEchec_EventScript_SignTour(ctx: Ctx) {
    msgbox(&ctx, Gris_Echec_Sign_Tour, MsgBox::Sign).await;
}

pub async fn GrisEchec_EventScript_SignUsine(ctx: Ctx) {
    msgbox(&ctx, Gris_Echec_Sign_Usine, MsgBox::Sign).await;
}

/// The Iron hidden behind the Centre (6, 4), once (`FLAG_SIL_ITEM_GRIS_ECHEC_IRON`).
pub async fn GrisEchec_EventScript_HiddenIron(ctx: Ctx) {
    // fallback: hidden items (the engine ignores `BgKind::HiddenItem`; the M9 finding of
    // "SIL1 Task F as built"): the map has a sign here instead.
    hidden_item(&ctx, FLAG_SIL_ITEM_GRIS_ECHEC_IRON, ITEM_IRON).await;
}

// ---- the Centre ----

/// The Centre is Gris-Échec's heal point (`HEAL_LOCATION_SIL_GRIS_ECHEC`, `game.ron`).
pub async fn GrisEchec_PokemonCenter_OnTransition(ctx: Ctx) {
    ctx.set_respawn(HEAL_LOCATION_SIL_GRIS_ECHEC).await;
}

pub async fn GrisEchec_PokemonCenter_EventScript_Nurse(ctx: Ctx) {
    pokecenter_nurse(&ctx).await;
}

pub async fn GrisEchec_PokemonCenter_EventScript_Npc(ctx: Ctx) {
    msgbox(&ctx, Gris_Echec_PC_Npc, MsgBox::Npc).await;
}

// ---- the Mart ----

/// The source's stock (§2, the Mart). `items.ron` has no Gris-Échec mart yet (an import gap);
/// the list lives here until M9's shop reads it.
const MART_STOCK: &[ItemId] = &[
    ItemId(ITEM_POKE_BALL),
    ItemId(ITEM_GREAT_BALL),
    ItemId(ITEM_POTION),
    ItemId(ITEM_SUPER_POTION),
    ItemId(ITEM_ANTIDOTE),
    ItemId(ITEM_PARALYZE_HEAL),
    ItemId(ITEM_AWAKENING),
    ItemId(ITEM_REPEL),
    ItemId(ITEM_SUPER_REPEL),
];

/// The drinks machine's stock (§2).
const DISTRIBUTEUR_STOCK: &[ItemId] = &[
    ItemId(ITEM_FRESH_WATER),
    ItemId(ITEM_SODA_POP),
    ItemId(ITEM_LEMONADE),
];

/// The clerk.
pub async fn GrisEchec_Mart_EventScript_Clerk(ctx: Ctx) {
    // fallback: mart (no shop screen before M9): greeting and goodbye, nothing sold.
    mart(&ctx, MART_STOCK).await;
}

pub async fn GrisEchec_Mart_EventScript_Npc(ctx: Ctx) {
    msgbox(&ctx, Gris_Echec_Mart_Npc, MsgBox::Npc).await;
}

/// The drinks machine: its line, then its own shop (Fresh Water, Soda Pop, Lemonade).
pub async fn GrisEchec_Mart_EventScript_Distributeur(ctx: Ctx) {
    msgbox(&ctx, Gris_Echec_Distributeur, MsgBox::Sign).await;
    // fallback: mart (no shop screen before M9): the line only, nothing sold.
    fallback(&ctx, "mart").await;
    ctx.log(if DISTRIBUTEUR_STOCK.is_empty() {
        "mart: empty stock"
    } else {
        "mart: stock not sold yet"
    })
    .await;
}

// ---- the factory (`GrisEchec_Usine`) ----

/// The factory's floor (BikeShop 528), walkable: the cut cable and the moved crates' cells.
const FLOOR: u16 = 528;

/// The security cable (25, 4) in front of J3: Lame-Fréquence, with Route S2's cable texts.
/// The source gives it no flag; `FLAG_SIL_USINE_CABLE_CUT` (script-only) keeps it cut.
const USINE_CABLE: Obstacle = Obstacle {
    module: FLAG_SIL_MODULE_COUPE,
    done: FLAG_SIL_USINE_CABLE_CUT,
    paint: &[(25, 4, FLOOR, false)],
    blocked: Route_S02_Cable_T1,
    ask: Some(Route_S02_Cable_T1),
    cleared: Route_S02_Cable_T2,
};

/// The metal crates (12–13, 2–4) closing the reserve: Champ Magnétique (after the badge).
// fallback: the Strength push (M10 field moves; the source's §9 "set_metatile to move the
// block one cell + flag"): the middle row (12–13, 3) opens. The texts are Chapter 3's Depot
// block lines, the source's only Champ Magnétique obstacle texts.
const CAISSES: Obstacle = Obstacle {
    module: FLAG_SIL_MODULE_FORCE,
    done: FLAG_SIL_USINE_CRATES_MOVED,
    paint: &[(12, 3, FLOOR, false), (13, 3, FLOOR, false)],
    blocked: Depot_Bloc_T1,
    ask: None,
    cleared: Depot_Bloc_T2,
};

/// The cut cable (also once J3 is cut) and the moved crates, redrawn on every load.
pub async fn GrisEchec_Usine_OnLoad(ctx: Ctx) {
    repaint_obstacle(&ctx, &USINE_CABLE).await;
    if ctx.flag(FLAG_SIL_GRIS_JAMMER_3).await {
        ctx.set_metatile(25, 4, FLOOR, false).await;
    }
    repaint_obstacle(&ctx, &CAISSES).await;
}

/// J3's sbire, watching the only cell in front of the cable.
pub async fn GrisEchec_Usine_EventScript_Sbire(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_USINE_SBIRE,
        TRAINER_SIL_SBIRE_BROUILLEUR_3,
        Usine_Sbire_Intro,
        Usine_Sbire_Defeat,
    )
    .await
    {
        after_battle(&ctx, Usine_Sbire_Defeat).await;
    }
}

pub async fn GrisEchec_Usine_EventScript_Cable(ctx: Ctx) {
    module_obstacle(&ctx, &USINE_CABLE).await;
}

/// J3 (26, 4), read from the cut cable's cell.
pub async fn GrisEchec_Usine_EventScript_Brouilleur3(ctx: Ctx) {
    jammer(&ctx, FLAG_SIL_GRIS_JAMMER_3).await;
}

pub async fn GrisEchec_Usine_EventScript_Caisses(ctx: Ctx) {
    if module_obstacle(&ctx, &CAISSES).await {
        fallback(&ctx, "strength").await;
    }
}

/// The Ether by the generators (27, 8).
pub async fn GrisEchec_Usine_EventScript_ItemEther(ctx: Ctx) {
    find_item(&ctx, ITEM_ETHER, 1).await;
}

/// The Metal Coat in the reserve (3, 4), behind the crates.
pub async fn GrisEchec_Usine_EventScript_ItemMetalCoat(ctx: Ctx) {
    find_item(&ctx, ITEM_METAL_COAT, 1).await;
}

/// The Super Repel (14, 18).
pub async fn GrisEchec_Usine_EventScript_ItemSuperRepel(ctx: Ctx) {
    find_item(&ctx, ITEM_SUPER_REPEL, 1).await;
}
