//! Novelty archive and integer fitness (Task B).
#![allow(unused_variables, dead_code)] // Wave 0 stub: remove when implemented.

use crate::config::{Config, Weights};
use crate::model::{AbstractKey, Evaluate, Evaluation, Fitness, NodeRow, SeenKind, StepResult};
use engine::data::{MapId, ScriptId};
use std::collections::BTreeMap;

/// Hit counts of everything seen so far.
#[derive(Default)]
pub struct Archive {
    flags: BTreeMap<u16, u32>,
    var_values: BTreeMap<(u16, u16), u32>,
    maps: BTreeMap<MapId, u32>,
    scripts: BTreeMap<ScriptId, u32>,
    cells: BTreeMap<(MapId, i16, i16), u32>,
    keys: BTreeMap<AbstractKey, u32>,
}

impl Archive {
    pub fn from_db(
        seen: &[(SeenKind, i64, u32)],
        keys: &[(AbstractKey, u32)],
        cells: &[(MapId, i16, i16, u32)],
    ) -> Archive {
        todo!("Task B")
    }
}

pub struct Evaluator {
    pub archive: Archive,
    pub cfg: Config,
}

impl Evaluate for Evaluator {
    /// key_parts/key/region, novelty vs the archive (then records), goals::progress, score.
    fn evaluate(&mut self, parent: Option<&NodeRow>, step: &StepResult) -> Evaluation {
        todo!("Task B")
    }
}

pub struct Novelty {
    pub flags: u32,
    pub var_values: u32,
    pub maps: u32,
    pub scripts: u32,
    pub cells: u32,
    pub new_key: bool,
}

pub fn score(novelty: &Novelty, progress_gain: u32, bugs: u32, w: &Weights) -> Fitness {
    todo!("Task B")
}

/// Selection weight: fitness.total - age*age_penalty - expanded*expanded_penalty, with a
/// 1/(1+hits) novelty decay on the node's key (integer arithmetic only).
pub fn selection_score(n: &NodeRow, generation: u32, key_hits: u32, cfg: &Config) -> i64 {
    todo!("Task B")
}
