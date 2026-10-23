//! Chapter 2, Gris-Échec, from Chapter 1's end (`VAR_SIL_STORY` 17, Route S2's west entrance)
//! to the Gamma barrier open (28), then the Usine's crates with the new module:
//! - Route S2: the arrival (Sc01), the Agent's Paralyze Heals, Adrien's Revive (Sc02), cable C
//!   (the Lame-Fréquence), the Technicien's TM, Gwada's battle (Sc03);
//! - Gris-Échec: the embargo (Sc01), the three jammers (the roof sbire, the sweeper, the Usine's
//!   cable), the arena guard (Sc03, the Tour door), the Centre;
//! - the Tour: both sbires by sight, Duval and the Carte Perforée (Tour_Sc01);
//! - the arena: the guard steps aside (Sc05); Salle 1's binary tiles (one wrong tile first: the
//!   robot, back to the hall; then the key 1 0 1 1 0 0 1 0 bottom-up), Salle 2's terminal and
//!   maze, the Chambre Sourde (the Mélomane, the Soothe Bell), the chessboard to the King, Yohan
//!   (Gym2_Sc10 to Sc12: the badge, the TM, the Champ Magnétique, Lainergie, `LOG_02`);
//! - out: Kimiko's stream (Sc06, story 28), then the Usine's crates and the Metal Coat.
//!
//! **Two ways in.** Chained: `ch00::steps()`, `ch01::steps()`, [`from_ch01`] (the Labo to Route
//! S2's west entrance), then [`steps`], from a new game. Or the documented shortcut [`start`]: a
//! new game moved to Route S2's west entrance (0, 3) with Chapter 1's end state set directly and
//! one FERALIGATR, level 50 (Bite first), so that the runner's battle policy (always the first
//! move) wins every battle. Either way the Centre heals twice on the way.

use super::ch00::ROUTE_S01_SIGHT;
use super::{Run, Start, Step, goto, goto_avoiding};
use content::consts::*;
use engine::data::{Dir, ItemId, MapId};
use engine::script::Ctx;

const ROUTE: MapId = MAP_ROUTE_S02;
const TOWN: MapId = MAP_GRIS_ECHEC;
const CENTER: MapId = MAP_GRIS_ECHEC_POKEMON_CENTER;
const USINE: MapId = MAP_GRIS_ECHEC_USINE;
const TOUR: MapId = MAP_GRIS_ECHEC_TOUR_OBSERVATION;
const TOP: MapId = MAP_GRIS_ECHEC_TOUR_OBSERVATION_TOP;
const GYM: MapId = MAP_GRIS_ECHEC_GYM;
const SOURDE: MapId = MAP_GRIS_ECHEC_GYM_SOURDE;

/// Chapter 2's start: Route S2's west entrance (0, 3), facing east.
pub fn start() -> Start {
    Start {
        map: ROUTE,
        x: 0,
        y: 3,
        facing: Dir::Right,
        setup: |ctx| Box::pin(end_of_chapter_1(ctx)),
    }
}

/// Chapter 1's end state (`02-gris-echec.md`'s prerequisites and Chapter 1's §8), on top of
/// Silice's new-game script: the story at 17 (the Secteur Bêta barrier open), the sync at 10,
/// the first badge, the Lame-Fréquence module, `LOG_01`, the ambush and the Labo hint; Chapter
/// 0's Mallette, running shoes and starter (TOTODILE's line); the party.
async fn end_of_chapter_1(ctx: Ctx) {
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
    ] {
        ctx.set_flag(f, true).await;
    }
    ctx.set_var(VAR_SIL_STORY, 17).await;
    ctx.set_var(VAR_SIL_SYNC, 10).await;
    ctx.set_var(VAR_SIL_STARTER, 2).await;
    ctx.set_var(VAR_SIL_CH0_GATE, 1).await;
    ctx.item_add(ItemId(ITEM_SIL_MALLETTE), 1).await;
    ctx.give_mon(SPECIES_FERALIGATR, 50).await;
}

/// Route S2's trainers' sight lines (Hugo, René, Zoé, Gérard, the sbire, Anaïs), avoided: the
/// chapter's main path fights only its story battles.
const ROUTE_SIGHT: &[(i16, i16)] = &[
    (12, 3),
    (12, 4),
    (12, 5),
    (7, 8),
    (8, 8),
    (9, 8),
    (26, 10),
    (27, 10),
    (28, 10),
    (29, 10),
    (24, 5),
    (24, 6),
    (24, 7),
    (37, 8),
    (38, 8),
    (39, 8),
    (40, 8),
    (41, 8),
    (44, 5),
    (44, 6),
    (44, 7),
    (44, 8),
];

/// The sweeper's sight (21, 13) left, avoided until the bin starts his battle.
const BALAYEUR_SIGHT: &[(i16, i16)] = &[(19, 13), (20, 13)];

/// The Usine sbire's sight (23, 5) right: J3 is reached along row 4, out of it.
const USINE_SIGHT: &[(i16, i16)] = &[(24, 5), (25, 5)];

/// Salle 1's binary tiles (x 6 and 8, y 30–37): never planned through (the steps walk them).
const S1_TILES: &[(i16, i16)] = &[
    (6, 30),
    (6, 31),
    (6, 32),
    (6, 33),
    (6, 34),
    (6, 35),
    (6, 36),
    (6, 37),
    (8, 30),
    (8, 31),
    (8, 32),
    (8, 33),
    (8, 34),
    (8, 35),
    (8, 36),
    (8, 37),
];

/// Chapter 2's steps.
pub fn steps() -> Vec<Step> {
    use Dir::*;
    let mut s = vec![
        Step::Check("ch2_start", |r| {
            assert_eq!(r.w.map(), ROUTE);
            assert_eq!(r.w.var(VAR_SIL_STORY), 17);
            assert!(r.w.flag(FLAG_SIL_MODULE_COUPE));
            assert!(r.w.flag(FLAG_SIL_BADGE_01));
            assert!(!r.w.flag(FLAG_SIL_BADGE_02));
            assert_eq!(r.pos(), (0, 3));
            assert!(!r.w.party().is_empty());
        }),
        // ---- Route S2 ----
        // The arrival (Route_S02_Sc01).
        Step::Walk(Right),
        Step::Check("route_sc01", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 20);
            assert!(r.has_text(Route_S02_Sc01_T1));
        }),
        // The Agent (16, 10): three Paralyze Heals, once.
        goto_avoiding(ROUTE, 15, 10, ROUTE_SIGHT),
        Step::Interact(Right),
        Step::Interact(Right),
        Step::Check("agent", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_ROUTE_S02_PARALYZE_HEAL));
            assert!(r.count(ITEM_PARALYZE_HEAL) >= 3);
            assert_eq!(r.times(Route_S02_Agent), 1);
        }),
        // Adrien (24, 14), Route_S02_Sc02: the Revive, story 21.
        goto_avoiding(ROUTE, 25, 14, ROUTE_SIGHT),
        Step::Interact(Left),
        Step::Check("route_sc02", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 21);
            assert!(r.w.flag(FLAG_SIL_ITEM_ADRIEN_CH2));
            assert!(r.count(ITEM_REVIVE) >= 1);
            r.printed_in_order(&[Route_S02_Sc02_T1, Route_S02_Sc02_T3, Route_S02_Sc02_T5]);
        }),
        // Cable C (31–32, 9): cut, then crossed.
        goto_avoiding(ROUTE, 30, 9, ROUTE_SIGHT),
        Step::Interact(Right),
        Step::Check("cable_2", |r| {
            assert!(r.w.flag(FLAG_SIL_ROUTE_S02_CABLE_2));
            assert!(r.has_text(Route_S02_Cable_T2));
        }),
        Step::Walk(Right),
        Step::Walk(Right),
        Step::Walk(Right),
        Step::Check("cable_2_crossed", |r| assert_eq!(r.pos(), (33, 9))),
        // The Technicien (34, 6): TM34.
        goto_avoiding(ROUTE, 34, 7, ROUTE_SIGHT),
        Step::Interact(Up),
        Step::Check("technicien", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_ROUTE_S02_TM_SHOCK_WAVE));
            assert!(r.count(ITEM_TM34) >= 1);
        }),
        // Gwada (Route_S02_Sc03), row 6 at story 21: the battle, his heal, story 22.
        goto_avoiding(ROUTE, 49, 6, ROUTE_SIGHT),
        Step::Check("route_sc03", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 22);
            assert!(r.beaten(TRAINER_SIL_GWADA_1));
            assert!(r.w.flag(FLAG_SIL_HIDE_ROUTE_S02_GWADA));
            assert!(r.healed());
            r.printed_in_order(&[
                Route_S02_Sc03_T1,
                Route_S02_Sc03_T2,
                Route_S02_Sc03_T3,
                Route_S02_Sc03_T4,
                Route_S02_Sc03_T5,
            ]);
        }),
        goto_avoiding(ROUTE, 55, 4, ROUTE_SIGHT),
        Step::Enter {
            dir: Right,
            map: TOWN,
        },
        // ---- Gris-Échec: the embargo ----
        Step::Walk(Right),
        Step::Check("town_sc01", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 23);
            assert_eq!(r.w.var(VAR_SIL_GRIS_ECHEC_STATE), 0);
            r.printed_in_order(&[Gris_Echec_Sc01_T1, Gris_Echec_Sc01_T2]);
        }),
        // J1 on the annex roof: the sbire sees the ladder top.
        goto(TOWN, 15, 5),
        Step::Check("sbire_toit", |r| {
            assert!(r.beaten(TRAINER_SIL_SBIRE_BROUILLEUR_1));
            assert!(r.has_text(Gris_Echec_Toit_Intro));
        }),
        goto(TOWN, 16, 4),
        Step::Interact(Right),
        Step::Check("jammer_1", |r| {
            assert!(r.w.flag(FLAG_SIL_GRIS_JAMMER_1));
            assert_eq!(r.w.var(VAR_SIL_GRIS_ECHEC_STATE), 1);
            r.printed_in_order(&[Gris_Echec_Sc02_T1, Gris_Echec_Sc02_T2, Gris_Echec_Sc02_T3]);
        }),
        // J2 in the plaza bin: the sweeper's battle first.
        goto_avoiding(TOWN, 20, 15, BALAYEUR_SIGHT),
        Step::Interact(Up),
        Step::Check("jammer_2", |r| {
            assert!(r.beaten(TRAINER_SIL_SBIRE_BROUILLEUR_2));
            assert!(r.w.flag(FLAG_SIL_GRIS_JAMMER_2));
            assert_eq!(r.w.var(VAR_SIL_GRIS_ECHEC_STATE), 2);
            r.printed_in_order(&[Gris_Echec_Poubelle, Gris_Echec_Balayeur_Intro]);
        }),
        // J3 in the Usine, behind its cable, along row 4 out of the sbire's sight.
        goto(TOWN, 30, 21),
        Step::Enter {
            dir: Up,
            map: USINE,
        },
        goto_avoiding(USINE, 24, 4, USINE_SIGHT),
        Step::Interact(Right),
        Step::Check("usine_cable", |r| {
            assert!(r.w.flag(FLAG_SIL_USINE_CABLE_CUT));
            assert!(!r.beaten(TRAINER_SIL_SBIRE_BROUILLEUR_3));
        }),
        Step::Walk(Right),
        Step::Interact(Right),
        Step::Check("jammer_3", |r| {
            assert_eq!(r.pos(), (25, 4));
            assert!(r.w.flag(FLAG_SIL_GRIS_JAMMER_3));
            assert_eq!(r.w.var(VAR_SIL_GRIS_ECHEC_STATE), 3);
            assert_eq!(r.w.var(VAR_SIL_STORY), 24);
            assert!(r.has_text(Gris_Echec_Sc02_T4));
        }),
        goto_avoiding(USINE, 14, 22, USINE_SIGHT),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        // The arena guard (Gris_Echec_Sc03): the Tour door opens.
        goto(TOWN, 34, 11),
        Step::Interact(Up),
        Step::Check("town_sc03", |r| {
            assert!(r.w.flag(FLAG_SIL_TOUR_OPEN));
            r.printed_in_order(&[Gris_Echec_Sc03_T1, Gris_Echec_Sc03_T2]);
        }),
    ];
    s.extend(heal());
    s.extend([
        // ---- the Tour d'Observation ----
        goto(TOWN, 21, 5),
        Step::Enter { dir: Up, map: TOUR },
        goto(TOUR, 5, 7),
        Step::Check("tour_sbire_1", |r| {
            assert!(r.beaten(TRAINER_SIL_SBIRE_TOUR_1))
        }),
        goto(TOUR, 5, 2),
        Step::Check("tour_sbire_2", |r| {
            assert!(r.beaten(TRAINER_SIL_SBIRE_TOUR_2))
        }),
        Step::Enter { dir: Up, map: TOP },
        // Duval (Tour_Sc01): the battle, the Carte Perforée, story 25.
        goto(TOP, 5, 3),
        Step::Interact(Up),
        Step::Check("tour_sc01", |r| {
            assert!(r.beaten(TRAINER_SIL_CHEF_SECURITE_DUVAL));
            assert!(r.w.flag(FLAG_SIL_CARTE_PERFOREE));
            assert!(r.count(ITEM_SIL_CARTE_PERFOREE) >= 1);
            assert_eq!(r.w.var(VAR_SIL_STORY), 25);
            r.printed_in_order(&[Tour_Sc01_T1, Tour_Sc01_T2, Tour_Sc01_T3]);
        }),
        goto(TOP, 2, 2),
        Step::Enter { dir: Up, map: TOUR },
        goto(TOUR, 5, 8),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
    ]);
    s.extend(heal());
    s.extend([
        // The guard steps aside (Gris_Echec_Sc05).
        goto(TOWN, 34, 11),
        Step::Interact(Up),
        Step::Check("town_sc05", |r| {
            assert!(r.w.flag(FLAG_SIL_HIDE_GRIS_GARDE));
            assert!(!r.w.flag(FLAG_SIL_HIDE_GRIS_GARDE_ECARTE));
            r.printed_in_order(&[Gris_Echec_Sc05_T1, Gris_Echec_Sc05_T2]);
        }),
        Step::Walk(Up),
        Step::Enter { dir: Up, map: GYM },
        // ---- the arena: Salle 1 ----
        // The two screens of the passage (6, 39) and (8, 39).
        goto(GYM, 7, 39),
        Step::Interact(Left),
        Step::Interact(Right),
        Step::Check("ecrans", |r| {
            r.printed_in_order(&[Gym2_S1_Ecran, Carte_Perforee_Lire, Gym2_Ecran_Hall]);
        }),
        goto(GYM, 7, 38),
        // A wrong first tile: the robot, then back to the hall's top.
        Step::Walk(Up),
        Step::Walk(Left),
        Step::Check("s1_erreur", |r| {
            assert!(r.beaten(TRAINER_SIL_ROBOT_BINAIRE));
            assert_eq!(r.w.map(), GYM);
            assert_eq!(r.pos(), (7, 38));
            r.printed_in_order(&[Gym2_S1_Erreur, Gym2_Robot_Intro]);
        }),
        // The key, bottom-up 1 0 1 1 0 0 1 0 (x 8 is 1, x 6 is 0; x 7 crosses between rows).
        Step::Walk(Up),
        Step::Walk(Right),
        Step::Walk(Left),
        Step::Walk(Up),
        Step::Walk(Left),
        Step::Walk(Right),
        Step::Walk(Up),
        Step::Walk(Right),
        Step::Walk(Up),
        Step::Walk(Left),
        Step::Walk(Up),
        Step::Walk(Left),
        Step::Walk(Up),
        Step::Walk(Right),
        Step::Walk(Up),
        Step::Walk(Right),
        Step::Walk(Left),
        Step::Walk(Up),
        Step::Walk(Left),
        Step::Walk(Right),
        Step::Walk(Up),
        Step::Check("salle_1", |r| {
            assert_eq!(r.pos(), (7, 29));
            assert_eq!(r.w.var(VAR_TEMP_2), 8);
            assert_eq!(r.times(Gym2_S1_Erreur), 1);
        }),
        // ---- Salle 2: the terminal, then the maze ----
        Step::Walk(Up),
        Step::Interact(Left),
        Step::Check("terminal", |r| {
            r.printed_in_order(&[Gym2_S2_Terminal, Gym2_S2_Sature]);
        }),
        // The Chambre Sourde, off the maze (11, 26).
        goto(GYM, 10, 26),
        Step::Enter {
            dir: Right,
            map: SOURDE,
        },
        goto(SOURDE, 2, 4),
        Step::Check("melomane", |r| {
            assert!(r.beaten(TRAINER_SIL_MELOMANE_SOURDE));
            assert!(r.has_text(Gym2_Melomane_Intro));
        }),
        goto(SOURDE, 4, 2),
        Step::Interact(Up),
        goto(SOURDE, 7, 4),
        Step::Interact(Up),
        Step::Check("coffre", |r| {
            assert!(r.has_text(Gym2_Sourde_Panneau));
            assert!(r.w.flag(FLAG_SIL_ITEM_GYM2_SOOTHE_BELL));
            assert!(r.count(ITEM_SOOTHE_BELL) >= 1);
            assert!(r.has_text(Gym2_Sourde_Coffre));
        }),
        goto(SOURDE, 2, 4),
        Step::Enter {
            dir: Left,
            map: GYM,
        },
        // ---- Salle 3: across the board to the King ----
        goto(GYM, 7, 14),
        Step::Check("gym2_sc10", |r| {
            assert!(r.beaten(TRAINER_SIL_YOHAN_1));
            assert!(r.w.flag(FLAG_SIL_BADGE_02));
            assert!(r.w.flag(FLAG_SIL_ITEM_YOHAN_TM));
            assert!(r.count(ITEM_TM23) >= 1);
            r.printed_in_order(&[
                Gym2_Sc10_T1,
                Gym2_Sc10_T2,
                Gym2_Sc10_T3,
                Gym2_Sc10_T4,
                Gym2_Sc10_T5,
                Gym2_Sc10_T6,
            ]);
            // The Fou watches every path; at least two pieces fought.
            assert!(r.beaten(TRAINER_SIL_ECHEC_FOU));
            let pieces = [
                TRAINER_SIL_ECHEC_PION_1,
                TRAINER_SIL_ECHEC_PION_2,
                TRAINER_SIL_ECHEC_CAVALIER,
                TRAINER_SIL_ECHEC_FOU,
                TRAINER_SIL_ECHEC_TOUR,
            ];
            assert!(pieces.iter().filter(|t| r.beaten(**t)).count() >= 2);
        }),
        Step::Check("gym2_sc11_sc12", |r| {
            assert!(r.w.flag(FLAG_SIL_MODULE_FORCE));
            assert!(r.w.flag(FLAG_SIL_GWADA_LAINERGIE));
            assert!(r.w.flag(FLAG_SIL_LOG_02));
            assert_eq!(r.w.var(VAR_SIL_SYNC), 20);
            assert_eq!(r.w.var(VAR_SIL_STORY), 27);
            for f in [
                FLAG_SIL_HIDE_GYM2_GWADA,
                FLAG_SIL_HIDE_GYM2_WATTOUAT,
                FLAG_SIL_HIDE_GYM2_LAINERGIE,
            ] {
                assert!(r.w.flag(f), "{f:?}");
            }
            r.printed_in_order(&[
                Gym2_Sc11_T1,
                Gym2_Sc11_T4,
                Gym2_Sc11_T6,
                Gym2_Sc11_T8,
                Gym2_Sc12_T1,
                LOG_02_Texte,
                Gym2_Sc12_T4,
                Gym2_Sc12_T5,
            ]);
        }),
        // Out, straight down Salle 1's middle column.
        goto_avoiding(GYM, 7, 44, S1_TILES),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        // ---- Kimiko's stream (Gris_Echec_Sc06) ----
        Step::Walk(Down),
        Step::Check("town_sc06", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 28);
            assert!(r.w.flag(FLAG_SIL_HIDE_GRIS_GARDE_S3));
            assert!(r.w.flag(FLAG_SIL_HIDE_GRIS_GWADA));
            assert!(r.w.flag(FLAG_SIL_HIDE_GRIS_LAINERGIE));
            r.printed_in_order(&[Gris_Echec_Sc06_T1, Gris_Echec_Sc06_T5, Gris_Echec_Sc06_T6]);
        }),
        // ---- the Champ Magnétique: the Usine's crates, the Metal Coat ----
        goto(TOWN, 30, 21),
        Step::Enter {
            dir: Up,
            map: USINE,
        },
        goto_avoiding(USINE, 14, 3, USINE_SIGHT),
        Step::Interact(Left),
        Step::Check("caisses", |r| {
            assert!(r.w.flag(FLAG_SIL_USINE_CRATES_MOVED));
        }),
        Step::Walk(Left),
        Step::Walk(Left),
        Step::Walk(Left),
        goto(USINE, 3, 3),
        Step::Interact(Down),
        Step::Check("metal_coat", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_USINE_METAL_COAT));
            assert!(r.count(ITEM_METAL_COAT) >= 1);
        }),
    ]);
    s
}

/// From Chapter 1's last step (the Brico-Bourg Labo's Chimiste) to Chapter 2's start: out of
/// the Labo, north through Route S1 and Bois-Éther, east onto Route S2 at (0, 3), where
/// [`start`] puts a run that does not play the chapters before.
pub fn from_ch01() -> Vec<Step> {
    use Dir::*;
    vec![
        goto(MAP_BRICO_BOURG_LABO, 6, 10),
        Step::Enter {
            dir: Down,
            map: MAP_BRICO_BOURG,
        },
        goto(MAP_BRICO_BOURG, 11, 1),
        Step::Enter {
            dir: Up,
            map: MAP_ROUTE_S01,
        },
        goto_avoiding(MAP_ROUTE_S01, 10, 2, ROUTE_S01_SIGHT),
        Step::Enter {
            dir: Up,
            map: MAP_BOIS_ETHER,
        },
        // Through the open Secteur Bêta barrier (33, 13–14), which the planner sees shut (a
        // `set_metatile`), then the east corridor: its y 12–15 meets the route's y 2–5.
        goto(MAP_BOIS_ETHER, 32, 13),
        Step::Walk(Right),
        Step::Walk(Right),
        Step::Enter {
            dir: Right,
            map: ROUTE,
        },
    ]
}

/// The Centre's nurse, from the town and back.
fn heal() -> [Step; 7] {
    use Dir::*;
    [
        goto(TOWN, 5, 9),
        Step::Enter {
            dir: Up,
            map: CENTER,
        },
        goto(CENTER, 7, 4),
        Step::Interact(Up),
        Step::Check("centre", |r| {
            assert!(r.healed());
            assert!(r.has_text(gText_RestoredPkmnToFullHealth));
        }),
        goto(CENTER, 6, 7),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
    ]
}

/// What the end of Chapter 2 asserts beyond its checkpoints: the story, the bag, no dropped
/// event, and every Chapter 2 fallback reported.
pub fn check_end(r: &Run) {
    assert_eq!(r.w.var(VAR_SIL_STORY), 28);
    assert!(r.w.flag(FLAG_SIL_BADGE_02));
    for (item, n) in [
        (ITEM_REVIVE, 1),
        (ITEM_TM34, 1),
        (ITEM_PARALYZE_HEAL, 3),
        (ITEM_SIL_CARTE_PERFOREE, 1),
        (ITEM_TM23, 1),
        (ITEM_SOOTHE_BELL, 1),
        (ITEM_METAL_COAT, 1),
    ] {
        assert!(r.count(item) >= n, "item {item}");
    }
    assert_eq!(r.w.events_dropped(), 0);
    for name in [
        "screen_flash",
        "play_se",
        "mallette_call",
        "end_call",
        "follower",
        "play_evolution",
        "mega_evolution",
        "show_sync",
        "sound rule",
        "jammer graphic",
        "key item use",
        "strength",
    ] {
        assert!(r.degraded.contains(name), "{name}: {:?}", r.degraded);
    }
}
