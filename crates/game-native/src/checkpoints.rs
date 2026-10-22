//! The real replayer and materialisation over a [`Store`] (Milestone 5 Task G).

use crate::store::{Replayed, Store};
use crate::trace::{CheckpointId, Parent, Trace, replay_with_hashes};
use engine::World;
use engine::checkpoint::{Checkpoint, build_hash, state_digest};
use engine::data::GameData;

/// Replays a trace from its parent (`None` = new game) on `game`, for [`Store::verify`].
///
/// The parent is `World::new` or `World::restore` (an error is a message); the world must be at
/// the trace's `start_frame`. The replay's hashes at the recorder's cadence fill
/// [`Replayed::hashes`] (what `verify --accept` re-baselines). A trace that does not end on a
/// capturable frame is an error.
pub fn replayer<'g, G: GameData>(
    game: &'g G,
) -> impl FnMut(Option<&Checkpoint>, &Trace) -> Result<Replayed, String> + 'g {
    move |parent: Option<&Checkpoint>, trace: &Trace| replay_from(game, parent, trace)
}

fn replay_from<G: GameData>(
    game: &G,
    parent: Option<&Checkpoint>,
    trace: &Trace,
) -> Result<Replayed, String> {
    let mut world = match parent {
        None => World::new(game),
        Some(cp) => World::restore(game, cp).map_err(|e| format!("restore: {e:?}"))?,
    };
    if world.frame() != trace.start_frame {
        return Err(format!(
            "the parent is at frame {}, the trace starts at {}",
            world.frame(),
            trace.start_frame
        ));
    }
    let (report, hashes) = replay_with_hashes(game, &mut world, trace, |_, _| {});
    let checkpoint = world
        .checkpoint(game)
        .map_err(|r| format!("trace ends while {r:?}"))?;
    Ok(Replayed {
        checkpoint,
        report,
        hashes,
    })
}

/// Node `id`'s checkpoint: the cached snapshot when its schema and build match, else replayed
/// from the root (caching every snapshot on the way).
///
/// A replay whose state differs from the node's recorded digest (a diverged node on this
/// build) is returned but not cached: `verify --accept` re-baselines it.
pub fn materialize<G: GameData>(
    store: &Store,
    game: &G,
    id: CheckpointId,
) -> Result<Checkpoint, String> {
    let build = build_hash(game);
    let cached = |id| -> Result<Option<Checkpoint>, String> {
        Ok(store
            .snapshot(id)
            .map_err(|e| format!("{id}: {e}"))?
            .filter(|cp| cp.header.build == build))
    };
    if let Some(cp) = cached(id)? {
        return Ok(cp);
    }
    // The chain up to the nearest cached ancestor (or the root), then down again.
    let mut chain = Vec::new();
    let mut at = id;
    let mut base = None;
    loop {
        let node = store.find(&at.to_string()).map_err(|e| e.to_string())?;
        if chain.iter().any(|n: &crate::store::Node| n.meta.id == at) {
            return Err(format!("parent cycle through {at}"));
        }
        let parent = node.trace.parent;
        chain.push(node);
        match parent {
            Parent::NewGame => break,
            Parent::Checkpoint(p) => {
                if let Some(cp) = cached(p)? {
                    base = Some(cp);
                    break;
                }
                at = p;
            }
        }
    }
    let mut replay = replayer(game);
    for node in chain.into_iter().rev() {
        let r = replay(base.as_ref(), &node.trace).map_err(|e| format!("{}: {e}", node.meta.id))?;
        if state_digest(&r.checkpoint.world) == node.meta.digest {
            store
                .put_snapshot(node.meta.id, &r.checkpoint)
                .map_err(|e| format!("{}: {e}", node.meta.id))?;
        }
        base = Some(r.checkpoint);
    }
    base.ok_or_else(|| format!("{id}: nothing replayed"))
}

/// The label prefix of the nodes [`seed`] writes.
pub const ROUTE_PREFIX: &str = "route:";

/// Writes the recorded route's [`Recording::nodes`](crate::scenario::Recording::nodes) as
/// `route:<name>` nodes, chained from a new game, after removing the earlier `route:` nodes
/// (never touching other nodes; a non-route node below a route node is an error). Each trace
/// is replayed here: it must reproduce the recording's hashes and end capturable.
pub fn seed<G: GameData>(
    store: &Store,
    game: &G,
    rec: &crate::scenario::Recording,
) -> Result<Vec<CheckpointId>, String> {
    let err = |e: std::io::Error| e.to_string();
    let order = store.topo_order().map_err(err)?;
    let is_route = |id: CheckpointId| {
        order
            .iter()
            .any(|n| n.meta.id == id && n.meta.label.starts_with(ROUTE_PREFIX))
    };
    for n in &order {
        if let Parent::Checkpoint(p) = n.trace.parent
            && is_route(p)
            && !n.meta.label.starts_with(ROUTE_PREFIX)
        {
            return Err(format!(
                "{} ({:?}) is below route node {p}: remove or move it first",
                n.meta.id, n.meta.label
            ));
        }
    }
    for n in order.iter().rev() {
        if n.meta.label.starts_with(ROUTE_PREFIX) {
            store.remove(n.meta.id).map_err(err)?;
        }
    }
    let mut replay = replayer(game);
    let mut ids: Vec<CheckpointId> = Vec::new();
    let mut parent_cp: Option<Checkpoint> = None;
    for (name, mut trace) in rec.traces(build_hash(game)) {
        if let Parent::Checkpoint(CheckpointId(k)) = trace.parent {
            trace.parent = Parent::Checkpoint(ids[k as usize - 1]);
        }
        let r = replay(parent_cp.as_ref(), &trace).map_err(|e| format!("{name}: {e}"))?;
        if let Some(m) = r.report.first_mismatch {
            return Err(format!(
                "{name}: the replay differs from the recording: {m:?}"
            ));
        }
        let id = store
            .add(&format!("{ROUTE_PREFIX}{name}"), trace, &r.checkpoint)
            .map_err(|e| format!("{name}: {e}"))?;
        ids.push(id);
        parent_cp = Some(r.checkpoint);
    }
    Ok(ids)
}
