//! Chapter 6, Manga-Square, from its start state ([`start`]: `VAR_SIL_STORY` 55, Chapter 5's
//! end, off the Pilgrims' Elevator at the foot of Route S6) to Adrien's promise under the Grand
//! Torii (68):
//! - Route S6: the signal moves (Sc01), the Princesse rescued and the two grunts (Sc02);
//! - Manga-Square: the arrival under the Torii Sud (Sc01);
//! - the Dojo: the code scroll, the signing queue's three fans, Kimiko and TM08 (Dojo Sc01);
//! - the Bibliothèque: three lore books, the tale of the Dawn Rooster (Bibliotheque Sc01);
//! - the gym: both samurai (Sc01, the yes/no fallback answered yes: the hasty variants), the
//!   five scroll desks (Sc02), the Kunoichi and the scribe monk, Tak (Sc03);
//! - Benou's modules at the Grand Torii (Sc02) and Adrien's promise (Sc03).
//!
//! **The party.** The lead trains in Route S6's north grass (wild 34–38) before the Dojo, before
//! the gym and before Tak, healing at Manga-Square's Center; the run also heals there after each
//! of the gym's fights (out through the Dojo and back; the open doors stay open).
//! Every battle is won by the battle policy. No hook: the gym quiz's answers are the books the
//! run reads (the `multichoice` fallback, `ch06_gym.rs`).

use super::{Run, Start, Step, TeachTm, goto, goto_avoiding};
use content::consts::*;
use engine::data::{Dir, ItemId, MapId, SpeciesId};
use engine::script::Ctx;

const ROUTE: MapId = MAP_ROUTE_S06;
const TOWN: MapId = MAP_MANGA_SQUARE;
const CENTER: MapId = MAP_MANGA_SQUARE_POKEMON_CENTER;
const DOJO: MapId = MAP_MANGA_SQUARE_DOJO;
const BIBLIO: MapId = MAP_MANGA_SQUARE_BIBLIOTHEQUE;
const GYM: MapId = MAP_MANGA_SQUARE_GYM;

/// Route S6's trainers' sight lines, walked round (they are optional).
const KOTARO_SIGHT: &[(i16, i16)] = &[(17, 57), (18, 57), (19, 57)];
const GASTON_SIGHT: &[(i16, i16)] = &[(8, 49), (9, 49), (10, 49), (11, 49)];
const NORTH_SIGHTS: &[(i16, i16)] = &[
    // Luna (6, 9), facing right.
    (7, 9),
    (8, 9),
    (9, 9),
    (10, 9),
    // Hoshi (20, 13), facing left.
    (16, 13),
    (17, 13),
    (18, 13),
    (19, 13),
];
/// Kenta (13, 15) on the Dojo's training mat, facing down.
const KENTA_SIGHT: &[(i16, i16)] = &[(13, 16), (13, 17), (13, 18)];

/// The Center's nurse, from the town and back to its door.
const CENTER_HEAL: &[Step] = &[
    goto(TOWN, 15, 24),
    Step::Enter {
        dir: Dir::Up,
        map: CENTER,
    },
    goto(CENTER, 6, 4),
    Step::Interact(Dir::Up),
    goto(CENTER, 6, 7),
    Step::Enter {
        dir: Dir::Down,
        map: TOWN,
    },
];

/// Train's heal: from Route S6's north grass up into the town, the Center, and back down.
const HEAL: &[Step] = &[
    goto_avoiding(ROUTE, 13, 1, NORTH_SIGHTS),
    Step::Enter {
        dir: Dir::Up,
        map: TOWN,
    },
    goto(TOWN, 15, 24),
    Step::Enter {
        dir: Dir::Up,
        map: CENTER,
    },
    goto(CENTER, 6, 4),
    Step::Interact(Dir::Up),
    goto(CENTER, 6, 7),
    Step::Enter {
        dir: Dir::Down,
        map: TOWN,
    },
    goto(TOWN, 22, 36),
    Step::Enter {
        dir: Dir::Down,
        map: ROUTE,
    },
];

/// Two tall-grass cells of Route S6's north meadow (x 4–9, y 2–4), paced by [`Step::Train`].
const GRASS: ((i16, i16), (i16, i16)) = ((5, 3), (6, 3));

const fn train(level: u8) -> Step {
    Step::Train {
        a: GRASS.0,
        b: GRASS.1,
        level,
        heal: HEAL,
    }
}

/// From the town down to the meadow.
const TO_GRASS: &[Step] = &[
    goto(TOWN, 22, 36),
    Step::Enter {
        dir: Dir::Down,
        map: ROUTE,
    },
];

/// From the meadow up to the town.
const TO_TOWN: &[Step] = &[
    goto_avoiding(ROUTE, 13, 1, NORTH_SIGHTS),
    Step::Enter {
        dir: Dir::Up,
        map: TOWN,
    },
];

/// Into the Dojo from the town, and out.
const INTO_DOJO: &[Step] = &[
    goto(TOWN, 36, 17),
    Step::Enter {
        dir: Dir::Up,
        map: DOJO,
    },
];
const OUT_OF_DOJO: &[Step] = &[
    goto_avoiding(DOJO, 4, 20, KENTA_SIGHT),
    Step::Enter {
        dir: Dir::Down,
        map: TOWN,
    },
];

/// Out of the gym, through the Dojo, to the town.
const OUT_OF_GYM: &[Step] = &[
    goto(GYM, 10, 28),
    Step::Enter {
        dir: Dir::Down,
        map: DOJO,
    },
    goto_avoiding(DOJO, 4, 20, KENTA_SIGHT),
    Step::Enter {
        dir: Dir::Down,
        map: TOWN,
    },
];

/// Into the gym from the town: the Dojo's back door, the monk aside from 64.
const INTO_GYM: &[Step] = &[
    goto(TOWN, 36, 17),
    Step::Enter {
        dir: Dir::Up,
        map: DOJO,
    },
    goto_avoiding(DOJO, 27, 3, KENTA_SIGHT),
    Step::Enter {
        dir: Dir::Up,
        map: GYM,
    },
];

/// Chapter 6's start: off the Pilgrims' Elevator on Route S6 (13, 62), facing north, Chapter
/// 5's end state ([`end_of_chapter_5`]). Chapters 5 and earlier are not chained into the spine.
pub fn start() -> Start {
    Start {
        map: ROUTE,
        x: 13,
        y: 62,
        facing: Dir::Up,
        setup: |ctx| Box::pin(end_of_chapter_5(ctx)),
    }
}

/// The start state's party: Chapter 5's end as its run (`sil6_ch5`) leaves it, the KADABRA
/// leading (the one [`Step::Train`] raises; Chapter 3's TM34 gives it Shock Wave for Tak's
/// Lapras, Ninjask and Charizard). The run ends with the KADABRA at 59 and the TYPHLOSION at 62
/// (`ch07::PARTY`).
pub const PARTY: &[(SpeciesId, u8)] = &[
    (SPECIES_KADABRA, 50),
    (SPECIES_TYPHLOSION, 52),
    (SPECIES_LAIRON, 35),
    (SPECIES_MAREEP, 14),
    (SPECIES_PORYGON, 25),
    (SPECIES_DUGTRIO, 26),
];

/// Chapter 5's end state (`06-manga-square.md`'s prerequisites), on top of Silice's new-game
/// script: the story at 55, the sync at 50, five badges, the Lame-Fréquence, Champ Magnétique,
/// Flash, Hydro-Propulseur and Sustentation modules, `LOG_01` to `LOG_05`; Chapter 0's
/// Mallette, running shoes and starter (CYNDAQUIL's line); the party ([`PARTY`]), a few potions,
/// the respawn at Owlys's Centre.
async fn end_of_chapter_5(ctx: Ctx) {
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
    ] {
        ctx.set_flag(f, true).await;
    }
    ctx.set_var(VAR_SIL_STORY, 55).await;
    ctx.set_var(VAR_SIL_SYNC, 50).await;
    ctx.set_var(VAR_SIL_STARTER, 1).await;
    ctx.set_var(VAR_SIL_CH0_GATE, 1).await;
    ctx.item_add(ItemId(ITEM_SIL_MALLETTE), 1).await;
    ctx.item_add(ItemId(ITEM_SUPER_POTION), 5).await;
    ctx.item_add(ItemId(ITEM_HYPER_POTION), 3).await;
    ctx.item_add(ItemId(ITEM_TM34), 1).await;
    ctx.money_add(15000).await;
    ctx.set_respawn(HEAL_LOCATION_SIL_OWLYS).await;
    for &(species, level) in PARTY {
        ctx.give_mon(species, level).await;
    }
}

/// The level the lead trains to before the Dojo's queue (three fans in a row, 41–42). A floor:
/// the KADABRA arrives above it (Chapter 5's end).
pub const LEVEL_DOJO: u8 = 45;
/// The level the lead trains to before the gym (the samurai's hasty variants at 43–44). A
/// floor, as [`LEVEL_DOJO`].
pub const LEVEL_GYM: u8 = 46;
/// The level the lead (the KADABRA, Shock Wave taught) trains to before Tak (six Pokémon,
/// 42–46), once the doors are open; then the TYPHLOSION trains to [`LEVEL_TAK_SECOND`]. Tak is
/// the run's risk: his NINJASK Baton-Passes Swords Dance to HITMONLEE and CHARIZARD, the battle
/// policy never switches, and the bench (LAIRON 35, MAREEP 14, PORYGON 25, DUGTRIO 26) cannot
/// stop a boosted CHARIZARD. Measured (KADABRA, TYPHLOSION): the KADABRA alone loses at 53–59,
/// 62 and 63 (wins at 60, 61 and 64); with the TYPHLOSION trained, 59/62 wins and so do its
/// four neighbours 58/62, 60/62, 59/61 and 59/63 (57/62 and 60/61 lose; 53/60, 54/60, 56/60 and
/// 57/60 win, 52/60 and 55/60 lose).
pub const LEVEL_TAK: u8 = 59;
/// The TYPHLOSION's level before Tak (see [`LEVEL_TAK`]).
pub const LEVEL_TAK_SECOND: u8 = 62;

/// Chapter 6's steps.
pub fn steps() -> Vec<Step> {
    use Dir::*;
    [
        &[
            Step::Check("ch6_entry", |r| {
                assert_eq!(r.w.map(), ROUTE);
                assert_eq!(r.w.var(VAR_SIL_STORY), 60);
                assert!(r.has_text(Route_S06_Sc01_T1));
            }),
            // The leaders' TM34 (Chapter 3) for the KADABRA: Shock Wave over Role Play, an
            // answer to Tak's Lapras, Scyther, Ninjask and Charizard.
            Step::Teach(TeachTm {
                item: ITEM_TM34,
                mon: 0,
                forget: 1,
            }),
            // Up the pilgrims' path, round Kotaro and Gaston, to the plateau's stairs top.
            goto_avoiding(ROUTE, 13, 50, KOTARO_SIGHT),
            goto_avoiding(ROUTE, 13, 29, GASTON_SIGHT),
            Step::Walk(Up),
            Step::Check("princesse", |r| {
                assert_eq!(r.w.var(VAR_SIL_STORY), 61);
                assert!(r.w.flag(FLAG_SIL_ROUTE_S06_RACCOURCI));
                assert!(r.beaten(TRAINER_SIL_SBIRE_S06_1));
                assert!(r.beaten(TRAINER_SIL_SBIRE_S06_2));
                assert!(r.w.flag(FLAG_SIL_HIDE_ROUTE_S06_ADRIEN));
                assert!(r.w.flag(FLAG_SIL_HIDE_ROUTE_S06_SBIRE_A));
                assert!(r.w.flag(FLAG_SIL_HIDE_ROUTE_S06_SBIRE_B));
                r.printed_in_order(&[
                    Route_S06_Sc02_T1,
                    Route_S06_Sc02_T5,
                    Route_S06_Sc02_T5b,
                    Route_S06_Sc02_T6,
                    Route_S06_Sc02_T9,
                ]);
            }),
            // North to the town, under the Torii Sud.
            goto_avoiding(ROUTE, 13, 1, NORTH_SIGHTS),
            Step::Enter { dir: Up, map: TOWN },
            goto(TOWN, 22, 34),
            Step::Walk(Up),
            Step::Check("arrivee", |r| {
                assert_eq!(r.w.var(VAR_SIL_STORY), 62);
                r.printed_in_order(&[Manga_Square_Sc01_T1, Manga_Square_Sc01_T2]);
            }),
        ][..],
        CENTER_HEAL,
        &[Step::Check("respawn", |r| {
            assert_eq!(r.w.respawn(), HEAL_LOCATION_SIL_MANGA_SQUARE);
        })][..],
        TO_GRASS,
        &[
            train(LEVEL_DOJO),
            Step::Check("trained_dojo", |r| {
                eprintln!("party before the Dojo: {:?}", party(r));
            }),
        ][..],
        TO_TOWN,
        CENTER_HEAL,
        INTO_DOJO,
        &[
            // The code scroll (the quiz's Q5), then Adrien in the signing queue.
            goto_avoiding(DOJO, 7, 3, KENTA_SIGHT),
            Step::Interact(Up),
            Step::Check("code", |r| {
                assert!(r.w.flag(FLAG_SIL_GYM06_LU_Q5));
                assert!(r.has_text(Dojo_Sign_Code));
            }),
            goto_avoiding(DOJO, 12, 10, KENTA_SIGHT),
            Step::Interact(Right),
            Step::Check("dedicace", |r| {
                assert_eq!(r.w.var(VAR_SIL_STORY), 63);
                for t in [
                    TRAINER_SIL_FAN_KEVIN,
                    TRAINER_SIL_FAN_BRUNO,
                    TRAINER_SIL_FAN_DIANE,
                ] {
                    assert!(r.beaten(t), "{t:?}");
                }
                assert_eq!(r.count(ITEM_TM08), 1);
                assert!(r.w.flag(FLAG_SIL_ITEM_DOJO_TM_GONFLETTE));
                assert!(r.w.flag(FLAG_SIL_HIDE_DOJO_ADRIEN));
                assert!(!r.w.flag(FLAG_SIL_HIDE_BIBLIO_ADRIEN));
                r.printed_in_order(&[
                    Manga_Square_Dojo_Sc01_T1,
                    Manga_Square_Dojo_Sc01_T3,
                    Manga_Square_Dojo_Sc01_T4,
                    Manga_Square_Dojo_Sc01_T5,
                    Manga_Square_Dojo_Sc01_T6,
                    Manga_Square_Dojo_Sc01_T7,
                    Manga_Square_Dojo_Sc01_T12,
                    Manga_Square_Dojo_Sc01_T13,
                ]);
            }),
        ][..],
        OUT_OF_DOJO,
        CENTER_HEAL,
        &[
            // The Bibliothèque: three lore books, then Adrien's tale.
            goto(TOWN, 22, 8),
            Step::Enter {
                dir: Up,
                map: BIBLIO,
            },
            goto(BIBLIO, 3, 9),
            Step::Interact(Up),
            goto(BIBLIO, 9, 9),
            Step::Interact(Up),
            goto(BIBLIO, 3, 13),
            Step::Interact(Up),
            Step::Check("livres", |r| {
                assert!(r.w.flag(FLAG_SIL_GYM06_LU_Q1));
                assert!(r.w.flag(FLAG_SIL_GYM06_LU_Q2));
                assert!(r.w.flag(FLAG_SIL_GYM06_LU_Q3));
                r.printed_in_order(&[Biblio_Livre_1, Biblio_Livre_2, Biblio_Livre_3]);
            }),
            goto(BIBLIO, 13, 8),
            Step::Walk(Up),
            Step::Check("conte", |r| {
                assert_eq!(r.w.var(VAR_SIL_STORY), 64);
                assert!(r.w.flag(FLAG_SIL_CONTE_COQ_LU));
                assert!(r.w.flag(FLAG_SIL_HIDE_DOJO_KIMIKO));
                assert!(r.w.flag(FLAG_SIL_HIDE_DOJO_DRONE_1));
                assert!(r.w.flag(FLAG_SIL_HIDE_DOJO_DRONE_2));
                r.printed_in_order(&[
                    Biblio_Sc01_T1,
                    Biblio_Conte_Ecran,
                    Biblio_Conte_Papier,
                    Biblio_Sc01_T5,
                    Biblio_Sc01_T7,
                ]);
            }),
            goto(BIBLIO, 12, 22),
            Step::Enter {
                dir: Down,
                map: TOWN,
            },
        ][..],
        TO_GRASS,
        &[
            train(LEVEL_GYM),
            Step::Check("trained_gym", |r| {
                eprintln!("party before the gym: {:?}", party(r));
            }),
        ][..],
        TO_TOWN,
        CENTER_HEAL,
        INTO_GYM,
        &[
            // The Allée des Regards: each samurai (yes to "draw first?": the hasty variant),
            // a heal at the Center after each fight of the gym.
            goto(GYM, 10, 26),
            Step::Walk(Up),
        ][..],
        OUT_OF_GYM,
        CENTER_HEAL,
        INTO_GYM,
        &[goto(GYM, 10, 23), Step::Walk(Up)][..],
        OUT_OF_GYM,
        CENTER_HEAL,
        INTO_GYM,
        &[
            Step::Check("samourais", |r| {
                assert!(r.beaten(TRAINER_SIL_SAMOURAI_MUSASHI));
                assert!(r.beaten(TRAINER_SIL_SAMOURAI_MUSASHI_HATIF));
                assert!(r.beaten(TRAINER_SIL_SAMOURAI_KOJIRO));
                assert!(r.beaten(TRAINER_SIL_SAMOURAI_KOJIRO_HATIF));
                assert_eq!(r.times(Gym_Sc01_T3), 2);
            }),
            // The scroll desks, then the Kunoichi's sight line walked into.
            goto(GYM, 10, 21),
            Step::Interact(Left),
            goto(GYM, 10, 17),
            Step::Interact(Left),
            goto(GYM, 10, 13),
            Step::Interact(Left),
            goto(GYM, 10, 11),
            Step::Walk(Up),
        ][..],
        OUT_OF_GYM,
        CENTER_HEAL,
        INTO_GYM,
        &[
            // The fourth desk, the scribe monk's sight line, the fifth desk.
            goto(GYM, 10, 9),
            Step::Interact(Left),
            goto(GYM, 10, 7),
            Step::Walk(Up),
            goto(GYM, 10, 5),
            Step::Interact(Left),
            Step::Check("parchemins", |r| {
                assert_eq!(r.w.var(VAR_SIL_STORY), 65);
                for f in [
                    FLAG_SIL_GYM06_PORTE_1,
                    FLAG_SIL_GYM06_PORTE_2,
                    FLAG_SIL_GYM06_PORTE_3,
                    FLAG_SIL_GYM06_PORTE_4,
                    FLAG_SIL_GYM06_PORTE_5,
                ] {
                    assert!(r.w.flag(f), "{f:?}");
                }
                assert_eq!(r.times(Gym_Sc02_Juste), 5);
                assert!(!r.has_text(Gym_Sc02_Faux));
                assert!(r.beaten(TRAINER_SIL_KUNOICHI_AYAME));
                assert!(r.beaten(TRAINER_SIL_MOINE_SEIJI));
            }),
        ][..],
        OUT_OF_GYM,
        TO_GRASS,
        &[
            train(LEVEL_TAK),
            // The TYPHLOSION next, as the lead, then back to second.
            Step::Swap(0, 1),
            train(LEVEL_TAK_SECOND),
            Step::Swap(0, 1),
            Step::Check("trained_tak", |r| {
                eprintln!("party before Tak: {:?}", party(r));
            }),
        ][..],
        TO_TOWN,
        CENTER_HEAL,
        INTO_GYM,
        &[
            goto(GYM, 10, 4),
            Step::Interact(Up),
            Step::Check("badge_06", |r| {
                assert_eq!(r.w.var(VAR_SIL_STORY), 66);
                assert!(r.w.flag(FLAG_SIL_BADGE_06));
                assert!(r.w.flag(FLAG_SIL_LOG_06));
                assert_eq!(r.w.var(VAR_SIL_SYNC), 60);
                assert!(r.beaten(TRAINER_SIL_TAK_1));
                assert_eq!(r.count(ITEM_TM40), 1);
                assert_eq!(r.count(ITEM_SIL_CHARIZARDITE_Y), 1);
                r.printed_in_order(&[
                    Gym_Sc03_T1,
                    Gym_Sc03_T4,
                    Gym_Sc03_T7,
                    Gym_Sc03_T10,
                    Gym_Sc03_Log06,
                    Gym_Sc03_T11,
                ]);
            }),
        ][..],
        OUT_OF_GYM,
        &[
            // Benou at the Grand Torii, then Adrien's promise on its exit cell.
            goto(TOWN, 37, 7),
            Step::Walk(Up),
            Step::Check("benou", |r| {
                assert_eq!(r.w.var(VAR_SIL_STORY), 67);
                assert!(r.w.flag(FLAG_SIL_MODULE_ECLATE_ROC));
                assert!(r.w.flag(FLAG_SIL_MODULE_MEGA));
                assert!(r.w.flag(FLAG_SIL_HIDE_MANGA_SQUARE_CONTROLEUR));
                assert!(r.w.flag(FLAG_SIL_HIDE_MANGA_SQUARE_BENOU));
                r.printed_in_order(&[
                    Manga_Square_Sc02_T1,
                    Manga_Square_Sc02_T5,
                    Manga_Square_Sc02_T6c,
                    Manga_Square_Sc02_T7,
                ]);
            }),
            goto(TOWN, 37, 2),
            Step::Walk(Up),
            Step::Check("promesse", |r| {
                assert_eq!(r.w.var(VAR_SIL_STORY), 68);
                assert!(r.w.flag(FLAG_SIL_HIDE_MANGA_SQUARE_ADRIEN_FIN));
                assert_eq!(r.pos(), (37, 1));
                r.printed_in_order(&[Manga_Square_Sc03_T1, Manga_Square_Sc03_T5]);
            }),
        ],
    ]
    .concat()
}

/// What the end of Chapter 6 asserts beyond its checkpoints.
pub fn check_end(r: &Run) {
    assert_eq!(r.w.events_dropped(), 0);
    for f in FALLBACKS {
        assert!(r.degraded.contains(f), "fallback {f} not reported");
    }
    eprintln!("party at the end: {:?}", party(r));
}

/// The fallbacks the chapter's path reports.
const FALLBACKS: &[&str] = &[
    "play_music",
    "battle_multi",
    "dawn_tint",
    "show_book",
    "play_se",
    "wait_input_or_timeout",
    "multichoice",
    "set_battle_weather",
    "mega_evolution",
    "play_fanfare",
    "show_sync",
    "log_menu",
];

/// The party's species and levels (for the log).
pub fn party(r: &Run) -> Vec<(u16, u8)> {
    r.w.party().iter().map(|m| (m.species.0, m.level)).collect()
}
