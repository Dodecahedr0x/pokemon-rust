//! Chapter 1, Bois-Éther, from Chapter 0's end (`VAR_SIL_STORY` 9 on Route S1's north edge) to
//! the Secteur Bêta barrier open (17), then the optional Brico-Bourg Labo:
//! - the village: arrival (Sc01), the meeting and Julien's call (Sc02, Sc03), the guardian's
//!   seed (Sc04), the Center, the Mart, the Herboriste (Sc05), the hidden Super Potion, the
//!   X Defend;
//! - the gym: Gym_Sc01, the guide, the spore airlock (contaminated, sent back, then the safe
//!   path past Jules and Iris), the workshop's valve (Milo, the crates, Rose, the workbench), the
//!   pressure airlock, Loïc (Gym_Sc10 to Sc12: the badge, the Lame-Frequence, `LOG_01`);
//! - the red sky (Sc06: Gogo then Romain), the secret grove (Sc08, the Evoluroc), the barrier
//!   (Sc07);
//! - the Labo: the Bricoleuse's hint, the brambles cut (Brico-Bourg (4, 9)), Brico_Labo_Sc01,
//!   both sbires, the items, the memo, the Chimiste.
//!
//! The party is Chapter 0's starter (Cyndaquil, trained to level 8 on Route S1), trained again
//! in Bois-Éther's west tall grass ([`Step::Train`], the wild levels 5–8 of §6) to
//! [`GYM_LEVEL`] before the gym, the level a player brings to Loïc's ace (16, §4); the run heals
//! at the Center before each part of the gym and at Maman's before the Labo's boss. Every
//! battle is won by the battle policy (`silice_spine` docs), the trainers' exp included. One
//! test hook, for the Labo's boss only: [`SECOND`].

use super::ch00::ROUTE_S01_SIGHT;
use super::{Hook, Run, Step, goto, goto_avoiding};
use content::consts::*;
use engine::data::{Dir, MapId};

const VILLAGE: MapId = MAP_BOIS_ETHER;
const GYM: MapId = MAP_BOIS_ETHER_GYM;
const CENTER: MapId = MAP_BOIS_ETHER_POKEMON_CENTER;
const MART: MapId = MAP_BOIS_ETHER_MART;
const HERBORISTE: MapId = MAP_BOIS_ETHER_HERBORISTE;
const LABO: MapId = MAP_BRICO_BOURG_LABO;

/// The lead's level at the gym's door, after [`Step::Train`] (Loïc's team is 12–16, §4).
pub const GYM_LEVEL: u8 = 16;

/// The party's second Pokémon, for the Chimiste's battle only (a test hook, the chapter's one):
/// a Mareep (Bois-Éther's wild list, §6) caught and trained to 12. Without it the run whites out
/// on Koffing's Self-Destruct, a double KO (see the plan's "SIL2 Ch1 scripts as built").
/// Re-measured with the learn policy (the starter has Quick Attack from 21): still lost without
/// the hook (Koffing left at 19 of 41 HP); a MAREEP at 5 (its catch level, untrained) passes
/// Chapter 1 and the chained runs, but the start states (which mirror the chain's party) then
/// lose Chapter 2 to Yohan's Alakazam, so 12 stays. A [`GYM_LEVEL`] of 15 passes Chapter 1
/// and loses the chained Chapter 2; 14 loses Chapter 1.
pub const SECOND: Hook =
    Hook::MonBefore("BricoBourg_Labo_EventScript_Chimiste", SPECIES_MAREEP, 12);

/// Two tall-grass cells of the west meadow (x 2–7, y 15–20), paced by [`Step::Train`].
const GRASS: ((i16, i16), (i16, i16)) = ((6, 17), (6, 18));

/// A heal at the Center, from anywhere in the village back to its door (27, 13).
const HEAL: &[Step] = &[
    goto(VILLAGE, 27, 13),
    Step::Enter {
        dir: Dir::Up,
        map: CENTER,
    },
    goto(CENTER, 6, 4),
    Step::Interact(Dir::Up),
    goto(CENTER, 5, 6),
    Step::Enter {
        dir: Dir::Down,
        map: VILLAGE,
    },
];

/// From the gym (the hall, or zones 1 to 4 once door D is open): out, [`HEAL`], and back in to
/// door D's front cell (8, 23), its panel opening it again (the map load closed it).
const GYM_HEAL: &[Step] = &[
    goto_avoiding(GYM, 7, 38, GRATES),
    Step::Enter {
        dir: Dir::Down,
        map: VILLAGE,
    },
    goto(VILLAGE, 27, 13),
    Step::Enter {
        dir: Dir::Up,
        map: CENTER,
    },
    goto(CENTER, 6, 4),
    Step::Interact(Dir::Up),
    goto(CENTER, 5, 6),
    Step::Enter {
        dir: Dir::Down,
        map: VILLAGE,
    },
    goto(VILLAGE, 17, 5),
    Step::Enter {
        dir: Dir::Up,
        map: GYM,
    },
    goto_avoiding(GYM, 8, 23, GRATES),
    Step::Interact(Dir::Up),
];

/// From the Labo: out by the porch, Maman's heal (Player_House_1F), and back in.
const LABO_HEAL: &[Step] = &[
    goto(LABO, 6, 10),
    Step::Enter {
        dir: Dir::Down,
        map: MAP_BRICO_BOURG,
    },
    goto(MAP_BRICO_BOURG, 5, 15),
    Step::Enter {
        dir: Dir::Up,
        map: MAP_BRICO_BOURG_PLAYER_HOUSE_1F,
    },
    goto(MAP_BRICO_BOURG_PLAYER_HOUSE_1F, 3, 2),
    Step::Interact(Dir::Left),
    goto(MAP_BRICO_BOURG_PLAYER_HOUSE_1F, 4, 7),
    Step::Enter {
        dir: Dir::Down,
        map: MAP_BRICO_BOURG,
    },
    goto(MAP_BRICO_BOURG, 4, 8),
    Step::Enter {
        dir: Dir::Up,
        map: LABO,
    },
];

/// The 20 active spore grates of the gym's zone 1.
pub const GRATES: &[(i16, i16)] = &[
    (5, 23),
    (11, 23),
    (4, 24),
    (7, 24),
    (8, 24),
    (9, 24),
    (5, 25),
    (9, 25),
    (11, 25),
    (3, 26),
    (6, 26),
    (8, 26),
    (5, 27),
    (8, 27),
    (10, 27),
    (4, 28),
    (7, 28),
    (11, 28),
    (6, 29),
    (9, 29),
];

/// The sight lines of the gym's zone-2 trainers (Milo (8, 17) left, Rose (11, 13) down),
/// avoided until they are fought.
const ZONE2_SIGHT: &[(i16, i16)] = &[
    (4, 17),
    (5, 17),
    (6, 17),
    (7, 17),
    (11, 14),
    (11, 15),
    (11, 16),
    (11, 17),
];

/// The Labo's sbire 1 (1, 8) looks right along y 8.
const LABO_SIGHT: &[(i16, i16)] = &[(2, 8), (3, 8), (4, 8), (5, 8)];

/// Chapter 1's steps.
pub fn steps() -> Vec<Step> {
    use Dir::*;
    let mut v = vec![
        Step::Check("ch1_start", |r| {
            assert_eq!(r.w.map(), MAP_ROUTE_S01);
            assert_eq!(r.w.var(VAR_SIL_STORY), 9);
            assert!(!r.w.flag(FLAG_SIL_MODULE_COUPE));
            assert!(!r.w.flag(FLAG_SIL_BADGE_01));
            assert_eq!(r.w.party().len(), 1);
            assert_eq!(r.w.party().get(0).unwrap().species, SPECIES_CYNDAQUIL);
        }),
        // ---- arrival and the meeting (Sc01 to Sc03) ----
        Step::Enter {
            dir: Up,
            map: VILLAGE,
        },
        goto(VILLAGE, 14, 24),
        Step::Check("sc01", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 10);
            assert!(r.has_text(Bois_Ether_Sc01_T1));
        }),
        goto(VILLAGE, 17, 13),
        Step::Check("sc02_sc03", |r| {
            assert_eq!(r.pos(), (17, 13));
            assert_eq!(r.w.var(VAR_SIL_STORY), 12);
            assert!(r.w.flag(FLAG_SIL_HIDE_BOIS_ETHER_ADRIEN));
            r.printed_in_order(&[
                Bois_Ether_Sc02_T1,
                Bois_Ether_Sc02_T2,
                Bois_Ether_Sc02_T3,
                Bois_Ether_Sc03_T1,
                Bois_Ether_Sc03_T2,
                Bois_Ether_Sc03_T3,
            ]);
        }),
        // The oak's guardian (Sc04): the Miracle Seed, once.
        goto(VILLAGE, 18, 12),
        Step::Interact(Right),
        Step::Interact(Right),
        Step::Check("sc04", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_BOIS_ETHER_MIRACLE_SEED));
            assert_eq!(r.count(ITEM_MIRACLE_SEED), 1);
            assert_eq!(r.times(Bois_Ether_Sc04_T1), 1);
            r.printed_in_order(&[Bois_Ether_Sc04_T1, Bois_Ether_Sc04_T2]);
        }),
        // ---- the Center: the respawn, the nurse ----
        goto(VILLAGE, 27, 13),
        Step::Enter {
            dir: Up,
            map: CENTER,
        },
        Step::Check("center_respawn", |r| {
            assert_eq!(r.w.respawn(), HEAL_LOCATION_SIL_BOIS_ETHER)
        }),
        goto(CENTER, 6, 4),
        Step::Interact(Up),
        Step::Check("nurse", |r| {
            assert!(r.healed());
            r.printed_in_order(&[
                gText_WouldYouLikeToRestYourPkmn,
                gText_IllTakeYourPkmn,
                gText_RestoredPkmnToFullHealth,
                gText_WeHopeToSeeYouAgain,
            ]);
        }),
        goto(CENTER, 3, 5),
        Step::Interact(Left),
        Step::Check("center_npc", |r| assert!(r.has_text(Bois_Ether_PC_Npc))),
        goto(CENTER, 5, 6),
        Step::Enter {
            dir: Down,
            map: VILLAGE,
        },
        // ---- the Mart (fallback: no shop screen) ----
        goto(VILLAGE, 27, 18),
        Step::Enter { dir: Up, map: MART },
        goto(MART, 3, 3),
        Step::Interact(Left),
        Step::Check("mart", |r| {
            r.printed_in_order(&[gText_HowMayIServeYou, gText_PleaseComeAgain]);
            assert!(r.degraded.contains("mart"));
        }),
        goto(MART, 4, 6),
        Step::Interact(Up),
        Step::Check("mart_client", |r| assert!(r.has_text(Bois_Ether_Mart_Npc))),
        Step::Enter {
            dir: Down,
            map: VILLAGE,
        },
        // ---- the Herboriste (Sc05): the sample once, the shop ----
        goto(VILLAGE, 5, 13),
        Step::Enter {
            dir: Up,
            map: HERBORISTE,
        },
        goto(HERBORISTE, 2, 4),
        Step::Interact(Left),
        Step::Interact(Left),
        Step::Check("sc05", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_HERBORISTE_SAMPLE));
            assert_eq!(r.count(ITEM_HEAL_POWDER), 1);
            assert_eq!(r.times(Herboriste_Cadeau), 1);
            assert!(!r.has_text(Herboriste_T2));
            r.printed_in_order(&[Herboriste_T1, Herboriste_Cadeau, gText_HowMayIServeYou]);
        }),
        goto(HERBORISTE, 3, 5),
        Step::Enter {
            dir: Down,
            map: VILLAGE,
        },
        // ---- the items: the hidden Super Potion (once), the X Defend ----
        goto(VILLAGE, 4, 13),
        Step::Interact(Down),
        Step::Interact(Down),
        Step::Check("hidden_super_potion", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_BOIS_ETHER_SUPER_POTION));
            assert_eq!(r.count(ITEM_SUPER_POTION), 1);
        }),
        goto(VILLAGE, 27, 8),
        Step::Interact(Right),
        Step::Check("x_defend", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_BOIS_ETHER_X_DEFEND));
            assert_eq!(r.count(ITEM_X_DEFEND), 1);
        }),
        // A sign and an NPC before the alert.
        goto(VILLAGE, 11, 7),
        Step::Interact(Left),
        Step::Check("jardiniere", |r| {
            assert!(r.has_text(Bois_Ether_Npc_Jardiniere))
        }),
        // The brambles before the module: the sign.
        goto(VILLAGE, 9, 6),
        Step::Interact(Left),
        Step::Check("sc08_blocked", |r| {
            assert!(r.has_text(Bois_Ether_Sc08_T1));
            assert!(!r.w.flag(FLAG_SIL_BOIS_ETHER_RONCES_CUT));
        }),
        // ---- training in the west meadow, then a heal ----
        Step::Train {
            a: GRASS.0,
            b: GRASS.1,
            level: GYM_LEVEL,
            heal: HEAL,
        },
        Step::Check("trained", |r| {
            let lead = r.w.party().get(0).unwrap();
            assert!(lead.level >= GYM_LEVEL);
            assert_eq!(lead.species, SPECIES_QUILAVA);
            assert_eq!(r.w.party().len(), 1);
        }),
    ];
    v.extend_from_slice(HEAL);
    v.extend([
        // ---- the gym ----
        goto(VILLAGE, 17, 5),
        Step::Enter { dir: Up, map: GYM },
        goto(GYM, 7, 38),
        Step::Check("gym_sc01", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 13);
            assert!(r.has_text(Gym_Sc01_T1));
        }),
        goto(GYM, 5, 36),
        Step::Interact(Up),
        Step::Check("conseiller", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_GYM1_FRESH_WATER));
            assert_eq!(r.count(ITEM_FRESH_WATER), 1);
            assert!(r.has_text(Gym_Conseiller));
        }),
        // Zone 1: a grate, then sent back to the shower from door D.
        goto(GYM, 7, 29),
        Step::Walk(Up),
        Step::Check("spores", |r| {
            assert_eq!(r.pos(), (7, 28));
            assert!(r.w.flag(FLAG_SIL_GYM1_CONTAMINE));
            assert!(r.has_text(Gym_Z1_Spores));
        }),
        goto_avoiding(GYM, 8, 23, GRATES),
        Step::Check("jules_iris", |r| {
            assert!(r.beaten(TRAINER_SIL_APPRENTI_JULES));
            assert!(r.beaten(TRAINER_SIL_BOTANISTE_IRIS));
            r.printed_in_order(&[Gym_Jules_Intro, Gym_Iris_Intro]);
        }),
        Step::Interact(Up),
        Step::Check("porte_refus", |r| {
            assert_eq!(r.w.map(), GYM);
            assert_eq!(r.pos(), (7, 30));
            assert!(!r.w.flag(FLAG_SIL_GYM1_CONTAMINE));
            assert!(r.has_text(Gym_Z1_Porte_Refus));
        }),
        // The shower, clean: nothing; contaminated: clean again.
        goto(GYM, 7, 29),
        Step::Walk(Up),
        Step::Walk(Down),
        Step::Check("douche", |r| {
            assert!(!r.w.flag(FLAG_SIL_GYM1_CONTAMINE));
            assert!(r.has_text(Gym_Z1_Douche));
        }),
        // The safe path, the door opens.
        goto_avoiding(GYM, 8, 23, GRATES),
        Step::Interact(Up),
        Step::Check("porte_ok", |r| {
            assert!(!r.w.flag(FLAG_SIL_GYM1_CONTAMINE));
            assert!(r.has_text(Gym_Z1_Porte_Ok));
        }),
        // Back onto a grate, then onto the open door D: sent back to the shower again.
        Step::Walk(Down),
        Step::Walk(Up),
        Step::Walk(Up),
        Step::Check("porte_passage", |r| {
            assert_eq!(r.pos(), (7, 30));
            assert!(!r.w.flag(FLAG_SIL_GYM1_CONTAMINE));
            assert_eq!(r.times(Gym_Z1_Porte_Refus), 2);
        }),
    ]);
    // A heal after zone 1's battles; back at door D, opened again.
    v.extend_from_slice(GYM_HEAL);
    v.extend([
        goto(GYM, 8, 20),
        Step::Check("zone_2", |r| assert_eq!(r.pos(), (8, 20))),
        // Zone 2: the workbench without the pieces; Paul; the crates.
        goto_avoiding(GYM, 5, 15, ZONE2_SIGHT),
        Step::Interact(Up),
        Step::Check("etabli_manque", |r| {
            assert!(r.has_text(Gym_Z2_Etabli_Manque));
            assert!(!r.w.flag(FLAG_SIL_GYM1_VALVE));
        }),
        goto_avoiding(GYM, 10, 18, ZONE2_SIGHT),
        Step::Interact(Right),
        Step::Check("paul", |r| assert!(r.has_text(Gym_Z2_Paul))),
        goto_avoiding(GYM, 4, 16, ZONE2_SIGHT),
        Step::Interact(Up),
        Step::Check("fusible", |r| {
            assert!(r.w.flag(FLAG_SIL_GYM1_FUSIBLE));
            assert!(r.has_text(Gym_Z2_Caisse_Fusible));
        }),
        goto_avoiding(GYM, 6, 16, ZONE2_SIGHT),
        Step::Interact(Up),
        Step::Check("ressort", |r| assert!(r.has_text(Gym_Z2_Caisse_Ressort))),
        goto_avoiding(GYM, 2, 16, ZONE2_SIGHT),
        Step::Interact(Up),
        Step::Check("caisse_potion", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_GYM1_SUPER_POTION));
            assert_eq!(r.count(ITEM_SUPER_POTION), 2);
            assert!(r.has_text(Gym_Z2_Caisse_Potion));
        }),
        // Milo and Rose by sight: the seal and the gauge.
        goto(GYM, 6, 17),
        Step::Check("milo", |r| {
            assert!(r.beaten(TRAINER_SIL_APPRENTI_MILO));
            assert!(r.w.flag(FLAG_SIL_GYM1_JOINT));
            assert!(r.has_text(Gym_Milo_Intro));
        }),
        goto(GYM, 11, 17),
        Step::Check("rose", |r| {
            assert!(r.beaten(TRAINER_SIL_BOTANISTE_ROSE));
            assert!(r.w.flag(FLAG_SIL_GYM1_MANO));
            assert!(r.has_text(Gym_Rose_Intro));
        }),
        // The valve: the airlock door (7, 11–12) opens.
        goto(GYM, 5, 15),
        Step::Interact(Up),
        Step::Check("valve", |r| {
            assert!(r.w.flag(FLAG_SIL_GYM1_VALVE));
            r.printed_in_order(&[Gym_Z2_Etabli_Ok, Gym_Z2_Valve]);
        }),
        // Zone 3, then the greenhouse.
        goto(GYM, 7, 11),
        Step::Walk(Up),
        Step::Check("sas2", |r| {
            assert_eq!(r.pos(), (7, 10));
            r.printed_in_order(&[Gym_Z3_T1, Gym_Z3_T2]);
        }),
        goto(GYM, 7, 7),
        goto(GYM, 12, 2),
        Step::Interact(Up),
        Step::Check("serre_privee", |r| assert!(r.has_text(Gym_Serre_Privee))),
    ]);
    // A heal before the leader: out through both airlocks (open, behind the player too) and
    // back; the map load keeps them open (BoisEther_Gym_OnLoad) and closes door D.
    v.extend_from_slice(GYM_HEAL);
    v.extend([
        // Loïc (Gym_Sc10 to Sc12).
        goto(GYM, 7, 3),
        Step::Interact(Up),
        Step::Check("loic", |r| {
            assert!(r.beaten(TRAINER_SIL_LOIC_1));
            assert!(r.w.flag(FLAG_SIL_BADGE_01));
            assert!(r.w.flag(FLAG_SIL_MODULE_COUPE));
            assert!(r.w.flag(FLAG_SIL_LOG_01));
            assert!(r.w.flag(FLAG_SIL_ITEM_LOIC_TM));
            assert!(r.w.flag(FLAG_SIL_HIDE_GYM1_LOIC));
            assert_eq!(r.count(ITEM_TM09), 1);
            assert_eq!(r.w.var(VAR_SIL_SYNC), 10);
            assert_eq!(r.w.var(VAR_SIL_STORY), 15);
            r.printed_in_order(&[
                Gym_Sc10_T1,
                Gym_Sc10_T2,
                Gym_Sc10_T3,
                Gym_Sc10_T4,
                Gym_Sc11_T1,
                Gym_Sc11_T2,
                Gym_Sc11_T3,
                Gym_Sc10_T5,
                Gym_Sc10_T6,
                Gym_Sc12_T1,
                Gym_Sc12_T2,
                LOG_01_Texte,
                Gym_Sc12_T3,
                Gym_Sc12_T4,
            ]);
        }),
        // Back down through both airlocks and door D, all open.
        goto_avoiding(GYM, 7, 30, GRATES),
        goto(GYM, 5, 36),
        Step::Interact(Up),
        Step::Check("conseiller_apres", |r| {
            assert!(r.has_text(Gym_Conseiller_Apres));
            assert_eq!(r.count(ITEM_FRESH_WATER), 1);
        }),
        goto(GYM, 7, 38),
        Step::Enter {
            dir: Down,
            map: VILLAGE,
        },
        // ---- the red sky (Sc06): Gogo, then Romain ----
        goto(VILLAGE, 17, 6),
        Step::Check("sc06", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 16);
            let red = content::silice::sky::SKY_ALERT_RED;
            assert_eq!(
                r.w.palette().map(|p| (p.color, p.current)),
                Some((red.color, red.amount)),
                "the red sky"
            );
            assert!(r.beaten(TRAINER_SIL_GOGO_1));
            assert!(r.beaten(TRAINER_SIL_ROMAIN_1));
            for f in [
                FLAG_SIL_BOIS_ETHER_RED_SKY,
                FLAG_SIL_BOIS_ETHER_AMBUSH,
                FLAG_SIL_LABO_HINT,
                FLAG_SIL_ITEM_MATHIEU_CH1,
                FLAG_SIL_HIDE_BOIS_ETHER_GOGO,
                FLAG_SIL_HIDE_BOIS_ETHER_ROMAIN,
                FLAG_SIL_HIDE_BOIS_ETHER_SBIRE_1,
                FLAG_SIL_HIDE_BOIS_ETHER_SBIRE_2,
                FLAG_SIL_HIDE_BOIS_ETHER_MATHIEU,
            ] {
                assert!(r.w.flag(f), "{f:?}");
            }
            assert_eq!(r.count(ITEM_SUPER_POTION), 4);
            r.printed_in_order(&[
                Bois_Ether_Sc06_T1,
                Bois_Ether_Sc06_T2,
                Bois_Ether_Sc06_T3,
                Bois_Ether_Sc06_T4,
                Bois_Ether_Sc06_T5,
                Bois_Ether_Sc06_T6,
                Bois_Ether_Sc06_T7,
                Bois_Ether_Sc06_T8,
                Bois_Ether_Sc06_T9,
            ]);
        }),
        // After the alert: the gardener's second line.
        goto(VILLAGE, 11, 7),
        Step::Interact(Left),
        Step::Check("jardiniere_2", |r| {
            assert!(r.has_text(Bois_Ether_Npc_Jardiniere_2))
        }),
        // ---- the secret grove (Sc08): cut, the Evoluroc ----
        goto(VILLAGE, 9, 6),
        Step::Interact(Left),
        Step::Check("sc08", |r| {
            assert!(r.w.flag(FLAG_SIL_BOIS_ETHER_RONCES_CUT));
            r.printed_in_order(&[Bois_Ether_Sc08_T2, Bois_Ether_Sc08_T3]);
        }),
        Step::Walk(Left),
        Step::Check("ronces_walkable", |r| assert_eq!(r.pos(), (8, 6))),
        goto(VILLAGE, 4, 6),
        Step::Interact(Down),
        Step::Check("evoluroc", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_BOIS_ETHER_EVOLUROC));
            assert_eq!(r.count(ITEM_SIL_EVOLUROC), 1);
        }),
        goto(VILLAGE, 7, 6),
        Step::Walk(Right),
        Step::Walk(Right),
        // ---- the barrier (Sc07): story 17, Route S2 open ----
        goto(VILLAGE, 32, 14),
        Step::Check("sc07", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 17);
            assert!(!r.w.flag(FLAG_SIL_BOIS_ETHER_RED_SKY));
            assert_eq!(r.w.palette(), None, "the red sky is gone");
            r.printed_in_order(&[Bois_Ether_Sc07_T1, Bois_Ether_Sc07_T2, Bois_Ether_Sc07_T3]);
        }),
        Step::Walk(Right),
        Step::Check("barrier_open", |r| assert_eq!(r.pos(), (33, 14))),
        // ---- the Labo (optional): a heal (the moves' PP), then back to Brico-Bourg ----
        goto(VILLAGE, 27, 13),
        Step::Enter {
            dir: Up,
            map: CENTER,
        },
        goto(CENTER, 6, 4),
        Step::Interact(Up),
        Step::Check("nurse_again", |r| assert!(r.healed())),
        goto(CENTER, 5, 6),
        Step::Enter {
            dir: Down,
            map: VILLAGE,
        },
        goto(VILLAGE, 14, 25),
        Step::Enter {
            dir: Down,
            map: MAP_ROUTE_S01,
        },
        goto_avoiding(MAP_ROUTE_S01, 8, 39, ROUTE_S01_SIGHT),
        Step::Enter {
            dir: Down,
            map: MAP_BRICO_BOURG,
        },
        goto(MAP_BRICO_BOURG, 8, 6),
        Step::Interact(Left),
        Step::Check("bricoleuse_2", |r| {
            assert!(r.has_text(Brico_Bourg_Npc_Bricoleuse_2))
        }),
        // The Labo's brambles (4, 9): Bois_Ether_Sc08's obstacle, cut with the module.
        goto(MAP_BRICO_BOURG, 4, 10),
        Step::Interact(Up),
        Step::Check("labo_ronces", |r| {
            assert!(r.w.flag(FLAG_SIL_BRICO_LABO_RONCES_CUT));
            assert_eq!(r.times(Bois_Ether_Sc08_T3), 2);
        }),
        Step::Walk(Up),
        Step::Walk(Up),
        Step::Check("labo_porch", |r| assert_eq!(r.pos(), (4, 8))),
        Step::Enter { dir: Up, map: LABO },
        goto(LABO, 6, 10),
        Step::Check("labo_sc01", |r| {
            assert_eq!(r.w.var(VAR_SIL_BRICO_LABO_STATE), 1);
            assert_eq!(
                r.w.palette().map(|p| p.pulse),
                Some(content::silice::sky::STROBES.pulse),
                "the strobes"
            );
            assert!(r.has_text(Brico_Labo_Sc01_T1));
        }),
        goto_avoiding(LABO, 9, 10, LABO_SIGHT),
        Step::Interact(Up),
        Step::Check("revive", |r| assert_eq!(r.count(ITEM_REVIVE), 1)),
        goto_avoiding(LABO, 5, 7, LABO_SIGHT),
        Step::Interact(Right),
        Step::Check("silice", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_BRICO_LABO_SILICE));
            assert_eq!(r.count(ITEM_SIL_POUSSIERE_SILICE), 1);
            assert!(r.has_text(Labo_Echantillon));
        }),
        // Sbire 1 by sight.
        goto(LABO, 5, 8),
        Step::Check("sbire_1", |r| {
            assert!(r.beaten(TRAINER_SIL_SBIRE_LABO_1));
            assert!(r.has_text(Brico_Labo_Sbire1_Intro));
        }),
        goto(LABO, 12, 4),
        Step::Interact(Up),
        Step::Check("tm06", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_BRICO_LABO_TM));
            assert_eq!(r.count(ITEM_TM06), 1);
        }),
        goto(LABO, 12, 3),
        Step::Interact(Up),
        Step::Check("memo", |r| assert!(r.has_text(Labo_Memo))),
        // Sbire 2 by sight, then the Chimiste.
        goto(LABO, 1, 3),
        Step::Check("sbire_2", |r| {
            assert!(r.beaten(TRAINER_SIL_SBIRE_LABO_2));
            assert!(r.has_text(Brico_Labo_Sbire2_Intro));
        }),
    ]);
    // A heal before the boss: Maman, across the square, and back through the porch.
    v.extend_from_slice(LABO_HEAL);
    v.extend([
        // The Chimiste's last Pokémon is a Koffing with Self-Destruct: against a one-Pokémon
        // party, a double KO is Gen3's B_OUTCOME_DREW, a whiteout. A second Pokémon, as a
        // player has caught by now, turns it into a win ([`SECOND`], the one test hook).
        Step::Hook(SECOND),
        goto(LABO, 1, 1),
        Step::Interact(Right),
        Step::Check("chimiste", |r| {
            assert!(r.beaten(TRAINER_SIL_CHIMISTE_LABO));
            assert_eq!(r.w.party().len(), 2);
            assert_eq!(r.w.var(VAR_SIL_BRICO_LABO_STATE), 2);
            assert!(r.w.flag(FLAG_SIL_HIDE_LABO_CHIMISTE));
            r.printed_in_order(&[Brico_Labo_Chimiste_Intro, Brico_Labo_Chimiste_Apres]);
        }),
    ]);
    v
}

/// What the end of Chapter 1 asserts beyond its checkpoints: the end-of-chapter story step, no
/// dropped event, and every Chapter 1 fallback reported.
pub fn check_end(r: &Run) {
    assert_eq!(r.w.var(VAR_SIL_STORY), 17);
    assert_eq!(r.w.events_dropped(), 0);
    r.no_sky_fallbacks();
    for name in [
        "mart",
        "show_sync",
        "battle_two_trainers",
        "hidden item",
        "play_se",
        "mallette_call",
        "end_call",
    ] {
        assert!(r.degraded.contains(name), "{name}: {:?}", r.degraded);
    }
}
