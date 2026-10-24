//! Chapter 5, Owlys, from the rift's quay (`VAR_SIL_STORY` 50, Route S5's first wake played) to
//! the fifth badge and the lift to Route S6 (55):
//! - the rift (`RouteS05_Faille`): the Escape Rope, the gargoyle;
//! - Owlys: the arrival (Sc01), the Centre, Mathieu's skewers and the deal (Sc02, story 52), the
//!   alley grunt and Milo (Sc03, TM46);
//! - the silo (optional, played): the guard, the three grunts, the three Umbreon junctions
//!   (Sc01), the Silo Chief and the sacks emptied (Sc02), the five hidden items;
//! - the East Cave: the Grand Praugect's duo battle (two single battles, the multi-battle
//!   fallback), Owl-Justice and the Shaded Feather (Sc01, story 53), Lilith, the Moon Stone, the
//!   cart's Ultra Ball;
//! - the gym: the door (Sc04, shut before the cave, open after), the blue and black lecterns,
//!   the court (all bridges), the four watchers (the walkway), Skycker (Sc01: the badge, the
//!   flight module, TM12, `LOG_05`, story 54);
//! - out: the city at night (Sc05, story 55), Mathieu, the lift to Route S6.
//!
//! **Surf.** The engine has no Surf yet (M10), and Route S5 is a sea: the chapter's first
//! scenes (the islet's sailor and fisherman) are reached from their own start state ([`ilot`],
//! story 49), and the main path starts on the rift's quay ([`start`]), past the water. Both
//! are **temporary start states**: once Surf lands, the chapter starts from Chapter 4's end on
//! Port-Ressac's jetty and plays the sea (its trainers, the wreck, the coral cave's Lokhlass,
//! the Banc), which is not played now.
//!
//! **The party** ([`PARTY`], [`end_of_chapter_4`]): Chapter 4's end as `sil5_ch4` leaves it
//! (KADABRA 41 leading, TYPHLOSION 49, LAIRON 35, MAREEP 14, PORYGON 25, DUGTRIO 26). There is
//! no Chapter-long training: the chapter's land (the rift, the East Cave) is cave floor, where
//! wild Pokémon are met (Gen3's land rule) but the paths cross it only where they must (the
//! rift's two wild battles are part of the run). The silo's and the cave's trainers are the
//! training, with a heal at the Centre after the alley, before the silo's third grunt, mid-silo,
//! before the Silo Chief, after the silo and the cave, and before the watchers and Skycker. The
//! one [`Step::Train`] is on the silo basement's cave floor, before the Silo Chief
//! ([`CHEF_LEVEL`]). No hook. The run ends with the KADABRA at 50 and the TYPHLOSION at 52.

use super::{Run, Start, Step, goto, goto_avoiding};
use content::consts::*;
use engine::data::{Dir, ItemId, MapId, SpeciesId};
use engine::script::Ctx;

const SEA: MapId = MAP_ROUTE_S05;
const FAILLE: MapId = MAP_ROUTE_S05_FAILLE;
const TOWN: MapId = MAP_OWLYS;
const CENTER: MapId = MAP_OWLYS_POKEMON_CENTER;
const GROTTE: MapId = MAP_OWLYS_GROTTE_EST;
const GYM: MapId = MAP_OWLYS_GYM;
const SILO: MapId = MAP_OWLYS_ENTREPOT;
const SILO_B1F: MapId = MAP_OWLYS_ENTREPOT_B1F;
const ROUTE_S06: MapId = MAP_ROUTE_S06;

/// The main path's start: the rift's natural quay (9, 13), across the rift's water from Route
/// S5's entrance (9, 2), facing south, story 50 (Route S5's first wake, which [`ilot`] plays).
/// Temporary, until Surf (M10): the rift's water is the only way here.
pub fn start() -> Start {
    Start {
        map: FAILLE,
        x: 9,
        y: 13,
        facing: Dir::Down,
        setup: |ctx| Box::pin(end_of_chapter_4(ctx, 50)),
    }
}

/// The islet's start: Route S5's islet (12, 9), by the sailor, facing east, story 49 (the
/// route's first frame plays Route_S05_Sc01). The only Route S5 land the engine reaches
/// without Surf.
pub fn ilot() -> Start {
    Start {
        map: SEA,
        x: 12,
        y: 9,
        facing: Dir::Right,
        setup: |ctx| Box::pin(end_of_chapter_4(ctx, 49)),
    }
}

/// The start state's party: Chapter 4's end as its run (`sil5_ch4`) leaves it.
pub const PARTY: &[(SpeciesId, u8)] = &[
    (SPECIES_KADABRA, 41),
    (SPECIES_TYPHLOSION, 49),
    (SPECIES_LAIRON, 35),
    (SPECIES_MAREEP, 14),
    (SPECIES_PORYGON, 25),
    (SPECIES_DUGTRIO, 26),
];

/// Chapter 4's end state (`05-owlys.md`'s prerequisites), on top of Silice's new-game script:
/// Chapters 0–2 as `ch02`'s end (the Mallette, the running shoes, CYNDAQUIL's line, badges 1–2,
/// their modules and logs), Chapter 3's badge, Flash module and log (sync 30), Chapter 4's
/// badge, Surf module and log (sync 40), the story at `story`; the party (module docs).
async fn end_of_chapter_4(ctx: Ctx, story: u16) {
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
        FLAG_SIL_LOG_02,
        // Chapter 3.
        FLAG_SIL_BADGE_03,
        FLAG_SIL_MODULE_FLASH,
        FLAG_SIL_LOG_03,
        // Chapter 4.
        FLAG_SIL_BADGE_04,
        FLAG_SIL_MODULE_SURF,
        FLAG_SIL_LOG_04,
    ] {
        ctx.set_flag(f, true).await;
    }
    ctx.set_var(VAR_SIL_STORY, story).await;
    ctx.set_var(VAR_SIL_SYNC, 40).await;
    ctx.set_var(VAR_SIL_STARTER, 1).await;
    ctx.set_var(VAR_SIL_CH0_GATE, 1).await;
    ctx.item_add(ItemId(ITEM_SIL_MALLETTE), 1).await;
    for &(species, level) in PARTY {
        ctx.give_mon(species, level).await;
    }
}

/// The islet's steps: Route_S05_Sc01 (story 50), the sailor's Punch Bottle then his shop
/// (Route_S05_Sc02, the mart fallback), the fisherman's Super Rod.
pub fn ilot_steps() -> Vec<Step> {
    use Dir::*;
    vec![
        Step::Check("route_s05_sc01", |r| {
            assert_eq!(r.w.map(), SEA);
            assert_eq!(r.w.var(VAR_SIL_STORY), 50);
            assert!(r.has_text(Route_S05_Sc01_T1));
        }),
        Step::Interact(Right),
        Step::Check("route_s05_sc02", |r| {
            assert!(r.w.flag(FLAG_SIL_ROUTE_S05_PUNCH));
            assert!(r.count(ITEM_SIL_BOUTEILLE_PUNCH) >= 1);
            r.printed_in_order(&[Route_S05_Sc02_T1, Route_S05_Sc02_T2]);
        }),
        Step::Interact(Right),
        Step::Check("route_s05_sc02_shop", |r| {
            assert!(r.has_text(Route_S05_Sc02_T3));
            assert!(r.degraded.contains("mart"), "{:?}", r.degraded);
            assert_eq!(r.count(ITEM_SIL_BOUTEILLE_PUNCH), 1);
        }),
        goto(SEA, 9, 11),
        Step::Interact(Down),
        Step::Check("pecheur", |r| {
            assert!(r.w.flag(FLAG_SIL_ROUTE_S05_SUPER_CANNE));
            assert!(r.count(ITEM_SUPER_ROD) >= 1);
            assert_eq!(r.times(Route_S05_Npc_Pecheur_Ilot), 1);
        }),
        Step::Interact(Down),
        Step::Check("pecheur_again", |r| {
            assert_eq!(r.times(Route_S05_Npc_Pecheur_Ilot), 1);
            assert_eq!(r.count(ITEM_SUPER_ROD), 1);
            assert_eq!(r.w.events_dropped(), 0);
        }),
    ]
}

/// The Centre's nurse, from the town and back.
const HEAL: &[Step] = &[
    goto(TOWN, 15, 30),
    Step::Enter {
        dir: Dir::Up,
        map: CENTER,
    },
    goto(CENTER, 6, 4),
    Step::Interact(Dir::Up),
    Step::Check("centre", |r| {
        assert!(r.healed());
        assert!(r.has_text(gText_RestoredPkmnToFullHealth));
    }),
    goto(CENTER, 6, 6),
    Step::Enter {
        dir: Dir::Down,
        map: TOWN,
    },
];

/// The alley grunt's sight (5, 31) up, 4 cells: avoided until the step that meets him.
const RUELLE_SIGHT: &[(i16, i16)] = &[(5, 27), (5, 28), (5, 29), (5, 30)];

/// The silo guard's sight (46, 9) right, 2 cells: the silo is off the path.
const SILO_SIGHT: &[(i16, i16)] = &[(47, 9), (48, 9)];

/// What the town's paths avoid: the alley and the silo guard.
const TOWN_AVOID: &[(i16, i16)] = &[(5, 27), (5, 28), (5, 29), (5, 30), (47, 9), (48, 9)];

/// [`HEAL`] from the warehouse (1F) and back in past the beaten guard.
const SILO_HEAL: &[Step] = &[
    goto(SILO, 14, 18),
    Step::Enter {
        dir: Dir::Down,
        map: TOWN,
    },
    goto_avoiding(TOWN, 15, 30, RUELLE_SIGHT),
    Step::Enter {
        dir: Dir::Up,
        map: CENTER,
    },
    goto(CENTER, 6, 4),
    Step::Interact(Dir::Up),
    goto(CENTER, 6, 6),
    Step::Enter {
        dir: Dir::Down,
        map: TOWN,
    },
    goto_avoiding(TOWN, 47, 9, RUELLE_SIGHT),
    Step::Enter {
        dir: Dir::Up,
        map: SILO,
    },
];

/// [`SILO_HEAL`] from the basement (B1F) and back down.
const B1F_HEAL: &[Step] = &[
    goto(SILO_B1F, 17, 2),
    Step::Enter {
        dir: Dir::Up,
        map: SILO,
    },
    goto(SILO, 14, 18),
    Step::Enter {
        dir: Dir::Down,
        map: TOWN,
    },
    goto_avoiding(TOWN, 15, 30, RUELLE_SIGHT),
    Step::Enter {
        dir: Dir::Up,
        map: CENTER,
    },
    goto(CENTER, 6, 4),
    Step::Interact(Dir::Up),
    goto(CENTER, 6, 6),
    Step::Enter {
        dir: Dir::Down,
        map: TOWN,
    },
    goto_avoiding(TOWN, 47, 9, RUELLE_SIGHT),
    Step::Enter {
        dir: Dir::Up,
        map: SILO,
    },
    goto(SILO, 4, 2),
    Step::Enter {
        dir: Dir::Up,
        map: SILO_B1F,
    },
];

/// The lead's level before the Silo Chief, after [`Step::Train`] on the basement's floor.
/// Measured with Chapter 4's end party (`SPINE_LEVEL_48`): 47–49 win (Houndoom is the risk),
/// 46 loses, 50 runs past the frame limit.
const CHEF_LEVEL: u8 = 48;

/// Lilith's sight in the East Cave (25, 13) left, 4 cells: off the hall's path.
const LILITH_SIGHT: &[(i16, i16)] = &[(21, 13), (22, 13), (23, 13), (24, 13)];

/// The gym's watchers' sight lines (2 cells each, toward the hall's middle) and both trapdoors
/// (9, 16) and (18, 16): never planned through.
const GYM_AVOID: &[(i16, i16)] = &[
    (10, 14),
    (11, 14),
    (17, 14),
    (18, 14),
    (10, 19),
    (11, 19),
    (17, 19),
    (18, 19),
    (9, 16),
    (18, 16),
];

/// Out of the gym to the Centre and back in (the door stays open: `Owlys_OnLoad`).
const GYM_HEAL: &[Step] = &[
    goto(GYM, 14, 46),
    Step::Enter {
        dir: Dir::Down,
        map: TOWN,
    },
    goto(TOWN, 15, 30),
    Step::Enter {
        dir: Dir::Up,
        map: CENTER,
    },
    goto(CENTER, 6, 4),
    Step::Interact(Dir::Up),
    Step::Check("centre_gym", |r| assert!(r.healed())),
    goto(CENTER, 6, 6),
    Step::Enter {
        dir: Dir::Down,
        map: TOWN,
    },
    goto(TOWN, 30, 13),
    Step::Enter {
        dir: Dir::Up,
        map: GYM,
    },
];

/// Chapter 5's main steps, from [`start`].
pub fn steps() -> Vec<Step> {
    use Dir::*;
    let mut s = vec![
        Step::Check("ch5_start", |r| {
            assert_eq!(r.w.map(), FAILLE);
            assert_eq!(r.w.var(VAR_SIL_STORY), 50);
            assert!(r.w.flag(FLAG_SIL_BADGE_04));
            assert!(!r.w.flag(FLAG_SIL_BADGE_05));
            assert_eq!(r.w.party().len(), PARTY.len());
        }),
        // ---- the rift ----
        goto(FAILLE, 14, 13),
        Step::Interact(Right),
        Step::Check("faille_corde", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_ROUTE_S05_FAILLE_CORDE));
            assert!(r.count(ITEM_ESCAPE_ROPE) >= 1);
        }),
        goto(FAILLE, 5, 34),
        Step::Interact(Down),
        Step::Check("faille_gargouille", |r| {
            assert!(r.has_text(Route_S05_Faille_Sign));
        }),
        goto(FAILLE, 9, 34),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        // ---- Owlys ----
        Step::Check("owlys_sc01", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 51);
            assert!(r.has_text(Owlys_Sc01_T1));
        }),
    ];
    s.extend_from_slice(HEAL);
    s.extend_from_slice(&[
        Step::Check("respawn", |r| {
            assert!(r.has_text(Owlys_Centre_Infirmiere_Remarque));
        }),
        // Mathieu on the square (31, 25): the skewers, the eastern grunt, the deal.
        goto_avoiding(TOWN, 31, 26, TOWN_AVOID),
        Step::Interact(Up),
        Step::Check("owlys_sc02", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 52);
            assert!(r.w.flag(FLAG_SIL_OWLYS_MATHIEU_BROCHETTES));
            assert!(r.count(ITEM_SUPER_POTION) >= 3);
            r.printed_in_order(&[Owlys_Sc02_T1, Owlys_Sc02_T2, Owlys_Sc02_T3, Owlys_Sc02_T5]);
        }),
        // The alley (5, 31): the grunt sees the player, then Milo and TM46.
        goto_avoiding(TOWN, 5, 26, SILO_SIGHT),
        Step::Walk(Down),
        Step::Check("owlys_sc03", |r| {
            assert!(r.beaten(TRAINER_SIL_OWLYS_SBIRE_RUELLE));
            assert!(r.w.flag(FLAG_SIL_OWLYS_MILO));
            assert!(r.count(ITEM_TM46) >= 1);
            r.printed_in_order(&[
                Owlys_Sc03_T1,
                Owlys_Sc03_Defeat,
                Owlys_Sc03_T2,
                Owlys_Sc03_T3,
            ]);
        }),
    ]);
    s.extend_from_slice(HEAL);
    s.extend_from_slice(&[
        // ---- the silo (optional; its battles are the chapter's training) ----
        // The guard by the warehouse door (46, 9), then the three grunts of the aisles.
        goto_avoiding(TOWN, 47, 9, RUELLE_SIGHT),
        Step::Check("silo_garde", |r| {
            assert!(r.beaten(TRAINER_SIL_OWLYS_SILO_GARDE));
        }),
        Step::Enter { dir: Up, map: SILO },
        goto(SILO, 11, 11),
        goto(SILO, 20, 7),
        goto(SILO, 26, 7),
        Step::Interact(Right),
    ]);
    // The rift's wild battles (cave floor) leave the party lower than its trainers alone did:
    // a heal before the third grunt.
    s.extend_from_slice(SILO_HEAL);
    s.extend_from_slice(&[
        goto(SILO, 6, 3),
        Step::Check("silo_sbires", |r| {
            for t in [
                TRAINER_SIL_OWLYS_SILO_SBIRE_1,
                TRAINER_SIL_OWLYS_SILO_SBIRE_2,
                TRAINER_SIL_OWLYS_SILO_SBIRE_3,
            ] {
                assert!(r.beaten(t), "{t:?}");
            }
            assert!(r.w.flag(FLAG_SIL_ITEM_OWLYS_SILO_CORDE));
        }),
    ]);
    s.extend_from_slice(SILO_HEAL);
    s.extend_from_slice(&[
        goto(SILO, 4, 2),
        Step::Enter {
            dir: Up,
            map: SILO_B1F,
        },
        // B1F: the Calcium, the three Umbreon junctions (with the Iron, the Link Stone and the
        // Zinc on the way), the Silo Chief and the sacks (yes: emptied), the Sablenite.
        goto(SILO_B1F, 16, 3),
        Step::Interact(Left),
        goto(SILO_B1F, 17, 6),
        Step::Check("silo_noctali_1", |r| {
            assert!(r.w.flag(FLAG_SIL_OWLYS_SILO_NOCTALI_1));
            assert!(r.has_text(Owlys_Entrepot_Sc01_T1));
        }),
        goto(SILO_B1F, 31, 3),
        Step::Interact(Up),
        goto(SILO_B1F, 5, 14),
        Step::Check("silo_noctali_2", |r| {
            assert!(r.w.flag(FLAG_SIL_OWLYS_SILO_NOCTALI_2));
        }),
        goto(SILO_B1F, 14, 14),
        Step::Interact(Right),
        goto(SILO_B1F, 5, 22),
        Step::Check("silo_noctali_3", |r| {
            assert!(r.w.flag(FLAG_SIL_OWLYS_SILO_NOCTALI_3));
        }),
        // The Silo Chief's three (37-38) beat a TYPHLOSION 38 more often than not: training
        // on the basement's cave floor first, then a heal.
        Step::Train {
            a: (12, 22),
            b: (13, 22),
            level: CHEF_LEVEL,
            heal: B1F_HEAL,
        },
        Step::Check("silo_trained", |r| {
            assert!(r.w.party().get(0).unwrap().level >= CHEF_LEVEL);
        }),
    ]);
    s.extend_from_slice(B1F_HEAL);
    s.extend_from_slice(&[
        goto(SILO_B1F, 9, 23),
        Step::Interact(Down),
        goto(SILO_B1F, 27, 24),
        Step::Check("silo_sc02", |r| {
            assert!(r.beaten(TRAINER_SIL_OWLYS_CHEF_SILO));
            assert!(r.w.flag(FLAG_SIL_OWLYS_SILO_DECIDE));
            assert!(r.w.flag(FLAG_SIL_OWLYS_SILO_VIDE));
            r.printed_in_order(&[
                Owlys_Entrepot_Sc02_T1,
                Owlys_Entrepot_Sc02_Defeat,
                Owlys_Entrepot_Sc02_T2,
                Owlys_Entrepot_Sc02_T5,
                Owlys_Entrepot_Sc02_T6,
            ]);
        }),
        goto(SILO_B1F, 32, 27),
        Step::Interact(Right),
        Step::Check("silo_hidden", |r| {
            for f in [
                FLAG_SIL_ITEM_OWLYS_SILO_FER,
                FLAG_SIL_ITEM_OWLYS_SILO_ZINC,
                FLAG_SIL_ITEM_OWLYS_SILO_CALCIUM,
                FLAG_SIL_ITEM_OWLYS_SILO_PIERRE_LIEN,
                FLAG_SIL_ITEM_OWLYS_SILO_SABLENITE,
            ] {
                assert!(r.w.flag(f), "{f:?}");
            }
            assert!(r.count(ITEM_SIL_SABLENITE) >= 1);
        }),
        goto(SILO_B1F, 17, 2),
        Step::Enter { dir: Up, map: SILO },
        goto(SILO, 14, 18),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
    ]);
    s.extend_from_slice(HEAL);
    s.extend_from_slice(&[
        // The gym door is shut before the cave.
        goto_avoiding(TOWN, 30, 13, TOWN_AVOID),
        Step::Interact(Up),
        Step::Check("owlys_sc04_ferme", |r| {
            assert!(r.has_text(Owlys_Sc04_Ferme));
            assert!(!r.w.flag(FLAG_SIL_OWLYS_PORTE_GYM));
        }),
        // ---- the East Cave ----
        goto_avoiding(TOWN, 54, 29, TOWN_AVOID),
        Step::Enter {
            dir: Up,
            map: GROTTE,
        },
        goto_avoiding(GROTTE, 6, 19, LILITH_SIGHT),
        Step::Check("grotte_sc01", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 53);
            assert!(r.beaten(TRAINER_SIL_OWLYS_CONVOYEUR_A));
            assert!(r.beaten(TRAINER_SIL_OWLYS_CONVOYEUR_B));
            assert!(r.w.flag(FLAG_SIL_OWLYS_PLUME));
            assert!(r.count(ITEM_SIL_PLUME_OMBREE) >= 1);
            assert!(r.w.flag(FLAG_SIL_HIDE_OWLYS_GROTTE_EST_MATHIEU));
            r.printed_in_order(&[
                Owlys_Grotte_Est_Sc01_T1,
                Owlys_Grotte_Est_Sc01_T3,
                Owlys_Grotte_Est_Sc01_DefeatA,
                Owlys_Grotte_Est_Sc01_Repli,
                Owlys_Grotte_Est_Sc01_DefeatB,
                Owlys_Grotte_Est_Sc01_T4,
                Owlys_Grotte_Est_Sc01_T8,
                Owlys_Grotte_Est_Sc01_T10,
            ]);
        }),
        // Lilith by sight, the Moon Stone above her cell, the cart's Ultra Ball.
        goto(GROTTE, 21, 13),
        Step::Interact(Up),
        Step::Check("grotte_lilith", |r| {
            assert!(r.beaten(TRAINER_SIL_OWLYS_GROTTE_MYSTI));
            assert!(r.w.flag(FLAG_SIL_ITEM_OWLYS_GROTTE_PIERRE_LUNE));
        }),
        goto(GROTTE, 36, 21),
        Step::Interact(Right),
        Step::Check("grotte_hidden", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_OWLYS_GROTTE_HYPER_BALL));
        }),
        goto_avoiding(GROTTE, 6, 21, LILITH_SIGHT),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
    ]);
    s.extend_from_slice(HEAL);
    s.extend_from_slice(&[
        // ---- the gym ----
        goto_avoiding(TOWN, 30, 13, TOWN_AVOID),
        Step::Interact(Up),
        Step::Check("owlys_sc04", |r| {
            assert!(r.w.flag(FLAG_SIL_OWLYS_PORTE_GYM));
            assert!(r.has_text(Owlys_Sc04_T1));
        }),
        Step::Enter { dir: Up, map: GYM },
        // The blue lectern (3, 30), then the black one (24, 30) and the Sharp Beak.
        goto_avoiding(GYM, 3, 31, GYM_AVOID),
        Step::Interact(Up),
        goto_avoiding(GYM, 24, 31, GYM_AVOID),
        Step::Interact(Up),
        goto_avoiding(GYM, 26, 32, GYM_AVOID),
        Step::Interact(Down),
        Step::Check("gym_beak", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_OWLYS_GYM_BEC_POINTU));
            assert!(r.count(ITEM_SHARP_BEAK) >= 1);
        }),
        // The court: every bridge down.
        goto_avoiding(GYM, 14, 24, GYM_AVOID),
        Step::Check("gym_cour", |r| {
            assert!(r.w.flag(FLAG_SIL_OWLYS_GYM_COUR_OK));
        }),
    ]);
    s.extend_from_slice(GYM_HEAL);
    s.extend_from_slice(&[
        // The four watchers, each from its side; the last one lowers the walkway.
        goto_avoiding(GYM, 9, 15, GYM_AVOID),
        Step::Interact(Up),
        goto_avoiding(GYM, 9, 18, GYM_AVOID),
        Step::Interact(Down),
    ]);
    s.extend_from_slice(GYM_HEAL);
    s.extend_from_slice(&[
        goto_avoiding(GYM, 19, 15, GYM_AVOID),
        Step::Interact(Up),
        goto_avoiding(GYM, 19, 18, GYM_AVOID),
        Step::Interact(Down),
        Step::Check("gym_watchers", |r| {
            for t in [
                TRAINER_SIL_OWLYS_GYM_ARIANE,
                TRAINER_SIL_OWLYS_GYM_MORGANE,
                TRAINER_SIL_OWLYS_GYM_CORVUS,
                TRAINER_SIL_OWLYS_GYM_NYX,
            ] {
                assert!(r.beaten(t), "{t:?}");
            }
            assert!(r.has_text(Owlys_Gym_Passerelle));
        }),
    ]);
    s.extend_from_slice(GYM_HEAL);
    s.extend_from_slice(&[
        // Skycker (14, 3).
        goto_avoiding(GYM, 14, 4, GYM_AVOID),
        Step::Interact(Up),
        Step::Check("gym_sc01", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 54);
            assert!(r.beaten(TRAINER_SIL_SKYCKER_1));
            assert!(r.w.flag(FLAG_SIL_BADGE_05));
            assert!(r.w.flag(FLAG_SIL_MODULE_VOL));
            assert!(r.w.flag(FLAG_SIL_LOG_05));
            assert_eq!(r.w.var(VAR_SIL_SYNC), 50);
            assert!(r.count(ITEM_TM12) >= 1);
            r.printed_in_order(&[
                Owlys_Gym_Sc01_T1,
                Owlys_Gym_Sc01_Defeat,
                Owlys_Gym_Sc01_T2,
                Owlys_Gym_Sc01_Badge,
                Owlys_Gym_Sc01_T3,
                Owlys_Gym_Sc01_Module,
                Owlys_Gym_Sc01_T4,
                Owlys_Gym_Sc01_T5,
                Owlys_Gym_Sc01_T7,
                Owlys_Gym_Sc01_Sync,
                Owlys_Gym_Log05,
            ]);
        }),
        Step::Interact(Up),
        Step::Check("skycker_apres", |r| {
            assert!(r.has_text(Owlys_Gym_Skycker_Apres));
        }),
        // ---- out: the city at night, Mathieu, the lift ----
        goto_avoiding(GYM, 14, 46, GYM_AVOID),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        Step::Check("owlys_sc05", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 55);
            assert!(r.has_text(Owlys_Sc05_T1));
        }),
        goto_avoiding(TOWN, 28, 30, TOWN_AVOID),
        Step::Interact(Up),
        Step::Check("mathieu_apres", |r| {
            assert!(r.has_text(Owlys_Npc_Mathieu_Apres));
        }),
        goto_avoiding(TOWN, 30, 4, TOWN_AVOID),
        Step::Enter {
            dir: Up,
            map: ROUTE_S06,
        },
    ]);
    s
}

/// What the end of Chapter 5 asserts beyond its checkpoints: the story, the badge and module,
/// the bag, no dropped event, and every Chapter 5 fallback the main path meets reported (the
/// mart's is the islet's: [`ilot_steps`]).
pub fn check_end(r: &Run) {
    assert_eq!(r.w.map(), ROUTE_S06);
    // Chapter 5 leaves the story at 55 as the player comes up the elevator; Route S6's
    // on-frame entry (Chapter 6's `RouteS06_Sc01`, the signal moves) plays on arrival and sets 60.
    assert_eq!(r.w.var(VAR_SIL_STORY), 60);
    assert!(r.has_text(Route_S06_Sc01_T1));
    assert!(r.w.flag(FLAG_SIL_BADGE_05));
    assert!(r.w.flag(FLAG_SIL_MODULE_VOL));
    assert!(r.w.flag(FLAG_SIL_LOG_05));
    assert_eq!(r.w.var(VAR_SIL_SYNC), 50);
    for (item, n) in [
        (ITEM_ESCAPE_ROPE, 1),
        (ITEM_SUPER_POTION, 3),
        (ITEM_TM46, 1),
        (ITEM_SIL_PLUME_OMBREE, 1),
        (ITEM_SHARP_BEAK, 1),
        (ITEM_TM12, 1),
        (ITEM_SIL_SABLENITE, 1),
        (ITEM_MOON_STONE, 1),
        (ITEM_ULTRA_BALL, 1),
    ] {
        assert!(r.count(item) >= n, "item {item}");
    }
    assert_eq!(r.w.events_dropped(), 0);
    for name in [
        "battle_multi",
        "play_fanfare",
        "show_sync",
        "moving light",
        "hidden item",
    ] {
        assert!(r.degraded.contains(name), "{name}: {:?}", r.degraded);
    }
}
