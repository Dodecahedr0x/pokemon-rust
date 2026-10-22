//! The slice's goal list, in story order (Task B, decision 15).
//!
//! A goal is a predicate over a node's end snapshot and the summary of the expansion that led
//! to it. [`progress`] only advances over consecutive goals, so it is monotone along a lineage.

use crate::model::SimSummary;
use engine::checkpoint::WorldSnapshot;
use engine::data::{FlagId, MapId, VarId};

pub struct Goal {
    pub name: &'static str,
    pub reached: fn(&GoalCtx) -> bool,
}

pub struct GoalCtx<'a> {
    pub snap: &'a WorldSnapshot,
    pub summary: &'a SimSummary,
}

impl GoalCtx<'_> {
    /// Whether `map` was entered during the expansion or is the end map.
    pub fn entered(&self, map: MapId) -> bool {
        self.snap.field.map == map || self.summary.maps.contains(&map)
    }

    /// Whether flag `f` is set at the end.
    pub fn flag(&self, f: FlagId) -> bool {
        self.snap.vars.flags.binary_search(&f).is_ok()
    }

    /// Var `v` at the end (0 when unset).
    pub fn var(&self, v: VarId) -> u16 {
        self.snap
            .vars
            .vars
            .binary_search_by_key(&v, |&(v, _)| v)
            .map_or(0, |i| self.snap.vars.vars[i].1)
    }

    /// The player's cell at the end, then every cell stood on during the expansion.
    pub fn cells(&self) -> impl Iterator<Item = (i16, i16)> + '_ {
        let end = self.snap.objects.objs.first().map(|p| p.pos);
        end.into_iter()
            .chain(self.summary.cells.iter().map(|&(_, x, y)| (x, y)))
    }
}

/// `BattleOutcome::Won` as stored in `BattleSummary::outcome`.
const OUTCOME_WON: u8 = engine::battle::BattleOutcome::Won as u8;

mod emerald {
    use super::Goal;
    use content::consts::*;

    /// Story order from the `BeforeRescue` start (decision 15).
    pub static GOALS: [Goal; 7] = [
        Goal {
            name: "rescue_started",
            reached: |c| c.entered(MAP_ROUTE101),
        },
        Goal {
            name: "starter",
            reached: |c| !c.snap.party.is_empty(),
        },
        Goal {
            name: "birch_rescued",
            reached: |c| c.flag(FLAG_RESCUED_BIRCH),
        },
        Goal {
            name: "lab",
            reached: |c| {
                // Birch has given the starter in his lab (`VAR_BIRCH_LAB_STATE` 3+).
                c.entered(MAP_LITTLEROOT_TOWN_PROFESSOR_BIRCHS_LAB)
                    && c.var(VAR_BIRCH_LAB_STATE) >= 3
            },
        },
        Goal {
            name: "oldale",
            reached: |c| c.entered(MAP_OLDALE_TOWN),
        },
        Goal {
            name: "route103",
            reached: |c| c.entered(MAP_ROUTE103),
        },
        Goal {
            name: "may_defeated",
            reached: |c| c.flag(FLAG_DEFEATED_RIVAL_ROUTE103),
        },
    ];
}

mod stub {
    use super::{Goal, OUTCOME_WON};

    /// The built-in stub world (`crates/content/build/stub.rs`): its one tall-grass cell, the
    /// east side and the bottom-left corner, as the stub route plays them.
    pub static GOALS: [Goal; 3] = [
        Goal {
            name: "wild_won",
            reached: |c| {
                c.summary
                    .battles
                    .iter()
                    .any(|b| b.outcome == Some(OUTCOME_WON))
            },
        },
        Goal {
            name: "east",
            reached: |c| c.cells().any(|(x, _)| x >= 10),
        },
        Goal {
            name: "far_corner",
            reached: |c| c.cells().any(|(x, y)| x <= 5 && y >= 12),
        },
    ];
}

/// The slice's goals in story order (`content::STUB` picks the stub list).
pub fn goals() -> &'static [Goal] {
    if content::STUB {
        &stub::GOALS
    } else {
        &emerald::GOALS
    }
}

/// `parent` goals reached so far; advances over consecutive goals satisfied by `ctx`.
pub fn progress(parent: u32, ctx: &GoalCtx) -> u32 {
    let list = goals();
    let mut p = parent;
    while let Some(g) = list.get(p as usize) {
        if !(g.reached)(ctx) {
            break;
        }
        p += 1;
    }
    p
}

/// The name of goal `i` (0-based), if any: `goals()[progress - 1]` is the last goal reached.
pub fn name(i: u32) -> Option<&'static str> {
    goals().get(i as usize).map(|g| g.name)
}
