//! Chapter 0, the Prologue, from a new game to `VAR_SIL_STORY` 9 at the Bois-Éther edge
//! (Route S1's north edge, (10, 2)): the bar (Bar_Sc01 to Sc04, with the exit guard while the
//! Mallette is left behind), the night chase, the house (Player_House_Sc01 to Sc03), the morning
//! (Brico_Bourg_Sc02 and Sc03: the starter, Cyndaquil, and the gifts), the hidden Potion, the
//! Labo's brambles (their sign; [`labo_brambles_probe`] cuts them with a hook, in the Chapter 0
//! test only), the north exit (Sc04), then Route S1: the first wild battle, the walker's
//! berries, training in the tall grass ([`TRAINER_LEVEL`]), a trainer by sight, the items.

use super::{Hook, Run, Step, goto, goto_avoiding};
use content::consts::*;
use engine::data::Dir;

/// The sight lines of the Route S1 trainers (§5), left out of the plans.
pub const ROUTE_S01_SIGHT: &[(i16, i16)] = &[
    (4, 24),
    (5, 24),
    (6, 24),
    (12, 19),
    (12, 20),
    (12, 21),
    (4, 12),
    (5, 12),
    (6, 12),
    (8, 4),
    (9, 4),
    (10, 4),
    (11, 4),
];

/// The starter's level before Route S1's trainers (levels 3–7, §5), after [`Step::Train`].
pub const TRAINER_LEVEL: u8 = 8;

/// Two tall-grass cells of Route S1's south meadow (x 8–13, y 26–30), out of every sight line.
const GRASS: ((i16, i16), (i16, i16)) = ((10, 28), (10, 29));

/// Maman's heal (Player_House: from the morning on), from Route S1 and back by its south edge.
const HEAL: &[Step] = &[
    goto_avoiding(ROUTE, 8, 39, ROUTE_S01_SIGHT),
    Step::Enter {
        dir: Dir::Down,
        map: TOWN,
    },
    goto(TOWN, 5, 15),
    Step::Enter {
        dir: Dir::Up,
        map: HOUSE_1F,
    },
    goto(HOUSE_1F, 3, 2),
    Step::Interact(Dir::Left),
    goto(HOUSE_1F, 4, 7),
    Step::Enter {
        dir: Dir::Down,
        map: TOWN,
    },
    goto(TOWN, 11, 0),
    Step::Enter {
        dir: Dir::Up,
        map: ROUTE,
    },
];

const BAR: engine::data::MapId = MAP_BRICO_BOURG_BAR;
const TOWN: engine::data::MapId = MAP_BRICO_BOURG;
const HOUSE_1F: engine::data::MapId = MAP_BRICO_BOURG_PLAYER_HOUSE_1F;
const HOUSE_2F: engine::data::MapId = MAP_BRICO_BOURG_PLAYER_HOUSE_2F;
const ROUTE: engine::data::MapId = MAP_ROUTE_S01;

/// Chapter 0's spine.
pub fn steps() -> Vec<Step> {
    spine(false)
}

/// Chapter 0's spine with its probe ([`labo_brambles_probe`], a test hook), for the Chapter 0
/// test only: the chained runs (`sil2_ch1.rs`) play [`steps`], so that Chapter 1 cuts the
/// brambles itself.
pub fn steps_with_probes() -> Vec<Step> {
    spine(true)
}

fn spine(probes: bool) -> Vec<Step> {
    use Dir::*;
    let mut v = vec![
        // ---- the Shaker, the evening (story 0) ----
        Step::Check("bar_sc01", |r| {
            assert_eq!(r.w.map(), BAR);
            assert!(r.w.flag(FLAG_SIL_INTRO_DONE));
            assert_eq!(r.w.var(VAR_SIL_STORY), 0);
            r.printed_in_order(&[Bar_Sc01_T1, Bar_Sc01_T2, Bar_Sc01_T3, Bar_Sc01_T4]);
        }),
        // A friend's line (Bar_Sc02): GWADA at (6, 4), from (6, 5).
        goto(BAR, 6, 5),
        Step::Interact(Up),
        Step::Check("bar_sc02", |r| assert!(r.has_text(Bar_Sc02_T2))),
        // Julien (Bar_Sc03): the explosion, story 1, the Mallette left on his table.
        goto(BAR, 12, 4),
        Step::Interact(Up),
        Step::Check("bar_sc03", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 1);
            assert!(r.w.flag(FLAG_SIL_HIDE_BAR_JULIEN));
            assert!(r.w.flag(FLAG_SIL_HIDE_BAR_SBIRE_A));
            assert!(r.w.flag(FLAG_SIL_HIDE_BAR_SBIRE_B));
            assert!(!r.w.flag(FLAG_SIL_HIDE_BAR_MALLETTE));
            r.printed_in_order(&[
                Bar_Sc03_T1,
                Bar_Sc03_T2,
                Bar_Sc03_T3,
                Bar_Sc03_T4,
                Bar_Sc03_T5,
                Bar_Sc03_T6,
            ]);
        }),
        // Out without the Mallette: Mathieu calls the player back to the Shaker's door.
        goto(BAR, 7, 9),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        Step::Check("shaker_door", |r| assert_eq!(r.pos(), (14, 14))),
        Step::Walk(Down),
        Step::Check("mallette_guard", |r| {
            assert_eq!(r.pos(), (14, 14));
            assert!(r.has_text(Bar_Sc04_T2));
            assert!(!r.w.flag(FLAG_SIL_BRICO_CHASE));
        }),
        Step::Enter { dir: Up, map: BAR },
        // The Mallette (Bar_Sc04), yes.
        goto(BAR, 13, 4),
        Step::Interact(Up),
        Step::Check("bar_sc04", |r| {
            assert!(r.w.flag(FLAG_SIL_HAS_MALLETTE));
            assert!(r.w.flag(FLAG_SIL_HIDE_BAR_MALLETTE));
            assert_eq!(r.count(ITEM_SIL_MALLETTE), 1);
            r.printed_in_order(&[Bar_Sc04_T1, Bar_Sc04_T3, Bar_Sc04_T4, Bar_Sc04_T5]);
        }),
        // Taken once: the object is gone and the scene does not run again.
        Step::Interact(Up),
        Step::Check("bar_sc04_once", |r| {
            assert_eq!(r.times(Bar_Sc04_T1), 1);
            assert_eq!(r.count(ITEM_SIL_MALLETTE), 1);
        }),
        goto(BAR, 7, 9),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        // ---- the night chase (Brico_Bourg_Sc01) ----
        goto(TOWN, 14, 15),
        Step::Check("brico_sc01", |r| {
            assert!(r.w.flag(FLAG_SIL_BRICO_CHASE));
            assert!(r.w.flag(FLAG_SIL_HIDE_BRICO_SBIRE));
            r.printed_in_order(&[
                Brico_Bourg_Sc01_T1,
                Brico_Bourg_Sc01_T2,
                Brico_Bourg_Sc01_T3,
            ]);
        }),
        // The north exit is guarded before the starter.
        goto(TOWN, 11, 3),
        Step::Walk(Up),
        Step::Check("north_guard", |r| {
            assert!(r.has_text(Brico_Bourg_Block_T1));
            assert_eq!(r.pos(), (11, 3));
        }),
        // ---- home (Player_House_Sc01 to Sc03) ----
        goto(TOWN, 5, 15),
        Step::Enter {
            dir: Up,
            map: HOUSE_1F,
        },
        Step::Check("house_sc01", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 2);
            assert!(r.has_text(Player_House_Sc01_T1));
        }),
        goto(HOUSE_1F, 8, 2),
        Step::Enter {
            dir: Up,
            map: HOUSE_2F,
        },
        // The bed first: not yet.
        goto(HOUSE_2F, 2, 2),
        Step::Interact(Left),
        Step::Check("bed_too_early", |r| {
            assert!(r.has_text(Player_House_Sc02_T3));
            assert_eq!(r.w.var(VAR_SIL_STORY), 2);
        }),
        // The desk, then the bed.
        goto(HOUSE_2F, 5, 3),
        Step::Interact(Up),
        Step::Check("desk", |r| assert!(r.w.flag(FLAG_SIL_MALLETTE_TRIED))),
        goto(HOUSE_2F, 2, 2),
        Step::Interact(Left),
        Step::Check("house_sc02", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 3);
            r.printed_in_order(&[
                Player_House_Sc02_T1,
                Player_House_Sc02_T4,
                Player_House_Sc02_T5,
                Player_House_Sc03_T1,
            ]);
        }),
        goto(HOUSE_2F, 7, 2),
        Step::Enter {
            dir: Up,
            map: HOUSE_1F,
        },
        // Maman: the running shoes, once.
        goto(HOUSE_1F, 3, 2),
        Step::Interact(Left),
        Step::Check("running_shoes", |r| {
            assert!(r.w.flag(FLAG_SIL_RUNNING_SHOES));
            r.printed_in_order(&[Player_House_Sc03_T2, Player_House_Sc03_T3]);
        }),
        // ---- the morning: click, the call, the starter (Brico_Bourg_Sc02, Sc03) ----
        // CYNDAQUIL, the middle ball: Chapter 1's leader is Grass/Poison (Ember from level 12).
        Step::Choose(1),
        goto(HOUSE_1F, 4, 7),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        Step::Check("brico_sc03", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 5);
            assert_eq!(r.w.var(VAR_SIL_CH0_GATE), 1);
            for f in [
                FLAG_SIL_MALLETTE_OPEN,
                FLAG_SIL_MALLETTE_MAP,
                FLAG_SIL_MALLETTE_DEX,
                FLAG_SIL_MALLETTE_CALLS,
                FLAG_SIL_ITEM_PROLOGUE_POTIONS,
                FLAG_SIL_ITEM_PROLOGUE_BALLS,
                FLAG_SIL_HIDE_BRICO_MATHIEU,
                FLAG_SIL_HIDE_BRICO_GWADA,
                FLAG_SIL_HIDE_BRICO_ADRIEN,
            ] {
                assert!(r.w.flag(f), "{f:?}");
            }
            let starter = r.w.var(VAR_SIL_STARTER);
            assert_eq!(starter, 1);
            assert_eq!(r.w.party().len(), 1);
            assert_eq!(
                r.w.party().get(0).unwrap().species,
                SIL_STARTERS[usize::from(starter)]
            );
            assert_eq!(r.count(ITEM_POTION), 2);
            assert_eq!(r.count(ITEM_POKE_BALL), 5);
            r.printed_in_order(&[
                Brico_Bourg_Sc02_T1,
                Brico_Bourg_Sc02_T2,
                Brico_Bourg_Sc02_T3,
                Brico_Bourg_Sc02_T4,
                Brico_Bourg_Sc03_T1,
                Brico_Bourg_Sc03_T2,
                Brico_Bourg_Sc03_T3,
                Brico_Bourg_Sc03_T4,
                Brico_Bourg_Sc03_T5,
                Brico_Bourg_Sc03_T6,
                Brico_Bourg_Sc03_T7,
                Brico_Bourg_Sc03_T8,
                Brico_Bourg_Sc03_T9,
                Brico_Bourg_Sc03_T10,
                Brico_Bourg_Sc03_T11,
                Brico_Bourg_Sc03_T12,
                Brico_Bourg_Sc03_T13,
            ]);
        }),
        // The hidden Potion behind the fountain, once.
        goto(TOWN, 10, 7),
        Step::Interact(Down),
        Step::Check("hidden_potion", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_BRICO_POTION));
            assert_eq!(r.count(ITEM_POTION), 3);
        }),
        Step::Interact(Down),
        Step::Check("hidden_potion_once", |r| {
            assert_eq!(r.count(ITEM_POTION), 3)
        }),
        // The Labo's brambles (4, 9): Ch0's sign (Chapter 1 cuts them).
        goto(TOWN, 4, 10),
        Step::Interact(Up),
        Step::Check("ronces_sign", |r| {
            assert!(r.has_text(Brico_Bourg_Sign_Ronces));
            assert!(!r.w.flag(FLAG_SIL_BRICO_LABO_RONCES_CUT));
        }),
    ];
    if probes {
        v.extend(labo_brambles_probe());
    }
    v.extend([
        // The north exit: the signal (Brico_Bourg_Sc04), then Route S1.
        goto(TOWN, 11, 1),
        Step::Check("brico_sc04", |r| {
            assert!(r.w.flag(FLAG_SIL_S01_SIGNAL_SEEN));
            assert!(r.has_text(Brico_Bourg_Sc04_T1));
        }),
        Step::Enter {
            dir: Up,
            map: ROUTE,
        },
        // ---- Route S1 ----
        // The first wild battle (Route_S01_Sc01).
        goto_avoiding(ROUTE, 9, 30, ROUTE_S01_SIGHT),
        Step::Check("route_sc01", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 6);
            assert!(!r.battles.is_empty());
            r.printed_in_order(&[Route_S01_Sc01_T1, Route_S01_Sc01_T2]);
        }),
        // The walker's Oran Berries, after it, once.
        goto_avoiding(ROUTE, 7, 32, ROUTE_S01_SIGHT),
        Step::Interact(Left),
        Step::Check("oran", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_ROUTE_S01_ORAN));
            assert_eq!(r.count(ITEM_ORAN_BERRY), 2);
            assert!(r.has_text(Route_S01_Npc_Promeneuse_Oran));
        }),
        Step::Interact(Left),
        Step::Check("oran_once", |r| assert_eq!(r.count(ITEM_ORAN_BERRY), 2)),
        // Training in the south meadow (the wild levels 2–6 of §6), healed by Maman.
        Step::Train {
            a: GRASS.0,
            b: GRASS.1,
            level: TRAINER_LEVEL,
            heal: HEAL,
        },
        Step::Check("trained", |r| {
            assert!(r.w.party().get(0).unwrap().level >= TRAINER_LEVEL)
        }),
        // Home to heal before the trainers (Maman, from the morning on), back by the south edge.
        goto_avoiding(ROUTE, 8, 39, ROUTE_S01_SIGHT),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        goto(TOWN, 5, 15),
        Step::Enter {
            dir: Up,
            map: HOUSE_1F,
        },
        goto(HOUSE_1F, 3, 2),
        Step::Interact(Left),
        Step::Check("maman_heals", |r| assert!(r.healed())),
        goto(HOUSE_1F, 4, 7),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        goto(TOWN, 11, 0),
        Step::Enter {
            dir: Up,
            map: ROUTE,
        },
        // A trainer by sight: TÉO sees the player on (6, 24).
        Step::Check("teo_unbeaten", |r| {
            assert!(!r.beaten(TRAINER_SIL_GAMIN_TEO))
        }),
        goto_avoiding(ROUTE, 6, 25, ROUTE_S01_SIGHT),
        goto(ROUTE, 6, 24),
        Step::Check("teo_by_sight", |r| {
            assert!(r.beaten(TRAINER_SIL_GAMIN_TEO));
            assert!(r.has_text(Route_S01_Teo_Intro));
        }),
        // The hidden Antidote, then the item balls.
        goto_avoiding(ROUTE, 5, 22, ROUTE_S01_SIGHT),
        Step::Interact(Left),
        Step::Check("antidote", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_ROUTE_S01_ANTIDOTE));
            assert_eq!(r.count(ITEM_ANTIDOTE), 1);
        }),
        goto_avoiding(ROUTE, 14, 23, ROUTE_S01_SIGHT),
        Step::Interact(Up),
        Step::Check("route_potion", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_ROUTE_S01_POTION));
            assert_eq!(r.count(ITEM_POTION), 4);
        }),
        goto_avoiding(ROUTE, 14, 7, ROUTE_S01_SIGHT),
        Step::Interact(Up),
        Step::Check("route_ball", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_ROUTE_S01_POKE_BALL));
            assert_eq!(r.count(ITEM_POKE_BALL), 6);
        }),
        // In sight of Bois-Éther (Route_S01_Sc02): story 9.
        goto_avoiding(ROUTE, 10, 2, ROUTE_S01_SIGHT),
        Step::Check("route_sc02", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 9);
            assert!(r.has_text(Route_S01_Sc02_T1));
        }),
    ]);
    v
}

/// The Labo's brambles cut early: with Chapter 1's Lame-Frequence granted for one try (a hook),
/// the generic obstacle cuts them, and they stay cut after a map load (`BricoBourg_OnLoad`). From
/// and back to (4, 10), in Brico-Bourg, below them.
pub fn labo_brambles_probe() -> Vec<Step> {
    use Dir::*;
    vec![
        Step::Hook(Hook::FlagDuring(
            "BricoBourg_EventScript_Ronces",
            FLAG_SIL_MODULE_COUPE,
        )),
        Step::Interact(Up),
        Step::Check("ronces_cut", |r| {
            assert!(r.w.flag(FLAG_SIL_BRICO_LABO_RONCES_CUT));
            assert!(!r.w.flag(FLAG_SIL_MODULE_COUPE));
            r.printed_in_order(&[Bois_Ether_Sc08_T2, Bois_Ether_Sc08_T3]);
        }),
        Step::Walk(Up),
        Step::Check("ronces_walkable", |r| assert_eq!(r.pos(), (4, 9))),
        goto(TOWN, 5, 15),
        Step::Enter {
            dir: Up,
            map: HOUSE_1F,
        },
        goto(HOUSE_1F, 4, 7),
        Step::Enter {
            dir: Down,
            map: TOWN,
        },
        goto(TOWN, 4, 10),
        Step::Walk(Up),
        Step::Check("ronces_still_cut", |r| assert_eq!(r.pos(), (4, 9))),
        Step::Walk(Down),
    ]
}

/// What the end of Chapter 0 asserts beyond its checkpoints: no dropped event, and every
/// fallback reported.
pub fn check_end(r: &Run) {
    assert_eq!(r.w.events_dropped(), 0);
    for name in [
        "play_se",
        "mallette_call",
        "end_call",
        "set_time_tint",
        "hidden item",
    ] {
        assert!(r.degraded.contains(name), "{name}: {:?}", r.degraded);
    }
}
