//! Chapter 3, Viber-City, from Chapter 2's end (`VAR_SIL_STORY` 28, the Gamma barrier open) to
//! Tauber's goodbye and the east barrier open (36):
//! - Route S3: the Goulet's entry (Route_S03_Sc01), the sbire at the depot's south portal;
//! - the depot: the server blocks (the Champ Magnétique, `strength` fallback: one cell a press),
//!   Gogo & Romain (Depot_Sc02), the Metal Coat and the Great Ball, out by the north portal;
//! - Viber-City: Mathieu's stand (Sc01), his heal, the giant neon (Sc02), the Center, the
//!   Nocturne's door (Sc03), PRAUGECT's diversion (Sc04);
//! - the Station Lumière: the tubes, the notice, the foreman (Station_Lumiere_Sc01), the fuse
//!   puzzle (levers 2 then 4: all three tubes dark), the black-out (Sc03, Viber_City_Sc05);
//! - the Casino (optional, played): Gogo & Romain (Casino_Sc03), the coin case, the poster's
//!   switch (Sc08), the basement: the terminal, the head croupier, the Porygon (B1F_Sc02);
//! - the Nocturne: the three tracks' consoles, Léna and Éliott, Tauber & Hocseat (Nocturne_Sc02:
//!   the Tempo badge, the Surcharge Optique, `LOG_03`, the Link Stone), the manager's Fresh Water;
//! - out: Tauber's fear and Julien's call (Viber_City_Sc06), the barrier to Route S4 open.
//!
//! **Two ways in**, as Chapter 2: chained, `ch00`, `ch01`, `ch02` then [`from_ch02`] (the Usine
//! to Route S3's south edge, by Gris-Échec's Centre); or [`start`], a new game moved to Route
//! S3's south edge (9, 49) with Chapter 2's end state set directly.
//!
//! **The party.** Chapter 2's: the trained QUILAVA (26), MAREEP (12) and the hooked ARON (30)
//! (`ch02::THIRD`). The run trains the lead in Viber-City's east tall grass (the garden, wild
//! levels 21–26 of §6) to [`STATION_LEVEL`] after arriving, and to [`GYM_LEVEL`] before the
//! Nocturne, healing at the Center; the Casino's Porygon joins on the way. Every battle is won
//! by the battle policy (`silice_spine` docs). One test hook, for the leaders: [`FIFTH`].

use super::{Hook, Run, Start, Step, goto, goto_avoiding};
use content::consts::*;
use engine::data::{Dir, ItemId, MapId};
use engine::script::Ctx;

const ROUTE: MapId = MAP_ROUTE_S03;
const DEPOT: MapId = MAP_ROUTE_S03_DEPOT;
const CITY: MapId = MAP_VIBER_CITY;
const CENTER: MapId = MAP_VIBER_CITY_POKEMON_CENTER;
const STATION: MapId = MAP_VIBER_CITY_STATION_LUMIERE;
const GYM: MapId = MAP_VIBER_CITY_GYM;
const CASINO: MapId = MAP_VIBER_CITY_CASINO;
const B1F: MapId = MAP_VIBER_CITY_CASINO_B1F;

/// The lead's level before the Station Lumière (its trainers are 26–29, §5).
/// Lower loses (re-measured with the learn policy: 32 and 30 lose a battle on the chained run).
pub const STATION_LEVEL: u8 = 34;

/// The lead's level before the Nocturne (Tauber & Hocseat's six are 28–34, Raikou the ace, §4).
/// 38 rather than 36 since the learn policy: at 36 (37 on the start state) the Dugtrio of
/// [`FIFTH`] meets Raikou too late and the run loses; 38, 39 and 40 win on both runs.
pub const GYM_LEVEL: u8 = 38;

/// The party's fifth Pokémon, from the leaders' battle on (a test hook, the chapter's one): a
/// DUGTRIO (Route S2's wild DIGLETT, Chapter 2's list) caught and evolved, at 26 (its evolution
/// level: the least Dugtrio there is). **The reason:** the leaders' ace RAIKOU (34) sets Rain
/// Dance and Calm Mind, then Thunder never misses; the run's party has no Ground type (Chapter
/// 3's wild lists have none, §6), and the starter's Fire moves are halved by the rain.
/// Measured with the learn policy (the trained TYPHLOSION knows Flame Wheel and Ember), without
/// the hook: lost to Raikou at a lead of 36 (chained run won, start state lost), 38 (both
/// lost), 40 (start state won, chained lost) and 42 (start state lost): Raikou keeps 6–74 HP
/// of 113. With the hook: DUGTRIO 26 to 32 win at a lead of 38; a DIGLETT 25 loses (chained);
/// a DUGTRIO 30 or 32 at a lead of 36 loses (chained). Before the learn policy the hook was a
/// DUGTRIO 32 at a lead of 36.
pub const FIFTH: Hook = Hook::MonBefore("ViberCity_Gym_Sc02", SPECIES_DUGTRIO, 26);

/// Chapter 3's start: Route S3's south edge (9, 49), facing north.
pub fn start() -> Start {
    Start {
        map: ROUTE,
        x: 9,
        y: 49,
        facing: Dir::Up,
        setup: |ctx| Box::pin(end_of_chapter_2(ctx)),
    }
}

/// Chapter 2's end state, on top of Silice's new-game script: Chapter 1's (as `ch02::start`),
/// then the story at 28 (the Gamma barrier open), the sync at 20, the second badge, the Champ
/// Magnétique, Lainergie, `LOG_02`, Chapter 2's bag, the respawn at Gris-Échec's Centre, and
/// Chapter 2's end party: the trained QUILAVA at `ch02::TOUR_LEVEL`, MAREEP 12 and the hooked
/// ARON 30. As in `ch02::start`, `give_mon` fills the level-up moveset (Quick Attack where the
/// trained one keeps Tackle), and the money is the chained run's at Chapter 2's end, rounded.
async fn end_of_chapter_2(ctx: Ctx) {
    for f in [
        // Chapter 0.
        FLAG_SIL_INTRO_DONE,
        FLAG_SIL_HAS_MALLETTE,
        FLAG_SIL_HIDE_BAR_MALLETTE,
        FLAG_SIL_MALLETTE_TRIED,
        FLAG_SIL_MALLETTE_OPEN,
        FLAG_SIL_MALLETTE_MAP,
        FLAG_SIL_MALLETTE_DEX,
        FLAG_SIL_MALLETTE_CALLS,
        FLAG_SIL_RUNNING_SHOES,
        FLAG_SIL_BRICO_CHASE,
        FLAG_SIL_S01_SIGNAL_SEEN,
        // Chapter 1.
        FLAG_SIL_BADGE_01,
        FLAG_SIL_MODULE_COUPE,
        FLAG_SIL_LOG_01,
        FLAG_SIL_BOIS_ETHER_AMBUSH,
        FLAG_SIL_LABO_HINT,
        // Chapter 2.
        FLAG_SIL_BADGE_02,
        FLAG_SIL_MODULE_FORCE,
        FLAG_SIL_GWADA_LAINERGIE,
        FLAG_SIL_LOG_02,
        FLAG_SIL_USINE_CRATES_MOVED,
        FLAG_SIL_HIDE_GRIS_GARDE_S3,
        FLAG_SIL_HIDE_GRIS_GWADA,
        FLAG_SIL_HIDE_GRIS_LAINERGIE,
    ] {
        ctx.set_flag(f, true).await;
    }
    ctx.set_var(VAR_SIL_STORY, 28).await;
    ctx.set_var(VAR_SIL_SYNC, 20).await;
    ctx.set_var(VAR_SIL_STARTER, 1).await;
    ctx.set_var(VAR_SIL_CH0_GATE, 1).await;
    for (item, n) in [
        (ITEM_SIL_MALLETTE, 1),
        (ITEM_REVIVE, 1),
        (ITEM_TM34, 1),
        (ITEM_PARALYZE_HEAL, 3),
        (ITEM_SIL_CARTE_PERFOREE, 1),
        (ITEM_TM23, 1),
        (ITEM_SOOTHE_BELL, 1),
        (ITEM_METAL_COAT, 1),
    ] {
        ctx.item_add(ItemId(item), n).await;
    }
    ctx.money_add(START_MONEY).await;
    ctx.set_respawn(HEAL_LOCATION_SIL_GRIS_ECHEC).await;
    ctx.give_mon(SPECIES_QUILAVA, super::ch02::TOUR_LEVEL).await;
    ctx.give_mon(SPECIES_MAREEP, 12).await;
    ctx.give_mon(SPECIES_ARON, 30).await;
}

/// The start state's money: about the chained run's at Chapter 2's end.
const START_MONEY: u32 = 8_000;

/// From Chapter 2's last step (the Usine's Metal Coat, (3, 3)) to Chapter 3's start: out of the
/// Usine, a heal at Gris-Échec's Centre, north through the open Gamma barrier (38, 0) onto Route
/// S3's south edge (9, 49), where [`start`] puts a run that does not play the chapters before.
pub fn from_ch02() -> Vec<Step> {
    use Dir::*;
    const USINE: MapId = MAP_GRIS_ECHEC_USINE;
    const TOWN: MapId = MAP_GRIS_ECHEC;
    const GRIS_CENTER: MapId = MAP_GRIS_ECHEC_POKEMON_CENTER;
    vec![
        goto_avoiding(USINE, 14, 22, &[(24, 5), (25, 5)]),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        goto(TOWN, 5, 9),
        Step::Enter {
            dir: Up,
            map: GRIS_CENTER,
        },
        goto(GRIS_CENTER, 7, 4),
        Step::Interact(Up),
        goto(GRIS_CENTER, 6, 7),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        goto(TOWN, 38, 0),
        Step::Enter {
            dir: Up,
            map: ROUTE,
        },
    ]
}

/// Route S3's trainers' sight lines (Roger, Sacha, Marco, Inès, Noé), avoided: the chapter's
/// main path fights only the sbire at the depot's portal, the one it cannot get round.
const ROUTE_SIGHT: &[(i16, i16)] = &[
    (13, 35),
    (14, 35),
    (15, 35),
    (16, 35),
    (9, 31),
    (10, 31),
    (11, 31),
    (12, 31),
    (10, 27),
    (11, 27),
    (12, 27),
    (6, 24),
    (7, 24),
    (8, 24),
    (9, 24),
    (8, 11),
    (8, 12),
    (8, 13),
];

/// The Station's trainers' sight lines (sbire 1 (2, 9) right, sbire 2 (3, 5) right, Karine
/// (13, 6) down), avoided: the foreman is the Station's story battle.
const STATION_SIGHT: &[(i16, i16)] = &[
    (3, 9),
    (4, 9),
    (5, 9),
    (6, 9),
    (4, 5),
    (5, 5),
    (6, 5),
    (13, 7),
];

/// The Casino's trainers' sight lines (Fernand (9, 4) down, Gaston (11, 7) left, Solène (13, 9)
/// left), avoided.
const CASINO_SIGHT: &[(i16, i16)] = &[(9, 5), (9, 7), (10, 7), (10, 9), (11, 9), (12, 9)];

/// The basement sbires' sight lines (1 (3, 3) right, 2 (11, 6) left, 3 (10, 2) down), avoided.
const B1F_SIGHT: &[(i16, i16)] = &[
    (4, 3),
    (5, 3),
    (6, 3),
    (8, 6),
    (9, 6),
    (10, 6),
    (10, 3),
    (10, 4),
];

/// Hugo's sight in the Nocturne's zone A ((10, 14), left).
const HUGO_SIGHT: &[(i16, i16)] = &[(7, 14), (8, 14), (9, 14)];

/// The Center's nurse, from the city and back.
const HEAL: &[Step] = &[
    goto(CITY, 6, 15),
    Step::Enter {
        dir: Dir::Up,
        map: CENTER,
    },
    goto(CENTER, 7, 4),
    Step::Interact(Dir::Up),
    goto(CENTER, 6, 7),
    Step::Enter {
        dir: Dir::Down,
        map: CITY,
    },
];

/// Two tall-grass cells of the city's east garden (x 26–30, y 20–22), paced by [`Step::Train`].
const GRASS: ((i16, i16), (i16, i16)) = ((27, 21), (27, 22));

const fn train(level: u8) -> Step {
    Step::Train {
        a: GRASS.0,
        b: GRASS.1,
        level,
        heal: HEAL,
    }
}

/// The depot's blocks, from the south portal (7, 13) to the north one (7, 0): A1 up the
/// corridor (Depot_Sc02 on (7, 10) on the way), the rest area's items, A1 aside to (8, 6), B1
/// up into the dock (7, 3), B2 aside to (8, 2).
fn depot() -> Vec<Step> {
    use Dir::*;
    vec![
        Step::Walk(Up),
        // The magnet on, then A1 up two cells.
        Step::Interact(Up),
        Step::Check("depot_magnet", |r| {
            assert!(r.has_text(Depot_Bloc_T2));
            assert!(r.w.flag(FLAG_SIL_DEPOT_MAGNET_ON));
        }),
        Step::Interact(Up),
        Step::Walk(Up),
        Step::Interact(Up),
        Step::Walk(Up),
        // ---- Gogo & Romain (Depot_Sc02) on (7, 10) ----
        Step::Check("depot_sc02", |r| {
            assert_eq!(r.pos(), (7, 10));
            assert_eq!(r.w.var(VAR_SIL_STORY), 31);
            assert!(r.beaten(TRAINER_SIL_GOGO_2));
            assert!(r.beaten(TRAINER_SIL_ROMAIN_2));
            assert!(r.w.flag(FLAG_SIL_HIDE_DEPOT_GOGO_ROMAIN));
            assert!(r.w.flag(FLAG_SIL_HIDE_DEPOT_SBIRE));
            assert!(!r.w.flag(FLAG_SIL_ITEM_DEPOT_METAL_COAT));
            r.printed_in_order(&[
                Depot_Sc02_T1,
                Depot_Sc02_T3,
                Depot_Sc02_Defaite_Gogo,
                Depot_Sc02_Defaite_Romain,
                Depot_Sc02_T4,
                Depot_Sc02_T6,
            ]);
        }),
        // A1 on up to (7, 6).
        Step::Interact(Up),
        Step::Walk(Up),
        Step::Interact(Up),
        Step::Walk(Up),
        Step::Interact(Up),
        Step::Walk(Up),
        Step::Check("depot_a1", |r| assert_eq!(r.pos(), (7, 7))),
        // The rest area's items.
        goto(DEPOT, 4, 6),
        Step::Interact(Left),
        goto(DEPOT, 10, 8),
        Step::Interact(Right),
        Step::Check("depot_items", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_DEPOT_METAL_COAT));
            assert!(r.w.flag(FLAG_SIL_ITEM_DEPOT_GREAT_BALL));
            assert!(r.count(ITEM_GREAT_BALL) >= 1);
        }),
        // A1 aside, B1 up, B2 aside.
        goto(DEPOT, 6, 6),
        Step::Interact(Right),
        goto(DEPOT, 7, 5),
        Step::Interact(Up),
        goto(DEPOT, 6, 2),
        Step::Interact(Right),
        goto(DEPOT, 7, 1),
        Step::Check("depot_solved", |r| assert_eq!(r.pos(), (7, 1))),
        Step::Enter {
            dir: Up,
            map: ROUTE,
        },
    ]
}

/// Chapter 3's steps.
pub fn steps() -> Vec<Step> {
    use Dir::*;
    let mut v = vec![
        Step::Check("ch3_start", |r| {
            assert_eq!(r.w.map(), ROUTE);
            assert_eq!(r.w.var(VAR_SIL_STORY), 28);
            assert!(r.w.flag(FLAG_SIL_MODULE_FORCE));
            assert!(r.w.party().len() >= 3);
        }),
        // ---- Route S3: the Goulet (Route_S03_Sc01), the sign ----
        goto(ROUTE, 9, 46),
        Step::Check("route_sc01", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 30);
            assert!(r.has_text(Route_S03_Sc01_T1));
        }),
        goto(ROUTE, 11, 48),
        Step::Interact(Up),
        Step::Check("route_panneau", |r| {
            assert!(r.has_text(Route_S03_Panneau_T1))
        }),
        // The sbire at the depot's south portal sees the player on (10, 21).
        goto_avoiding(ROUTE, 10, 21, ROUTE_SIGHT),
        Step::Check("route_sbire", |r| {
            assert!(r.beaten(TRAINER_SIL_GRUNT_S03_1));
            assert!(r.has_text(Route_S03_Sbire_Intro));
        }),
        Step::Enter {
            dir: Up,
            map: DEPOT,
        },
    ];
    v.extend(depot());
    v.extend([
        // ---- Route S3's north part, to Viber-City ----
        goto_avoiding(ROUTE, 10, 0, ROUTE_SIGHT),
        Step::Enter { dir: Up, map: CITY },
        // ---- Viber-City: Mathieu's stand (Sc01) ----
        // The scene walks the player to the stand: a step onto (20, 29), not a `goto` (which
        // would walk back to its target).
        goto(CITY, 20, 30),
        Step::Walk(Up),
        Step::Check("city_sc01", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 32);
            assert_eq!(r.pos(), (20, 17));
            assert_eq!(r.count(ITEM_SIL_BROCHETTE_NEON), 3);
            assert_eq!(r.count(ITEM_LEMONADE), 2);
            assert!(r.w.flag(FLAG_SIL_HIDE_VIBER_ADRIEN));
            r.printed_in_order(&[
                Viber_City_Sc01_T1,
                Viber_City_Sc01_T3,
                Viber_City_Sc01_T4,
                Viber_City_Sc01_T6,
            ]);
        }),
        // Mathieu's heal, once a visit.
        Step::Interact(Up),
        Step::Check("mathieu_heal", |r| {
            assert!(r.healed());
            assert!(r.w.flag(FLAG_SIL_TMP_MATHIEU_HEAL));
        }),
        // The giant neon (Sc02).
        goto(CITY, 20, 13),
        Step::Interact(Up),
        Step::Check("city_sc02", |r| {
            assert!(r.w.flag(FLAG_SIL_VIBER_NEON_SCANNED));
            assert!(r.has_text(Viber_City_Sc02_T1));
        }),
        // ---- the Center: the respawn, the nurse ----
        goto(CITY, 6, 15),
        Step::Enter {
            dir: Up,
            map: CENTER,
        },
        Step::Check("center_respawn", |r| {
            assert_eq!(r.w.respawn(), HEAL_LOCATION_SIL_VIBER_CITY)
        }),
        goto(CENTER, 7, 4),
        Step::Interact(Up),
        Step::Check("nurse", |r| {
            assert!(r.healed());
            assert!(r.has_text(gText_RestoredPkmnToFullHealth));
        }),
        goto(CENTER, 6, 7),
        Step::Enter {
            dir: Down,
            map: CITY,
        },
        // ---- training in the east garden ----
        train(STATION_LEVEL),
    ]);
    v.extend_from_slice(HEAL);
    v.extend([
        // ---- the Nocturne's door (Sc03) ----
        goto(CITY, 28, 6),
        Step::Check("city_sc03", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 33);
            assert!(r.w.flag(FLAG_SIL_HIDE_VIBER_TAUBER));
            r.printed_in_order(&[Viber_City_Sc03_T1, Viber_City_Sc03_T2, Viber_City_Sc03_T6]);
        }),
        // ---- PRAUGECT's diversion (Sc04) ----
        goto(CITY, 8, 6),
        Step::Check("city_sc04", |r| {
            assert!(r.w.flag(FLAG_SIL_HIDE_STATION_GARDE));
            assert!(r.w.flag(FLAG_SIL_HIDE_VIBER_MATHIEU));
            r.printed_in_order(&[Viber_City_Sc04_T1, Viber_City_Sc04_T2, Viber_City_Sc04_T5]);
        }),
        goto(CITY, 8, 5),
        Step::Enter {
            dir: Up,
            map: STATION,
        },
        // ---- the Station Lumière ----
        goto(STATION, 9, 9),
        Step::Interact(Down),
        Step::Check("station_note", |r| {
            assert!(r.has_text(Station_Lumiere_Note_T1))
        }),
        goto_avoiding(STATION, 7, 6, STATION_SIGHT),
        Step::Interact(Up),
        Step::Check("station_sc01", |r| {
            assert!(r.beaten(TRAINER_SIL_CONTREMAITRE_1));
            assert!(r.w.flag(FLAG_SIL_STATION_CONTREMAITRE));
            r.printed_in_order(&[
                Station_Lumiere_Sc01_T1,
                Station_Lumiere_Sc01_Defaite,
                Station_Lumiere_Sc01_T2,
            ]);
        }),
        goto(STATION, 9, 2),
        Step::Interact(Up),
        Step::Check("station_tube", |r| {
            assert!(r.has_text(Station_Lumiere_Tube_T1));
            assert_eq!(r.w.var(VAR_SIL_STATION_STATE), 7);
        }),
        // Lever 2 (yellow), then lever 4 (blue and violet): all three dark.
        goto(STATION, 6, 2),
        Step::Interact(Up),
        Step::Check("station_levier2", |r| {
            assert_eq!(r.w.var(VAR_SIL_STATION_STATE) & 7, 0b101);
        }),
        goto(STATION, 8, 2),
        Step::Interact(Up),
        // ---- the black-out (Station_Lumiere_Sc03, Viber_City_Sc05) ----
        Step::Check("blackout", |r| {
            assert_eq!(r.w.map(), CITY);
            assert_eq!(r.w.var(VAR_SIL_STORY), 34);
            assert!(r.w.flag(FLAG_SIL_VIBER_BLACKOUT));
            assert!(r.w.flag(FLAG_SIL_VIBER_NEONS_BLANCS));
            assert!(r.w.flag(FLAG_SIL_HIDE_STATION_SBIRES));
            assert!(!r.w.flag(FLAG_SIL_HIDE_VIBER_MATHIEU));
            r.printed_in_order(&[
                Station_Lumiere_Sc03_T1,
                Viber_City_Sc05_T1,
                Viber_City_Sc05_T5,
                Viber_City_Sc05_T6,
                Viber_City_Sc05_T7,
            ]);
        }),
    ]);
    v.extend_from_slice(HEAL);
    v.extend(casino());
    v.push(train(GYM_LEVEL));
    v.extend_from_slice(HEAL);
    v.extend(gym());
    v.extend([
        // ---- Tauber's fear, Julien's call (Sc06) ----
        goto(CITY, 28, 6),
        Step::Check("city_sc06", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 36);
            assert!(r.w.flag(FLAG_SIL_VIBER_BARRIERE_EST));
            r.printed_in_order(&[
                Viber_City_Sc06_T1,
                Viber_City_Sc06_T2,
                Viber_City_Sc06_T3,
                Viber_City_Sc06_T4,
            ]);
        }),
        // Through the open barrier (37, 15).
        goto(CITY, 37, 15),
        Step::Check("barriere", |r| assert_eq!(r.pos(), (37, 15))),
    ]);
    v
}

/// The Casino (optional, played): Gogo & Romain, the coin case, the poster's switch, the
/// basement and the Porygon.
fn casino() -> Vec<Step> {
    use Dir::*;
    vec![
        goto(CITY, 20, 23),
        Step::Enter {
            dir: Up,
            map: CASINO,
        },
        goto_avoiding(CASINO, 3, 3, CASINO_SIGHT),
        Step::Check("casino_sc03", |r| {
            assert_eq!(r.w.var(VAR_SIL_CASINO_STATE), 1);
            r.printed_in_order(&[Casino_Sc03_T1, Casino_Sc03_T3, Casino_Sc03_T6]);
        }),
        // The coin counter (3, 7) across its counter without a coin case, the case (500 ₽)
        // from the seller (2, 9), then 50 coins.
        goto_avoiding(CASINO, 3, 9, CASINO_SIGHT),
        Step::Interact(Up),
        Step::Check("casino_sans_boite", |r| assert!(r.has_text(Casino_Sc02_T4))),
        Step::Interact(Left),
        Step::Check("casino_sc01", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_VIBER_COIN_CASE));
            assert_eq!(r.count(ITEM_COIN_CASE), 1);
            r.printed_in_order(&[Casino_Sc01_T1, Casino_Sc01_T2]);
        }),
        Step::Interact(Up),
        Step::Check("casino_sc02", |r| {
            assert_eq!(r.w.var(VAR_SIL_COINS), 50);
            assert!(r.has_text(Casino_Sc02_T2));
        }),
        // The coins under a machine (14, 3).
        goto_avoiding(CASINO, 13, 3, CASINO_SIGHT),
        Step::Interact(Right),
        Step::Check("casino_jetons_caches", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_CASINO_COINS_FLOOR));
            assert_eq!(r.w.var(VAR_SIL_COINS), 70);
        }),
        // The poster's switch (Sc08), the stairs.
        goto_avoiding(CASINO, 17, 1, CASINO_SIGHT),
        Step::Interact(Up),
        Step::Check("casino_sc08", |r| {
            assert_eq!(r.w.var(VAR_SIL_CASINO_STATE), 2);
            r.printed_in_order(&[Casino_Sc08_T1, Casino_Sc08_T3, Casino_Pnj04_T2]);
        }),
        Step::Walk(Right),
        Step::Enter {
            dir: Down,
            map: B1F,
        },
        // ---- the basement: the terminal, the head croupier, the Porygon ----
        goto_avoiding(B1F, 6, 8, B1F_SIGHT),
        Step::Interact(Up),
        Step::Check("b1f_sc01", |r| {
            assert!(r.w.flag(FLAG_SIL_CASINO_PREUVE));
            r.printed_in_order(&[Casino_B1F_Sc01_T1, Casino_B1F_Sc01_T3]);
        }),
        goto_avoiding(B1F, 10, 8, B1F_SIGHT),
        Step::Interact(Down),
        Step::Check("b1f_chef", |r| {
            assert!(r.beaten(TRAINER_SIL_CROUPIER_CHEF_1));
            assert!(r.w.flag(FLAG_SIL_CASINO_CHEF));
        }),
        goto(B1F, 9, 10),
        Step::Interact(Left),
        Step::Check("b1f_sc02", |r| {
            assert_eq!(r.w.var(VAR_SIL_CASINO_STATE), 3);
            assert!(r.w.flag(FLAG_SIL_CASINO_LOYAL));
            assert!(r.w.flag(FLAG_SIL_PORYGON_LIBERE));
            let party = r.w.party();
            assert!(
                party
                    .iter()
                    .any(|m| m.species == SPECIES_PORYGON && m.level == 25)
            );
            assert_eq!(r.count(ITEM_UP_GRADE), 1);
            r.printed_in_order(&[Casino_B1F_Sc02_T1, Casino_B1F_Sc02_T3, Casino_B1F_Sc02_T4]);
        }),
        goto(B1F, 10, 10),
        Step::Interact(Right),
        Step::Check("b1f_rare_candy", |r| {
            assert_eq!(r.count(ITEM_RARE_CANDY), 1)
        }),
        goto_avoiding(B1F, 1, 2, B1F_SIGHT),
        Step::Enter {
            dir: Up,
            map: CASINO,
        },
        goto_avoiding(CASINO, 9, 9, CASINO_SIGHT),
        Step::Enter {
            dir: Down,
            map: CITY,
        },
    ]
}

/// From the city to the leaders' stage in the Nocturne, the tracks played (the map load plays
/// blue): console A twice (violet), console B twice (yellow), console C once (violet).
const GYM_IN: &[Step] = &[
    goto(CITY, 28, 5),
    Step::Enter {
        dir: Dir::Up,
        map: GYM,
    },
    goto_avoiding(GYM, 3, 13, HUGO_SIGHT),
    Step::Interact(Dir::Left),
    Step::Interact(Dir::Left),
    goto_avoiding(GYM, 9, 10, HUGO_SIGHT),
    Step::Interact(Dir::Up),
    Step::Interact(Dir::Up),
    goto(GYM, 7, 6),
    goto(GYM, 2, 6),
    Step::Interact(Dir::Left),
];

/// The Nocturne: a first visit for its trainers (Léna by sight, the manager, Éliott), a heal,
/// then the leaders.
fn gym() -> Vec<Step> {
    use Dir::*;
    let mut v = vec![
        goto(CITY, 28, 5),
        Step::Enter { dir: Up, map: GYM },
        Step::Walk(Up),
        Step::Check("lena", |r| assert!(r.beaten(TRAINER_SIL_NOCTURNE_LENA_1))),
        goto_avoiding(GYM, 4, 13, HUGO_SIGHT),
        Step::Interact(Down),
        Step::Check("gerant", |r| assert!(r.has_text(Nocturne_Gerant_T1))),
        goto_avoiding(GYM, 3, 13, HUGO_SIGHT),
        Step::Interact(Left),
        Step::Check("console_a", |r| {
            assert_eq!(r.w.var(VAR_SIL_NOCTURNE_STATE), 1);
            assert!(r.has_text(Nocturne_Sc01_T1));
        }),
        Step::Interact(Left),
        goto_avoiding(GYM, 9, 10, HUGO_SIGHT),
        Step::Interact(Up),
        Step::Interact(Up),
        Step::Check("console_b", |r| {
            assert_eq!(r.w.var(VAR_SIL_NOCTURNE_STATE), 1)
        }),
        goto(GYM, 7, 6),
        Step::Interact(Left),
        Step::Check("eliott", |r| {
            assert!(r.beaten(TRAINER_SIL_NOCTURNE_ELIOTT_1))
        }),
        // Back through the violet barrier (9–10, 11): console B once more.
        goto(GYM, 9, 10),
        Step::Interact(Up),
        goto(GYM, 9, 12),
        goto_avoiding(GYM, 6, 14, HUGO_SIGHT),
        Step::Enter {
            dir: Down,
            map: CITY,
        },
    ];
    v.extend_from_slice(HEAL);
    v.extend_from_slice(GYM_IN);
    v.extend([
        Step::Check("console_c", |r| {
            assert_eq!(r.w.var(VAR_SIL_NOCTURNE_STATE), 2);
            assert!(r.has_text(Nocturne_Sc01_T2));
        }),
        // ---- Tauber & Hocseat (Nocturne_Sc02) ----
        Step::Hook(FIFTH),
        goto(GYM, 6, 3),
        Step::Check("nocturne_sc02", |r| {
            assert!(r.beaten(TRAINER_SIL_TAUBER_HOCSEAT_1));
            assert_eq!(r.w.var(VAR_SIL_STORY), 35);
            assert!(r.w.flag(FLAG_SIL_BADGE_03));
            assert!(r.w.flag(FLAG_SIL_MODULE_FLASH));
            assert!(r.w.flag(FLAG_SIL_LOG_03));
            assert!(r.w.flag(FLAG_SIL_ITEM_NOCTURNE_PIERRE_LIEN));
            assert_eq!(r.w.var(VAR_SIL_SYNC), 30);
            assert_eq!(r.count(ITEM_SIL_PIERRE_LIEN), 1);
            assert!(r.count(ITEM_TM34) >= 1);
            r.printed_in_order(&[
                Nocturne_Sc02_T1,
                Nocturne_Sc02_T3,
                Nocturne_Sc02_Defaite,
                Nocturne_Sc02_T4,
                Nocturne_Sc02_T9,
                Nocturne_Sc02_T10,
                Nocturne_Sc02_T11,
                Nocturne_Sc02_T14,
            ]);
        }),
        // Back down: console C twice (blue, yellow), console B once (violet).
        goto(GYM, 2, 6),
        Step::Interact(Left),
        Step::Interact(Left),
        goto(GYM, 9, 10),
        Step::Interact(Up),
        // The manager's Fresh Water, after the badge.
        goto_avoiding(GYM, 4, 13, HUGO_SIGHT),
        Step::Interact(Down),
        Step::Check("gerant_apres", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_NOCTURNE_FRESH_WATER));
            assert!(r.has_text(Nocturne_Gerant_T2));
        }),
        goto_avoiding(GYM, 6, 14, HUGO_SIGHT),
        Step::Enter {
            dir: Down,
            map: CITY,
        },
    ]);
    v
}

/// What the end of Chapter 3 asserts beyond its checkpoints: the story, the bag, no dropped
/// event, and every Chapter 3 fallback the main path meets reported.
pub fn check_end(r: &Run) {
    assert_eq!(r.w.var(VAR_SIL_STORY), 36);
    assert!(r.w.flag(FLAG_SIL_BADGE_03));
    for (item, n) in [
        (ITEM_SIL_BROCHETTE_NEON, 3),
        (ITEM_LEMONADE, 2),
        (ITEM_GREAT_BALL, 1),
        (ITEM_COIN_CASE, 1),
        (ITEM_UP_GRADE, 1),
        (ITEM_SIL_PIERRE_LIEN, 1),
        (ITEM_FRESH_WATER, 1),
    ] {
        assert!(r.count(item) >= n, "item {item}");
    }
    assert_eq!(r.w.events_dropped(), 0);
    for name in [
        "strength",
        "battle_two_trainers",
        "screen_flash",
        "stop_music",
        "set_flash_level",
        "neon tiles",
        "lever metatiles",
        "multichoice",
        "hidden coins message",
        "capsule metatile",
        "give_mon custom",
        "play_music",
        "party_valid_count",
        "strobe",
        "show_sync",
        "log_menu",
        "play_se",
        "mallette_call",
        "end_call",
    ] {
        assert!(r.degraded.contains(name), "{name}: {:?}", r.degraded);
    }
}
