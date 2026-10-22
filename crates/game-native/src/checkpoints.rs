//! The real replayer and materialisation over a [`Store`] (Milestone 5 Task G).

use crate::store::{Node, Replayed, Store};
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

/// What [`materialize`] gives.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Materialized {
    pub checkpoint: Checkpoint,
    /// This build's replay reaches another state than the node's recorded digest (the node is
    /// Diverged until `verify --accept`).
    pub diverged: bool,
}

/// Node `id`'s checkpoint: the cached snapshot when its schema and build match and its state is
/// the node's (`state_digest == meta.digest`: snapshots are an ignored cache, which a
/// reseeded tree reusing ids would otherwise serve stale), else replayed from the nearest such
/// ancestor or the root, caching every snapshot on the way.
///
/// A replay whose state differs from a node's recorded digest (a diverged node on this build)
/// is used but not cached, and flagged for the requested node.
pub fn materialize<G: GameData>(
    store: &Store,
    game: &G,
    id: CheckpointId,
) -> Result<Materialized, String> {
    let build = build_hash(game);
    let cached = |node: &Node| -> Result<Option<Checkpoint>, String> {
        let id = node.meta.id;
        Ok(store
            .snapshot(id)
            .map_err(|e| format!("{id}: {e}"))?
            .filter(|cp| cp.header.build == build && state_digest(&cp.world) == node.meta.digest))
    };
    let find = |id: CheckpointId| store.find(&id.to_string()).map_err(|e| e.to_string());
    let target = find(id)?;
    if let Some(checkpoint) = cached(&target)? {
        return Ok(Materialized {
            checkpoint,
            diverged: false,
        });
    }
    // The chain up to the nearest cached ancestor (or the root), then down again.
    let mut chain = vec![target];
    let mut base = None;
    while let Parent::Checkpoint(p) = chain.last().expect("non-empty").trace.parent {
        if chain.iter().any(|n| n.meta.id == p) {
            return Err(format!("parent cycle through {p}"));
        }
        let node = find(p)?;
        if let Some(cp) = cached(&node)? {
            base = Some(cp);
            break;
        }
        chain.push(node);
    }
    let mut replay = replayer(game);
    let mut diverged = false;
    for node in chain.into_iter().rev() {
        let r = replay(base.as_ref(), &node.trace).map_err(|e| format!("{}: {e}", node.meta.id))?;
        diverged = state_digest(&r.checkpoint.world) != node.meta.digest;
        if !diverged {
            store
                .put_snapshot(node.meta.id, &r.checkpoint)
                .map_err(|e| format!("{}: {e}", node.meta.id))?;
        }
        base = Some(r.checkpoint);
    }
    Ok(Materialized {
        checkpoint: base.expect("the chain holds the node"),
        diverged,
    })
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
    let err = |e: crate::store::StoreError| e.to_string();
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
    // Replay everything first: a failure leaves the tree as it was.
    let build = build_hash(game);
    let mut replay = replayer(game);
    let mut planned = Vec::new();
    let mut parent_cp: Option<Checkpoint> = None;
    for (name, input) in rec.node_inputs() {
        // A replay reads only the inputs and hashes; the parent is the state given.
        let r = replay(parent_cp.as_ref(), &input.trace(Parent::NewGame, build))
            .map_err(|e| format!("{name}: {e}"))?;
        if let Some(m) = r.report.first_mismatch {
            return Err(format!(
                "{name}: the replay differs from the recording: {m:?}"
            ));
        }
        parent_cp = Some(r.checkpoint.clone());
        planned.push((name, input, r.checkpoint));
    }
    for n in order.iter().rev() {
        if n.meta.label.starts_with(ROUTE_PREFIX) {
            store.remove(n.meta.id).map_err(err)?;
        }
    }
    // Each node's parent is the one added before it.
    let mut ids: Vec<CheckpointId> = Vec::new();
    for (name, input, cp) in planned {
        let parent = ids
            .last()
            .map_or(Parent::NewGame, |&id| Parent::Checkpoint(id));
        let id = store
            .add(
                &format!("{ROUTE_PREFIX}{name}"),
                input.trace(parent, build),
                &cp,
            )
            .map_err(|e| format!("{name}: {e}"))?;
        ids.push(id);
    }
    Ok(ids)
}
