//! Chapter 8, Évalon, from Route S8's palier 3 (`VAR_SIL_STORY` 80) to the departure (86):
//! - Route S8's palier 3: the messenger pigeon (Sc02, 81), Ambre, the cracked rock and Gwada's
//!   braises (the hidden Hyper Potion), Galahad, the old role-player, the sign, the brambles
//!   and the Rare Candy, north to Évalon;
//! - Évalon: the arrival (Sc01, 82), the fairy and the fountain, the friends at the Taverne
//!   (Sc02), Q3 (the brambles, Plume-d'Argent, the keeper's Heal Powder), the Tour du Scribe
//!   (the bookcases, the crates and the trapdoor, the Copiste and TM30, the Chronique, the
//!   Scribe's trial), the Galeries (Q1's rock, the dark gallery's ore and Link Stone, the Moon
//!   Stone, the Nugget), Q1's Larme de Fée, Q2's Macho Brace and the Galekingite, the Anciens'
//!   trial (Sc05), the hidden Revive, the bard's ballad, the Porte du Fer (Sc04: three guards,
//!   the trials' end, 83), the Anciens' PP Max (Légende), the bronze bell (Sc07: Gogo and
//!   Romain, 84);
//! - the Citadelle: the innkeeper (Sc08), the slabs and Tristan, the golems' bridge, Renard, the
//!   crypt and Morgane, the Sphinge's riddles, the right chest, Élise; Loulpix and Doris (Sc09:
//!   the eighth badge, the Sonde de Pression, the Cœur d'Évalon, the Épée de Légende, 85);
//! - out by the south-east path (Sc10, 86).
//!
//! Palier 1 (Route_S08_Sc01, 76 → 80, the hiker, the cape, the Ultra Ball, Gaston) is its own
//! short list ([`route_start`], [`route_steps`]): palier 1 and palier 3 are only joined by
//! Waterfall and Surf, which the engine does not have, so the chapter's main list starts on
//! palier 3's first row ([`start`]).
//!
//! **The fallbacks the path relies on** (`ch08_*.rs`): the Champ Magnétique is an A press on a
//! bookcase, a crate or a golem; the dark gallery and the crypt are lit, their coords only test
//! the module; the skull slabs are coord traps; the multichoices are yes/no (the honest answer,
//! Mathieu as partner, the right riddle); the Multi battle is two single battles with a heal;
//! the gym's double battle is a single one; the hidden items are signs.
//!
//! **The party** ([`PARTY`]): Chapter 7's end, as its e2e leaves it (`SPINE_DEBUG=1`): the
//! ALAKAZAM 56 leading, the AMPHAROS 57, the TYPHLOSION and the AGGRON 47, the WAILORD 45. No
//! hook and no [`Step::Train`]: the chapter's trainers (46–53) are the training, and the run
//! heals at the Taverne before the guards and before the gym.

use super::{Run, Start, Step, goto, goto_avoiding};
use content::consts::*;
use engine::data::{Dir, ItemId, MapId, SpeciesId};
use engine::script::Ctx;

const ROUTE: MapId = MAP_ROUTE_S08;
const TOWN: MapId = MAP_EVALON;
const TAVERNE: MapId = MAP_EVALON_TAVERNE;
const FORGE: MapId = MAP_EVALON_FORGE;
const CONSEIL: MapId = MAP_EVALON_CONSEIL;
const TOUR: MapId = MAP_EVALON_TOUR_SCRIBE;
const TOUR_2F: MapId = MAP_EVALON_TOUR_SCRIBE_2F;
const TOUR_3F: MapId = MAP_EVALON_TOUR_SCRIBE_3F;
const GALERIES: MapId = MAP_EVALON_GALERIES;
const GYM: MapId = MAP_EVALON_CITADELLE;
const GYM_2F: MapId = MAP_EVALON_CITADELLE_2F;

/// The chapter's main start: palier 3's first row (31, 21), at the top of waterfall 3, facing
/// north, Chapter 7's end state ([`end_of_chapter_7`]) with Route_S08_Sc01 played (80).
pub fn start() -> Start {
    Start {
        map: ROUTE,
        x: 31,
        y: 21,
        facing: Dir::Up,
        setup: |ctx| {
            Box::pin(async move {
                end_of_chapter_7(ctx.clone()).await;
                ctx.set_var(VAR_SIL_STORY, 80).await;
            })
        },
    }
}

/// Palier 1: Route S8's arrival mat from Reverse-City (19, 79), facing north, Chapter 7's end
/// state (76), where Chapter 7's e2e ends.
pub fn route_start() -> Start {
    Start {
        map: ROUTE,
        x: 19,
        y: 79,
        facing: Dir::Up,
        setup: |ctx| Box::pin(end_of_chapter_7(ctx)),
    }
}

/// The start state's party (see the module docs).
pub const PARTY: &[(SpeciesId, u8)] = &[
    (SPECIES_ALAKAZAM, 56),
    (SPECIES_AMPHAROS, 57),
    (SPECIES_TYPHLOSION, 47),
    (SPECIES_AGGRON, 47),
    (SPECIES_WAILORD, 45),
];

/// Chapter 7's end state (`08-evalon.md`'s prerequisites), on top of Silice's new-game script:
/// Chapters 0–6 as `ch07`'s start, Chapter 7's (corrupted) badge, its Flux Turbine
/// (`FLAG_SIL_MODULE_CASCADE`) and its log, the sync at 70, the story at 76
/// (Reverse_City_Sc07); the party ([`PARTY`]), a few potions, the respawn at Reverse-City.
async fn end_of_chapter_7(ctx: Ctx) {
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
        // Chapters 1 to 6.
        FLAG_SIL_BADGE_01,
        FLAG_SIL_BADGE_02,
        FLAG_SIL_BADGE_03,
        FLAG_SIL_BADGE_04,
        FLAG_SIL_BADGE_05,
        FLAG_SIL_BADGE_06,
        FLAG_SIL_MODULE_COUPE,
        FLAG_SIL_MODULE_FORCE,
        FLAG_SIL_MODULE_FLASH,
        FLAG_SIL_MODULE_SURF,
        FLAG_SIL_MODULE_VOL,
        FLAG_SIL_MODULE_ECLATE_ROC,
        FLAG_SIL_MODULE_MEGA,
        FLAG_SIL_LOG_01,
        FLAG_SIL_LOG_02,
        FLAG_SIL_LOG_03,
        FLAG_SIL_LOG_04,
        FLAG_SIL_LOG_05,
        FLAG_SIL_LOG_06,
        // Chapter 7.
        FLAG_SIL_BADGE_07,
        FLAG_SIL_BADGE_07_CORROMPU,
        FLAG_SIL_MODULE_CASCADE,
        FLAG_SIL_LOG_07,
        FLAG_SIL_REVERSE_PORTE_NORD_OUVERTE,
    ] {
        ctx.set_flag(f, true).await;
    }
    ctx.set_var(VAR_SIL_STORY, 76).await;
    ctx.set_var(VAR_SIL_SYNC, 70).await;
    ctx.set_var(VAR_SIL_STARTER, 1).await;
    ctx.set_var(VAR_SIL_CH0_GATE, 1).await;
    ctx.item_add(ItemId(ITEM_SIL_MALLETTE), 1).await;
    ctx.item_add(ItemId(ITEM_SUPER_POTION), 5).await;
    ctx.item_add(ItemId(ITEM_HYPER_POTION), 3).await;
    ctx.money_add(20000).await;
    ctx.set_respawn(HEAL_LOCATION_SIL_REVERSE_CITY).await;
    for &(species, level) in PARTY {
        ctx.give_mon(species, level).await;
    }
}

/// Palier 1's steps, from [`route_start`]: Route_S08_Sc01 (80), the sign, the hiker, Mathieu's
/// cape, the Ultra Ball and Gaston.
pub fn route_steps() -> Vec<Step> {
    use Dir::*;
    vec![
        Step::Walk(Up),
        Step::Walk(Up),
        Step::Check("route_s08_sc01", |r| {
            assert_eq!(r.w.map(), ROUTE);
            assert_eq!(r.w.var(VAR_SIL_STORY), 80);
            assert!(r.has_text(Route_S08_Sc01_T1));
        }),
        goto(ROUTE, 17, 77),
        Step::Interact(Up),
        goto(ROUTE, 22, 75),
        Step::Interact(Up),
        goto(ROUTE, 5, 74),
        Step::Interact(Up),
        Step::Check("palier_1_people", |r| {
            r.printed_in_order(&[Route_S08_Sign_1, Route_S08_Npc_1, Route_S08_Cape]);
        }),
        goto(ROUTE, 35, 70),
        Step::Interact(Up),
        Step::Check("ultra_ball", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_S08_ULTRA_BALL));
            assert!(r.count(ITEM_ULTRA_BALL) >= 1);
        }),
        // Gaston (35, 64) sees the bank's column (35, 65–67).
        Step::Walk(Up),
        Step::Walk(Up),
        Step::Walk(Up),
        Step::Check("gaston", |r| {
            assert!(r.beaten(TRAINER_SIL_S08_GASTON));
            assert!(r.has_text(Route_S08_Gaston_Intro));
        }),
    ]
}

/// Ambre's line of sight (25, 18), 4 cells west.
const AMBRE_SIGHT: &[(i16, i16)] = &[(21, 18), (22, 18), (23, 18), (24, 18)];
/// Galahad's (13, 12), 3 cells south.
const GALAHAD_SIGHT: &[(i16, i16)] = &[(13, 13), (13, 14), (13, 15)];
/// Both, until each is met.
const PALIER_3_AVOID: &[(i16, i16)] = &[
    (21, 18),
    (22, 18),
    (23, 18),
    (24, 18),
    (13, 13),
    (13, 14),
    (13, 15),
];
/// The Copiste's (11, 10), 2 cells north.
const COPISTE_SIGHT: &[(i16, i16)] = &[(11, 9), (11, 8)];
/// Élise's (6, 3), 2 cells west.
const ELISE_SIGHT: &[(i16, i16)] = &[(5, 3), (4, 3)];
/// Room 1's 43 skull slabs (coord traps), never planned through.
const TRAPS: &[(i16, i16)] = &[
    (2, 41),
    (3, 41),
    (4, 41),
    (6, 41),
    (7, 41),
    (8, 41),
    (2, 42),
    (3, 42),
    (4, 42),
    (6, 42),
    (7, 42),
    (8, 42),
    (2, 43),
    (3, 43),
    (4, 43),
    (6, 43),
    (7, 43),
    (8, 43),
    (2, 44),
    (3, 44),
    (4, 44),
    (8, 44),
    (2, 45),
    (3, 45),
    (4, 45),
    (5, 45),
    (8, 45),
    (8, 46),
    (2, 47),
    (4, 47),
    (5, 47),
    (6, 47),
    (7, 47),
    (8, 47),
    (2, 48),
    (7, 48),
    (8, 48),
    (2, 49),
    (3, 49),
    (4, 49),
    (6, 49),
    (7, 49),
    (8, 49),
];
/// The traps and Tristan's cell (6, 46), until he is met.
const ROOM_1_AVOID: &[(i16, i16)] = &[
    (2, 41),
    (3, 41),
    (4, 41),
    (6, 41),
    (7, 41),
    (8, 41),
    (2, 42),
    (3, 42),
    (4, 42),
    (6, 42),
    (7, 42),
    (8, 42),
    (2, 43),
    (3, 43),
    (4, 43),
    (6, 43),
    (7, 43),
    (8, 43),
    (2, 44),
    (3, 44),
    (4, 44),
    (8, 44),
    (2, 45),
    (3, 45),
    (4, 45),
    (5, 45),
    (8, 45),
    (8, 46),
    (2, 47),
    (4, 47),
    (5, 47),
    (6, 47),
    (7, 47),
    (8, 47),
    (2, 48),
    (7, 48),
    (8, 48),
    (2, 49),
    (3, 49),
    (4, 49),
    (6, 49),
    (7, 49),
    (8, 49),
    (6, 46),
];

/// Into the Taverne, a rest at the innkeeper's, and out.
const HEAL: &[Step] = &[
    goto(TOWN, 22, 27),
    Step::Enter {
        dir: Dir::Up,
        map: TAVERNE,
    },
    goto(TAVERNE, 6, 5),
    Step::Interact(Dir::Up),
    Step::Check("taverne_heal", |r| {
        assert!(r.healed());
        assert!(r.has_text(Evalon_Taverne_Soin_T2));
    }),
    goto(TAVERNE, 7, 10),
    Step::Enter {
        dir: Dir::Down,
        map: TOWN,
    },
];

/// Up the Taverne's stairs to the Pigeonnier (a warp inside the map), the keeper, and back
/// down: from the Taverne's door and back to it.
const PIGEONNIER: &[Step] = &[
    goto(TAVERNE, 11, 2),
    Step::Walk(Dir::Up),
    goto(TAVERNE, 19, 5),
    Step::Interact(Dir::Up),
    goto(TAVERNE, 18, 2),
    Step::Walk(Dir::Up),
    goto(TAVERNE, 7, 10),
];

/// Chapter 8's steps, from [`start`].
pub fn steps() -> Vec<Step> {
    use Dir::*;
    let mut s = vec![
        // ---- Route S8, palier 3 ----
        Step::Walk(Left),
        Step::Check("route_s08_sc02", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 81);
            assert!(r.w.flag(FLAG_SIL_HIDE_S08_PIGEON));
            r.printed_in_order(&[Route_S08_Sc02_T1, Route_S08_Sc02_T2]);
        }),
        goto_avoiding(ROUTE, 24, 19, AMBRE_SIGHT),
        Step::Walk(Up),
        Step::Check("ambre", |r| assert!(r.beaten(TRAINER_SIL_S08_AMBRE))),
        // The cracked rock (31, 10), Chapter 7's scan, then the braises nook.
        goto_avoiding(ROUTE, 31, 11, GALAHAD_SIGHT),
        Step::Interact(Up),
        goto(ROUTE, 32, 7),
        Step::Interact(Up),
        goto(ROUTE, 33, 7),
        Step::Interact(Up),
        Step::Check("braises", |r| {
            r.printed_in_order(&[Route_S07_Bloc_Scan, Route_S07_Bloc_Casse, Route_S08_Braises]);
            assert!(r.w.flag(FLAG_SIL_ITEM_S08_CACHE_1));
            assert!(r.count(ITEM_HYPER_POTION) >= 4);
        }),
        goto_avoiding(ROUTE, 13, 16, GALAHAD_SIGHT),
        Step::Walk(Up),
        Step::Check("galahad", |r| assert!(r.beaten(TRAINER_SIL_S08_GALAHAD))),
        goto(ROUTE, 10, 6),
        Step::Interact(Up),
        goto(ROUTE, 16, 4),
        Step::Interact(Up),
        // The brambles (5, 13) and the Rare Candy (6, 20) behind them.
        goto(ROUTE, 5, 12),
        Step::Interact(Down),
        goto(ROUTE, 6, 19),
        Step::Interact(Down),
        Step::Check("ronces_rare_candy", |r| {
            r.printed_in_order(&[Route_S08_Npc_3, Route_S08_Sign_2, Bois_Ether_Sc08_T3]);
            assert!(r.w.flag(FLAG_SIL_ITEM_S08_RACCOURCI));
            assert!(r.count(ITEM_RARE_CANDY) >= 1);
        }),
        goto_avoiding(ROUTE, 19, 1, PALIER_3_AVOID),
        Step::Enter { dir: Up, map: TOWN },
        // ---- Évalon: the arrival ----
        goto(TOWN, 24, 46),
        Step::Walk(Up),
        Step::Check("evalon_sc01", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 82);
            assert_eq!(r.w.var(VAR_SIL_EVALON_STATE), 1);
            assert!(r.w.flag(FLAG_SIL_HIDE_EVALON_SCRIBE));
            r.printed_in_order(&[
                Evalon_Sc01_T1,
                Evalon_Sc01_T2,
                Evalon_Sc01_T3,
                Evalon_Sc01_T4,
                Evalon_Sc01_T5,
                Evalon_Sc01_T6,
                Evalon_Sc01_T7,
                Evalon_Sc01_T8,
            ]);
        }),
        // The fairy (Q1's request) and the dry fountain.
        goto(TOWN, 18, 42),
        Step::Interact(Left),
        goto(TOWN, 19, 42),
        Step::Interact(Up),
        Step::Check("q1_fee", |r| {
            r.printed_in_order(&[Evalon_Q1_T1, Evalon_Sign_5]);
        }),
        // ---- the Taverne: the friends, the keeper (Q3) ----
        goto(TOWN, 22, 27),
        Step::Enter {
            dir: Up,
            map: TAVERNE,
        },
        Step::Check("taverne_friends", |r| {
            assert!(!r.w.flag(FLAG_SIL_HIDE_EVALON_TAVERNE_MATHIEU));
        }),
        goto(TAVERNE, 2, 8),
        Step::Interact(Up),
    ];
    s.extend_from_slice(PIGEONNIER);
    s.extend_from_slice(&[
        Step::Check("q3_t1", |r| {
            r.printed_in_order(&[Evalon_Taverne_Mathieu_T1, Evalon_Q3_T1]);
        }),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        // Q3: the brambles (44, 27) and Plume-d'Argent (46, 27).
        goto(TOWN, 43, 27),
        Step::Interact(Right),
        Step::Walk(Right),
        Step::Walk(Right),
        Step::Interact(Right),
        Step::Check("q3_pigeon", |r| {
            assert!(r.w.flag(FLAG_SIL_HIDE_EVALON_PIGEON));
            assert!(r.has_text(Evalon_Q3_Pigeon));
        }),
        goto(TOWN, 22, 27),
        Step::Enter {
            dir: Up,
            map: TAVERNE,
        },
    ]);
    s.extend_from_slice(PIGEONNIER);
    s.extend_from_slice(&[
        Step::Check("q3_done", |r| {
            assert!(r.w.flag(FLAG_SIL_EVALON_Q3));
            assert!(r.has_text(Evalon_Q3_T2));
            assert!(r.count(ITEM_HEAL_POWDER) >= 3);
            assert_eq!(r.w.var(VAR_SIL_EVALON_RENOM), 1);
        }),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        // ---- the Tour du Scribe ----
        goto(TOWN, 6, 21),
        Step::Enter { dir: Up, map: TOUR },
        goto(TOUR, 7, 9),
        Step::Interact(Up),
        goto(TOUR, 3, 9),
        Step::Interact(Up),
        // Bookcase 1 left into its niche, bookcase 2 up into its niche.
        goto(TOUR, 6, 7),
        Step::Interact(Left),
        goto(TOUR, 8, 5),
        Step::Interact(Up),
        Step::Check("rayonnages", |r| {
            assert!(r.w.flag(FLAG_SIL_EVALON_TOUR_RAYONNAGES));
            r.printed_in_order(&[Evalon_Sc03_Scribe_T1, Evalon_Sc03_Lutrin_T1, Depot_Bloc_T2]);
        }),
        goto(TOUR, 11, 2),
        Step::Enter {
            dir: Up,
            map: TOUR_2F,
        },
        // The crates onto the slabs: A left, B up, C down, D right.
        goto_avoiding(TOUR_2F, 6, 5, COPISTE_SIGHT),
        Step::Interact(Left),
        goto_avoiding(TOUR_2F, 7, 6, COPISTE_SIGHT),
        Step::Interact(Up),
        goto_avoiding(TOUR_2F, 5, 6, COPISTE_SIGHT),
        Step::Interact(Down),
        goto_avoiding(TOUR_2F, 7, 8, COPISTE_SIGHT),
        Step::Interact(Right),
        Step::Check("trappe", |r| {
            assert!(r.w.flag(FLAG_SIL_EVALON_TOUR_TRAPPE));
        }),
        // The Copiste (11, 10), met on purpose.
        goto_avoiding(TOUR_2F, 11, 7, COPISTE_SIGHT),
        Step::Walk(Down),
        Step::Check("copiste", |r| {
            assert!(r.beaten(TRAINER_SIL_EVALON_COPISTE));
            assert!(r.w.flag(FLAG_SIL_ITEM_EVALON_CT_BALL_OMBRE));
            assert!(r.count(ITEM_TM30) >= 1);
        }),
        goto(TOUR_2F, 10, 2),
        Step::Enter {
            dir: Up,
            map: TOUR_3F,
        },
        goto(TOUR_3F, 5, 4),
        Step::Interact(Down),
        Step::Check("chronique", |r| {
            assert!(r.count(ITEM_SIL_CHRONIQUE) >= 1);
            r.printed_in_order(&[Evalon_Sc03_Chronique_T1, Evalon_Sc03_Chronique_T2]);
        }),
        goto(TOUR_3F, 2, 2),
        Step::Enter {
            dir: Up,
            map: TOUR_2F,
        },
        goto(TOUR_2F, 2, 2),
        Step::Enter { dir: Up, map: TOUR },
        goto(TOUR, 7, 9),
        Step::Interact(Up),
        Step::Check("epreuve_scribe", |r| {
            assert!(r.w.flag(FLAG_SIL_EVALON_EPREUVE_SCRIBE));
            assert!(r.has_text(Evalon_Sc03_Scribe_T2));
            assert_eq!(r.w.var(VAR_SIL_EVALON_RENOM), 3);
        }),
        goto(TOUR, 6, 11),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        // ---- the Galeries ----
        goto(TOWN, 3, 13),
        Step::Enter {
            dir: Up,
            map: GALERIES,
        },
        goto(GALERIES, 24, 15),
        Step::Interact(Right),
        Step::Check("q1_rocher", |r| {
            assert!(r.w.flag(FLAG_SIL_EVALON_Q1_ROCHER));
            r.printed_in_order(&[Evalon_Q1_Rocher, Route_S07_Bloc_Casse]);
        }),
        goto(GALERIES, 13, 3),
        Step::Interact(Up),
        goto(GALERIES, 18, 2),
        Step::Interact(Up),
        goto(GALERIES, 3, 15),
        Step::Interact(Down),
        goto(GALERIES, 9, 11),
        Step::Interact(Up),
        Step::Check("galeries_items", |r| {
            assert!(r.count(ITEM_SIL_MINERAI_FERREUX) >= 1);
            assert!(r.count(ITEM_SIL_PIERRE_LIEN) >= 1);
            assert!(r.count(ITEM_MOON_STONE) >= 1);
            assert!(r.count(ITEM_NUGGET) >= 1);
        }),
        goto(GALERIES, 15, 22),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        // ---- Q1's reward, Q2 at the Forge ----
        goto(TOWN, 18, 42),
        Step::Interact(Left),
        Step::Check("q1_done", |r| {
            assert!(r.w.flag(FLAG_SIL_EVALON_Q1));
            assert!(r.count(ITEM_SIL_LARME_FEE) >= 1);
        }),
        goto(TOWN, 32, 31),
        Step::Enter {
            dir: Up,
            map: FORGE,
        },
        goto(FORGE, 5, 5),
        Step::Interact(Up),
        Step::Interact(Up),
        Step::Check("q2_done", |r| {
            assert!(r.w.flag(FLAG_SIL_EVALON_Q2));
            assert_eq!(r.count(ITEM_SIL_MINERAI_FERREUX), 0);
            assert!(r.count(ITEM_MACHO_BRACE) >= 1);
            assert!(r.count(ITEM_SIL_AGGRONITE) >= 1);
            r.printed_in_order(&[Evalon_Q2_T2, Evalon_Q2_T3]);
            assert_eq!(r.w.var(VAR_SIL_EVALON_RENOM), 5);
        }),
        goto(FORGE, 4, 6),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        // ---- the Anciens (Sc05) ----
        goto(TOWN, 42, 23),
        Step::Enter {
            dir: Up,
            map: CONSEIL,
        },
        goto(CONSEIL, 2, 5),
        Step::Interact(Right),
        Step::Check("epreuve_esprit", |r| {
            assert!(r.w.flag(FLAG_SIL_EVALON_EPREUVE_ESPRIT));
            assert!(r.count(ITEM_SIL_FICHE_PERSONNAGE) >= 1);
            assert_eq!(r.w.var(VAR_SIL_EVALON_RENOM), 8);
            r.printed_in_order(&[
                Evalon_Sc05_T1,
                Evalon_Sc05_R1_Bon,
                Evalon_Sc05_R2_Bon,
                Evalon_Sc05_R3_Bon,
                Evalon_Sc05_T2,
                Evalon_Fiche_Paladin,
            ]);
            assert_eq!(r.w.var(VAR_SIL_STORY), 82);
        }),
        goto(CONSEIL, 6, 8),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        goto(TOWN, 31, 16),
        Step::Interact(Up),
        Step::Check("hidden_revive", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_EVALON_CACHE_1));
        }),
        // ---- a rest, the bard's ballad (Paladin) ----
        goto(TOWN, 22, 27),
        Step::Enter {
            dir: Up,
            map: TAVERNE,
        },
        goto(TAVERNE, 6, 5),
        Step::Interact(Up),
        goto(TAVERNE, 12, 8),
        Step::Interact(Right),
        Step::Check("barde", |r| assert!(r.has_text(Evalon_Barde_Ballade))),
        goto(TAVERNE, 7, 10),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        // ---- the Porte du Fer (Sc04): the last trial ----
        goto(TOWN, 25, 14),
        Step::Interact(Up),
        Step::Check("epreuve_fer", |r| {
            assert!(r.beaten(TRAINER_SIL_GARDE_BERTRAND));
            assert!(r.beaten(TRAINER_SIL_GARDE_IOLANDA));
            assert!(r.beaten(TRAINER_SIL_CAPITAINE_PERCEVAL));
            assert!(r.w.flag(FLAG_SIL_EVALON_EPREUVE_FER));
            assert!(r.w.flag(FLAG_SIL_ITEM_EVALON_LUM));
            assert_eq!(r.w.var(VAR_SIL_EVALON_RENOM), 10);
            assert_eq!(r.w.var(VAR_SIL_STORY), 83);
            r.printed_in_order(&[
                Evalon_Sc04_T1,
                Evalon_Sc04_T2,
                Evalon_Sc04_T3,
                Evalon_Sc04_T4,
                Evalon_EpreuvesFinies_T1,
            ]);
            assert!(!r.w.flag(FLAG_SIL_HIDE_EVALON_AMIS));
        }),
        // The Anciens' PP Max at the Légende rank.
        goto(TOWN, 42, 23),
        Step::Enter {
            dir: Up,
            map: CONSEIL,
        },
        goto(CONSEIL, 2, 5),
        Step::Interact(Right),
        Step::Check("pp_max", |r| {
            assert!(r.w.flag(FLAG_SIL_EVALON_PP_MAX));
            assert!(r.count(ITEM_PP_MAX) >= 1);
            assert!(r.has_text(Evalon_Fiche_Legende));
        }),
        goto(CONSEIL, 6, 8),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
    ]);
    s.extend_from_slice(HEAL);
    s.extend_from_slice(&[
        // ---- the bronze bell (Sc07) ----
        goto(TOWN, 25, 11),
        Step::Walk(Up),
        Step::Check("evalon_sc07", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 84);
            assert!(r.w.flag(FLAG_SIL_EVALON_INVASION));
            assert!(r.beaten(TRAINER_SIL_GOGO_4));
            assert!(r.beaten(TRAINER_SIL_ROMAIN_4));
            assert_eq!(r.w.var(VAR_SIL_EVALON_PARTENAIRE), 3);
            assert!(r.count(ITEM_SIL_PLAN_PRISE_EAU) >= 1);
            assert!(r.count(ITEM_SIL_PAIN_LEMBAS) >= 1);
            r.printed_in_order(&[
                Evalon_Sc07_T1,
                Evalon_Sc07_T3,
                Evalon_Sc07_T8,
                Evalon_Sc07_Partner_T2,
                Evalon_Sc07_Others_T2,
                Evalon_Sc07_Repli_T1,
                Evalon_Sc07_T9,
                Evalon_Sc07_T12,
                Evalon_Sc07_T16,
            ]);
        }),
        // ---- the Citadelle ----
        goto(TOWN, 25, 4),
        Step::Enter { dir: Up, map: GYM },
        Step::Check("evalon_sc08", |r| assert!(r.has_text(Evalon_Sc08_T1))),
        // Room 1: the safe slabs, Tristan on (6, 46).
        goto_avoiding(GYM, 5, 46, ROOM_1_AVOID),
        Step::Walk(Right),
        Step::Check("tristan", |r| {
            assert!(r.beaten(TRAINER_SIL_EVALON_GYM_TRISTAN));
            assert!(!r.has_text(Evalon_Gym_Piege));
        }),
        goto_avoiding(GYM, 5, 40, TRAPS),
        // Room 2: each golem twice up onto its plate.
        goto(GYM, 3, 37),
        Step::Interact(Up),
        Step::Walk(Up),
        Step::Interact(Up),
        goto(GYM, 7, 37),
        Step::Interact(Up),
        Step::Walk(Up),
        Step::Interact(Up),
        Step::Check("pont", |r| assert!(r.w.flag(FLAG_SIL_EVALON_GYM_PONT))),
        goto(GYM, 5, 31),
        Step::Walk(Up),
        Step::Check("renard", |r| assert!(r.beaten(TRAINER_SIL_EVALON_GYM_RENARD))),
        // Room 3: the crypt (the Surcharge Optique), Morgane.
        goto(GYM, 5, 25),
        Step::Walk(Up),
        Step::Check("morgane", |r| {
            assert!(r.beaten(TRAINER_SIL_EVALON_GYM_MORGANE));
        }),
        // Room 4: the Sphinge's riddles.
        goto(GYM, 5, 12),
        Step::Interact(Up),
        Step::Check("sphinge", |r| {
            assert!(r.w.flag(FLAG_SIL_EVALON_GYM_SPHINGE_PASSE));
            assert!(!r.beaten(TRAINER_SIL_EVALON_GYM_SPHINGE));
            r.printed_in_order(&[
                Evalon_Gym_Sphinge_T1,
                Evalon_Gym_Sphinge_Q1,
                Evalon_Gym_Sphinge_Q2,
                Evalon_Gym_Sphinge_Q3,
                Evalon_Gym_Sphinge_Ciel,
            ]);
        }),
        // Room 5: the right chest, Élise.
        goto_avoiding(GYM, 8, 6, ELISE_SIGHT),
        Step::Interact(Up),
        Step::Check("orbe", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_EVALON_ORBE));
            assert!(r.count(ITEM_SIL_ORBE_EVEIL) >= 1);
        }),
        goto_avoiding(GYM, 5, 4, ELISE_SIGHT),
        Step::Walk(Up),
        Step::Check("elise", |r| assert!(r.beaten(TRAINER_SIL_EVALON_GYM_ELISE))),
        goto(GYM, 5, 2),
        Step::Enter {
            dir: Up,
            map: GYM_2F,
        },
        // ---- Loulpix and Doris (Sc09) ----
        goto(GYM_2F, 5, 6),
        Step::Interact(Up),
        Step::Check("evalon_sc09", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 85);
            assert!(r.w.flag(FLAG_SIL_BADGE_08));
            assert!(r.w.flag(FLAG_SIL_MODULE_PLONGEE));
            assert!(r.w.flag(FLAG_SIL_COEUR_EVALON));
            assert!(r.w.flag(FLAG_SIL_LOG_08));
            assert!(r.w.flag(FLAG_SIL_HIDE_EVALON_AMIS));
            assert_eq!(r.w.var(VAR_SIL_SYNC), 80);
            for (item, n) in [
                (ITEM_SIL_COEUR_EVALON, 1),
                (ITEM_TM19, 1),
                (ITEM_TM23, 1),
                (ITEM_SIL_EPEE_LEGENDE, 1),
                (ITEM_PERSIM_BERRY, 3),
            ] {
                assert!(r.count(item) >= n, "item {item}");
            }
            r.printed_in_order(&[
                Evalon_Sc09_T1,
                Evalon_Sc09_T2,
                Evalon_Sc09_T3,
                Evalon_Sc09_T5,
                Evalon_Sc09_T7,
                Evalon_Sc09_Epee_T1,
                Evalon_Sc09_T8,
                Evalon_Sc09_T9,
            ]);
        }),
        goto(GYM_2F, 6, 6),
        Step::Interact(Up),
        goto(GYM_2F, 5, 10),
        Step::Enter {
            dir: Down,
            map: GYM,
        },
        goto_avoiding(GYM, 5, 53, TRAPS),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        // ---- the departure (Sc10) by the south-east path ----
        goto(TOWN, 42, 44),
        Step::Walk(Down),
    ]);
    s
}

/// What the end of Chapter 8 asserts beyond its checkpoints: the story, the badge, module and
/// log, the bag, no dropped event, and every Chapter 8 fallback the path meets reported.
pub fn check_end(r: &Run) {
    assert_eq!(r.w.map(), TOWN);
    assert_eq!(r.w.var(VAR_SIL_STORY), 86);
    assert!(r.has_text(Evalon_Sc10_T1));
    assert!(r.w.flag(FLAG_SIL_BADGE_08));
    assert!(r.w.flag(FLAG_SIL_MODULE_PLONGEE));
    assert!(r.w.flag(FLAG_SIL_LOG_08));
    assert_eq!(r.w.var(VAR_SIL_SYNC), 80);
    assert_eq!(r.w.var(VAR_SIL_EVALON_RENOM), 10);
    assert_eq!(r.w.events_dropped(), 0);
    for name in [
        "play_se",
        "screen_shake",
        "strength",
        "dark_map",
        "hidden item",
        "read_item",
        "multichoice",
        "play_music",
        "battle_multi",
        "mega_evolution",
        "play_fanfare",
        "coeur_evalon",
        "fade_music",
        "pc",
    ] {
        assert!(r.degraded.contains(name), "{name}: {:?}", r.degraded);
    }
}
