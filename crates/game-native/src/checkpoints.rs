//! The real replayer, materialisation over a [`Store`] (Milestone 5 Task G), and the
//! [`Materializer`] with its seek keyframes (Milestone 6 Task C).

use crate::Session;
use crate::controls::SaveRequest;
use crate::store::{Node, Replayed, Store, StoreError};
use crate::trace::{CheckpointId, Parent, Trace, TraceRecorder, replay_with_hashes};
use engine::checkpoint::{BuildHash, Checkpoint, build_hash, state_digest};
use engine::data::GameData;
use engine::{Buttons, World};
use std::collections::VecDeque;
use std::sync::Arc;

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

/// [`replayer`] for [`Store::verify_nodes_par`]'s threads.
///
/// [`Store::verify_nodes_par`]: crate::store::Store::verify_nodes_par
pub fn par_replayer<'g, G: GameData + Sync>(
    game: &'g G,
) -> impl Fn(Option<&Checkpoint>, &Trace) -> Result<Replayed, String> + Sync + 'g {
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

/// One node of a [`Materialized::chain`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Link {
    pub id: CheckpointId,
    /// Not replayed by this call: served from the in-memory LRU or the disk cache, or an
    /// ancestor of such a node (its state was not needed).
    pub cached: bool,
    /// This build's replay of the node reaches another state than its recorded digest. Known
    /// for the nodes replayed by this call or an earlier one of the same [`Materializer`];
    /// `false` for nodes never replayed (a valid snapshot cache implies a matching state).
    pub diverged: bool,
}

/// What [`Materializer::get`] (and [`materialize`]) gives.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Materialized {
    pub checkpoint: Checkpoint,
    /// This build's replay reaches another state than the node's recorded digest (the node is
    /// Diverged until `verify --accept`).
    pub diverged: bool,
    /// The node and its ancestors, root first.
    pub chain: Vec<Link>,
}

/// Frames between seek keyframes ([`Materializer::keyframes`]).
pub const KEYFRAME_EVERY: u32 = 600;

/// Nodes whose keyframes a [`Materializer`] keeps (each holds a trace's worth of checkpoints).
pub const KEYFRAME_NODES: usize = 4;

/// A cached node state. `digest` is the node's recorded digest when cached: a node file
/// rewritten under the same id (removed and re-added, reseeded) misses.
struct State {
    id: CheckpointId,
    build: BuildHash,
    digest: u64,
    checkpoint: Checkpoint,
    diverged: bool,
}

struct Keyframes {
    id: CheckpointId,
    build: BuildHash,
    digest: u64,
    frames: Arc<Vec<(u32, Checkpoint)>>,
    /// The replay's hashes at the recorder's cadence (this build's, not the recorded ones).
    hashes: Arc<Vec<(u32, u64)>>,
    /// The parent's state differs on this build (the trace's own hashes are compared by
    /// [`SeekStart::run`], up to the seek target only).
    parent_diverged: bool,
}

/// A keyframe replay to run away from the [`Materializer`] (it is `Send`: a node and a
/// checkpoint), from [`Materializer::prepare_seek`]. [`KeyframePlan::build`] replays the node's
/// trace; [`Materializer::insert_keyframes`] caches the result.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeyframePlan {
    pub node: Node,
    /// The parent's state (`None`: a new game).
    pub parent: Option<Checkpoint>,
    pub parent_diverged: bool,
    /// The seek target, clamped to the trace.
    pub frames: u32,
}

/// The outcome of [`KeyframePlan::build`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BuiltKeyframes {
    plan: KeyframePlan,
    build: BuildHash,
    frames: Arc<Vec<(u32, Checkpoint)>>,
    hashes: Arc<Vec<(u32, u64)>>,
    /// The node's end state, when it is the node's (digest).
    end: Option<Checkpoint>,
}

/// What [`Materializer::prepare_seek`] gives: a seek start, or the keyframe replay to run first.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SeekPrep {
    Ready(SeekStart),
    Keyframes(KeyframePlan),
}

impl KeyframePlan {
    /// Replays the node's trace from its parent on `game`, capturing a keyframe at the first
    /// capturable frame at or after every [`KEYFRAME_EVERY`] frames.
    pub fn build<G: GameData>(self, game: &G) -> Result<BuiltKeyframes, String> {
        let node = &self.node;
        let id = node.meta.id;
        let mut world = match &self.parent {
            None => World::new(game),
            Some(cp) => World::restore(game, cp).map_err(|e| format!("{id}: restore: {e:?}"))?,
        };
        if world.frame() != node.trace.start_frame {
            return Err(format!(
                "{id}: the parent is at frame {}, the trace starts at {}",
                world.frame(),
                node.trace.start_frame
            ));
        }
        let mut frames = Vec::new();
        let (mut i, mut due) = (0u32, KEYFRAME_EVERY);
        let (_, hashes) = replay_with_hashes(game, &mut world, &node.trace, |w, _| {
            i += 1;
            if i >= due
                && let Ok(cp) = w.checkpoint(game)
            {
                frames.push((i, cp));
                due = (i / KEYFRAME_EVERY + 1) * KEYFRAME_EVERY;
            }
        });
        let end =
            (world.checkpoint(game).ok()).filter(|cp| state_digest(&cp.world) == node.meta.digest);
        Ok(BuiltKeyframes {
            build: build_hash(game),
            frames: Arc::new(frames),
            hashes: Arc::new(hashes),
            end,
            plan: self,
        })
    }
}

impl BuiltKeyframes {
    /// The seek start these keyframes give for the plan's target.
    pub fn start(&self) -> SeekStart {
        let p = &self.plan;
        start_from(
            &p.node,
            p.frames,
            p.parent.as_ref(),
            p.parent_diverged,
            &self.frames,
            &self.hashes,
        )
    }
}

/// A seek start into `node` at `frames`: from the last keyframe at or before them, else from
/// `parent`.
fn start_from(
    node: &Node,
    frames: u32,
    parent: Option<&Checkpoint>,
    parent_diverged: bool,
    keyframes: &[(u32, Checkpoint)],
    hashes: &[(u32, u64)],
) -> SeekStart {
    let mut start = SeekStart {
        id: node.meta.id,
        parent: node.trace.parent,
        recorded: node.trace.hashes.clone(),
        trace: node.trace.clone(),
        from: 0,
        base: parent.cloned(),
        frames,
        parent_diverged,
    };
    if let Some((at, cp)) = keyframes.iter().rev().find(|(at, _)| *at <= frames) {
        start.from = *at;
        start.base = Some(cp.clone());
        // This build's hashes up to the keyframe (a diverged node's recorded ones are another
        // build's).
        start.trace.hashes = hashes.to_vec();
    }
    start
}

/// [`materialize`] with an in-memory LRU of node states in front of the disk cache, and the
/// seek keyframes of the last [`KEYFRAME_NODES`] nodes. It holds only checkpoints, so it can be
/// shared between pane threads (`Arc<Mutex<_>>`).
pub struct Materializer<G: GameData> {
    game: G,
    build: BuildHash,
    capacity: usize,
    /// Least recently used first.
    states: VecDeque<State>,
    keyframes: VecDeque<Keyframes>,
}

impl<G: GameData> Materializer<G> {
    /// An LRU of `capacity` node states (at least 1).
    pub fn new(game: G, capacity: usize) -> Self {
        Self {
            build: build_hash(&game),
            game,
            capacity: capacity.max(1),
            states: VecDeque::new(),
            keyframes: VecDeque::new(),
        }
    }

    pub fn game(&self) -> &G {
        &self.game
    }

    /// Node states held in memory.
    pub fn len(&self) -> usize {
        self.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    /// Drops every cached state and keyframe.
    pub fn clear(&mut self) {
        self.states.clear();
        self.keyframes.clear();
    }

    /// The cached state of `node`, made most recently used.
    fn hit(&mut self, node: &Node) -> Option<(Checkpoint, bool)> {
        let (id, build, digest) = (node.meta.id, self.build, node.meta.digest);
        let i = (self.states.iter())
            .position(|s| s.id == id && s.build == build && s.digest == digest)?;
        let s = self.states.remove(i).expect("in range");
        let out = (s.checkpoint.clone(), s.diverged);
        self.states.push_back(s);
        Some(out)
    }

    /// The last known divergence of `node`, without touching the LRU order.
    fn known_diverged(&self, node: &Node) -> bool {
        (self.states.iter()).any(|s| {
            s.id == node.meta.id
                && s.build == self.build
                && s.digest == node.meta.digest
                && s.diverged
        })
    }

    fn insert(&mut self, node: &Node, checkpoint: Checkpoint, diverged: bool) {
        let (id, build) = (node.meta.id, self.build);
        self.states.retain(|s| !(s.id == id && s.build == build));
        if self.states.len() == self.capacity {
            self.states.pop_front();
        }
        self.states.push_back(State {
            id,
            build,
            digest: node.meta.digest,
            checkpoint,
            diverged,
        });
    }

    /// Node `id`'s checkpoint: from the LRU, else the cached snapshot when its schema and build
    /// match and its state is the node's (`state_digest == meta.digest`: snapshots are an
    /// ignored cache, which a reseeded tree reusing ids would otherwise serve stale), else
    /// replayed from the nearest such ancestor or the root, caching every state on the way
    /// (each snapshot file is parsed once per call).
    ///
    /// A replay whose state differs from a node's recorded digest (a diverged node on this
    /// build) is used and kept in memory with its flag, but never written to the disk cache.
    pub fn get(&mut self, store: &Store, id: CheckpointId) -> Result<Materialized, String> {
        let find = |id: CheckpointId| store.find(&id.to_string()).map_err(|e| e.to_string());
        // The chain, target first.
        let mut nodes = vec![find(id)?];
        while let Parent::Checkpoint(p) = nodes.last().expect("non-empty").trace.parent {
            if nodes.iter().any(|n| n.meta.id == p) {
                return Err(format!("parent cycle through {p}"));
            }
            nodes.push(find(p)?);
        }
        nodes.reverse();
        // The deepest node whose state is cached.
        let mut base: Option<(usize, Checkpoint, bool)> = None;
        for (i, node) in nodes.iter().enumerate().rev() {
            if let Some((cp, diverged)) = self.hit(node) {
                base = Some((i, cp, diverged));
                break;
            }
            let nid = node.meta.id;
            let disk = (store.snapshot(nid).map_err(|e| format!("{nid}: {e}"))?).filter(|cp| {
                cp.header.build == self.build && state_digest(&cp.world) == node.meta.digest
            });
            if let Some(cp) = disk {
                self.insert(node, cp.clone(), false);
                base = Some((i, cp, false));
                break;
            }
        }
        let mut chain: Vec<Link> = Vec::with_capacity(nodes.len());
        let (start, mut state, base_diverged) = match base {
            Some((i, cp, d)) => (i + 1, Some(cp), d),
            None => (0, None, false),
        };
        for node in &nodes[..start] {
            chain.push(Link {
                id: node.meta.id,
                cached: true,
                diverged: self.known_diverged(node),
            });
        }
        if let Some(last) = chain.last_mut() {
            last.diverged = base_diverged;
        }
        let mut replayed = Vec::new();
        {
            let mut replay = replayer(&self.game);
            for node in &nodes[start..] {
                let nid = node.meta.id;
                let r = replay(state.as_ref(), &node.trace).map_err(|e| format!("{nid}: {e}"))?;
                let diverged = state_digest(&r.checkpoint.world) != node.meta.digest;
                if !diverged {
                    store
                        .put_snapshot(nid, &r.checkpoint)
                        .map_err(|e| format!("{nid}: {e}"))?;
                }
                chain.push(Link {
                    id: nid,
                    cached: false,
                    diverged,
                });
                replayed.push((node, r.checkpoint.clone(), diverged));
                state = Some(r.checkpoint);
            }
        }
        for (node, cp, diverged) in replayed {
            self.insert(node, cp, diverged);
        }
        Ok(Materialized {
            checkpoint: state.expect("the chain holds the node"),
            diverged: chain.last().expect("non-empty").diverged,
            chain,
        })
    }

    /// Keyframes of node `id`'s trace: (frames since the parent, checkpoint) at the first
    /// capturable frame at or after every [`KEYFRAME_EVERY`] frames (none past the end, none
    /// at 0: that is the parent). Built by one replay from the parent, then cached (the
    /// node's end state goes to the LRU too).
    pub fn keyframes(
        &mut self,
        store: &Store,
        id: CheckpointId,
    ) -> Result<Arc<Vec<(u32, Checkpoint)>>, String> {
        let node = store.find(&id.to_string()).map_err(|e| e.to_string())?;
        Ok(self.keyframes_of(store, &node)?.frames.clone())
    }

    fn keyframes_of(&mut self, store: &Store, node: &Node) -> Result<&Keyframes, String> {
        if self.cached_keyframes(node).is_none() {
            let plan = self.keyframe_plan(store, node, 0)?;
            let built = plan.build(&self.game)?;
            self.insert_keyframes(built);
        }
        Ok(self.cached_keyframes(node).expect("inserted"))
    }

    /// `node`'s cached keyframes, made most recently used.
    fn cached_keyframes(&mut self, node: &Node) -> Option<&Keyframes> {
        let (id, build, digest) = (node.meta.id, self.build, node.meta.digest);
        let i = (self.keyframes.iter())
            .position(|k| k.id == id && k.build == build && k.digest == digest)?;
        let k = self.keyframes.remove(i).expect("in range");
        self.keyframes.push_back(k);
        self.keyframes.back()
    }

    /// The keyframe replay of `node` (one [`Materializer::get`] of its parent).
    fn keyframe_plan(
        &mut self,
        store: &Store,
        node: &Node,
        frames: u32,
    ) -> Result<KeyframePlan, String> {
        let (parent, parent_diverged) = match node.trace.parent {
            Parent::NewGame => (None, false),
            Parent::Checkpoint(p) => {
                let m = self.get(store, p)?;
                (Some(m.checkpoint), m.diverged)
            }
        };
        Ok(KeyframePlan {
            node: node.clone(),
            parent,
            parent_diverged,
            frames,
        })
    }

    /// Caches keyframes built by [`KeyframePlan::build`] (and the node's end state). Keyframes
    /// of another build are ignored.
    pub fn insert_keyframes(&mut self, b: BuiltKeyframes) {
        if b.build != self.build {
            return;
        }
        let node = &b.plan.node;
        let (id, digest) = (node.meta.id, node.meta.digest);
        if let Some(end) = b.end
            && !self.states.iter().any(|s| s.id == id && s.digest == digest)
        {
            self.insert(node, end, false);
        }
        let build = self.build;
        self.keyframes
            .retain(|k| !(k.id == id && k.build == build && k.digest == digest));
        if self.keyframes.len() == KEYFRAME_NODES {
            self.keyframes.pop_front();
        }
        self.keyframes.push_back(Keyframes {
            id,
            build,
            digest,
            frames: b.frames,
            hashes: b.hashes,
            parent_diverged: b.plan.parent_diverged,
        });
    }

    /// The start of a [`seek`] to `frames` frames after node `id`'s parent, or, when the
    /// node's keyframes are not cached yet, the keyframe replay to run first: run it without
    /// holding a shared `Materializer`'s lock ([`KeyframePlan::build`]), take its
    /// [`BuiltKeyframes::start`] and hand it back with [`Materializer::insert_keyframes`].
    /// At most one [`Materializer::get`] (the parent's).
    pub fn prepare_seek(
        &mut self,
        store: &Store,
        id: CheckpointId,
        frames: u32,
    ) -> Result<SeekPrep, String> {
        let node = store.find(&id.to_string()).map_err(|e| e.to_string())?;
        let frames = frames.min(node.trace.frames());
        if frames >= KEYFRAME_EVERY {
            let Some(k) = self.cached_keyframes(&node) else {
                return Ok(SeekPrep::Keyframes(
                    self.keyframe_plan(store, &node, frames)?,
                ));
            };
            let (kfs, hashes, parent_diverged) =
                (k.frames.clone(), k.hashes.clone(), k.parent_diverged);
            if kfs.iter().any(|(at, _)| *at <= frames) {
                return Ok(SeekPrep::Ready(start_from(
                    &node,
                    frames,
                    None,
                    parent_diverged,
                    &kfs,
                    &hashes,
                )));
            }
        }
        let plan = self.keyframe_plan(store, &node, frames)?;
        Ok(SeekPrep::Ready(start_from(
            &node,
            frames,
            plan.parent.as_ref(),
            plan.parent_diverged,
            &[],
            &[],
        )))
    }

    /// The start of a [`seek`] to `frames` frames after node `id`'s parent: the nearest
    /// keyframe at or before them, else the parent ([`Materializer::prepare_seek`] with the
    /// keyframe replay run here). The stepping is left to [`SeekStart::run`].
    pub fn seek_start(
        &mut self,
        store: &Store,
        id: CheckpointId,
        frames: u32,
    ) -> Result<SeekStart, String> {
        match self.prepare_seek(store, id, frames)? {
            SeekPrep::Ready(start) => Ok(start),
            SeekPrep::Keyframes(plan) => {
                let built = plan.build(&self.game)?;
                let start = built.start();
                self.insert_keyframes(built);
                Ok(start)
            }
        }
    }
}

/// Where a [`seek`] starts from ([`Materializer::seek_start`]). Only checkpoints and a trace:
/// it can cross threads.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SeekStart {
    pub id: CheckpointId,
    pub parent: Parent,
    /// The node's trace (its hashes up to `from` are this build's when `from > 0`).
    pub trace: Trace,
    /// The node's recorded hashes.
    pub recorded: Vec<(u32, u64)>,
    /// Frames since the parent of `base`.
    pub from: u32,
    /// The state at `from`: a keyframe, the parent's checkpoint, or `None` for a new game.
    pub base: Option<Checkpoint>,
    /// The target, clamped to the trace.
    pub frames: u32,
    /// The parent's state, or the replay up to a keyframe, differs on this build.
    pub parent_diverged: bool,
}

/// A session [`seek`] left inside a node's trace. Must be built on the thread that will own the
/// session (`World` is `!Send`).
pub struct Seeked<G: GameData> {
    pub session: Session<G>,
    pub parent: Parent,
    /// The trace's first `frames` keys, to go on recording from (`Controls::from_seek`).
    pub recorder: TraceRecorder,
    /// The parent diverged on this build, or so did the trace up to here (a recorded hash).
    pub diverged: bool,
}

impl SeekStart {
    /// A NewGame `trace`'s state after `frames` frames (clamped to it), from a new game: base
    /// `None`, `from` 0, the trace's own hashes as the recorded ones (divergence detection). It
    /// belongs to no node: `id` is `CheckpointId(0)`, which only names it in errors. Used for
    /// traces that are not store nodes (explorer findings, `studio` `NativeCmd::OpenTrace`).
    pub fn from_trace(trace: Trace, frames: u32) -> SeekStart {
        SeekStart {
            id: CheckpointId(0),
            parent: Parent::NewGame,
            recorded: trace.hashes.clone(),
            frames: frames.min(trace.frames()),
            trace,
            from: 0,
            base: None,
            parent_diverged: false,
        }
    }

    /// Restores `base` (or starts a new game) and steps a `Session` up to `frames`.
    pub fn run<G: GameData>(self, game: G) -> Result<Seeked<G>, String> {
        let id = self.id;
        let mut session = match &self.base {
            None => Session::new(game),
            Some(cp) => Session::restore(game, cp).map_err(|e| format!("{id}: restore: {e:?}"))?,
        };
        let at = session.world().frame();
        if at != self.trace.start_frame + self.from {
            return Err(format!(
                "{id}: the seek starts at frame {at}, not {}+{}",
                self.trace.start_frame, self.from
            ));
        }
        let keys: Vec<Buttons> = self.trace.keys().take(self.frames as usize).collect();
        let mut recorder = match self.from {
            0 => TraceRecorder::new(self.trace.start_frame),
            from => {
                let mut r = TraceRecorder::from_prefix(&self.trace, from - 1);
                r.push(keys[from as usize - 1], session.world().hash());
                r
            }
        };
        for &k in &keys[self.from as usize..] {
            session.step(k);
            recorder.push(k, session.world().hash());
        }
        let diverged = self.parent_diverged
            || (self.recorded.iter())
                .filter(|&&(at, _)| at <= self.frames)
                .any(|&(at, h)| recorder.hash_after(at) != Some(h));
        Ok(Seeked {
            session,
            parent: self.parent,
            recorder,
            diverged,
        })
    }
}

/// Node `id`'s state `frames` frames after its parent: restore the nearest keyframe at or
/// before them (or the parent), then step a `Session` with the trace's keys. `frames` is
/// clamped to the trace. Inside a battle (never capturable) it replays from the last
/// capturable keyframe before it.
pub fn seek<G: GameData + Clone>(
    m: &mut Materializer<G>,
    store: &Store,
    id: CheckpointId,
    frames: u32,
) -> Result<Seeked<G>, String> {
    let start = m.seek_start(store, id, frames)?;
    start.run(m.game().clone())
}

/// Node `id`'s checkpoint ([`Materializer::get`] without memory across calls).
pub fn materialize<G: GameData + Clone>(
    store: &Store,
    game: &G,
    id: CheckpointId,
) -> Result<Materialized, String> {
    Materializer::new(game.clone(), 1).get(store, id)
}

/// Label suffixes [`add_save`] tries before giving up.
const SUFFIXES: u32 = 1000;

/// Adds a save to `store`, retrying with a ` #2`, ` #3`, … label suffix while its label is
/// refused (`StoreError::Label`: taken; a save's own label is never empty nor an id).
pub fn add_save(store: &Store, req: &SaveRequest) -> Result<CheckpointId, StoreError> {
    let mut label = req.label.clone();
    for n in 2..=SUFFIXES {
        match store.add(&label, req.trace.clone(), &req.checkpoint) {
            Err(StoreError::Label(_)) => label = format!("{} #{n}", req.label),
            r => return r,
        }
    }
    store.add(&label, req.trace.clone(), &req.checkpoint)
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
