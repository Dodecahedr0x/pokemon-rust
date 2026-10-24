//! Chapter 7, Reverse-City, from Route S7's south end (`VAR_SIL_STORY` 68, Chapter 6's end, the
//! Grand Torii behind) to the Porte Êta-Nord open (76):
//! - Route S7, the Pont de la Logique Brisée: the first data wall (Sc01, 70), the Full Heal, the
//!   four silica blocks broken with the Scanner de Faille (C1, C2, C-Puzzle, the trap's way out,
//!   C3), the five trainers, the Twisted Spoon, the Rare Candy, the Ultra Ball, the hidden
//!   Ether, the Max Revive, Adrien's holographic call (Sc02, 71), the north sign;
//! - Reverse-City: the welcome (Sc01, 72), the four mirror signs' plaques, the lighthouse (Oscar,
//!   TM11 at the lamp: Sc06, the Max Ether), the grunts, the emitters, the Centre, the
//!   "villain's" Revival Herb, the docker's Poudre Silice, the hidden items, the Supermarché,
//!   Gogo and Romain (Sc02, the Multi Exp V2, 73);
//! - the Palais: the guide, Anna and Xela on the rink, the middle mirror door, Nomis, Reverse
//!   (Gym Sc01, 74) and Benou (Gym Sc02: the corrupted badge, the Flux Turbine, sync 70,
//!   `LOG_07`, 75), the Lum Berry and the Alakazite;
//! - out: the vault (the alley block, the plaques' code, the Master Ball: Sc04), Benou at the
//!   north gate (Sc07, 76), Route S8.
//!
//! The controls are inverted outside until Reverse falls (the driver presses the mapped keys).
//! **The fallbacks the path relies on** (`ch07_*.rs`): the Gogo–Romain double is two single
//! battles (each with its injector, SIL3), Reverse's battle is inverse (SIL3: its rule, from
//! the back), the vault's dial is one yes/no that dials the plaques' code.
//!
//! **The party** ([`PARTY`]): Chapter 6's end as `sil7_ch6` leaves it (KADABRA 60 leading, with
//! Chapter 6's Shock Wave, `givemon`'s moves; TYPHLOSION 64; LAIRON 35, MAREEP 14, PORYGON 25,
//! DUGTRIO 26). No hook. The chapter's trainers (44–49) train the lead; the run heals at the
//! Centre between the city's fights and before the Palais. Reverse's Wobbuffet (Counter, Mirror
//! Coat) outlasts the KADABRA, so the TYPHLOSION, second, is trained in the town's grass to
//! [`REVERSE_LEVEL`] (66) first. Measured (`SPINE_LEVEL_66`): 62 and 64 lose to Reverse; 63 and
//! 65–69 win (measured on the normal chart; the inverse one, SIL3, wins at 66 from the 60/64
//! start). The run ends with the KADABRA at 62 and the TYPHLOSION at 66.

use super::{Run, Start, Step, goto, goto_avoiding};
use content::consts::*;
use engine::data::{Dir, ItemId, MapId, SpeciesId};
use engine::script::Ctx;

const ROUTE: MapId = MAP_ROUTE_S07;
const TOWN: MapId = MAP_REVERSE_CITY;
const CENTER: MapId = MAP_REVERSE_CITY_POKEMON_CENTER;
const MART: MapId = MAP_REVERSE_CITY_MART;
const HOUSE: MapId = MAP_REVERSE_CITY_HOUSE_1;
const PHARE: MapId = MAP_REVERSE_CITY_PHARE;
const COFFRE: MapId = MAP_REVERSE_CITY_COFFRE;
const GYM: MapId = MAP_REVERSE_CITY_GYM;
const ROUTE_S08: MapId = MAP_ROUTE_S08;

/// Chapter 7's start: Route S7's south end (14, 70), just through the Grand Torii from
/// Manga-Square, facing north, Chapter 6's end state ([`end_of_chapter_6`]). Chapter 6 is not
/// chained into the spine yet.
pub fn start() -> Start {
    Start {
        map: ROUTE,
        x: 14,
        y: 70,
        facing: Dir::Up,
        setup: |ctx| Box::pin(end_of_chapter_6(ctx)),
    }
}

/// The lighthouse's mat (6, 19) at `VAR_SIL_STORY` 122, Chapter 12's Thomas scene (`Reverse_City_PG_Sc04`):
/// `ReverseCity_Phare_OnTransition` must show him.
pub fn phare_at_122() -> Start {
    Start {
        map: PHARE,
        x: 6,
        y: 19,
        facing: Dir::Up,
        setup: |ctx| {
            Box::pin(async move {
                end_of_chapter_6(ctx.clone()).await;
                ctx.set_var(VAR_SIL_STORY, 122).await;
            })
        },
    }
}

/// Chapter 7's start with a PIERRE LIEN in the bag (D-30): the item-triggered trade evolution
/// on the lead KADABRA.
pub fn with_pierre_lien() -> Start {
    Start {
        setup: |ctx| {
            Box::pin(async move {
                end_of_chapter_6(ctx.clone()).await;
                ctx.item_add(ItemId(ITEM_SIL_PIERRE_LIEN), 1).await;
            })
        },
        ..start()
    }
}

/// The same at 123, Thomas's hide flag cleared as his scene left it: the transition must hide
/// him again (the flag is written both ways).
pub fn phare_at_123() -> Start {
    Start {
        map: PHARE,
        x: 6,
        y: 19,
        facing: Dir::Up,
        setup: |ctx| {
            Box::pin(async move {
                end_of_chapter_6(ctx.clone()).await;
                ctx.set_var(VAR_SIL_STORY, 123).await;
                ctx.set_flag(FLAG_SIL_HIDE_THOMAS_PHARE, false).await;
            })
        },
    }
}

/// The start state's party: Chapter 6's end as its run (`sil7_ch6`) leaves it, the KADABRA
/// (with Chapter 6's Shock Wave, taught first) leading.
pub const PARTY: &[(SpeciesId, u8)] = &[
    (SPECIES_KADABRA, 60),
    (SPECIES_TYPHLOSION, 64),
    (SPECIES_LAIRON, 35),
    (SPECIES_MAREEP, 14),
    (SPECIES_PORYGON, 25),
    (SPECIES_DUGTRIO, 26),
];

/// Chapter 6's end state (`07-reverse-city.md`'s prerequisites), on top of Silice's new-game
/// script: Chapters 0–5 as `ch05`'s end, Chapter 6's badge, its Scanner de Faille
/// (`FLAG_SIL_MODULE_ECLATE_ROC`, the silica blocks) and Mega modules and its log, the sync at
/// 60, the story at 68 (Manga_Square_Sc03); the party ([`PARTY`]), a few potions, the respawn
/// at Manga-Square's Centre (Chapter 6's last).
async fn end_of_chapter_6(ctx: Ctx) {
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
        // Chapters 1 to 5.
        FLAG_SIL_BADGE_01,
        FLAG_SIL_BADGE_02,
        FLAG_SIL_BADGE_03,
        FLAG_SIL_BADGE_04,
        FLAG_SIL_BADGE_05,
        FLAG_SIL_MODULE_COUPE,
        FLAG_SIL_MODULE_FORCE,
        FLAG_SIL_MODULE_FLASH,
        FLAG_SIL_MODULE_SURF,
        FLAG_SIL_MODULE_VOL,
        FLAG_SIL_LOG_01,
        FLAG_SIL_LOG_02,
        FLAG_SIL_LOG_03,
        FLAG_SIL_LOG_04,
        FLAG_SIL_LOG_05,
        // Chapter 6.
        FLAG_SIL_BADGE_06,
        FLAG_SIL_MODULE_ECLATE_ROC,
        FLAG_SIL_MODULE_MEGA,
        FLAG_SIL_LOG_06,
    ] {
        ctx.set_flag(f, true).await;
    }
    ctx.set_var(VAR_SIL_STORY, 68).await;
    ctx.set_var(VAR_SIL_SYNC, 60).await;
    ctx.set_var(VAR_SIL_STARTER, 1).await;
    ctx.set_var(VAR_SIL_CH0_GATE, 1).await;
    ctx.item_add(ItemId(ITEM_SIL_MALLETTE), 1).await;
    ctx.item_add(ItemId(ITEM_SUPER_POTION), 5).await;
    ctx.item_add(ItemId(ITEM_HYPER_POTION), 3).await;
    ctx.money_add(20000).await;
    ctx.set_respawn(HEAL_LOCATION_SIL_MANGA_SQUARE).await;
    // The lead with Chapter 6's Shock Wave (`givemon`'s moves), then the others.
    super::give_kadabra_with_shock_wave(&ctx, PARTY[0].1).await;
    for &(species, level) in &PARTY[1..] {
        ctx.give_mon(species, level).await;
    }
}

/// The TYPHLOSION's level before the Palais: Reverse's Wobbuffet (Counter, Mirror Coat) is
/// fought by whoever stands after the lead, the TYPHLOSION (measures in the module docs).
const REVERSE_LEVEL: u8 = 66;

/// The Centre's nurse, from the town and back to its door.
const HEAL: &[Step] = &[
    goto_avoiding(TOWN, 4, 21, TOWN_AVOID),
    Step::Enter {
        dir: Dir::Up,
        map: CENTER,
    },
    goto(CENTER, 7, 4),
    Step::Interact(Dir::Up),
    Step::Check("centre", |r| {
        assert!(r.healed());
        assert!(r.has_text(Reverse_PC_Infirmiere_Bonus));
        assert!(r.has_text(gText_RestoredPkmnToFullHealth));
    }),
    goto(CENTER, 6, 7),
    Step::Enter {
        dir: Dir::Down,
        map: TOWN,
    },
];

/// What the town's paths avoid: the emitters' and the grunts' sight lines (each one is met on
/// purpose by its own step), and the cells next to the fence's gap (Sc02 is met on purpose).
const TOWN_AVOID: &[(i16, i16)] = &[
    (4, 12),
    (5, 12),
    (34, 12),
    (35, 12),
    (12, 33),
    (32, 22),
    (33, 22),
    (34, 22),
    (36, 27),
    (36, 28),
    (36, 29),
];

/// The gym's lying doors (room 2's left and right mirrors): never planned through.
const GYM_AVOID: &[(i16, i16)] = &[(2, 14), (3, 14), (10, 14), (11, 14)];

/// Out of the Palais from room 3 (its exit mat back to room 1) to the Centre, and back to room
/// 3 through the middle mirror (the beaten trainers stay quiet).
const GYM_HEAL: &[Step] = &[
    goto(GYM, 6, 10),
    Step::Walk(Dir::Down),
    goto(GYM, 6, 35),
    Step::Enter {
        dir: Dir::Down,
        map: TOWN,
    },
    goto_avoiding(TOWN, 4, 21, TOWN_AVOID),
    Step::Enter {
        dir: Dir::Up,
        map: CENTER,
    },
    goto(CENTER, 7, 4),
    Step::Interact(Dir::Up),
    Step::Check("centre_gym", |r| assert!(r.healed())),
    goto(CENTER, 6, 7),
    Step::Enter {
        dir: Dir::Down,
        map: TOWN,
    },
    goto_avoiding(TOWN, 5, 9, TOWN_AVOID),
    Step::Enter {
        dir: Dir::Up,
        map: GYM,
    },
    goto_avoiding(GYM, 6, 15, GYM_AVOID),
    Step::Walk(Dir::Up),
];

/// Chapter 7's steps, from [`start`].
pub fn steps() -> Vec<Step> {
    use Dir::*;
    let mut s = vec![
        // ---- Route S7 ----
        Step::Check("route_s07_sc01", |r| {
            assert_eq!(r.w.map(), ROUTE);
            assert_eq!(r.w.var(VAR_SIL_STORY), 70);
            assert!(r.has_text(Route_S07_Sc01_T1));
            assert!(r.w.flag(FLAG_SIL_BADGE_06));
            assert!(!r.w.flag(FLAG_SIL_BADGE_07));
            assert_eq!(r.w.party().len(), PARTY.len());
        }),
        Step::Check("lead_shock_wave", |r| {
            assert!(
                r.w.party()
                    .iter()
                    .next()
                    .unwrap()
                    .moves
                    .contains(&Some(MOVE_SHOCK_WAVE))
            );
        }),
        // S7-A: the entrance sign, the worker, the Full Heal, C1 across the bridge.
        goto(ROUTE, 16, 69),
        Step::Interact(Right),
        goto(ROUTE, 15, 65),
        Step::Interact(Right),
        goto(ROUTE, 17, 68),
        Step::Interact(Up),
        Step::Check("route_s07_a", |r| {
            assert!(r.has_text(Route_S07_Sign_Entree));
            assert!(r.has_text(Route_S07_Npc_Ouvrier));
            assert!(r.w.flag(FLAG_SIL_ITEM_ROUTE_S07_FULL_HEAL));
        }),
        goto(ROUTE, 13, 63),
        Step::Interact(Up),
        goto(ROUTE, 14, 63),
        Step::Interact(Up),
        Step::Check("route_s07_c1", |r| {
            assert!(r.has_text(Route_S07_Sign_Bloc));
            r.printed_in_order(&[Route_S07_Bloc_Scan, Route_S07_Bloc_Casse]);
        }),
        // The first technician (17, 56) sees the bridge's row; C2 and the Twisted Spoon.
        goto(ROUTE, 14, 56),
        Step::Check("route_s07_tech1", |r| {
            assert!(r.beaten(TRAINER_SIL_TECHNICIEN_S07_1));
        }),
        goto(ROUTE, 11, 52),
        Step::Interact(Left),
        goto(ROUTE, 9, 52),
        Step::Interact(Left),
        Step::Check("route_s07_c2", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_ROUTE_S07_TWISTED_SPOON));
            assert!(r.count(ITEM_TWISTED_SPOON) >= 1);
        }),
        // S7-B, the puzzle: the premises, the Rare Candy in the trap, Pascal, the Ultra Ball.
        goto(ROUTE, 14, 45),
        Step::Interact(Left),
        goto(ROUTE, 14, 40),
        Step::Interact(Left),
        // The arrows slide: the trap's only way out is C-Puzzle, broken (then up the west arrows).
        goto(ROUTE, 13, 40),
        Step::Interact(Left),
        goto(ROUTE, 18, 37),
        Step::Interact(Down),
        goto(ROUTE, 10, 37),
        Step::Check("route_s07_b", |r| {
            assert!(r.has_text(Route_S07_Sign_Premisses));
            assert!(r.w.flag(FLAG_SIL_ITEM_ROUTE_S07_RARE_CANDY));
            assert!(r.w.flag(FLAG_SIL_ITEM_ROUTE_S07_ULTRA_BALL));
            assert!(r.beaten(TRAINER_SIL_LOGICIEN_PASCAL));
        }),
        // Adrien's call at the Pile du Milieu's entrance.
        goto(ROUTE, 14, 34),
        Step::Check("route_s07_sc02", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 71);
            assert!(r.w.flag(FLAG_SIL_ADRIEN_TRADUCTION_NZ));
            r.printed_in_order(&[
                Route_S07_Sc02_T1,
                Route_S07_Sc02_T2,
                Route_S07_Sc02_T7,
                Route_S07_Sc02_T8,
            ]);
        }),
        // S7-C: the hidden Ether, the philosopher, Sélène and Mira.
        goto(ROUTE, 20, 32),
        Step::Interact(Right),
        goto(ROUTE, 19, 30),
        Step::Interact(Up),
        goto(ROUTE, 14, 30),
        goto(ROUTE, 19, 27),
        Step::Check("route_s07_c", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_ROUTE_S07_HIDDEN_ETHER));
            assert!(r.has_text(Route_S07_Npc_Philosophe));
            assert!(r.beaten(TRAINER_SIL_PSY_SELENE));
            assert!(r.beaten(TRAINER_SIL_MYSTIQUE_MIRA));
        }),
        // S7-D: the second technician across the walkway, C3 and the Max Revive.
        goto(ROUTE, 14, 16),
        goto(ROUTE, 15, 9),
        Step::Interact(Right),
        goto(ROUTE, 17, 9),
        Step::Interact(Right),
        Step::Check("route_s07_d", |r| {
            assert!(r.beaten(TRAINER_SIL_TECHNICIEN_S07_2));
            assert!(r.w.flag(FLAG_SIL_ITEM_ROUTE_S07_MAX_REVIVE));
        }),
        // The gate: its sign, the tourist, Reverse-City.
        goto(ROUTE, 12, 2),
        Step::Interact(Up),
        Step::Interact(Left),
        Step::Check("route_s07_porte", |r| {
            assert!(r.has_text(Route_S07_Sign_Porte));
            assert!(r.has_text(Route_S07_Npc_Touriste));
        }),
        goto(ROUTE, 14, 2),
        Step::Enter { dir: Up, map: TOWN },
        // ---- Reverse-City ----
        Step::Walk(Up),
        Step::Check("reverse_sc01", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 72);
            // Inverted controls from the welcome (the driver presses the mapped keys).
            assert_eq!(r.w.input_map(&r.g), engine::InputMap::Inverted);
            r.printed_in_order(&[
                Reverse_City_Sc01_T1,
                Reverse_City_Sc01_T2,
                Reverse_City_Sc01_T3,
                Reverse_City_Sc01_T4,
            ]);
        }),
        // The south plaques: the city's sign (1) and the lighthouse's (3).
        goto(TOWN, 29, 33),
        Step::Interact(Down),
        goto_avoiding(TOWN, 31, 29, TOWN_AVOID),
        Step::Interact(Up),
        Step::Check("plaques_sud", |r| {
            assert!(r.w.flag(FLAG_SIL_REVERSE_PLAQUE_1));
            assert!(r.w.flag(FLAG_SIL_REVERSE_PLAQUE_3));
        }),
        // The lighthouse: Oscar on the ramp, the lamp's TM (Sc06), the Max Ether.
        goto_avoiding(TOWN, 29, 29, TOWN_AVOID),
        Step::Enter {
            dir: Up,
            map: PHARE,
        },
        goto(PHARE, 12, 8),
        goto(PHARE, 6, 8),
        Step::Interact(Up),
        goto(PHARE, 1, 12),
        Step::Interact(Up),
        Step::Check("phare", |r| {
            assert!(r.beaten(TRAINER_SIL_GARDIEN_OSCAR));
            assert!(r.w.flag(FLAG_SIL_ITEM_PHARE_TM_ZENITH));
            assert!(r.count(ITEM_TM11) >= 1);
            assert!(r.w.flag(FLAG_SIL_ITEM_PHARE_MAX_ETHER));
            r.printed_in_order(&[Phare_Oscar_Intro, Phare_Sign_Lampe, Phare_T1]);
        }),
        goto(PHARE, 6, 18),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
    ];
    s.extend_from_slice(HEAL);
    s.extend_from_slice(&[
        // The east pontoon's hidden Poudre Silice, then the grunt on the quays (36, 30).
        goto_avoiding(TOWN, 27, 30, TOWN_AVOID),
        Step::Interact(Left),
        goto(TOWN, 36, 29),
        Step::Check("sbire_2", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_REVERSE_POUDRE_SILICE_2));
            assert!(r.beaten(TRAINER_SIL_SBIRE_REVERSE_2));
        }),
    ]);
    s.extend_from_slice(HEAL);
    s.extend_from_slice(&[
        // The other grunt (35, 22), the docker's Poudre Silice (Sc05), bin 4's HP Up.
        goto_avoiding(TOWN, 33, 21, TOWN_AVOID),
        Step::Walk(Down),
        Step::Check("sbire_1", |r| {
            assert!(r.beaten(TRAINER_SIL_SBIRE_REVERSE_1));
        }),
        goto_avoiding(TOWN, 22, 26, TOWN_AVOID),
        Step::Interact(Left),
        goto_avoiding(TOWN, 24, 24, TOWN_AVOID),
        Step::Interact(Down),
        Step::Check("poudre_silice", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_REVERSE_POUDRE_SILICE));
            assert!(r.count(ITEM_SIL_POUDRE_SILICE) >= 2);
            r.printed_in_order(&[Poudre_Silice_Ramasse]);
            assert!(r.w.flag(FLAG_SIL_ITEM_REVERSE_HP_UP));
        }),
        // The third emitter by the south quay (12, 34).
        goto_avoiding(TOWN, 13, 33, TOWN_AVOID),
        Step::Walk(Left),
        Step::Check("emetteur_3", |r| {
            assert!(r.beaten(TRAINER_SIL_EMETTEUR_3));
            assert!(r.w.flag(FLAG_SIL_REVERSE_EMETTEUR_3));
            assert!(r.has_text(Emetteur_T3));
        }),
        // The "villain's" house: his Revival Herb; bin 2's PP Up in the west alley.
        goto_avoiding(TOWN, 10, 21, TOWN_AVOID),
        Step::Enter {
            dir: Up,
            map: HOUSE,
        },
        goto(HOUSE, 2, 4),
        Step::Interact(Up),
        Step::Check("vieil_homme", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_REVERSE_REVIVAL_HERB));
            assert!(r.count(ITEM_REVIVAL_HERB) >= 1);
            assert!(r.has_text(Reverse_House1_T1));
        }),
        goto(HOUSE, 3, 5),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        goto_avoiding(TOWN, 2, 23, TOWN_AVOID),
        Step::Interact(Up),
        Step::Check("pp_up", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_REVERSE_PP_UP));
        }),
    ]);
    s.extend_from_slice(HEAL);
    s.extend_from_slice(&[
        // Gogo and Romain at the fence's gap (Sc02).
        goto_avoiding(TOWN, 19, 17, TOWN_AVOID),
        Step::Walk(Up),
        Step::Check("reverse_sc02", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 73);
            assert!(r.beaten(TRAINER_SIL_GOGO_3));
            assert!(r.beaten(TRAINER_SIL_ROMAIN_3));
            assert!(r.w.flag(FLAG_SIL_ITEM_REVERSE_MULTI_EXP_V2));
            assert!(r.count(ITEM_SIL_MULTI_EXP_V2) >= 1);
            assert!(r.w.flag(FLAG_SIL_HIDE_REVERSE_GARDE_PALAIS));
            r.printed_in_order(&[
                Reverse_City_Sc02_T1,
                Reverse_City_Sc02_T4,
                Reverse_City_Sc02_D1,
                Reverse_City_Sc02_D2,
                Reverse_City_Sc02_T5,
                Reverse_City_Sc02_T8,
                Reverse_City_Sc02_T9,
            ]);
        }),
    ]);
    s.extend_from_slice(HEAL);
    s.extend_from_slice(&[
        // The north: the two emitters, the Supermarché's plaque (2) and its cashier.
        goto_avoiding(TOWN, 34, 13, TOWN_AVOID),
        Step::Walk(Up),
        goto_avoiding(TOWN, 34, 9, TOWN_AVOID),
        Step::Interact(Up),
        goto_avoiding(TOWN, 31, 9, TOWN_AVOID),
        Step::Enter { dir: Up, map: MART },
        goto(MART, 3, 3),
        Step::Interact(Left),
        goto(MART, 2, 7),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        goto_avoiding(TOWN, 4, 13, TOWN_AVOID),
        Step::Walk(Up),
        Step::Check("nord", |r| {
            assert!(r.beaten(TRAINER_SIL_EMETTEUR_1));
            assert!(r.beaten(TRAINER_SIL_EMETTEUR_2));
            assert!(r.w.flag(FLAG_SIL_REVERSE_EMETTEUR_1));
            assert!(r.w.flag(FLAG_SIL_REVERSE_EMETTEUR_2));
            assert!(r.w.flag(FLAG_SIL_REVERSE_PLAQUE_2));
            assert!(r.degraded.contains("mart"), "{:?}", r.degraded);
        }),
        // The Palais' plaque (4).
        goto_avoiding(TOWN, 2, 10, TOWN_AVOID),
        Step::Interact(Up),
        Step::Check("plaque_4", |r| {
            assert!(r.w.flag(FLAG_SIL_REVERSE_PLAQUE_4));
        }),
    ]);
    // The TYPHLOSION trained for Reverse (see [`REVERSE_LEVEL`]), in the town's grass.
    s.extend_from_slice(&[
        Step::Swap(0, 1),
        Step::Train {
            a: (17, 21),
            b: (18, 21),
            level: REVERSE_LEVEL,
            heal: HEAL,
        },
        Step::Swap(0, 1),
    ]);
    s.extend_from_slice(HEAL);
    s.extend_from_slice(&[
        // ---- the Palais ----
        goto_avoiding(TOWN, 5, 9, TOWN_AVOID),
        Step::Enter { dir: Up, map: GYM },
        goto(GYM, 5, 32),
        Step::Interact(Left),
        // Room 1: the Lum Berry, then Anna and Xela on the way up.
        goto(GYM, 3, 25),
        Step::Interact(Left),
        goto(GYM, 6, 18),
        goto_avoiding(GYM, 6, 15, GYM_AVOID),
        Step::Check("gym_patinoire", |r| {
            assert!(r.has_text(Gym07_Guide));
            assert!(r.w.flag(FLAG_SIL_ITEM_GYM07_LUM_BERRY));
            assert!(r.beaten(TRAINER_SIL_PATINEUSE_ANNA));
            assert!(r.beaten(TRAINER_SIL_MIME_XELA));
        }),
        // Room 2: the two statues, then the middle mirror.
        goto_avoiding(GYM, 4, 15, GYM_AVOID),
        Step::Interact(Up),
        goto_avoiding(GYM, 9, 15, GYM_AVOID),
        Step::Interact(Up),
        goto_avoiding(GYM, 6, 15, GYM_AVOID),
        Step::Walk(Up),
        // Room 3: Nomis.
        goto(GYM, 6, 7),
        Step::Check("gym_nomis", |r| {
            assert!(r.has_text(Gym07_Statue_1));
            assert!(r.has_text(Gym07_Statue_2));
            assert!(r.beaten(TRAINER_SIL_PSY_NOMIS));
        }),
    ]);
    s.extend_from_slice(GYM_HEAL);
    s.extend_from_slice(&[
        // Reverse (6, 4), from the row behind him; Benou follows at once.
        goto(GYM, 6, 6),
        Step::Walk(Up),
        Step::Check("gym_sc01_sc02", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 75);
            assert!(r.beaten(TRAINER_SIL_REVERSE_1));
            assert!(r.w.flag(FLAG_SIL_REVERSE_BATTU));
            assert!(r.w.flag(FLAG_SIL_BADGE_07));
            assert!(r.w.flag(FLAG_SIL_BADGE_07_CORROMPU));
            assert!(r.w.flag(FLAG_SIL_MODULE_CASCADE));
            assert!(r.w.flag(FLAG_SIL_LOG_07));
            assert_eq!(r.w.var(VAR_SIL_SYNC), 70);
            r.printed_in_order(&[
                Gym07_Sc01_T1,
                Gym07_Sc01_T3,
                Gym07_Sc01_D,
                Gym07_Sc01_T4,
                Gym07_Sc01_T7,
                Gym07_Sc02_T1,
                Gym07_Sc02_T3,
                Gym07_Sc02_T5,
                Gym07_Sc02_T6,
                Gym07_Sc02_Log07,
                Gym07_Sc02_T9,
            ]);
        }),
        // The Alakazite where Reverse stood.
        Step::Interact(Up),
        Step::Check("alakazite", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_GYM07_ALAKAZITE));
            assert!(r.count(ITEM_SIL_ALAKAZITE) >= 1);
        }),
        goto(GYM, 6, 10),
        Step::Walk(Down),
        goto(GYM, 6, 35),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        // ---- out: the vault, the north gate ----
        goto_avoiding(TOWN, 10, 7, TOWN_AVOID),
        Step::Interact(Up),
        goto(TOWN, 13, 4),
        Step::Enter {
            dir: Up,
            map: COFFRE,
        },
        goto(COFFRE, 4, 8),
        Step::Interact(Up),
        Step::Check("coffre_sc04", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_REVERSE_MASTER_BALL));
            assert!(r.count(ITEM_MASTER_BALL) >= 1);
            r.printed_in_order(&[Coffre_T1, Coffre_T2, Coffre_T3]);
        }),
        goto(COFFRE, 4, 8),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        // The alley block is back (a temp flag, as Gen3's rocks): broken again on the way out.
        goto(TOWN, 10, 5),
        Step::Interact(Down),
        goto_avoiding(TOWN, 27, 4, TOWN_AVOID),
        Step::Walk(Up),
        Step::Check("reverse_sc07", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 76);
            assert!(r.w.flag(FLAG_SIL_REVERSE_PORTE_NORD_OUVERTE));
            r.printed_in_order(&[
                Reverse_City_Sc07_T1,
                Reverse_City_Sc07_T2,
                Reverse_City_Sc07_T4,
            ]);
        }),
        goto(TOWN, 27, 1),
        Step::Enter {
            dir: Up,
            map: ROUTE_S08,
        },
    ]);
    s
}

/// What the end of Chapter 7 asserts beyond its checkpoints: the story, the badge, module and
/// log, the bag, no dropped event, and every Chapter 7 fallback the path meets reported.
pub fn check_end(r: &Run) {
    assert_eq!(r.w.map(), ROUTE_S08);
    assert_eq!(r.w.var(VAR_SIL_STORY), 76);
    assert!(r.w.flag(FLAG_SIL_BADGE_07));
    assert!(r.w.flag(FLAG_SIL_MODULE_CASCADE));
    assert!(r.w.flag(FLAG_SIL_LOG_07));
    assert_eq!(r.w.var(VAR_SIL_SYNC), 70);
    for (item, n) in [
        (ITEM_FULL_HEAL, 1),
        (ITEM_TWISTED_SPOON, 1),
        (ITEM_RARE_CANDY, 1),
        (ITEM_ULTRA_BALL, 1),
        (ITEM_ETHER, 1),
        (ITEM_MAX_REVIVE, 1),
        (ITEM_TM11, 1),
        (ITEM_MAX_ETHER, 1),
        (ITEM_SIL_MULTI_EXP_V2, 1),
        (ITEM_REVIVAL_HERB, 1),
        (ITEM_PP_UP, 1),
        (ITEM_HP_UP, 1),
        (ITEM_LUM_BERRY, 1),
        (ITEM_SIL_ALAKAZITE, 1),
        (ITEM_MASTER_BALL, 1),
    ] {
        assert!(r.count(item) >= n, "item {item}");
    }
    assert_eq!(r.w.events_dropped(), 0);
    for name in [
        "play_se",
        "show_hologram",
        "hide_hologram",
        "play_music",
        "battle_two_trainers",
        "idle_timer",
        "dark_map",
        "black_light",
        "multichoice",
        "ice",
        "play_fanfare",
        "show_sync",
        "mart",
        "hidden item",
    ] {
        assert!(r.degraded.contains(name), "{name}: {:?}", r.degraded);
    }
}
