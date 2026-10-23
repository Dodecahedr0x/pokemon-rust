//! The goal lists, per act, in story order (Task B, decision 15; Milestone 8 Task K).
//!
//! A goal is a predicate over a node's end state only (Milestone 8: no expansion summary), so
//! a node's goals are a function of its state and its parent's progress, as its key is.
//! [`progress`] only advances over consecutive goals, so it is monotone along a lineage.
//!
//! Every constant is a `content::consts` name, checked against the decomp's headers by
//! `tests/m8_k_goals.rs`.

use engine::checkpoint::WorldSnapshot;
use engine::data::{FlagId, MapId, VarId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy)]
pub struct Goal {
    pub name: &'static str,
    pub reached: fn(&WorldSnapshot) -> bool,
}

/// Which goal list a db explores (`meta.act`, `Config::act`: the build's, which the spine's `Expect`s check).
/// The acts are per flavour ([`Act::of`]): a build explores only its own ([`Act::in_build`]).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Act {
    /// The Milestone 7 slice: from the new game to May beaten on Route 103 (7 goals).
    Slice,
    /// Act 1: the slice, then Petalburg, Wally's tutorial, Petalburg Woods and Rustboro, to
    /// Roxanne's badge (15 goals; decision Q1). The default act on Emerald and the stub.
    Act1,
    /// Silice's Prologue (SIL1): from the new game to `VAR_SIL_STORY` 9 at the Bois-Éther edge.
    /// 10 goals (the spine's `Expect`s). The default act on Silice.
    SilCh0,
}

/// The build's default act: Act 1 on Emerald and the stub, the Prologue on Silice.
impl Default for Act {
    fn default() -> Act {
        match content::FLAVOR {
            content::Flavor::Stub | content::Flavor::Emerald => Act::Act1,
            content::Flavor::Silice => Act::SilCh0,
        }
    }
}

impl Act {
    pub const ALL: [Act; 3] = [Act::Slice, Act::Act1, Act::SilCh0];
    /// The Emerald acts (and the stub's, which shares one list).
    pub const EMERALD: [Act; 2] = [Act::Slice, Act::Act1];
    /// The Silice acts (`sil_ch1` and onwards come with their chapters).
    pub const SILICE: [Act; 1] = [Act::SilCh0];

    /// `slice`, `act1` or `sil_ch0` (the CLI's and `meta.act`'s spelling).
    pub fn name(self) -> &'static str {
        match self {
            Act::Slice => "slice",
            Act::Act1 => "act1",
            Act::SilCh0 => "sil_ch0",
        }
    }

    /// Any act by name, whatever the build ([`Act::in_build`] says whether this build has it).
    pub fn parse(s: &str) -> Option<Act> {
        Act::ALL.into_iter().find(|a| a.name() == s)
    }

    /// The acts of `flavor`: [`Act::EMERALD`] for Emerald and the stub (which shares one goal
    /// list for both), [`Act::SILICE`] for Silice.
    pub const fn of(flavor: content::Flavor) -> &'static [Act] {
        match flavor {
            content::Flavor::Stub | content::Flavor::Emerald => &Act::EMERALD,
            content::Flavor::Silice => &Act::SILICE,
        }
    }

    /// Whether this build ([`content::FLAVOR`]) has the act: an act of another flavour has no
    /// goals here, and the CLI and a db's `meta.act` refuse it.
    pub fn in_build(self) -> bool {
        Act::of(content::FLAVOR).contains(&self)
    }

    /// [`Act::parse`] limited to this build's acts, with the error the CLI prints.
    pub fn parse_in_build(s: &str) -> Result<Act, String> {
        let names = |acts: &[Act]| acts.iter().map(|a| a.name()).collect::<Vec<_>>().join(", ");
        let own = names(Act::of(content::FLAVOR));
        match Act::parse(s) {
            Some(a) if a.in_build() => Ok(a),
            Some(a) => Err(format!(
                "act {:?} is not an act of this build (the {} content, acts: {own}); build \
                 with {} to explore it",
                a.name(),
                content::FLAVOR.name(),
                if Act::SILICE.contains(&a) {
                    "PKR_GAME=silice (`--game silice`)"
                } else {
                    "PKR_GAME=emerald (`--game emerald`)"
                }
            )),
            None => Err(format!("unknown act {s:?} (this build's acts: {own})")),
        }
    }
}

/// Whether flag `f` is set.
pub fn flag(snap: &WorldSnapshot, f: FlagId) -> bool {
    snap.vars.flags.binary_search(&f).is_ok()
}

/// Var `v` (0 when unset).
pub fn var(snap: &WorldSnapshot, v: VarId) -> u16 {
    snap.vars
        .vars
        .binary_search_by_key(&v, |&(v, _)| v)
        .map_or(0, |i| snap.vars.vars[i].1)
}

/// Whether the player is on `map`.
pub fn on(snap: &WorldSnapshot, map: MapId) -> bool {
    snap.field.map == map
}

/// The player's cell.
fn pos(snap: &WorldSnapshot) -> (i16, i16) {
    snap.objects.objs.first().map_or((0, 0), |p| p.pos)
}

#[cfg(not(pkr_silice))]
mod emerald {
    use super::{Goal, flag, on, var};
    use content::consts::*;
    use content::prelude::trainer_flag;
    use engine::data::TrainerId;
    use std::sync::OnceLock;

    /// The slice, in story order from the `BeforeRescue` start (decision 15). Act 1 begins
    /// with these seven.
    pub static SLICE: [Goal; 7] = [
        Goal {
            name: "rescue_started",
            // `Route101_OnFrame` sets it to 1 on the first frame on Route 101 (the rescue's
            // trigger is armed); `Route101_EventScript_StartBirchRescue` sets 2 after the
            // chase, `..._BirchsBag` 3. So 1+ is "on Route 101 once", M7's `entered` goal.
            reached: |s| var(s, VAR_ROUTE101_STATE) >= 1,
        },
        Goal {
            name: "starter",
            reached: |s| !s.party.is_empty(),
        },
        Goal {
            name: "birch_rescued",
            reached: |s| flag(s, FLAG_RESCUED_BIRCH),
        },
        Goal {
            name: "lab",
            // Birch has given the starter in his lab (`VAR_BIRCH_LAB_STATE` 3+, set there).
            reached: |s| var(s, VAR_BIRCH_LAB_STATE) >= 3,
        },
        Goal {
            name: "oldale",
            // `OldaleTown_OnTransition` sets it on entering: a state that only ran through the
            // town (or is already in its Pokémon Center) still counts.
            reached: |s| flag(s, FLAG_VISITED_OLDALE_TOWN),
        },
        Goal {
            name: "route103",
            reached: |s| on(s, MAP_ROUTE103),
        },
        Goal {
            name: "may_defeated",
            reached: |s| flag(s, FLAG_DEFEATED_RIVAL_ROUTE103),
        },
    ];

    /// Act 1 (decision Q1): the slice's seven, then to Roxanne's badge.
    pub static ACT1: [Goal; 15] = [
        SLICE[0],
        SLICE[1],
        SLICE[2],
        SLICE[3],
        SLICE[4],
        SLICE[5],
        SLICE[6],
        Goal {
            name: "pokedex",
            // `LittlerootTown_ProfessorBirchsLab_EventScript_ReceivePokedex`.
            reached: |s| flag(s, FLAG_RECEIVED_POKEDEX_FROM_BIRCH),
        },
        Goal {
            name: "petalburg",
            // `PetalburgCity_OnTransition`.
            reached: |s| flag(s, FLAG_VISITED_PETALBURG_CITY),
        },
        Goal {
            name: "wally_tutorial",
            // `PetalburgCity_Gym_EventScript_ReturnFromWallyTutorial` sets 2 (the var's comment:
            // "0-1: Wally tutorial, 2-6: 0-4 badges"); the gym badges only add to it.
            reached: |s| var(s, VAR_PETALBURG_GYM_STATE) >= 2,
        },
        Goal {
            name: "route104",
            reached: |s| on(s, MAP_ROUTE104),
        },
        Goal {
            name: "woods_grunt_defeated",
            // `PetalburgWoods_EventScript_DevonResearcher*`: `trainerbattle_no_intro
            // TRAINER_GRUNT_PETALBURG_WOODS` sets its trainer flag.
            reached: |s| flag(s, trainer_flag(woods_grunt())),
        },
        Goal {
            name: "rustboro",
            // `RustboroCity_OnTransition`.
            reached: |s| flag(s, FLAG_VISITED_RUSTBORO_CITY),
        },
        Goal {
            name: "roxanne_defeated",
            reached: |s| flag(s, FLAG_DEFEATED_RUSTBORO_GYM),
        },
        Goal {
            name: "badge1",
            reached: |s| flag(s, FLAG_BADGE01_GET),
        },
    ];

    /// The decomp constants the lists name, for the drift test.
    pub const CONSTANTS: &[(&str, u16)] = &[
        ("VAR_ROUTE101_STATE", VAR_ROUTE101_STATE.0),
        ("FLAG_RESCUED_BIRCH", FLAG_RESCUED_BIRCH.0),
        ("VAR_BIRCH_LAB_STATE", VAR_BIRCH_LAB_STATE.0),
        ("FLAG_VISITED_OLDALE_TOWN", FLAG_VISITED_OLDALE_TOWN.0),
        ("MAP_ROUTE103", MAP_ROUTE103.0),
        (
            "FLAG_DEFEATED_RIVAL_ROUTE103",
            FLAG_DEFEATED_RIVAL_ROUTE103.0,
        ),
        (
            "FLAG_RECEIVED_POKEDEX_FROM_BIRCH",
            FLAG_RECEIVED_POKEDEX_FROM_BIRCH.0,
        ),
        ("FLAG_VISITED_PETALBURG_CITY", FLAG_VISITED_PETALBURG_CITY.0),
        ("VAR_PETALBURG_GYM_STATE", VAR_PETALBURG_GYM_STATE.0),
        ("MAP_ROUTE104", MAP_ROUTE104.0),
        ("FLAG_VISITED_RUSTBORO_CITY", FLAG_VISITED_RUSTBORO_CITY.0),
        ("FLAG_DEFEATED_RUSTBORO_GYM", FLAG_DEFEATED_RUSTBORO_GYM.0),
        ("FLAG_BADGE01_GET", FLAG_BADGE01_GET.0),
    ];

    /// `TRAINER_GRUNT_PETALBURG_WOODS`. The stub build's `content::consts` names no trainers
    /// but `TRAINER_NONE` (its battle data has none), so the value is read from the decomp
    /// names table that the extracted build's constant is generated from
    /// (`extract::names::TRAINERS`); `tests/m8_k_goals.rs` checks it against both.
    pub fn woods_grunt() -> TrainerId {
        static ID: OnceLock<TrainerId> = OnceLock::new();
        *ID.get_or_init(|| {
            let v = extract::names::TRAINERS
                .value("TRAINER_GRUNT_PETALBURG_WOODS")
                .expect("a decomp trainer name");
            TrainerId(u16::try_from(v).expect("a trainer id"))
        })
    }
}

mod stub {
    use super::{Goal, pos};
    use engine::checkpoint::WorldSnapshot;
    use std::sync::OnceLock;

    /// The lead's exp in the stub's settled new game (the stub starts with its starter).
    fn new_game_exp() -> u32 {
        static EXP: OnceLock<u32> = OnceLock::new();
        *EXP.get_or_init(|| {
            let mut s = game_native::Session::new(content::Emerald::for_shells());
            for _ in 0..6000 {
                if let Ok(snap) = s.world().snapshot(s.game()) {
                    return snap.party.first().map_or(0, |m| m.exp);
                }
                s.step(engine::Buttons::NONE);
            }
            0
        })
    }

    fn won(s: &WorldSnapshot) -> bool {
        s.party.first().is_some_and(|m| m.exp > new_game_exp())
    }

    /// The built-in stub world (`crates/content/build/stub.rs`): a won wild battle (the
    /// lead's exp grew), the east side and the bottom-left corner, as the stub route plays
    /// them. The stub has no story: both acts use this list.
    pub static GOALS: [Goal; 3] = [
        Goal {
            name: "wild_won",
            reached: won,
        },
        Goal {
            name: "east",
            reached: |s| pos(s).0 >= 10,
        },
        Goal {
            name: "far_corner",
            reached: |s| {
                let (x, y) = pos(s);
                x <= 5 && y >= 12
            },
        },
    ];
}

/// Silice's goal lists (SIL1), named through `content::consts` of the Silice build (none
/// elsewhere: the act is not in those builds).
mod silice {
    use super::Goal;

    /// The Prologue's goals, in story order: the bar (the intro, Julien's explosion, the
    /// Mallette), the night chase, home, the night, the starter, Route S1 and its first wild
    /// battle, then `VAR_SIL_STORY` 9 at the Bois-Éther edge.
    #[cfg(pkr_silice)]
    pub static CH0: &[Goal] = {
        use super::{flag, on, var};
        use content::consts::*;
        &[
            Goal {
                name: "intro_done",
                reached: |s| flag(s, FLAG_SIL_INTRO_DONE),
            },
            Goal {
                name: "explosion",
                reached: |s| var(s, VAR_SIL_STORY) >= 1,
            },
            Goal {
                name: "mallette",
                reached: |s| flag(s, FLAG_SIL_HAS_MALLETTE),
            },
            Goal {
                name: "chase",
                reached: |s| flag(s, FLAG_SIL_BRICO_CHASE),
            },
            Goal {
                name: "home",
                reached: |s| var(s, VAR_SIL_STORY) >= 2,
            },
            Goal {
                name: "night",
                reached: |s| var(s, VAR_SIL_STORY) >= 3,
            },
            Goal {
                name: "starter",
                reached: |s| var(s, VAR_SIL_STORY) >= 5 && !s.party.is_empty(),
            },
            Goal {
                name: "route_s1",
                reached: |s| on(s, MAP_ROUTE_S01),
            },
            Goal {
                name: "first_battle",
                reached: |s| var(s, VAR_SIL_STORY) >= 6,
            },
            Goal {
                name: "bois_ether_edge",
                reached: |s| var(s, VAR_SIL_STORY) >= 9,
            },
        ]
    };

    #[cfg(not(pkr_silice))]
    pub static CH0: &[Goal] = &[];
}

/// `act`'s goals in story order: the stub list on the stub, none for an act of another flavour
/// ([`Act::in_build`]).
pub fn goals_of(act: Act) -> &'static [Goal] {
    if !act.in_build() {
        return &[];
    }
    match act {
        _ if content::STUB => &stub::GOALS,
        #[cfg(not(pkr_silice))]
        Act::Slice => &emerald::SLICE,
        #[cfg(not(pkr_silice))]
        Act::Act1 => &emerald::ACT1,
        #[cfg(pkr_silice)]
        Act::Slice | Act::Act1 => &[],
        Act::SilCh0 => silice::CH0,
    }
}

/// The default act's goals ([`Act::default`]: Act 1, which begins with the slice's).
pub fn goals() -> &'static [Goal] {
    goals_of(Act::default())
}

/// The `(name, value)` of every decomp constant the Emerald lists name (the drift test).
#[cfg(not(pkr_silice))]
pub fn constants() -> Vec<(&'static str, u16)> {
    let mut c = emerald::CONSTANTS.to_vec();
    c.push(("TRAINER_GRUNT_PETALBURG_WOODS", emerald::woods_grunt().0));
    c
}

/// `parent` goals of `act` reached so far; advances over consecutive goals `snap` satisfies.
pub fn progress(act: Act, parent: u32, snap: &WorldSnapshot) -> u32 {
    let list = goals_of(act);
    let mut p = parent;
    while let Some(g) = list.get(p as usize) {
        if !(g.reached)(snap) {
            break;
        }
        p += 1;
    }
    p
}

/// The goals of `act` a state with no lineage has reached (the root of a `--from` db, Task N):
/// one past the last goal it satisfies. A goal list is in story order, so a later goal implies
/// the earlier ones, including the place goals ("on Route 103") a later state no longer
/// satisfies; [`progress`] from 0 would stop at the first of those.
pub fn state_progress(act: Act, snap: &WorldSnapshot) -> u32 {
    goals_of(act)
        .iter()
        .rposition(|g| (g.reached)(snap))
        .map_or(0, |i| i as u32 + 1)
}

/// The name of goal `i` (0-based) of the default act, if any. Act 1 begins with the slice's
/// goals, so this also names a slice db's goals.
pub fn name(i: u32) -> Option<&'static str> {
    name_of(Act::default(), i)
}

/// The name of goal `i` (0-based) of `act`, if any: `goals_of(act)[progress - 1]` is the last
/// goal reached.
pub fn name_of(act: Act, i: u32) -> Option<&'static str> {
    goals_of(act).get(i as usize).map(|g| g.name)
}
