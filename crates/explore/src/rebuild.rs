//! Re-validation after a build change (Task C).

use crate::cache::{self, SnapCache, replay_node, session_at};
use crate::config::Config;
use crate::db::{Db, DbError};
use crate::guard;
use crate::model::{AbstractKey, Macro, NodeId, NodeRow, NodeStatus};
use crate::runs;
use engine::checkpoint::{Checkpoint, build_hash, state_digest};
use engine::data::GameData;
use game_native::Session;
use game_native::trace::Run;
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Re-expands a macro from a start state: new inputs and end state, or `None` (broken).
/// The lifetime lets the closure borrow (the contract's `&(dyn Fn … + Sync)`).
pub type Repair<'a> = dyn Fn(&Checkpoint, &Macro) -> Option<(Vec<Run>, Checkpoint)> + Sync + 'a;

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
/// macro with `repair`. The root re-expands only when the new game's first frame is capturable.
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
    let start = match start {
        Some(cp) => Some(cp.clone()),
        None => guard::catch(|| Session::new(game.clone()).checkpoint().ok())
            .ok()
            .flatten(),
    };
    let Some(start) = start else {
        return Outcome::Broken;
    };
    match guard::catch(|| repair(&start, &n.macro_)) {
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
/// Merged nodes of another build are replayed too (they can be ancestors of live nodes) and
/// stay Merged unless they break. A repaired node whose new key is already live merges with
/// that node (fewer total frames wins, then the lower id). Total frames are recomputed down
/// the tree. Each level is one transaction; at the end, `meta.build_*` becomes this build,
/// snapshots of other builds are dropped and open findings of other builds become `stale`.
/// Runs on the caller's rayon pool.
pub fn revalidate<G: GameData + Clone + Send + Sync>(
    db: &mut Db,
    game: &G,
    cfg: &Config,
    repair: &Repair<'_>,
) -> Result<RevalidateReport, DbError> {
    guard::install_hook();
    let build = build_hash(game);
    let todo = |n: &NodeRow| {
        n.status == NodeStatus::Stale
            || (matches!(n.status, NodeStatus::Valid | NodeStatus::Merged) && n.build != build)
    };

    let mut rows: BTreeMap<NodeId, NodeRow> = db.nodes()?.into_iter().map(|n| (n.id, n)).collect();
    let mut live: BTreeMap<AbstractKey, NodeId> = rows
        .values()
        .filter(|n| is_live(n.status))
        .map(|n| (n.key, n.id))
        .collect();
    let mut broken: BTreeSet<NodeId> = rows
        .values()
        .filter(|n| n.status == NodeStatus::Broken)
        .map(|n| n.id)
        .collect();
    let mut levels: BTreeMap<u32, Vec<NodeId>> = BTreeMap::new();
    for n in rows.values().filter(|n| todo(n)) {
        levels.entry(n.depth).or_default().push(n.id);
    }

    let mut report = RevalidateReport::default();
    let mut fallback = SnapCache::new(cfg.cache_capacity);
    // End states of the previous level's revalidated nodes.
    let mut prev: BTreeMap<NodeId, Arc<Checkpoint>> = BTreeMap::new();

    for (depth, ids) in levels {
        // Start states, on this thread (the db stays here).
        let mut jobs: Vec<(NodeId, Option<Arc<Checkpoint>>)> = Vec::new();
        let mut batch: BTreeMap<NodeId, NodeRow> = BTreeMap::new();
        for id in ids {
            let n = &rows[&id];
            let start = match n.parent {
                None => Some(None),
                Some(p) if broken.contains(&p) => None,
                Some(p) => match prev.get(&p) {
                    Some(cp) => Some(Some(cp.clone())),
                    None => cache::reach(game, db, &mut fallback, p).ok().map(Some),
                },
            };
            match start {
                Some(s) => jobs.push((id, s)),
                None => {
                    let mut n = n.clone();
                    n.status = NodeStatus::Broken;
                    n.merged_into = None;
                    batch.insert(id, n);
                }
            }
        }
        let outcomes: Vec<Outcome> = {
            let rows = &rows;
            jobs.par_iter()
                .map(|(id, start)| check(game, &rows[id], start.as_deref(), repair))
                .collect()
        };

        let mut next: BTreeMap<NodeId, Arc<Checkpoint>> = BTreeMap::new();
        for ((id, _), outcome) in jobs.iter().zip(outcomes) {
            let mut n = rows[id].clone();
            let parent_total = n.parent.map_or(0, |p| rows[&p].total_frames);
            let cp = match outcome {
                Outcome::Broken => {
                    n.status = NodeStatus::Broken;
                    n.merged_into = None;
                    batch.insert(n.id, n);
                    continue;
                }
                Outcome::Valid(cp) => {
                    report.valid += 1;
                    cp
                }
                Outcome::Repaired(inputs, cp) => {
                    report.repaired += 1;
                    let parts = crate::key::key_parts(&cp.world, &cfg.key);
                    n.inputs = inputs;
                    n.frames = runs::frames(&n.inputs);
                    n.key = crate::key::key(&parts);
                    n.region = crate::key::region(&parts);
                    n.digest = state_digest(&cp.world);
                    n.map = cp.world.field.map;
                    n.pos = cp.world.objects.objs.first().map_or(n.pos, |o| o.pos);
                    cp
                }
            };
            n.total_frames = parent_total + u64::from(n.frames);
            n.end_frame = cp.world.frame;
            n.world_hash = cp.header.world_hash;
            n.build = build;
            if n.status == NodeStatus::Stale {
                n.status = NodeStatus::Valid;
            }
            if is_live(n.status) {
                merge_live_key(&mut n, &mut rows, &mut live, &mut batch);
            }
            if cfg.snapshot_every_depth > 0 && depth % cfg.snapshot_every_depth == 0 {
                db.put_snapshot(n.id, &cp)?;
            }
            next.insert(n.id, Arc::new(cp));
            batch.insert(n.id, n);
        }

        for n in batch.values() {
            if n.status == NodeStatus::Broken && !broken.contains(&n.id) {
                broken.insert(n.id);
                report.broken += 1;
                if live.get(&n.key) == Some(&n.id) {
                    live.remove(&n.key);
                }
            }
            rows.insert(n.id, n.clone());
        }
        db.set_status(&batch.into_values().collect::<Vec<_>>())?;
        prev = next;
    }

    db.drop_snapshots_except(build)?;
    db.set_build(build)?;
    Ok(report)
}

/// Keeps one live node per key: if `n`'s key is held by another live node, the one with more
/// total frames (then the higher id) becomes Merged into the other.
fn merge_live_key(
    n: &mut NodeRow,
    rows: &mut BTreeMap<NodeId, NodeRow>,
    live: &mut BTreeMap<AbstractKey, NodeId>,
    batch: &mut BTreeMap<NodeId, NodeRow>,
) {
    // A repaired node gives up its old key.
    let old = rows[&n.id].key;
    if old != n.key && live.get(&old) == Some(&n.id) {
        live.remove(&old);
    }
    match live.get(&n.key).copied() {
        None => {
            live.insert(n.key, n.id);
        }
        Some(other) if other == n.id => {}
        Some(other) => {
            let mut o = batch.get(&other).unwrap_or(&rows[&other]).clone();
            if (n.total_frames, n.id) < (o.total_frames, o.id) {
                o.status = NodeStatus::Merged;
                o.merged_into = Some(n.id);
                rows.insert(other, o.clone());
                batch.insert(other, o);
                live.insert(n.key, n.id);
            } else {
                n.status = NodeStatus::Merged;
                n.merged_into = Some(other);
            }
        }
    }
}
