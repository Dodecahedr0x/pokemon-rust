//! Novelty archive and integer fitness (Task B, decision 4: no floats).
#![deny(clippy::float_arithmetic)]

use crate::config::{Config, Weights};
use crate::goals::{self, GoalCtx};
use crate::key;
use crate::model::{
    AbstractKey, Evaluate, Evaluation, Fitness, NodeRow, SeenKind, StepResult, Touch,
};
use engine::data::{FlagId, MapId, ScriptId, VarId};
use std::collections::{BTreeMap, BTreeSet};

/// Hit counts of everything seen so far.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Archive {
    flags: BTreeMap<u16, u32>,
    var_values: BTreeMap<(u16, u16), u32>,
    maps: BTreeMap<MapId, u32>,
    scripts: BTreeMap<ScriptId, u32>,
    cells: BTreeMap<(MapId, i16, i16), u32>,
    keys: BTreeMap<AbstractKey, u32>,
}

/// The `seen.id` of a `(var, value)` sighting: `var << 16 | value`.
pub fn var_value_id(var: VarId, value: u16) -> i64 {
    i64::from(var.0) << 16 | i64::from(value)
}

/// Adds one hit; true on a first sighting.
fn hit<K: Ord>(m: &mut BTreeMap<K, u32>, k: K) -> bool {
    let n = m.entry(k).or_insert(0);
    *n = n.saturating_add(1);
    *n == 1
}

impl Archive {
    pub fn from_db(
        seen: &[(SeenKind, i64, u32)],
        keys: &[(AbstractKey, u32)],
        cells: &[(MapId, i16, i16, u32)],
    ) -> Archive {
        let mut a = Archive::default();
        for &(kind, id, hits) in seen {
            match kind {
                SeenKind::Flag => {
                    a.flags.insert(id as u16, hits);
                }
                SeenKind::VarValue => {
                    a.var_values.insert(((id >> 16) as u16, id as u16), hits);
                }
                SeenKind::Map => {
                    a.maps.insert(MapId(id as u16), hits);
                }
                SeenKind::Script => {
                    a.scripts.insert(ScriptId(id as u16), hits);
                }
            }
        }
        a.keys.extend(keys.iter().copied());
        a.cells
            .extend(cells.iter().map(|&(m, x, y, hits)| ((m, x, y), hits)));
        a
    }

    /// Hits of abstract key `k` (0 = never seen).
    pub fn key_hits(&self, k: AbstractKey) -> u32 {
        self.keys.get(&k).copied().unwrap_or(0)
    }

    /// Hits of a `seen` entry (0 = never seen).
    pub fn seen_hits(&self, kind: SeenKind, id: i64) -> u32 {
        let h = match kind {
            SeenKind::Flag => self.flags.get(&(id as u16)),
            SeenKind::VarValue => self.var_values.get(&((id >> 16) as u16, id as u16)),
            SeenKind::Map => self.maps.get(&MapId(id as u16)),
            SeenKind::Script => self.scripts.get(&ScriptId(id as u16)),
        };
        h.copied().unwrap_or(0)
    }

    /// Hits of a coverage cell (0 = never stood on).
    pub fn cell_hits(&self, map: MapId, x: i16, y: i16) -> u32 {
        self.cells.get(&(map, x, y)).copied().unwrap_or(0)
    }

    /// `(SeenKind, id, hits)` rows in kind then id order (the `seen` table's shape).
    pub fn seen_rows(&self) -> Vec<(SeenKind, i64, u32)> {
        let mut v = Vec::new();
        v.extend(
            self.flags
                .iter()
                .map(|(&f, &h)| (SeenKind::Flag, i64::from(f), h)),
        );
        v.extend(
            self.var_values
                .iter()
                .map(|(&(var, val), &h)| (SeenKind::VarValue, var_value_id(VarId(var), val), h)),
        );
        v.extend(
            self.maps
                .iter()
                .map(|(&m, &h)| (SeenKind::Map, i64::from(m.0), h)),
        );
        v.extend(
            self.scripts
                .iter()
                .map(|(&s, &h)| (SeenKind::Script, i64::from(s.0), h)),
        );
        v
    }

    /// `(key, hits)`, key ascending.
    pub fn key_rows(&self) -> Vec<(AbstractKey, u32)> {
        self.keys.iter().map(|(&k, &h)| (k, h)).collect()
    }

    /// `(map, x, y, hits)`, ascending.
    pub fn cell_rows(&self) -> Vec<(MapId, i16, i16, u32)> {
        self.cells
            .iter()
            .map(|(&(m, x, y), &h)| (m, x, y, h))
            .collect()
    }
}

pub struct Evaluator {
    pub archive: Archive,
    pub cfg: Config,
}

impl Evaluator {
    pub fn new(archive: Archive, cfg: Config) -> Evaluator {
        Evaluator { archive, cfg }
    }
}

impl Evaluate for Evaluator {
    /// key_parts/key/region, novelty vs the archive (then records), goals::progress, score.
    ///
    /// `seen` lists every distinct sighting of the step (story flags and `(var, value)`s set at
    /// the end, maps entered or ended on, scripts started), first sightings or not: the db
    /// upsert adds a hit to existing rows, as the archive does here. `touches` are the maps and
    /// scripts of the step.
    fn evaluate(&mut self, parent: Option<&NodeRow>, step: &StepResult) -> Evaluation {
        let snap = &step.end.world;
        let kcfg = &self.cfg.key;
        let parts = key::key_parts(snap, kcfg);
        let k = key::key(&parts);
        let region = key::region(&parts);

        let flags: Vec<FlagId> = snap
            .vars
            .flags
            .iter()
            .copied()
            .filter(|&f| key::story_flag(f, kcfg))
            .collect();
        let vars: Vec<(VarId, u16)> = snap
            .vars
            .vars
            .iter()
            .copied()
            .filter(|&(v, value)| value != 0 && key::story_var(v.0, kcfg))
            .collect();
        let maps: BTreeSet<MapId> = step
            .summary
            .maps
            .iter()
            .copied()
            .chain([snap.field.map])
            .collect();
        let scripts = &step.summary.scripts;

        let a = &mut self.archive;
        let mut nov = Novelty::default();
        let mut seen = Vec::new();
        for &f in &flags {
            nov.flags += u32::from(hit(&mut a.flags, f.0));
            seen.push((SeenKind::Flag, i64::from(f.0)));
        }
        for &(v, value) in &vars {
            nov.var_values += u32::from(hit(&mut a.var_values, (v.0, value)));
            seen.push((SeenKind::VarValue, var_value_id(v, value)));
        }
        for &m in &maps {
            nov.maps += u32::from(hit(&mut a.maps, m));
            seen.push((SeenKind::Map, i64::from(m.0)));
        }
        for &s in scripts {
            nov.scripts += u32::from(hit(&mut a.scripts, s));
            seen.push((SeenKind::Script, i64::from(s.0)));
        }
        for &c in &step.summary.cells {
            nov.cells += u32::from(hit(&mut a.cells, c));
        }
        nov.new_key = hit(&mut a.keys, k);

        let before = parent.map_or(0, |p| p.progress);
        let progress = goals::progress(
            before,
            &GoalCtx {
                snap,
                summary: &step.summary,
            },
        );
        let fitness = score(&nov, progress - before, 0, &self.cfg.weights);

        let touches = maps
            .iter()
            .map(|&m| Touch::Map(m))
            .chain(scripts.iter().map(|&s| Touch::Script(s)))
            .collect();
        Evaluation {
            key: k,
            region,
            progress,
            fitness,
            seen,
            touches,
        }
    }
}

/// First sightings of one step.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Novelty {
    pub flags: u32,
    pub var_values: u32,
    pub maps: u32,
    pub scripts: u32,
    pub cells: u32,
    pub new_key: bool,
}

pub fn score(novelty: &Novelty, progress_gain: u32, bugs: u32, w: &Weights) -> Fitness {
    let n = novelty;
    let novelty = w.flag * i64::from(n.flags)
        + w.var_value * i64::from(n.var_values)
        + w.map * i64::from(n.maps)
        + w.script * i64::from(n.scripts)
        + w.cell * i64::from(n.cells)
        + if n.new_key { w.new_key } else { 0 };
    let progress = w.goal * i64::from(progress_gain);
    let bug = w.bug * i64::from(bugs);
    let balance = 0;
    Fitness {
        novelty,
        progress,
        bug,
        balance,
        total: novelty + progress + bug + balance,
    }
}

/// Selection weight: fitness.total - age*age_penalty - expanded*expanded_penalty, with a
/// 1/(1+hits) novelty decay on the node's key (integer arithmetic only).
pub fn selection_score(n: &NodeRow, generation: u32, key_hits: u32, cfg: &Config) -> i64 {
    let age = i64::from(generation.saturating_sub(n.generation));
    let decayed = n.fitness.total / (1 + i64::from(key_hits));
    decayed
        .saturating_sub(age.saturating_mul(cfg.age_penalty))
        .saturating_sub(i64::from(n.expanded).saturating_mul(cfg.expanded_penalty))
}
