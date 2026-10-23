//! Chapter 4, Port-Ressac, from its entry (`VAR_SIL_STORY` 36, Chapter 3's end, on Route S4's
//! west edge) to Gwada beaten (49) and Julien's call on the jetty.

use super::entry::Entry;
use super::{Run, Step, goto};
use content::consts::*;
use engine::data::{Dir, MapId};

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

/// Chapter 4's entry: Chapter 3's end.
pub static ENTRY: Entry = Entry {
    map: TUNNEL,
    x: 1,
    y: 8,
    flags: &[
        FLAG_SIL_INTRO_DONE,
        FLAG_SIL_HAS_MALLETTE,
        FLAG_SIL_MALLETTE_OPEN,
        FLAG_SIL_MALLETTE_MAP,
        FLAG_SIL_MALLETTE_DEX,
        FLAG_SIL_MALLETTE_CALLS,
        FLAG_SIL_RUNNING_SHOES,
        FLAG_SIL_BADGE_01,
        FLAG_SIL_BADGE_02,
        FLAG_SIL_BADGE_03,
        FLAG_SIL_MODULE_COUPE,
        FLAG_SIL_MODULE_FORCE,
        FLAG_SIL_MODULE_FLASH,
        FLAG_SIL_LOG_01,
        FLAG_SIL_LOG_02,
        FLAG_SIL_LOG_03,
    ],
    vars: &[
        (VAR_SIL_STORY, 36),
        (VAR_SIL_CH0_GATE, 1),
        (VAR_SIL_STARTER, 1),
        (VAR_SIL_SYNC, 30),
    ],
    party: &[
        (SPECIES_QUILAVA, 33),
        (SPECIES_AMPHAROS, 32),
        (SPECIES_KADABRA, 32),
        (SPECIES_MAGNETON, 31),
    ],
    items: &[
        (ITEM_POTION, 5),
        (ITEM_SUPER_POTION, 3),
        (ITEM_POKE_BALL, 5),
    ],
    money: 6000,
    respawn: HEAL_LOCATION_SIL_BOIS_ETHER,
};

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
                level: 35,
                heal: HEAL,
            },
            Step::Check("trained_35", |r| {
                eprintln!("party before the cleaning: {:?}", party(r));
            }),
        ][..],
        HEAL,
        &[
            // The quay cleaning: the four grunts along the corridor (a heal after the first
            // two), then the foreman.
            goto(PORT, 10, 20),
        ][..],
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
                level: 38,
                heal: HEAL,
            },
            Step::Check("trained", |r| {
                eprintln!("party before the gym: {:?}", party(r));
            }),
        ][..],
        HEAL,
        &[
            // The Zenith-Grog: the deck's crew (Patrice, Jessy), then back to the Taverne.
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
            // Gus over the ladder, then Samuel at the helm.
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
    "use_field_module",
    "strength",
    "battle_continue_on_loss",
    "battle_start_status",
    "battle_terrain",
    "play_fanfare",
    "battle_start_stats",
    "log_menu",
    "screen_shake",
    "play_se",
    "mallette_call",
    "show_sync",
];

/// The party's species and levels (for the log).
pub fn party(r: &Run) -> Vec<(u16, u8)> {
    r.w.party().iter().map(|m| (m.species.0, m.level)).collect()
}
