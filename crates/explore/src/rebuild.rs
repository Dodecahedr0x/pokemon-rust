//! Re-validation after a build change (Task C).

use crate::cache::{self, ReachError, SnapCache, replay_node, session_at};
use crate::config::Config;
use crate::db::{Db, DbError};
use crate::guard;
use crate::model::{AbstractKey, Macro, NodeId, NodeRow, NodeStatus};
use crate::runs;
use engine::checkpoint::{Checkpoint, WorldSnapshot, build_hash, state_digest};
use engine::data::GameData;
use game_native::trace::Run;
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Re-expands a macro from a start state (`None` = a new game, for the root): new inputs and
/// end state, or `None` (broken). The lifetime lets the closure borrow.
pub type Repair<'a> =
    dyn Fn(Option<&Checkpoint>, &Macro) -> Option<(Vec<Run>, Checkpoint)> + Sync + 'a;

/// A repaired end state's abstract key and region (`key::key_parts` + `key::key`/`region` in
/// [`revalidate`]; tests pass fakes to [`revalidate_with_key`]).
pub type KeyFn<'a> = dyn Fn(&WorldSnapshot) -> (AbstractKey, u64) + 'a;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct RevalidateReport {
    /// Replayed to their stored digest (merged ancestors included).
    pub valid: u64,
    /// Re-expanded by `repair`.
    pub repaired: u64,
    /// Newly broken, subtrees included.
    pub broken: u64,
}

/// What one node's check gave.
enum Outcome {
    Valid(Checkpoint),
    Repaired(Vec<Run>, Checkpoint),
    Broken,
}

/// Replays `n` from `start` (a new game for `None`); on divergence (or a panic) re-expands its
/// macro with `repair` from the same start.
fn check<G: GameData + Clone>(
    game: &G,
    n: &NodeRow,
    start: Option<&Checkpoint>,
    repair: &Repair<'_>,
) -> Outcome {
    let replay = guard::catch(|| {
        let mut s = session_at(game, start)?;
        replay_node(&mut s, n)
    });
    if let Ok(Ok(cp)) = replay {
        return Outcome::Valid(cp);
    }
    match guard::catch(|| repair(start, &n.macro_)) {
        Ok(Some((inputs, cp))) => Outcome::Repaired(inputs, cp),
        _ => Outcome::Broken,
    }
}

fn is_live(s: NodeStatus) -> bool {
    matches!(s, NodeStatus::Valid | NodeStatus::Stale)
}

/// After a build change: every Stale node, parents first, depth levels in parallel (rayon).
/// Replays its inputs from its parent's (new) end state; digest equal -> Valid on this build;
/// otherwise `repair(start, macro)` re-expands; Some -> Valid with new inputs, digest, key and
/// pos (repaired); None -> Broken, and its subtree Broken.
///
/// - Merged nodes of another build are replayed too (they can be ancestors of live nodes).
///   They stay Merged unless they break, or unless the node they were merged into broke or
///   lost the key, in which case they claim the key again.
/// - One live node per key. A node whose key is held by a node already checked in this run
///   merges with it (as `Runner::merge`: more goals reached wins, then fewer total frames,
///   then the lower id). If the holder is not
///   checked yet, the node waits merged into it: when the holder keeps the key the same rule
///   applies, otherwise (it broke or was repaired to another key) the best waiting node
///   becomes the holder. A holder that breaks hands the key to the best node merged into it.
/// - Total frames are recomputed down the tree. A repaired node keeps its `progress` and
///   fitness (not re-scored: `goals::progress` needs the expansions' summaries).
/// - Each level is one transaction (statuses and the new snapshots, at depths that are
///   multiples of `cfg.snapshot_every_depth`). At the end `meta.build_*` becomes this build,
///   snapshots of other builds are dropped and open findings of other builds become `stale`.
/// - A db error while reaching a parent outside the run is returned; a parent that cannot be
///   reached by replay breaks its child.
///
/// Runs on the caller's rayon pool; the result does not depend on its size.
pub fn revalidate<G: GameData + Clone + Send + Sync>(
    db: &mut Db,
    game: &G,
    cfg: &Config,
    repair: &Repair<'_>,
) -> Result<RevalidateReport, DbError> {
    let key = |w: &WorldSnapshot| {
        let parts = crate::key::key_parts(w, &cfg.key);
        (crate::key::key(&parts), crate::key::region(&parts))
    };
    revalidate_with_key(db, game, cfg, repair, &key)
}

/// [`revalidate`] with the repaired nodes' key and region from `key`.
#[doc(hidden)]
pub fn revalidate_with_key<G: GameData + Clone + Send + Sync>(
    db: &mut Db,
    game: &G,
    cfg: &Config,
    repair: &Repair<'_>,
    key: &KeyFn<'_>,
) -> Result<RevalidateReport, DbError> {
    guard::install_hook();
    let build = build_hash(game);
    let rows: BTreeMap<NodeId, NodeRow> = db.nodes()?.into_iter().map(|n| (n.id, n)).collect();
    let todo: BTreeSet<NodeId> = rows
        .values()
        .filter(|n| {
            n.status == NodeStatus::Stale
                || (matches!(n.status, NodeStatus::Valid | NodeStatus::Merged) && n.build != build)
        })
        .map(|n| n.id)
        .collect();
    let mut levels: BTreeMap<u32, Vec<NodeId>> = BTreeMap::new();
    for id in &todo {
        levels.entry(rows[id].depth).or_default().push(*id);
    }
    let mut st = State {
        live: rows
            .values()
            .filter(|n| is_live(n.status))
            .map(|n| (n.key, n.id))
            .collect(),
        broken: rows
            .values()
            .filter(|n| n.status == NodeStatus::Broken)
            .map(|n| n.id)
            .collect(),
        rows,
        todo,
        processed: BTreeSet::new(),
        pending: BTreeMap::new(),
        dirty: BTreeSet::new(),
        report: RevalidateReport::default(),
    };
    let mut fallback = SnapCache::new(cfg.cache_capacity);
    // End states of the previous level's checked nodes.
    let mut prev: BTreeMap<NodeId, Arc<Checkpoint>> = BTreeMap::new();

    for (depth, ids) in levels {
        // Start states, on this thread (the db stays here). `None` = the parent is broken.
        let mut jobs: Vec<(NodeId, Option<Arc<Checkpoint>>)> = Vec::new();
        let mut orphans = Vec::new();
        for id in ids {
            let start = match st.rows[&id].parent {
                None => Some(None),
                Some(p) if st.broken.contains(&p) => None,
                Some(p) => match prev.get(&p) {
                    Some(cp) => Some(Some(cp.clone())),
                    None => match cache::reach(game, db, &mut fallback, p) {
                        Ok(cp) => Some(Some(cp)),
                        Err(ReachError::Db(e)) => return Err(e),
                        Err(_) => None,
                    },
                },
            };
            match start {
                Some(s) => jobs.push((id, s)),
                None => orphans.push(id),
            }
        }
        let outcomes: Vec<Outcome> = {
            let rows = &st.rows;
            jobs.par_iter()
                .map(|(id, start)| check(game, &rows[id], start.as_deref(), repair))
                .collect()
        };

        // Apply in id order.
        let mut results: BTreeMap<NodeId, Option<Outcome>> =
            orphans.into_iter().map(|id| (id, None)).collect();
        results.extend(
            jobs.iter()
                .map(|(id, _)| *id)
                .zip(outcomes.into_iter().map(Some)),
        );
        let mut next: BTreeMap<NodeId, Arc<Checkpoint>> = BTreeMap::new();
        let mut snaps: Vec<(NodeId, Arc<Checkpoint>)> = Vec::new();
        for (id, outcome) in results {
            let mut n = st.rows[&id].clone();
            let old_key = n.key;
            let cp = match outcome {
                None | Some(Outcome::Broken) => {
                    st.break_node(id);
                    continue;
                }
                Some(Outcome::Valid(cp)) => {
                    st.report.valid += 1;
                    cp
                }
                Some(Outcome::Repaired(inputs, cp)) => {
                    st.report.repaired += 1;
                    let (k, region) = key(&cp.world);
                    n.inputs = inputs;
                    n.frames = runs::frames(&n.inputs);
                    n.key = k;
                    n.region = region;
                    n.digest = state_digest(&cp.world);
                    n.map = cp.world.field.map;
                    n.pos = cp.world.objects.objs.first().map_or(n.pos, |o| o.pos);
                    cp
                }
            };
            n.total_frames = n.parent.map_or(0, |p| st.rows[&p].total_frames) + u64::from(n.frames);
            n.end_frame = cp.world.frame;
            n.world_hash = cp.header.world_hash;
            n.build = build;
            if n.status == NodeStatus::Stale {
                n.status = NodeStatus::Valid;
            }
            st.processed.insert(id);
            st.set(n);
            st.settle_key(id, old_key);
            let cp = Arc::new(cp);
            if cfg.snapshot_every_depth > 0 && depth % cfg.snapshot_every_depth == 0 {
                snaps.push((id, cp.clone()));
            }
            next.insert(id, cp);
        }

        let batch: Vec<NodeRow> = std::mem::take(&mut st.dirty)
            .into_iter()
            .map(|id| st.rows[&id].clone())
            .collect();
        let snaps: Vec<(NodeId, &Checkpoint)> =
            snaps.iter().map(|(id, cp)| (*id, cp.as_ref())).collect();
        db.set_status_and_snapshots(&batch, &snaps)?;
        prev = next;
    }

    db.drop_snapshots_except(build)?;
    db.set_build(build)?;
    Ok(st.report)
}

/// The run's view of the tree and of the live keys.
struct State {
    /// The latest version of every row.
    rows: BTreeMap<NodeId, NodeRow>,
    /// Rows changed since the last level was written.
    dirty: BTreeSet<NodeId>,
    /// The live node of each key.
    live: BTreeMap<AbstractKey, NodeId>,
    /// Checked nodes waiting, merged, for the not yet checked holder of a key.
    pending: BTreeMap<AbstractKey, BTreeSet<NodeId>>,
    /// Nodes this run checks, and those checked so far.
    todo: BTreeSet<NodeId>,
    processed: BTreeSet<NodeId>,
    broken: BTreeSet<NodeId>,
    report: RevalidateReport,
}

impl State {
    fn set(&mut self, n: NodeRow) {
        self.dirty.insert(n.id);
        self.rows.insert(n.id, n);
    }

    /// Checked in this run, or not part of it (already valid on this build).
    fn settled(&self, id: NodeId) -> bool {
        !self.todo.contains(&id) || self.processed.contains(&id)
    }

    /// The usual winner, as `Runner::merge`: more goals reached, then fewer total frames, then
    /// the lower id.
    fn better(&self, a: NodeId, b: NodeId) -> bool {
        let rank = |id: NodeId| {
            let n = &self.rows[&id];
            (std::cmp::Reverse(n.progress), n.total_frames, id)
        };
        rank(a) < rank(b)
    }

    fn merge_into(&mut self, loser: NodeId, winner: NodeId) {
        let mut n = self.rows[&loser].clone();
        n.status = NodeStatus::Merged;
        n.merged_into = Some(winner);
        self.set(n);
    }

    /// Makes the best of `candidates` the live node of `k`; the others merge into it.
    fn crown(&mut self, k: AbstractKey, candidates: BTreeSet<NodeId>) {
        let Some(mut w) = candidates.first().copied() else {
            return;
        };
        for &c in &candidates {
            if self.better(c, w) {
                w = c;
            }
        }
        let mut n = self.rows[&w].clone();
        n.status = NodeStatus::Valid;
        n.merged_into = None;
        self.set(n);
        self.live.insert(k, w);
        for c in candidates.into_iter().filter(|&c| c != w) {
            self.merge_into(c, w);
        }
    }

    /// `id` no longer holds `k` (it broke or changed key): the best waiting node, or node
    /// merged into it, takes the key.
    fn release(&mut self, k: AbstractKey, id: NodeId) {
        if self.live.get(&k) != Some(&id) {
            return;
        }
        self.live.remove(&k);
        let mut candidates = self.pending.remove(&k).unwrap_or_default();
        candidates.extend(
            self.rows
                .values()
                .filter(|m| {
                    m.status == NodeStatus::Merged
                        && m.merged_into == Some(id)
                        && m.key == k
                        && !self.broken.contains(&m.id)
                        && self.settled(m.id)
                })
                .map(|m| m.id),
        );
        self.crown(k, candidates);
    }

    fn break_node(&mut self, id: NodeId) {
        let mut n = self.rows[&id].clone();
        let k = n.key;
        n.status = NodeStatus::Broken;
        n.merged_into = None;
        self.set(n);
        self.processed.insert(id);
        self.broken.insert(id);
        self.report.broken += 1;
        for waiting in self.pending.values_mut() {
            waiting.remove(&id);
        }
        self.release(k, id);
    }

    /// After checking `id` (now with its final key and status): keeps one live node per key.
    fn settle_key(&mut self, id: NodeId, old_key: AbstractKey) {
        let n = &self.rows[&id];
        let k = n.key;
        if k != old_key {
            self.release(old_key, id);
        }
        let n = &self.rows[&id];
        if n.status == NodeStatus::Merged {
            // Still merged unless its holder broke or no longer holds the key.
            let target = n.merged_into;
            let holder = self.live.get(&k).copied();
            if target.is_some() && holder == target && !self.broken.contains(&target.unwrap()) {
                return;
            }
            let mut n = n.clone();
            n.status = NodeStatus::Valid;
            n.merged_into = None;
            self.set(n);
        }
        match self.live.get(&k).copied() {
            None => {
                self.live.insert(k, id);
            }
            Some(h) if h == id => {
                // The holder kept its key: the waiting nodes compete with it.
                if let Some(mut waiting) = self.pending.remove(&k) {
                    waiting.insert(id);
                    self.crown(k, waiting);
                }
            }
            Some(h) if self.settled(h) => {
                if self.better(id, h) {
                    self.merge_into(h, id);
                    self.live.insert(k, id);
                } else {
                    self.merge_into(id, h);
                }
            }
            Some(h) => {
                self.merge_into(id, h);
                self.pending.entry(k).or_default().insert(id);
            }
        }
    }
}
