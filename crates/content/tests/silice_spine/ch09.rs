//! Chapter 9, the Prise d'Eau and Nébula, from Évalon's south-east path (`VAR_SIL_STORY` 86) to
//! the crystal elevator (99):
//! - Route S9: the signal (Sc01, 90), the hidden Full Restore and the Ultra Ball, the refugee,
//!   Julien's call (Sc02, 91), Anselme on the pontoon; Surf (the Hydro-Propulseur) across the
//!   lake past Marin and Océane; the security cables (Lame-Fréquence);
//! - the Station de Pompage: pump A, the cabinet pushed off the breaker (Champ Magnétique), TM24,
//!   Gérard, pump B (Sc03, 92), pump C's warning, the Elixir;
//! - back on the lake: the two Cocket divers, then Dive (the Sonde de Pression) at P2 into the
//!   conduit (Sc04's first-dive text), the calm currents, the Max Revive, a porthole, and the
//!   south arrow warp onto Nébula's canal;
//! - Nébula: the arrival (Sc01, 93), the Gargote (Sc02, 94: two Full Restores, Mme Fenouil's
//!   Leftovers, Gwada), the Centre (the robot nurse, the respawn, the vending machine's mart),
//!   the Comptoir's mart, the Élecsprintite, a crowd line, the diversion (Sc03, 95);
//! - the Bassin's dive under the Tower: B1F (Sc04's text, 96; the two sbires, the Max Elixir),
//!   the hall (the hidden Hyper Potion, the bay window; Sbire 3, Viktor, Nadia and Kroll, each
//!   opening a sas; 97) and the elevator (Tour_Hall_Sc02: the heal, 99, the cabin
//!   `TourNebulaAscenseur` (3, 6), §5.7's proposal).
//!
//! **Real engine features the path drives:** Surf and Dive by an A press (gated by the module
//! flags, `field_moves.rs`), the dismount by walking ashore, the south arrow warp, Nébula's
//! fixed dive warps (`DIVE_WARPS`), and two mart screens (the driver leaves a shop with B).
//!
//! **The fallbacks the path relies on** (`ch09_*.rs`): the Champ Magnétique is an A press on the
//! cabinet; the hidden items are signs; Benou's and Julien's calls are plain messages; the
//! camera pan of the diversion stays on the player; no script changes the music.
//!
//! **The party** ([`PARTY`]): Chapter 8's end as `sil9_ch8` leaves it (KADABRA 69 leading,
//! TYPHLOSION 76, LAIRON 35, MAREEP 14, PORYGON 25, DUGTRIO 26). No hook, no training: the
//! chapter's trainers are 50–53. The run heals at Nébula's Centre before the diversion, and
//! again before Kroll (B1F's pool dives back up into the Bassin); for Kroll's TYRANITAR the
//! TYPHLOSION learns Brick Break (TM31, Chapter 4's Port gift, added to the start bag).

use super::{Run, Start, Step, TeachTm, goto, goto_avoiding};
use content::consts::*;
use engine::data::{Dir, ItemId, MapId, SpeciesId};
use engine::script::Ctx;

const EVALON: MapId = MAP_EVALON;
const ROUTE: MapId = MAP_ROUTE_S09;
const STATION: MapId = MAP_ROUTE_S09_STATION;
const CONDUITE: MapId = MAP_ROUTE_S09_UNDERWATER;
const TOWN: MapId = MAP_NEBULA;
const CENTER: MapId = MAP_NEBULA_POKEMON_CENTER;
const MART: MapId = MAP_NEBULA_MART;
const GARGOTE: MapId = MAP_NEBULA_GARGOTE;
const B1F: MapId = MAP_NEBULA_TOUR_HALL_B1F;
const HALL: MapId = MAP_NEBULA_TOUR_HALL;
const CABIN: MapId = MAP_TOUR_NEBULA_ASCENSEUR;

/// The chapter's start: Évalon's south-east path (42, 45), where Chapter 8's e2e ends, facing
/// south, Chapter 8's end state ([`end_of_chapter_8`]).
pub fn start() -> Start {
    Start {
        map: EVALON,
        x: 42,
        y: 45,
        facing: Dir::Down,
        setup: |ctx| Box::pin(end_of_chapter_8(ctx)),
    }
}

/// The start state's party (see the module docs).
pub const PARTY: &[(SpeciesId, u8)] = &[
    (SPECIES_KADABRA, 69),
    (SPECIES_TYPHLOSION, 76),
    (SPECIES_LAIRON, 35),
    (SPECIES_MAREEP, 14),
    (SPECIES_PORYGON, 25),
    (SPECIES_DUGTRIO, 26),
];

/// Chapter 8's end state (`09-nebula.md`'s prerequisites): Chapter 7's ([`super::ch08`]'s
/// `chapter_7_state`), then Chapter 8's badge, its Sonde de Pression (`FLAG_SIL_MODULE_PLONGEE`)
/// and log, the Cœur d'Évalon, the friends gone to Nébula, the sync at 80, the story at 86
/// (Evalon_Sc10); the chapter's key items and TMs, the party ([`PARTY`]), the respawn at Évalon.
async fn end_of_chapter_8(ctx: Ctx) {
    super::ch08::chapter_7_state(&ctx).await;
    for f in [
        FLAG_SIL_BADGE_08,
        FLAG_SIL_MODULE_PLONGEE,
        FLAG_SIL_LOG_08,
        FLAG_SIL_COEUR_EVALON,
        FLAG_SIL_HIDE_EVALON_AMIS,
    ] {
        ctx.set_flag(f, true).await;
    }
    ctx.set_var(VAR_SIL_STORY, 86).await;
    ctx.set_var(VAR_SIL_SYNC, 80).await;
    for (item, n) in [
        (ITEM_SIL_COEUR_EVALON, 1),
        (ITEM_SIL_EPEE_LEGENDE, 1),
        (ITEM_TM19, 1),
        (ITEM_TM23, 1),
        // Chapter 4's Port gift (`ch04_port.rs`), unused by the spines since.
        (ITEM_TM31, 1),
    ] {
        ctx.item_add(ItemId(item), n).await;
    }
    ctx.set_respawn(HEAL_LOCATION_SIL_EVALON).await;
    for &(species, level) in PARTY {
        ctx.give_mon(species, level).await;
    }
}

/// Anselme's line of sight (19, 22), 3 cells west along the pontoon's north row.
const ANSELME_SIGHT: &[(i16, i16)] = &[(16, 22), (17, 22), (18, 22)];
/// Sbire 3's (18, 12), 3 cells east before sas 1.
const SBIRE_3_SIGHT: &[(i16, i16)] = &[(19, 12), (20, 12), (21, 12)];

/// The chapter's steps, from [`start`].
pub fn steps() -> Vec<Step> {
    use Dir::*;
    let mut s = vec![
        // ---- Évalon → Route S9 ----
        goto(EVALON, 47, 44),
        Step::Enter {
            dir: Right,
            map: ROUTE,
        },
        goto(ROUTE, 4, 5),
        Step::Check("route_s09_sc01", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 90);
            r.printed_in_order(&[
                Route_S09_Sc01_T1,
                Route_S09_Sc01_T2,
                Route_S09_Sc01_T3,
                Route_S09_Sc01_T4,
                Route_S09_Sc01_T5,
            ]);
        }),
        // The shepherd's path: the last tree's Full Restore (13, 5), the Ultra Ball (14, 3),
        // the sign, the refugee.
        goto(ROUTE, 12, 5),
        Step::Interact(Right),
        goto(ROUTE, 14, 4),
        Step::Interact(Up),
        goto(ROUTE, 7, 7),
        Step::Interact(Up),
        goto(ROUTE, 8, 7),
        Step::Interact(Right),
        Step::Check("shepherd_path", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_S09_CACHE_1));
            assert!(r.w.flag(FLAG_SIL_ITEM_S09_ULTRA_BALL));
            assert!(r.count(ITEM_FULL_RESTORE) >= 1);
            assert!(r.count(ITEM_ULTRA_BALL) >= 1);
            r.printed_in_order(&[Route_S09_Sign_1, Route_S09_Npc_1]);
        }),
        // ---- Julien's call (Sc02) on the berge, the second sign, Anselme ----
        goto(ROUTE, 8, 14),
        Step::Check("route_s09_sc02", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 91);
            r.printed_in_order(&[
                Route_S09_Sc02_T1,
                Route_S09_Sc02_T2,
                Route_S09_Sc02_T3,
                Route_S09_Sc02_T4,
            ]);
        }),
        goto(ROUTE, 13, 20),
        Step::Interact(Right),
        goto(ROUTE, 16, 22),
        Step::Interact(Right),
        Step::Check("anselme", |r| {
            assert!(r.beaten(TRAINER_SIL_S09_ANSELME));
            assert!(r.has_text(Route_S09_Sign_2));
            r.printed_in_order(&[Route_S09_Anselme_Intro, Route_S09_Npc_2]);
        }),
        // ---- the lake: Surf from the pontoon's end ----
        goto_avoiding(ROUTE, 20, 23, ANSELME_SIGHT),
        Step::Interact(Right),
        Step::Check("surf", |r| {
            assert!(r.w.objects().surfing());
            assert!(r.has_text(gText_WantToUseSurf));
        }),
        // Marin (27, 17) sees (27, 18–21); Océane (41, 13) sees (37–40, 13).
        goto(ROUTE, 27, 21),
        goto(ROUTE, 37, 13),
        Step::Check("swimmers", |r| {
            assert!(r.beaten(TRAINER_SIL_S09_MARIN));
            assert!(r.beaten(TRAINER_SIL_S09_OCEANE));
        }),
        // Ashore on the station pontoon; the cables (Lame-Fréquence); the technician.
        goto(ROUTE, 45, 19),
        Step::Check("dismount", |r| assert!(!r.w.objects().surfing())),
        goto(ROUTE, 46, 19),
        Step::Interact(Right),
        Step::Check("cables", |r| {
            assert!(r.w.flag(FLAG_SIL_S09_CABLES));
            assert!(r.has_text(Route_S02_Cable_T2));
        }),
        goto(ROUTE, 52, 20),
        Step::Interact(Down),
        Step::Check("technicienne", |r| assert!(r.has_text(Route_S09_Npc_3))),
        goto(ROUTE, 53, 18),
        Step::Enter {
            dir: Up,
            map: STATION,
        },
        // ---- the Station de Pompage (Sc03) ----
        goto(STATION, 3, 11),
        Step::Interact(Up),
        Step::Check("pompe_a", |r| {
            assert!(r.w.flag(FLAG_SIL_S09_POMPE_A));
            assert_eq!(r.w.var(VAR_SIL_STORY), 91);
            r.printed_in_order(&[Route_S09_Sc03_T1, Route_S09_Sc03_T2]);
        }),
        // The glass office: the cabinet pushed east off the breaker, the breaker, TM24.
        goto(STATION, 1, 2),
        Step::Interact(Right),
        goto(STATION, 2, 2),
        Step::Interact(Up),
        goto(STATION, 7, 4),
        Step::Interact(Up),
        Step::Check("disjoncteur", |r| {
            assert!(r.w.flag(FLAG_SIL_S09_DISJONCTEUR));
            assert!(r.w.flag(FLAG_SIL_ITEM_S09_CT_TONNERRE));
            assert!(r.count(ITEM_TM24) >= 1);
            r.printed_in_order(&[Depot_Bloc_T2, Route_S09_Sc03_T5]);
        }),
        // Gérard (13, 11) sees (10–12, 11), pump B's console row; he steps back aside.
        goto(STATION, 10, 11),
        goto(STATION, 11, 11),
        Step::Interact(Up),
        Step::Check("pompe_b", |r| {
            assert!(r.beaten(TRAINER_SIL_S09_TECHNICIEN));
            assert!(r.w.flag(FLAG_SIL_S09_POMPE_B));
            assert_eq!(r.w.var(VAR_SIL_STORY), 92);
            r.printed_in_order(&[
                Route_S09_Technicien_Intro,
                Route_S09_Sc03_T2,
                Route_S09_Sc03_T3,
            ]);
        }),
        goto(STATION, 12, 11),
        Step::Interact(Right),
        goto(STATION, 17, 11),
        Step::Interact(Up),
        goto(STATION, 20, 10),
        Step::Interact(Up),
        Step::Check("station", |r| {
            assert!(r.has_text(Route_S09_Technicien_Apres));
            assert!(r.has_text(Route_S09_Station_PompeC));
            assert!(r.w.flag(FLAG_SIL_ITEM_S09_STATION_ELIXIR));
        }),
        goto(STATION, 10, 15),
        Step::Enter {
            dir: Down,
            map: ROUTE,
        },
        // ---- back on the lake: the divers, then Dive at P2 ----
        goto(ROUTE, 45, 19),
        Step::Interact(Down),
        // N°12 (44, 33) sees (44, 34–37); N°7 (26, 31) sees (27–30, 31).
        goto(ROUTE, 44, 37),
        goto(ROUTE, 30, 31),
        Step::Check("divers", |r| {
            assert!(r.beaten(TRAINER_SIL_S09_PLONGEUSE_12));
            assert!(r.beaten(TRAINER_SIL_S09_PLONGEUR_7));
        }),
        goto(ROUTE, 35, 39),
        Step::Interact(Up),
        // ---- the conduit ----
        Step::Check("route_s09_sc04", |r| {
            assert_eq!(r.w.map(), CONDUITE);
            assert!(r.w.flag(FLAG_SIL_S09_PREMIERE_PLONGEE));
            assert!(r.has_text(Text_WantToDive));
            assert!(r.has_text(Route_S09_Sc04_T1));
        }),
        goto(CONDUITE, 37, 52),
        Step::Interact(Right),
        goto(CONDUITE, 45, 64),
        Step::Interact(Down),
        Step::Check("conduite", |r| {
            assert!(!r.has_text(Route_S09_UW_Courant));
            assert!(r.has_text(Route_S09_UW_Hublot));
            assert!(r.w.flag(FLAG_SIL_ITEM_S09_UW_4));
            assert!(r.count(ITEM_MAX_REVIVE) >= 1);
        }),
        goto(CONDUITE, 39, 73),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        // ---- Nébula: the quai (Sc01) ----
        Step::Check("nebula_sc01", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 93);
            r.printed_in_order(&[
                Nebula_Sc01_T1,
                Nebula_Sc01_T2,
                Nebula_Annonce_T1,
                Nebula_Sc01_T3,
            ]);
        }),
        goto(TOWN, 30, 48),
        goto(TOWN, 11, 44),
        Step::Interact(Left),
        Step::Check("quai", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_NEBULA_CACHE_2));
            assert!(r.count(ITEM_REVIVE) >= 1);
        }),
        // ---- the Gargote (Sc02) ----
        goto(TOWN, 5, 32),
        Step::Enter {
            dir: Up,
            map: GARGOTE,
        },
        Step::Check("nebula_sc02", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 94);
            assert!(r.count(ITEM_FULL_RESTORE) >= 3);
            r.printed_in_order(&[
                Nebula_Sc02_T1,
                Nebula_Sc02_T2,
                Nebula_Sc02_T3,
                Nebula_Sc02_T4,
                Nebula_Sc02_T5,
                Nebula_Sc02_T6,
                Nebula_Sc02_T7,
                Nebula_Sc02_T8,
                Nebula_Sc02_T9,
            ]);
        }),
        goto(GARGOTE, 4, 5),
        Step::Interact(Up),
        goto(GARGOTE, 5, 6),
        Step::Interact(Down),
        Step::Check("gargote", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_NEBULA_RESTES));
            assert!(r.count(ITEM_LEFTOVERS) >= 1);
            r.printed_in_order(&[Nebula_Gargote_Fenouil_T2, Nebula_Gargote_Gwada_T1]);
        }),
        goto(GARGOTE, 5, 9),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        // ---- the Centre: the respawn, the heal, the vending machine ----
        goto(TOWN, 17, 34),
        Step::Enter {
            dir: Up,
            map: CENTER,
        },
        goto(CENTER, 6, 4),
        Step::Interact(Up),
        goto(CENTER, 3, 4),
        Step::Interact(Up),
        Step::Check("centre", |r| {
            assert!(r.healed());
            r.printed_in_order(&[
                Nebula_Centre_Soin_T1,
                Nebula_Centre_Soin_T2,
                Nebula_Distributeur,
            ]);
        }),
        goto(CENTER, 5, 7),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        // ---- the Comptoir Cocket ----
        goto(TOWN, 42, 34),
        Step::Enter { dir: Up, map: MART },
        goto(MART, 3, 3),
        Step::Interact(Left),
        Step::Check("comptoir", |r| {
            r.printed_in_order(&[Nebula_Mart_T1, gText_PleaseComeAgain]);
        }),
        goto(MART, 4, 8),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        // ---- the Place: the Élecsprintite, a crowd line ----
        goto(TOWN, 33, 23),
        Step::Interact(Up),
        goto(TOWN, 28, 26),
        Step::Interact(Up),
        Step::Check("place", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_NEBULA_MANECTITE));
            assert!(r.count(ITEM_SIL_MANECTITE) >= 1);
            r.printed_in_order(&[Nebula_Manectite, Nebula_Npc_1]);
        }),
        // ---- the diversion (Sc03) at the Bassin's fence ----
        goto(TOWN, 43, 13),
        Step::Interact(Right),
        Step::Check("nebula_sc03", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 95);
            assert!(r.w.flag(FLAG_SIL_NEBULA_DIVERSION));
            assert!(r.w.flag(FLAG_SIL_HIDE_NEBULA_GARDE_BASSIN));
            assert!(r.w.flag(FLAG_SIL_HIDE_NEBULA_AMIS_GARGOTE));
            assert!(!r.w.flag(FLAG_SIL_HIDE_NEBULA_MATHIEU));
            assert!(!r.w.flag(FLAG_SIL_HIDE_NEBULA_ADRIEN));
            r.printed_in_order(&[
                Nebula_Sc03_T0,
                Nebula_Sc03_T1,
                Nebula_Sc03_T2,
                Nebula_Sc03_T3,
                Nebula_Sc03_T4,
                Nebula_Sc03_T5,
                Nebula_Sc03_T6,
                Nebula_Sc03_T7,
            ]);
        }),
        // ---- the Bassin: through the gate, Surf, Dive under the Tower ----
        goto(TOWN, 45, 12),
        Step::Interact(Right),
        goto(TOWN, 51, 11),
        Step::Interact(Right),
        Step::Check("nebula_sc04", |r| {
            assert_eq!(r.w.map(), B1F);
            assert_eq!(r.w.var(VAR_SIL_STORY), 96);
            assert!(r.has_text(Nebula_Sc04_T1));
        }),
        // ---- B1F, the Salle des Pompes ----
        goto(B1F, 10, 11),
        // Sbire 2 (17, 13) sees (14–16, 13); Sbire 1 (13, 10) sees (14–15, 10).
        goto(B1F, 14, 13),
        goto(B1F, 18, 13),
        Step::Interact(Down),
        goto(B1F, 15, 10),
        Step::Check("b1f", |r| {
            assert!(r.beaten(TRAINER_SIL_TOUR_SBIRE_1));
            assert!(r.beaten(TRAINER_SIL_TOUR_SBIRE_2));
            assert!(r.w.flag(FLAG_SIL_ITEM_TOUR_B1F_ELIXIR));
            assert!(r.count(ITEM_MAX_ELIXIR) >= 1);
            r.printed_in_order(&[Tour_Sbire2_Intro_Ch09, Tour_Sbire1_Intro_Ch09]);
        }),
        goto(B1F, 17, 2),
        Step::Enter { dir: Up, map: HALL },
    ];
    s.extend_from_slice(&[
        // ---- the hall: the desk, the bay window, the security corridor (Sc01) ----
        goto_avoiding(HALL, 10, 12, SBIRE_3_SIGHT),
        Step::Interact(Down),
        goto_avoiding(HALL, 2, 13, SBIRE_3_SIGHT),
        Step::Interact(Left),
        Step::Check("hall", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_TOUR_HALL_ACCUEIL));
            assert!(r.count(ITEM_HYPER_POTION) >= 1);
            assert!(r.has_text(Tour_Hall_Vitre));
        }),
        // Sas 1: Sbire 3.
        goto(HALL, 20, 12),
        Step::Check("sas_1", |r| assert!(r.beaten(TRAINER_SIL_TOUR_SBIRE_3))),
        // Sas 2: Viktor (16, 8) sees (16, 9–10), Nadia (15, 11) sees (15, 9–10).
        goto(HALL, 16, 10),
        goto(HALL, 15, 10),
        Step::Check("sas_2", |r| {
            assert!(r.beaten(TRAINER_SIL_GARDE_COCKET_VIKTOR));
            assert!(r.beaten(TRAINER_SIL_GARDE_COCKET_NADIA));
        }),
        // Kroll's five (51–53) after B1F's and the hall's four battles: back to the Centre the
        // way the player came (B1F's pool dives back up into the Bassin), then in again.
        goto(HALL, 20, 14),
        Step::Enter { dir: Down, map: B1F },
        goto(B1F, 10, 11),
        Step::Interact(Left),
        goto(B1F, 7, 10),
        Step::Interact(Left),
        Step::Check("bassin_retour", |r| assert_eq!(r.w.map(), TOWN)),
        goto(TOWN, 17, 34),
        Step::Enter {
            dir: Up,
            map: CENTER,
        },
        goto(CENTER, 6, 4),
        Step::Interact(Up),
        Step::Check("centre_kroll", |r| assert!(r.healed())),
        goto(CENTER, 5, 7),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        goto(TOWN, 45, 12),
        Step::Interact(Right),
        goto(TOWN, 51, 11),
        Step::Interact(Right),
        goto(B1F, 17, 2),
        Step::Enter { dir: Up, map: HALL },
        // Kroll's TYRANITAR (Psychic-immune, Dragon Dance, 53) outlasts the TYPHLOSION's
        // resisted Flamethrower: the TYPHLOSION learns Brick Break (TM31, Chapter 4's gift) over
        // Quick Attack.
        Step::Teach(TeachTm {
            item: ITEM_TM31,
            mon: 1,
            forget: 0,
        }),
        // Sas 3: Kroll (14, 6) sees (11–13, 6).
        goto(HALL, 12, 6),
        Step::Check("tour_hall_sc01", |r| {
            assert!(r.beaten(TRAINER_SIL_CHEF_SECURITE_KROLL));
            assert_eq!(r.w.var(VAR_SIL_STORY), 97);
            r.printed_in_order(&[Tour_Kroll_Intro, Tour_Hall_Kroll_Apres]);
        }),
        // ---- the crystal elevator (Sc02) ----
        goto(HALL, 12, 4),
        Step::Walk(Up),
    ]);
    s
}

/// What the end of Chapter 9 asserts beyond its checkpoints: the cabin, the story, the heal,
/// the chapter's flags and trainers, no dropped event, and every Chapter 9 fallback the path
/// meets reported.
pub fn check_end(r: &Run) {
    assert_eq!(r.w.map(), CABIN);
    assert_eq!(r.pos(), (3, 6));
    assert_eq!(r.w.var(VAR_SIL_STORY), 99);
    assert!(r.healed());
    r.printed_in_order(&[Tour_Hall_Sc02_T1, Tour_Hall_Sc02_T2]);
    for f in [
        FLAG_SIL_S09_POMPE_A,
        FLAG_SIL_S09_POMPE_B,
        FLAG_SIL_S09_CABLES,
        FLAG_SIL_NEBULA_DIVERSION,
    ] {
        assert!(r.w.flag(f), "{f:?}");
    }
    for t in [
        TRAINER_SIL_S09_ANSELME,
        TRAINER_SIL_S09_MARIN,
        TRAINER_SIL_S09_OCEANE,
        TRAINER_SIL_S09_PLONGEUR_7,
        TRAINER_SIL_S09_PLONGEUSE_12,
        TRAINER_SIL_S09_TECHNICIEN,
        TRAINER_SIL_TOUR_SBIRE_1,
        TRAINER_SIL_TOUR_SBIRE_2,
        TRAINER_SIL_TOUR_SBIRE_3,
        TRAINER_SIL_GARDE_COCKET_VIKTOR,
        TRAINER_SIL_GARDE_COCKET_NADIA,
        TRAINER_SIL_CHEF_SECURITE_KROLL,
    ] {
        assert!(r.beaten(t), "{t:?}");
    }
    assert_eq!(r.w.events_dropped(), 0);
    for name in [
        "play_music",
        "screen_flash",
        "mallette_call",
        "end_call",
        "strength",
        "play_se",
        "hidden item",
        "camera_pan",
    ] {
        assert!(r.degraded.contains(name), "{name}: {:?}", r.degraded);
    }
}
