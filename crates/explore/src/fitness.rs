//! Novelty archive and integer fitness (Task B, decision 4: no floats).
#![deny(clippy::float_arithmetic)]

use crate::config::{Config, Weights};
use crate::goals;
use crate::key;
use crate::model::{AbstractKey, Evaluate, Evaluation, Fitness, NodeRow, SeenKind, StepResult};
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
    /// Keys seen (their hit counts live in `Graph::key_hits`).
    keys: BTreeSet<AbstractKey>,
}

/// The `seen.id` of a `(var, value)` sighting: `var << 16 | value`.
pub fn var_value_id(var: VarId, value: u16) -> i64 {
    i64::from(var.0) << 16 | i64::from(value)
}

/// `(var, value)` of a `seen.id` (`var << 16 | value`), if it is one.
fn split_var_value(id: i64) -> Option<(u16, u16)> {
    let var = u16::try_from(id >> 16).ok()?;
    Some((var, (id & 0xFFFF) as u16))
}

/// Adds one hit; true on a first sighting.
fn hit<K: Ord>(m: &mut BTreeMap<K, u32>, k: K) -> bool {
    let n = m.entry(k).or_insert(0);
    *n = n.saturating_add(1);
    *n == 1
}

impl Archive {
    /// The archive from the db's `seen` and `coverage` rows and the graph's keys (`keys`' hit
    /// counts are ignored: only whether a key exists matters here). Rows whose id does not fit
    /// their kind (a corrupt or foreign db) are skipped.
    pub fn from_db(
        seen: &[(SeenKind, i64, u32)],
        keys: &[(AbstractKey, u32)],
        cells: &[(MapId, i16, i16, u32)],
    ) -> Archive {
        let mut a = Archive::default();
        for &(kind, id, hits) in seen {
            match kind {
                SeenKind::VarValue => {
                    if let Some(vv) = split_var_value(id) {
                        a.var_values.insert(vv, hits);
                    }
                }
                _ => {
                    let Ok(id) = u16::try_from(id) else { continue };
                    match kind {
                        SeenKind::Flag => a.flags.insert(id, hits),
                        SeenKind::Map => a.maps.insert(MapId(id), hits),
                        SeenKind::Script => a.scripts.insert(ScriptId(id), hits),
                        SeenKind::VarValue => unreachable!(),
                    };
                }
            }
        }
        a.keys.extend(keys.iter().map(|&(k, _)| k));
        a.cells
            .extend(cells.iter().map(|&(m, x, y, hits)| ((m, x, y), hits)));
        a
    }

    /// Whether abstract key `k` was seen.
    pub fn has_key(&self, k: AbstractKey) -> bool {
        self.keys.contains(&k)
    }

    /// Hits of a `seen` entry (0 = never seen).
    pub fn seen_hits(&self, kind: SeenKind, id: i64) -> u32 {
        let h = match (kind, u16::try_from(id)) {
            (SeenKind::VarValue, _) => split_var_value(id).and_then(|vv| self.var_values.get(&vv)),
            (_, Err(_)) => None,
            (SeenKind::Flag, Ok(id)) => self.flags.get(&id),
            (SeenKind::Map, Ok(id)) => self.maps.get(&MapId(id)),
            (SeenKind::Script, Ok(id)) => self.scripts.get(&ScriptId(id)),
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

    /// The keys seen, ascending.
    pub fn keys(&self) -> Vec<AbstractKey> {
        self.keys.iter().copied().collect()
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
    /// The progress the root job's steps start from: 0 for a new game, the root state's
    /// `goals::state_progress` for a `--from` db (Task N).
    pub root_progress: u32,
}

impl Evaluator {
    pub fn new(archive: Archive, cfg: Config) -> Evaluator {
        Evaluator {
            archive,
            cfg,
            root_progress: 0,
        }
    }
}

impl Evaluate for Evaluator {
    /// key_parts/key, novelty vs the archive (then records), goals::progress (the config's
    /// act, on the end state), score.
    ///
    /// `seen` lists every distinct sighting of the step (story flags and `(var, value)`s set at
    /// the end, maps entered or ended on, scripts started), first sightings or not: the db
    /// upsert adds a hit to existing rows, as the archive does here. `touches` are the maps and
    /// scripts of the step.
    ///
    /// Attribution: the runner (Task D) emits `seen` and the summary's cells for every evaluated
    /// step against the node that step resolved to; for a duplicate that is the live node of its
    /// key. So the db's hit counts match this archive's.
    ///
    /// `fitness.bug` is always 0 here (a step carries no findings): the bug weight is applied
    /// by the runner and the wiring (Tasks D and G) through [`score`]'s `bugs`.
    fn evaluate(&mut self, parent: Option<&NodeRow>, step: &StepResult) -> Evaluation {
        let snap = &step.end.world;
        let kcfg = &self.cfg.key;
        let parts = key::key_parts(snap, kcfg);
        let k = key::key(&parts);

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
        nov.new_key = a.keys.insert(k);

        let before = parent.map_or(self.root_progress, |p| p.progress);
        let progress = goals::progress(self.cfg.act, before, snap);
        let fitness = score(&nov, progress - before, 0, &self.cfg.weights);

        let touches = crate::incremental::touches(&step.summary, snap.field.map);
        Evaluation {
            key: k,
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

/// The integer fitness of a step: `Σ weight × count` (saturating, so extreme weights from a
/// config file clamp instead of overflowing).
pub fn score(novelty: &Novelty, progress_gain: u32, bugs: u32, w: &Weights) -> Fitness {
    let n = novelty;
    let term = |weight: i64, count: u32| weight.saturating_mul(i64::from(count));
    let novelty = [
        term(w.flag, n.flags),
        term(w.var_value, n.var_values),
        term(w.map, n.maps),
        term(w.script, n.scripts),
        term(w.cell, n.cells),
        if n.new_key { w.new_key } else { 0 },
    ]
    .into_iter()
    .fold(0i64, i64::saturating_add);
    let progress = term(w.goal, progress_gain);
    let bug = term(w.bug, bugs);
    let balance = 0;
    Fitness {
        novelty,
        progress,
        bug,
        balance,
        total: novelty
            .saturating_add(progress)
            .saturating_add(bug)
            .saturating_add(balance),
    }
}

/// Selection weight: fitness.total - age*age_penalty - expanded*expanded_penalty, with a
/// 1/(1+hits) novelty decay on the node's key (integer arithmetic only).
///
/// `key_hits` comes from `Graph::key_hits`: the step results that resolved to the node's key
/// beyond the first (rows per key − 1), not from the [`Archive`].
pub fn selection_score(n: &NodeRow, generation: u32, key_hits: u32, cfg: &Config) -> i64 {
    let age = i64::from(generation.saturating_sub(n.generation));
    let decayed = n.fitness.total / (1 + i64::from(key_hits));
    decayed
        .saturating_sub(age.saturating_mul(cfg.age_penalty))
        .saturating_sub(i64::from(n.expanded).saturating_mul(cfg.expanded_penalty))
}
