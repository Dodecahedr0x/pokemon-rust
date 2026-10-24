//! Chapter 4, Port-Ressac, from its start state ([`start`]: `VAR_SIL_STORY` 36, Chapter 3's
//! end, on Route S4's west edge) to Gwada beaten (49) and Julien's call on the jetty:
//! - Route S4, the Tunnel de Verre: Adrien and the Magicarpe (Sc02), the dark wall, M8 (Sc03);
//! - Port-Ressac: the arrival (Sc01), Crow's lookout (Sc02), Gogo and Romain (Sc03), the cargo's
//!   log (Navire Sc01), the log shown to Samuel (Sc04), the quay cleaning and the foreman (Sc05);
//! - the Zenith-Grog: the deck's crew, the hold's barrels (pushed with Champ Magnétique) and its
//!   explosive barrels (the shortcut back for a heal), Gus, Samuel (Gym Sc01–Sc02), Gwada on
//!   the coupée (Sc03); Julien's call on the jetty (Sc06).
//!
//! **The party.** Chapter 3's end party and a KADABRA lead ([`PARTY`]). The lead trains in the
//! port's dunes to [`CLEANING_LEVEL`] before the cleaning and to [`GYM_LEVEL`] before the gym;
//! the run heals at the Taverne between the quay grunts and between the gym's fights (out by
//! the gangway, back through the blown barrels). Every battle is won by the battle policy.

use super::{Run, Start, Step, goto};
use content::consts::*;
use engine::data::{Dir, ItemId, MapId, SpeciesId};
use engine::script::Ctx;

const TUNNEL: MapId = MAP_ROUTE_S04;
const PORT: MapId = MAP_PORT_RESSAC;
const TAVERNE: MapId = MAP_PORT_RESSAC_TAVERNE;
const NAVIRE: MapId = MAP_PORT_RESSAC_NAVIRE_COCKET;
const GYM: MapId = MAP_PORT_RESSAC_GYM;
const CALE: MapId = MAP_PORT_RESSAC_GYM_CALE;

/// Out of the Zenith-Grog by its gangway, from anywhere on the deck.
const OUT_OF_GYM: &[Step] = &[
    goto(GYM, 9, 20),
    Step::Enter {
        dir: Dir::Down,
        map: PORT,
    },
];

/// The Taverne's nurse, from the port and back to its door (Train's heal).
const HEAL: &[Step] = &[
    goto(PORT, 4, 8),
    Step::Enter {
        dir: Dir::Up,
        map: TAVERNE,
    },
    goto(TAVERNE, 9, 4),
    Step::Interact(Dir::Up),
    goto(TAVERNE, 7, 9),
    Step::Enter {
        dir: Dir::Down,
        map: PORT,
    },
];

/// Chapter 4's start: Route S4's west edge (1, 8), facing east, Chapter 3's end state
/// ([`end_of_chapter_3`]). Chapter 3 is not chained into the spine yet.
pub fn start() -> Start {
    Start {
        map: TUNNEL,
        x: 1,
        y: 8,
        facing: Dir::Right,
        setup: |ctx| Box::pin(end_of_chapter_3(ctx)),
    }
}

/// The start state's party: Chapter 3's end party as its runs (`sil4_ch3`, both ways in) leave
/// it, the TYPHLOSION 44, LAIRON 35 (both trained for the leaders' double battle), MAREEP 13,
/// the Casino's PORYGON 25 and the leaders' DUGTRIO 26 (`ch03::FIFTH`), and one more, a KADABRA
/// at 24: Chapter 3's own Viber-City grass has one at 23–24 (`rencontres.ron`, 5 %), caught and
/// never used. The KADABRA leads: the player's answer to a Fighting gym, and the one
/// [`Step::Train`] raises (it trains the lead) to [`CLEANING_LEVEL`], where it has learnt
/// Psychic (36). (The chained Chapter 3 run ends with a FLAAFFY 17 instead of the MAREEP 13.)
///
/// A TYPHLOSION lead loses to Samuel with the learn policy too, at a lead of 44, 47, 49 and 51
/// (it fights with Flame Wheel, Ember and Swift; Hariyama, Machoke, Medicham and Poliwrath are
/// left standing). See the plan's Ch4 section for the measures.
pub const PARTY: &[(SpeciesId, u8)] = &[
    (SPECIES_KADABRA, 24),
    (SPECIES_TYPHLOSION, 44),
    (SPECIES_LAIRON, 35),
    (SPECIES_MAREEP, 13),
    (SPECIES_PORYGON, 25),
    (SPECIES_DUGTRIO, 26),
];

/// The lead's level before the quay cleaning (its grunts and the foreman). 35, 36 and 37 win;
/// 34 (with a gym level of 36) loses in the hold.
pub const CLEANING_LEVEL: u8 = 36;

/// The lead's level before the Zenith-Grog (Samuel's six are 33–38, then Gwada's three, 35–37,
/// without a heal). The cleaning already brings the KADABRA to 37; 36, 37 and 38 win.
pub const GYM_LEVEL: u8 = 37;

/// Chapter 3's end state (`04-port-ressac.md`'s prerequisites), on top of Silice's new-game
/// script: the story at 36, the sync at 30, three badges, the Lame-Fréquence, Champ Magnétique
/// and Flash modules, `LOG_01` to `LOG_03`; Chapter 0's Mallette, running shoes and starter
/// (CYNDAQUIL's line); the party ([`PARTY`]), a few Potions, the respawn at Gris-Échec's Centre
/// (the last one on the way).
async fn end_of_chapter_3(ctx: Ctx) {
    chapter_3_state(&ctx).await;
    for &(species, level) in PARTY {
        ctx.give_mon(species, level).await;
    }
}

/// [`end_of_chapter_3`] without the party.
async fn chapter_3_state(ctx: &Ctx) {
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
        // Chapters 1 to 3.
        FLAG_SIL_BADGE_01,
        FLAG_SIL_BADGE_02,
        FLAG_SIL_BADGE_03,
        FLAG_SIL_MODULE_COUPE,
        FLAG_SIL_MODULE_FORCE,
        FLAG_SIL_MODULE_FLASH,
        FLAG_SIL_LOG_01,
        FLAG_SIL_LOG_02,
        FLAG_SIL_LOG_03,
    ] {
        ctx.set_flag(f, true).await;
    }
    ctx.set_var(VAR_SIL_STORY, 36).await;
    ctx.set_var(VAR_SIL_SYNC, 30).await;
    ctx.set_var(VAR_SIL_STARTER, 1).await;
    ctx.set_var(VAR_SIL_CH0_GATE, 1).await;
    ctx.item_add(ItemId(ITEM_SIL_MALLETTE), 1).await;
    ctx.item_add(ItemId(ITEM_POTION), 5).await;
    ctx.item_add(ItemId(ITEM_SUPER_POTION), 3).await;
    ctx.money_add(6000).await;
    ctx.set_respawn(HEAL_LOCATION_SIL_GRIS_ECHEC).await;
}

// ---- Samuel's defeat branch (R-7) ----

/// The money both defeat runs start with: no whiteout (R-7), Samuel takes his fixed 2000 (the
/// `take_money_percent` fallback).
const DEFEAT_MONEY: u32 = 6000;

/// In front of Samuel at the helm (13, 4), facing him, the port freed (`VAR_SIL_STORY` 47), with
/// a party that loses: a MAGIKARP at 5. Before any loss: the breath test runs.
pub fn samuel_first_loss() -> Start {
    Start {
        map: GYM,
        x: 13,
        y: 4,
        facing: Dir::Up,
        setup: |ctx| Box::pin(at_samuel(ctx, false)),
    }
}

/// [`samuel_first_loss`] after a first loss and Gwada beaten (`VAR_SIL_STORY` 49): the
/// rematch prompt, no breath test, the Hydro-Propulseur already given.
pub fn samuel_rematch() -> Start {
    Start {
        map: GYM,
        x: 13,
        y: 4,
        facing: Dir::Up,
        setup: |ctx| Box::pin(at_samuel(ctx, true)),
    }
}

async fn at_samuel(ctx: Ctx, rematch: bool) {
    chapter_3_state(&ctx).await;
    for f in [
        FLAG_SIL_ROUTE_S04_MURS,
        FLAG_SIL_ROUTE_S04_ADRIEN,
        FLAG_SIL_PORT_CROW_GUET,
        FLAG_SIL_PORT_GARDE_DORT,
        FLAG_SIL_PORT_VIEUX_PATCH,
        FLAG_SIL_LOG_04,
        FLAG_SIL_PORT_SAMUEL_ALLIE,
        FLAG_SIL_HIDE_PORT_CARGO,
        FLAG_SIL_PORT_LIBERE,
    ] {
        ctx.set_flag(f, true).await;
    }
    let story = if rematch {
        for f in [
            FLAG_SIL_PORT_GYM_ALCOOTEST,
            FLAG_SIL_SAMUEL_PERDU,
            FLAG_SIL_MODULE_SURF,
            FLAG_SIL_PORT_GWADA_BATTU,
        ] {
            ctx.set_flag(f, true).await;
        }
        49
    } else {
        47
    };
    ctx.set_var(VAR_SIL_STORY, story).await;
    ctx.money_remove(u32::MAX).await;
    ctx.money_add(DEFEAT_MONEY).await;
    ctx.give_mon(SPECIES_MAGIKARP, 5).await;
}

/// The first loss: the breath test (yes to every shot: the second is wrong, so the lead starts
/// confused), the battle lost without a whiteout (R-7), and the defeat branch at once, on the
/// quarterdeck.
pub fn samuel_first_loss_steps() -> Vec<Step> {
    vec![
        Step::Interact(Dir::Up),
        Step::Settle,
        Step::Check("first_loss", |r| {
            assert_eq!(r.w.map(), GYM);
            assert_eq!(r.pos(), (13, 4));
            assert_eq!(r.w.var(VAR_SIL_STORY), 48);
            assert_eq!(r.w.var(VAR_SIL_PORT_RESSAC_STATE), 0);
            assert!(r.w.flag(FLAG_SIL_SAMUEL_PERDU));
            assert!(r.w.flag(FLAG_SIL_MODULE_SURF));
            assert!(!r.w.flag(FLAG_SIL_BADGE_04));
            assert!(!r.w.flag(FLAG_SIL_PORT_GYM_CONFUS));
            assert!(!r.beaten(TRAINER_SIL_SAMUEL_1));
            assert_eq!(r.w.money(), DEFEAT_MONEY - 2000);
            assert_eq!(r.count(ITEM_TM01), 0);
            r.printed_in_order(&[
                Port_Ressac_Gym_Sc01_T1,
                Port_Ressac_Gym_Sc01_Rate,
                Port_Ressac_Gym_Sc01_T2,
                Port_Ressac_Gym_Sc02_Perdu,
                Port_Ressac_Gym_Sc02_Perdu_Surf,
                Port_Ressac_Gym_Sc02_Module,
                Port_Ressac_Gym_Sc02_Perdu_Badge,
            ]);
            assert_eq!(r.times(Port_Ressac_Gym_Sc02_Perdu), 1);
            for f in ["take_money_percent", "take_heal_items"] {
                assert!(r.degraded.contains(f), "fallback {f} not reported");
            }
        }),
    ]
}

/// The rematch lost: the prompt, straight to the battle, and the second defeat line; the story
/// does not go back.
pub fn samuel_rematch_steps() -> Vec<Step> {
    vec![
        Step::Interact(Dir::Up),
        Step::Settle,
        Step::Check("rematch_loss", |r| {
            assert_eq!(r.w.map(), GYM);
            assert_eq!(r.w.var(VAR_SIL_STORY), 49);
            assert_eq!(r.w.var(VAR_SIL_PORT_RESSAC_STATE), 0);
            assert!(!r.w.flag(FLAG_SIL_BADGE_04));
            assert_eq!(r.w.money(), DEFEAT_MONEY - 2000);
            assert!(!r.has_text(Port_Ressac_Gym_Sc01_T1));
            assert!(!r.has_text(Port_Ressac_Gym_Sc02_Perdu_Surf));
            r.printed_in_order(&[
                Port_Ressac_Gym_Revanche,
                Port_Ressac_Gym_Sc01_T2,
                Port_Ressac_Gym_Sc02_Perdu,
                Port_Ressac_Gym_Sc02_Perdu2,
            ]);
        }),
    ]
}

/// Chapter 4's steps.
pub fn steps() -> Vec<Step> {
    use Dir::*;
    [
        &[
            Step::Check("ch4_entry", |r| {
                assert_eq!(r.w.map(), TUNNEL);
                assert_eq!(r.w.var(VAR_SIL_STORY), 40);
                assert!(r.has_text(Route_S04_Sc01_T1));
            }),
            goto(TUNNEL, 12, 3),
            Step::Interact(Up),
            Step::Check("m1_dark", |r| assert!(r.has_text(Route_S04_Mur_Sombre))),
            goto(TUNNEL, 26, 8),
            Step::Check("sc02", |r| {
                assert_eq!(r.w.var(VAR_SIL_STORY), 41);
                assert!(r.w.flag(FLAG_SIL_ROUTE_S04_MURS));
                assert!(r.w.flag(FLAG_SIL_ROUTE_S04_ADRIEN));
                assert!(r.w.flag(FLAG_SIL_HIDE_ROUTE_S04_ADRIEN));
                assert!(r.w.flag(FLAG_SIL_HIDE_ROUTE_S04_MAGICARPE));
                r.printed_in_order(&[
                    Route_S04_Sc02_T1,
                    Route_S04_Sc02_T2,
                    Route_S04_Sc02_T3,
                    Route_S04_Sc02_T4,
                ]);
            }),
            goto(TUNNEL, 58, 3),
            Step::Interact(Up),
            Step::Interact(Up),
            Step::Check("sc03", |r| {
                assert!(r.w.flag(FLAG_SIL_ROUTE_S04_M8));
                assert_eq!(r.times(Route_S04_Sc03_T1), 1);
            }),
            goto(TUNNEL, 56, 3),
            Step::Enter { dir: Up, map: PORT },
            Step::Check("port_sc01", |r| {
                assert_eq!(r.w.var(VAR_SIL_STORY), 42);
                assert!(r.has_text(Port_Ressac_Sc01_T1));
            }),
            // Crow's lookout: yes to everything (the settle policy), so the patrols are called
            // right and the two others whistled for nothing (2 out of 4, no Star Piece).
            goto(PORT, 44, 16),
            Step::Interact(Up),
            Step::Check("crow", |r| {
                assert_eq!(r.w.var(VAR_SIL_STORY), 43);
                assert!(r.w.flag(FLAG_SIL_PORT_CROW_GUET));
                assert!(r.w.flag(FLAG_SIL_PORT_GARDE_DORT));
                assert_eq!(r.count(ITEM_TM31), 1);
                assert_eq!(r.w.var(VAR_SIL_PORT_RESSAC_STATE), 0);
                r.printed_in_order(&[
                    Port_Ressac_Sc02_T3,
                    Port_Ressac_Sc02_P1,
                    Port_Ressac_Sc02_P2,
                    Port_Ressac_Sc02_P3,
                    Port_Ressac_Sc02_P4,
                    Port_Ressac_Sc02_T4,
                    Port_Ressac_Sc02_T6,
                ]);
                assert_eq!(r.times(Port_Ressac_Sc02_Bon), 2);
            }),
            // Gogo and Romain on the jetty: the Vieux Patch.
            goto(PORT, 44, 21),
            Step::Check("jetty", |r| {
                assert_eq!(r.w.var(VAR_SIL_STORY), 44);
                assert_eq!(r.count(ITEM_SIL_VIEUX_PATCH), 1);
                assert!(r.has_text(Port_Ressac_Sc03_T6));
            }),
            // The cargo, its guard asleep: both grunts, then the log at the terminal.
        ][..],
        HEAL,
        &[
            goto(PORT, 7, 26),
            Step::Enter {
                dir: Up,
                map: NAVIRE,
            },
            // Into the second grunt's sight (7, 9) too: the path to the terminal can skirt it.
            goto(NAVIRE, 7, 10),
            Step::Walk(Up),
            goto(NAVIRE, 19, 2),
            Step::Interact(Up),
            Step::Check("log_04", |r| {
                assert_eq!(r.w.var(VAR_SIL_STORY), 45);
                assert!(r.w.flag(FLAG_SIL_LOG_04));
                assert!(r.beaten(TRAINER_SIL_PORT_CARGO_1));
                assert!(r.beaten(TRAINER_SIL_PORT_CARGO_2));
                assert!(r.has_text(Port_Ressac_Navire_Log04));
            }),
            goto(NAVIRE, 3, 12),
            Step::Enter {
                dir: Down,
                map: PORT,
            },
            // The log shown to Samuel at the Zenith-Grog's gangway.
            goto(PORT, 31, 26),
            Step::Interact(Right),
            Step::Check("samuel_ally", |r| {
                assert_eq!(r.w.var(VAR_SIL_STORY), 46);
                assert!(r.w.flag(FLAG_SIL_PORT_SAMUEL_ALLIE));
                r.printed_in_order(&[
                    Port_Ressac_Sc04_T1,
                    Port_Ressac_Sc04_T2,
                    Port_Ressac_Sc04_T5,
                    Port_Ressac_Sc04_T6,
                ]);
            }),
            // Training in the dunes before the cleaning (wild 26–30), healing at the Taverne.
            Step::Train {
                a: (11, 12),
                b: (12, 12),
                level: CLEANING_LEVEL,
                heal: HEAL,
            },
            Step::Check("trained_cleaning", |r| {
                eprintln!("party before the cleaning: {:?}", party(r));
            }),
        ][..],
        HEAL,
        &[
            // The quay cleaning: the four grunts along the corridor, a heal after each, then
            // the foreman.
            goto(PORT, 12, 20),
        ][..],
        HEAL,
        &[goto(PORT, 10, 20)][..],
        HEAL,
        &[goto(PORT, 8, 20)][..],
        HEAL,
        &[goto(PORT, 6, 20)][..],
        HEAL,
        &[
            goto(PORT, 5, 20),
            Step::Check("port_libere", |r| {
                assert_eq!(r.w.var(VAR_SIL_STORY), 47);
                assert!(r.w.flag(FLAG_SIL_PORT_LIBERE));
                assert!(r.w.flag(FLAG_SIL_HIDE_PORT_CARGO));
                for t in [
                    TRAINER_SIL_PORT_SBIRE_1,
                    TRAINER_SIL_PORT_SBIRE_2,
                    TRAINER_SIL_PORT_SBIRE_3,
                    TRAINER_SIL_PORT_SBIRE_4,
                ] {
                    assert!(r.beaten(t), "{t:?}");
                }
                assert!(r.has_text(Port_Ressac_Sc05_T4));
            }),
            // Training again before the gym.
            Step::Train {
                a: (11, 12),
                b: (12, 12),
                level: GYM_LEVEL,
                heal: HEAL,
            },
            Step::Check("trained", |r| {
                eprintln!("party before the gym: {:?}", party(r));
            }),
        ][..],
        HEAL,
        &[
            // The Zenith-Grog: the deck's crew (Patrice, then Jessy), back to the Taverne after
            // each.
            goto(PORT, 32, 26),
            Step::Enter { dir: Up, map: GYM },
            goto(GYM, 9, 17),
        ][..],
        OUT_OF_GYM,
        HEAL,
        &[
            goto(PORT, 32, 26),
            Step::Enter { dir: Up, map: GYM },
            goto(GYM, 13, 14),
        ][..],
        OUT_OF_GYM,
        HEAL,
        &[
            goto(PORT, 32, 26),
            Step::Enter { dir: Up, map: GYM },
            goto(GYM, 13, 14),
            Step::Enter { dir: Up, map: CALE },
            // The hold's barrels: B up twice, D right, A up twice.
            goto(CALE, 2, 7),
            Step::Interact(Up),
            Step::Walk(Up),
            Step::Interact(Up),
            goto(CALE, 3, 4),
            Step::Interact(Right),
            goto(CALE, 4, 4),
            Step::Interact(Up),
            Step::Walk(Up),
            Step::Interact(Up),
            Step::Check("barrels", |r| {
                assert!(r.has_text(Depot_Bloc_T2));
                assert_eq!(r.pos(), (4, 3));
            }),
            // Timothé by the ladder, then both explosive barrels: the shortcut back to the
            // hatch (the pushed barrels go home on every load), for a heal before the
            // quarterdeck.
            goto(CALE, 8, 2),
            Step::Interact(Down),
            Step::Walk(Down),
            Step::Walk(Down),
            goto(CALE, 7, 5),
            Step::Interact(Down),
            Step::Check("explosives", |r| {
                assert!(r.beaten(TRAINER_SIL_PORT_GYM_TIMOTHE));
                assert!(r.w.flag(FLAG_SIL_PORT_CALE_FUT_1));
                assert!(r.w.flag(FLAG_SIL_PORT_CALE_FUT_2));
                assert!(r.has_text(Port_Ressac_Cale_Explosif_T1));
            }),
            goto(CALE, 5, 7),
            Step::Enter {
                dir: Down,
                map: GYM,
            },
        ][..],
        OUT_OF_GYM,
        HEAL,
        &[
            goto(PORT, 32, 26),
            Step::Enter { dir: Up, map: GYM },
            goto(GYM, 13, 14),
            Step::Enter { dir: Up, map: CALE },
            goto(CALE, 8, 1),
            Step::Enter { dir: Up, map: GYM },
            // Gus over the ladder; back through the hold's shortcut for a heal; Samuel.
            goto(GYM, 5, 5),
            Step::Enter { dir: Up, map: CALE },
            goto(CALE, 5, 7),
            Step::Enter {
                dir: Down,
                map: GYM,
            },
        ][..],
        OUT_OF_GYM,
        HEAL,
        &[
            goto(PORT, 32, 26),
            Step::Enter { dir: Up, map: GYM },
            goto(GYM, 13, 14),
            Step::Enter { dir: Up, map: CALE },
            goto(CALE, 8, 1),
            Step::Enter { dir: Up, map: GYM },
            goto(GYM, 13, 4),
            Step::Interact(Up),
            Step::Check("badge_04", |r| {
                assert!(r.w.flag(FLAG_SIL_BADGE_04));
                assert!(r.w.flag(FLAG_SIL_MODULE_SURF));
                assert_eq!(r.w.var(VAR_SIL_STORY), 48);
                assert_eq!(r.w.var(VAR_SIL_SYNC), 40);
                assert_eq!(r.w.var(VAR_SIL_PORT_RESSAC_STATE), 0);
                assert_eq!(r.count(ITEM_TM01), 1);
                for t in [
                    TRAINER_SIL_PORT_GYM_PATRICE,
                    TRAINER_SIL_PORT_GYM_JESSY,
                    TRAINER_SIL_PORT_GYM_TIMOTHE,
                    TRAINER_SIL_PORT_GYM_GUS,
                    TRAINER_SIL_SAMUEL_1,
                ] {
                    assert!(r.beaten(t), "{t:?}");
                }
            }),
            // Out through the open grille: Gwada on the coupée.
            goto(GYM, 9, 18),
            Step::Check("gwada", |r| {
                assert_eq!(r.w.var(VAR_SIL_STORY), 49);
                assert!(r.w.flag(FLAG_SIL_PORT_GWADA_BATTU));
                assert_eq!(r.count(ITEM_DRAGON_SCALE), 1);
                assert_eq!(r.count(ITEM_SIL_MILLESIME_SAM), 0);
            }),
            goto(GYM, 9, 20),
            Step::Enter {
                dir: Down,
                map: PORT,
            },
            goto(PORT, 42, 21),
            Step::Check("julien", |r| {
                assert!(r.w.flag(FLAG_SIL_PORT_APPEL_JULIEN));
                assert!(r.has_text(Port_Ressac_Sc06_T1));
            }),
        ],
    ]
    .concat()
}

/// What the end of Chapter 4 asserts beyond its checkpoints.
pub fn check_end(r: &Run) {
    assert_eq!(r.w.events_dropped(), 0);
    for f in FALLBACKS {
        assert!(r.degraded.contains(f), "fallback {f} not reported");
    }
    eprintln!("party at the end: {:?}", party(r));
}

/// The fallbacks the chapter's path reports.
const FALLBACKS: &[&str] = &[
    "dark_map",
    "strength",
    "play_fanfare",
    "play_se",
    "mallette_call",
    "show_sync",
];

/// The party's species and levels (for the log).
pub fn party(r: &Run) -> Vec<(u16, u8)> {
    r.w.party().iter().map(|m| (m.species.0, m.level)).collect()
}
