//! The explorer's side of the vanilla oracle (Emerald parity plan, Task O2): what the `vanilla`
//! crate (BPEE in mGBA) needs from the planner, and the bookkeeping that turns per-step
//! [`semantic::diff`]s into [`FindingKind::Parity`] findings.
//!
//! - [`Planner`]: our planner over our map data, with overrides and occupied cells the caller
//!   reads from another game's RAM (Gen3 map ids are ours);
//! - [`overrides_from_grid`]: the `setmetatile` overrides of a Gen3 `gBackupMapLayout` grid;
//! - [`ParityLog`]: deltas to findings, one per new or changed (field, values) along a run;
//! - [`tolerance`]: the RNG switches a run's history sets.

use crate::macros;
use crate::model::{FindingDraft, FindingKind};
use crate::path::Grid;
use crate::semantic::{Delta, Tolerance};
use crate::spine::Step;
use engine::data::{Behavior, Dir, GameData, MapData, MapId};
use std::collections::BTreeMap;

/// Gen3's map grid border (`MAP_OFFSET`): `gBackupMapLayout` is the map plus 7 cells around
/// it on the left and top, 8 on the right (`MAP_OFFSET_W` = 15) and 7 below (`MAP_OFFSET_H`).
pub const GEN3_MAP_OFFSET: i16 = 7;
pub const GEN3_MAP_OFFSET_W: usize = 15;
pub const GEN3_MAP_OFFSET_H: usize = 14;

/// The cells of `map` whose block in a Gen3 live grid (`width` x `height` u16, borders
/// included) differs from the map data: the `setmetatile` overrides, sorted by cell (the
/// planner's order). `None` when the grid is not this map's (its size differs).
pub fn overrides_from_grid(
    map: &MapData,
    width: usize,
    height: usize,
    grid: &[u16],
) -> Option<Vec<((i16, i16), u16)>> {
    overrides_of_blocks(
        (map.width as usize, map.height as usize, map.blocks),
        width,
        height,
        grid,
    )
}

/// [`overrides_from_grid`] on a map's size and blocks.
pub fn overrides_of_blocks(
    (w, h, blocks): (usize, usize, &[u16]),
    width: usize,
    height: usize,
    grid: &[u16],
) -> Option<Vec<((i16, i16), u16)>> {
    if width != w + GEN3_MAP_OFFSET_W
        || height != h + GEN3_MAP_OFFSET_H
        || grid.len() < width * height
    {
        return None;
    }
    let off = GEN3_MAP_OFFSET as usize;
    let mut out = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let live = grid[(y + off) * width + x + off];
            if live != blocks[y * w + x] {
                out.push(((x as i16, y as i16), live));
            }
        }
    }
    out.sort_by_key(|&(c, _)| c);
    Some(out)
}

/// Our planner on one of our maps, for a game that is not our engine.
pub struct Planner<'a, G: GameData> {
    grid: Grid<'a, G>,
}

impl<'a, G: GameData> Planner<'a, G> {
    /// `overrides` sorted by cell ([`overrides_from_grid`]); `occupied`: the cells the
    /// objects (but the player) stand on, and any cell to keep out of (a trainer's sight).
    pub fn new(
        game: &'a G,
        map: MapId,
        overrides: &'a [((i16, i16), u16)],
        occupied: Vec<(i16, i16)>,
    ) -> Self {
        Planner {
            grid: Grid::new(game, map, overrides, occupied),
        }
    }

    /// This planner on map `id`'s layout `layout` (Gen3 `gSaveBlock1Ptr->mapLayoutId`, a
    /// `setmaplayoutindex` override) instead of its own ([`Grid::with_layout`]).
    pub fn with_layout(mut self, id: MapId, layout: Option<u16>) -> Self {
        self.grid = self.grid.with_layout(id, layout);
        self
    }

    /// This planner from the player's elevation (Gen3 `ObjectEvent.currentElevation`: on a
    /// bridge, under it), [`Grid::with_elevation`].
    pub fn with_elevation(mut self, elevation: Option<u8>) -> Self {
        if let Some(e) = elevation {
            self.grid = self.grid.with_elevation(e);
        }
        self
    }

    /// The first direction of a shortest path, around grass when there is a way.
    pub fn first_dir(&self, from: (i16, i16), to: (i16, i16)) -> Option<Dir> {
        self.grid
            .first_dir(from, to, true)
            .or_else(|| self.grid.first_dir(from, to, false))
    }

    pub fn behavior(&self, c: (i16, i16)) -> Behavior {
        self.grid.behavior(c)
    }

    /// Open terrain and no object (grass allowed).
    pub fn walkable(&self, c: (i16, i16)) -> bool {
        self.grid.walkable(c, false)
    }

    /// The cells (and the facing) from which A reaches an object on `at` (`expand`'s talk).
    pub fn talk_approaches(&self, at: (i16, i16)) -> Vec<((i16, i16), Dir)> {
        macros::talk_approaches(&self.grid, at)
    }

    /// The cell to walk to before moving `dir` into warp `at` (`expand`'s enter).
    pub fn enter_approach(&self, at: (i16, i16), dir: Dir) -> (i16, i16) {
        macros::enter_approach(self.grid.behavior(at), at, dir)
    }

    /// The neighbour a `Grass` macro paces to and from (`expand`'s grass).
    pub fn grass_partner(&self, at: (i16, i16)) -> Option<(i16, i16)> {
        Dir::ALL.into_iter().find_map(|d| {
            let (dx, dy) = d.delta();
            let c = (at.0 + dx, at.1 + dy);
            let ledge = matches!(self.grid.behavior(c), Behavior::Ledge(_));
            (self.grid.walkable(c, false) && !ledge).then_some(c)
        })
    }
}

/// What a run's history says about the RNG-dependent fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct History {
    /// A wild battle other than the scripted first one was fought (random species and
    /// levels: levels and exp diverge).
    pub random_battle: bool,
    /// A battle was fought since the last full heal (the damage taken is random).
    pub hurt_since_heal: bool,
}

/// The tolerance at a step boundary after `h`: objects compared in the shared view, by
/// presence; levels until a random battle; HP until a battle since the last heal.
pub fn tolerance(h: History) -> Tolerance {
    Tolerance {
        levels: !h.random_battle,
        hp: !h.hurt_since_heal,
        in_view: true,
        ..Tolerance::default()
    }
}

/// A step's kind as a finding signs it (no map ids or cells: the signature drops digits, and
/// the map is the finding's own column).
pub fn step_kind(step: &Step) -> String {
    let s = format!("{step:?}");
    s.split([' ', '(', '{'])
        .next()
        .unwrap_or_default()
        .to_string()
}

/// Turns step-boundary diffs into `Parity` findings: a delta is new when its field did not
/// differ at the previous boundary. A field that stays different is reported once, where it
/// started, even as its values move on (a counter only one game counts, a money offset).
#[derive(Clone, Debug, Default)]
pub struct ParityLog {
    last: BTreeMap<String, (String, String)>,
    /// `(position index, finding)`, in order.
    pub findings: Vec<(usize, FindingDraft)>,
}

impl ParityLog {
    /// Records the deltas at position `at` (after `step`, in `segment`), on `map` at `pos`
    /// (ours); returns the new ones.
    pub fn record(
        &mut self,
        at: usize,
        segment: Option<&str>,
        step: Option<&Step>,
        map: MapId,
        pos: (i16, i16),
        deltas: &[Delta],
    ) -> Vec<Delta> {
        let mut now = BTreeMap::new();
        let mut new = Vec::new();
        for d in deltas {
            let v = (d.ours.clone(), d.vanilla.clone());
            if !self.last.contains_key(&d.field) {
                new.push(d.clone());
                self.findings.push((
                    at,
                    FindingDraft {
                        kind: FindingKind::Parity,
                        message: parity_message(segment, step, d),
                        location: None,
                        macro_: None,
                        tail: Vec::new(),
                        map,
                        pos,
                        script: None,
                        log: Vec::new(),
                    },
                ));
            }
            now.insert(d.field.clone(), v);
        }
        self.last = now;
        new
    }
}

/// `after <segment>, <step kind>: <field>: ours … / vanilla …`.
pub fn parity_message(segment: Option<&str>, step: Option<&Step>, d: &Delta) -> String {
    format!(
        "after {}, {}: {d}",
        segment.unwrap_or("the new game"),
        step.map_or_else(|| "start".to_string(), step_kind)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delta(field: &str, ours: &str, vanilla: &str) -> Delta {
        Delta {
            field: field.into(),
            ours: ours.into(),
            vanilla: vanilla.into(),
        }
    }

    #[test]
    fn a_lasting_delta_is_one_finding() {
        let mut log = ParityLog::default();
        let money = delta("money", "0", "3000");
        let step = Step::Wait(3);
        assert_eq!(
            log.record(
                0,
                None,
                None,
                MapId(1),
                (0, 0),
                std::slice::from_ref(&money)
            )
            .len(),
            1
        );
        assert!(
            log.record(
                1,
                None,
                Some(&step),
                MapId(1),
                (0, 0),
                std::slice::from_ref(&money)
            )
            .is_empty()
        );
        // Other values of a field that kept differing: not new; gone then back: new again.
        let more = delta("money", "200", "3200");
        assert!(
            log.record(
                2,
                None,
                Some(&step),
                MapId(1),
                (0, 0),
                std::slice::from_ref(&more)
            )
            .is_empty()
        );
        log.record(3, None, Some(&step), MapId(1), (0, 0), &[]);
        assert_eq!(
            log.record(4, None, Some(&step), MapId(1), (0, 0), &[more])
                .len(),
            1
        );
        assert_eq!(log.findings.len(), 2);
        assert_eq!(log.findings[0].1.kind, FindingKind::Parity);
        assert_eq!(
            log.findings[0].1.message,
            "after the new game, start: money: ours 0 / vanilla 3000"
        );
        assert_eq!(
            log.findings[1].1.message,
            "after the new game, Wait: money: ours 200 / vanilla 3200"
        );
    }

    #[test]
    fn step_kinds_drop_their_fields() {
        assert_eq!(step_kind(&Step::Wait(3)), "Wait");
        assert_eq!(step_kind(&Step::WaitScript), "WaitScript");
        assert_eq!(
            step_kind(&Step::Goto {
                map: MapId(3),
                x: 1,
                y: 2
            }),
            "Goto"
        );
    }

    #[test]
    fn the_history_sets_the_tolerance() {
        let t = tolerance(History::default());
        assert!(t.levels && t.hp && t.in_view && !t.object_cells);
        let t = tolerance(History {
            random_battle: true,
            hurt_since_heal: true,
        });
        assert!(!t.levels && !t.hp);
    }

    #[test]
    fn overrides_are_the_changed_cells() {
        let blocks = [1u16, 2, 3, 4, 5, 6];
        let map = (3, 2, &blocks[..]);
        let (w, h) = (3 + GEN3_MAP_OFFSET_W, 2 + GEN3_MAP_OFFSET_H);
        let mut grid = vec![0x3FFu16; w * h];
        for y in 0..2 {
            for x in 0..3 {
                grid[(y + 7) * w + x + 7] = blocks[y * 3 + x];
            }
        }
        assert_eq!(overrides_of_blocks(map, w, h, &grid), Some(vec![]));
        grid[8 * w + 9] = 0x0C05; // (2, 1): another metatile, impassable
        assert_eq!(
            overrides_of_blocks(map, w, h, &grid),
            Some(vec![((2, 1), 0x0C05)])
        );
        assert_eq!(overrides_of_blocks(map, w + 1, h, &grid), None);
    }
}
