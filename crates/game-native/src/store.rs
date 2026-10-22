//! The checkpoint tree on disk (Milestone 5 Task D):
//!
//! ```text
//! nodes/c00001.ron       Node { meta, trace }            committed (no ROM data)
//! snapshots/c00001.ron   Checkpoint (RON, pretty)        git-ignored cache (ROM-derived nicknames)
//! status.ron             BTreeMap<CheckpointId, Status>  git-ignored, written by verify
//! ```
//!
//! A layered store ([`Store::layered`], the `play` bin's) writes its own nodes in a local root
//! (`checkpoints/{flavor}/local/`, git-ignored, or `$PKR_CHECKPOINTS`) and reads the committed
//! tree as a read-only base: base nodes are listed, found and may be parents, but never
//! written. Local ids start above [`LOCAL_IDS`], away from the committed ones.
//!
//! Every file is pretty RON with a fixed format (declaration field order, `\n` lines, 4-space
//! indent), written to a per-writer `*.tmp` file and renamed into place (a new node is
//! hard-linked, so two writers never publish the same id). Nothing in a file depends on the host,
//! the clock or the store's location, so writing the same node twice gives the same bytes.

use crate::trace::{CheckpointId, HashMismatch, Parent, ReplayReport, TRACE_SCHEMA, Trace};
use engine::checkpoint::{BuildHash, Checkpoint, SCHEMA, WorldSnapshot};
use engine::data::MapId;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct NodeMeta {
    pub id: CheckpointId,
    pub label: String,
    pub schema: u16,
    pub build: BuildHash,
    pub frame: u32,
    pub map: MapId,
    /// `state_digest` of the node's snapshot.
    pub digest: u64,
    pub world_hash: u64,
}

impl NodeMeta {
    /// The meta of node `id` whose state is `cp`.
    fn of(id: CheckpointId, label: String, cp: &Checkpoint) -> Self {
        Self {
            id,
            label,
            schema: cp.header.schema,
            build: cp.header.build,
            frame: cp.header.frame,
            map: cp.world.field.map,
            digest: digest(&cp.world),
            world_hash: cp.header.world_hash,
        }
    }
}

/// `nodes/c00042.ron` (committed).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Node {
    pub meta: NodeMeta,
    pub trace: Trace,
}

impl Node {
    /// The column titles of [`Node::row`].
    pub fn row_header() -> String {
        format!(
            "{:<7} {:>6} {:>7}  {:<8} {:<28}",
            "id", "frame", "map", "parent", "label"
        )
    }

    /// The node as a line of a tree table (`play --list`, `cargo xtask checkpoints`): id,
    /// frame, map, parent, label.
    pub fn row(&self) -> String {
        format!(
            "{:<7} {:>6} {:#07x}  {:<8} {:<28}",
            self.meta.id.to_string(),
            self.meta.frame,
            self.meta.map.0,
            self.trace.parent,
            self.meta.label
        )
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Status {
    Valid,
    Stale {
        schema: u16,
    },
    Diverged {
        hash: Option<HashMismatch>,
        parent_diverged: bool,
    },
    Broken(String),
}

impl Status {
    /// The tree's health: every node must be Valid. This is what `cargo xtask checkpoints
    /// verify` (and so `xtask parity`) and the committed-tree test accept. A Stale node fails
    /// too: a schema bump needs `verify --accept`, like a behaviour change.
    pub fn healthy<'a>(statuses: impl IntoIterator<Item = &'a Status>) -> bool {
        statuses.into_iter().all(|s| *s == Status::Valid)
    }
}

/// One node's outcome of [`Store::verify_nodes`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Verdict {
    pub id: CheckpointId,
    pub status: Status,
    /// Valid, but recorded on another build whose build, world hash or trace hashes differ
    /// from this replay's: `verify --accept` refreshes them (and did, if it ran with it).
    pub rehash: bool,
}

/// A node's state rebuilt by replaying its trace.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Replayed {
    pub checkpoint: Checkpoint,
    pub report: ReplayReport,
    /// The hashes this replay observed at the recorder's cadence (`trace::replay_with_hashes`):
    /// the node's `trace.hashes` once `verify --accept` re-baselines it.
    pub hashes: Vec<(u32, u64)>,
}

/// A checkpoint tree rooted at a directory.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Store {
    root: PathBuf,
    /// The read-only tree under this one ([`Store::layered`]).
    base: Option<PathBuf>,
}

/// A layered store's own nodes get ids above this one (`c10001`, ...), so they never collide
/// with the committed tree's.
pub const LOCAL_IDS: u32 = 10_000;

fn node_file(root: &Path, id: CheckpointId) -> PathBuf {
    root.join("nodes").join(format!("{id}.ron"))
}

/// The ids of `root/nodes/*.ron` (a missing directory has none).
fn node_ids(root: &Path) -> io::Result<Vec<CheckpointId>> {
    let entries = match fs::read_dir(root.join("nodes")) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut ids = Vec::new();
    for entry in entries {
        let name = entry?.file_name();
        let name = name.to_string_lossy();
        let Some(stem) = name.strip_suffix(".ron") else {
            continue; // `*.tmp` leftovers, editor files
        };
        let id = parse_id(stem)
            .filter(|id| stem == id.to_string())
            .ok_or_else(|| invalid(format!("nodes/{name}: not a node file name")))?;
        ids.push(id);
    }
    Ok(ids)
}

/// The semantic identity of a snapshot (`engine::checkpoint::state_digest`).
fn digest(w: &WorldSnapshot) -> u64 {
    engine::checkpoint::state_digest(w)
}

/// `c00012` or `12`.
fn parse_id(s: &str) -> Option<CheckpointId> {
    let digits = s.strip_prefix('c').unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok().map(CheckpointId)
}

fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

fn ron_config() -> ron::ser::PrettyConfig {
    ron::ser::PrettyConfig::new()
        .new_line("\n".into())
        .indentor("    ".into())
        .separator(" ".into())
        .struct_names(false)
        .separate_tuple_members(false)
        .enumerate_arrays(false)
        .compact_arrays(false)
}

/// Pretty RON with a trailing newline.
fn to_ron<T: Serialize>(v: &T) -> io::Result<String> {
    let mut s = ron::ser::to_string_pretty(v, ron_config()).map_err(|e| invalid(e.to_string()))?;
    s.push('\n');
    Ok(s)
}

fn read_ron<T: DeserializeOwned>(path: &Path) -> io::Result<T> {
    let text = fs::read_to_string(path)?;
    ron::from_str(&text).map_err(|e| {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        invalid(format!("{name}: {e}"))
    })
}

/// Writes `text` to a temp file next to `path`, named `{name}.{pid}.{counter}.tmp` so that
/// concurrent writers never share one, and syncs it.
fn write_tmp(path: &Path, text: &str) -> io::Result<PathBuf> {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(
        ".{}.{}.tmp",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let tmp = PathBuf::from(tmp);
    let mut f = fs::File::create(&tmp)?;
    f.write_all(text.as_bytes())?;
    f.sync_all()?;
    Ok(tmp)
}

/// Writes `path` through a temp file and a rename, so readers never see half a file.
fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    let tmp = write_tmp(path, text)?;
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

/// Publishes `path` whole, unless it exists (`AlreadyExists`): a hard link of a temp file.
fn write_new(path: &Path, text: &str) -> io::Result<()> {
    let tmp = write_tmp(path, text)?;
    let r = fs::hard_link(&tmp, path);
    let _ = fs::remove_file(&tmp);
    r
}

/// Only the schema of a snapshot file (the rest may not parse under this build).
#[derive(Deserialize)]
struct SchemaProbe {
    header: HeaderProbe,
}

#[derive(Deserialize)]
struct HeaderProbe {
    schema: u16,
}

impl Store {
    /// The committed tree: `<workspace>/checkpoints/{stub|emerald}`.
    pub fn default_root(stub: bool) -> PathBuf {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("crates/game-native is two levels below the workspace");
        workspace
            .join("checkpoints")
            .join(if stub { "stub" } else { "emerald" })
    }

    /// Where personal saves go (the `play` bin): `$PKR_CHECKPOINTS`, else
    /// `<default_root>/local` (git-ignored).
    pub fn local_root(stub: bool) -> PathBuf {
        match std::env::var_os("PKR_CHECKPOINTS").filter(|d| !d.is_empty()) {
            Some(dir) => dir.into(),
            None => Self::default_root(stub).join("local"),
        }
    }

    /// A store at `root`. Nothing is created until a node is added: a missing root is an
    /// empty store.
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Store> {
        let root = root.into();
        if root.exists() && !root.is_dir() {
            return Err(invalid(format!("{} is not a directory", root.display())));
        }
        Ok(Store { root, base: None })
    }

    /// A store writing at `local` over the read-only tree at `base`: `nodes`, `find`,
    /// `topo_order` and `materialize` see both, a local node may have a base parent, and
    /// `add`, `relabel`, `remove` and the snapshot cache only touch `local`. A base node is
    /// never rewritten (`relabel`, `remove`, or a `verify --accept` that would change it fail).
    pub fn layered(base: impl Into<PathBuf>, local: impl Into<PathBuf>) -> io::Result<Store> {
        let base = Store::open(base)?.root;
        Ok(Store {
            base: Some(base),
            ..Store::open(local)?
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn node_path(&self, id: CheckpointId) -> PathBuf {
        node_file(&self.root, id)
    }

    /// Node `id` lives in the read-only base (not in this store's own root).
    fn in_base(&self, id: CheckpointId) -> bool {
        self.base
            .as_deref()
            .is_some_and(|base| !self.node_path(id).exists() && node_file(base, id).exists())
    }

    fn read_only(id: CheckpointId) -> io::Error {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{id} is in the read-only base tree"),
        )
    }

    fn snapshot_path(&self, id: CheckpointId) -> PathBuf {
        self.root.join("snapshots").join(format!("{id}.ron"))
    }

    fn read_node(&self, id: CheckpointId) -> io::Result<Node> {
        let path = match &self.base {
            Some(base) if self.in_base(id) => node_file(base, id),
            _ => self.node_path(id),
        };
        let node: Node = read_ron(&path).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => io::Error::new(e.kind(), format!("no node {id}")),
            _ => e,
        })?;
        if node.meta.id != id {
            return Err(invalid(format!("{id}.ron holds node {}", node.meta.id)));
        }
        Ok(node)
    }

    fn write_node(&self, node: &Node) -> io::Result<()> {
        if self.in_base(node.meta.id) {
            return Err(Self::read_only(node.meta.id));
        }
        write_atomic(&self.node_path(node.meta.id), &to_ron(node)?)
    }

    /// Every node (with the base's, if layered), by id.
    pub fn nodes(&self) -> io::Result<Vec<Node>> {
        let mut ids = node_ids(&self.root)?;
        if let Some(base) = &self.base {
            let local: BTreeSet<CheckpointId> = ids.iter().copied().collect();
            for id in node_ids(base)? {
                if local.contains(&id) {
                    return Err(invalid(format!(
                        "{id} is both a local node and one of {}",
                        base.display()
                    )));
                }
                ids.push(id);
            }
        }
        ids.sort_unstable();
        ids.into_iter().map(|id| self.read_node(id)).collect()
    }

    /// By `c00012`, `12` or label.
    pub fn find(&self, id_or_label: &str) -> io::Result<Node> {
        if let Some(id) = parse_id(id_or_label) {
            return self.read_node(id);
        }
        self.nodes()?
            .into_iter()
            .find(|n| n.meta.label == id_or_label)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("no node labelled {id_or_label:?}"),
                )
            })
    }

    /// A label is non-empty, cannot be read as an id, and no other node has it.
    fn check_label(nodes: &[Node], label: &str, id: Option<CheckpointId>) -> io::Result<()> {
        if label.is_empty() || parse_id(label).is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("label {label:?} is empty or reads as an id"),
            ));
        }
        if let Some(other) = nodes
            .iter()
            .find(|n| n.meta.label == label && Some(n.meta.id) != id)
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("label {label:?} is taken by {}", other.meta.id),
            ));
        }
        Ok(())
    }

    /// Adds a node (id: max + 1, above [`LOCAL_IDS`] in a layered store; removing the highest
    /// node frees its id for reuse) whose `trace` leads from its parent to `cp`, and caches
    /// `cp`.
    /// The parent must exist, the trace must start at the parent's frame (0 for a new game)
    /// and end at `cp`'s.
    pub fn add(&self, label: &str, trace: Trace, cp: &Checkpoint) -> io::Result<CheckpointId> {
        let nodes = self.nodes()?;
        Self::check_label(&nodes, label, None)?;
        if trace.schema != TRACE_SCHEMA || cp.header.schema != SCHEMA {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "trace schema {} / checkpoint schema {} (this build: {TRACE_SCHEMA} / {SCHEMA})",
                    trace.schema, cp.header.schema
                ),
            ));
        }
        let parent_frame = match trace.parent {
            Parent::NewGame => 0,
            Parent::Checkpoint(p) => match nodes.iter().find(|n| n.meta.id == p) {
                Some(n) => n.meta.frame,
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("parent {p} does not exist"),
                    ));
                }
            },
        };
        let end = trace.start_frame.checked_add(trace.frames());
        if trace.start_frame != parent_frame || end != Some(cp.header.frame) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "trace frames {}+{} do not lead from the parent (frame {parent_frame}) to the \
                     checkpoint (frame {})",
                    trace.start_frame,
                    trace.frames(),
                    cp.header.frame
                ),
            ));
        }
        // Above every node (and above `LOCAL_IDS` in a layered store).
        let floor = if self.base.is_some() { LOCAL_IDS } else { 0 };
        let mut next = nodes
            .last()
            .map_or(floor, |n| n.meta.id.0.max(floor))
            .checked_add(1);
        let mut node = Node {
            meta: NodeMeta::of(CheckpointId(0), label.into(), cp),
            trace,
        };
        // Another writer may publish the same id meanwhile: take the next one.
        loop {
            let id = CheckpointId(next.ok_or_else(|| invalid("no checkpoint id left"))?);
            node.meta.id = id;
            match write_new(&self.node_path(id), &to_ron(&node)?) {
                Ok(()) => {
                    self.put_snapshot(id, cp)?;
                    return Ok(id);
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => next = id.0.checked_add(1),
                Err(e) => return Err(e),
            }
        }
    }

    pub fn relabel(&self, id: CheckpointId, label: &str) -> io::Result<()> {
        let nodes = self.nodes()?;
        Self::check_label(&nodes, label, Some(id))?;
        let mut node = self.read_node(id)?;
        node.meta.label = label.into();
        self.write_node(&node)
    }

    /// Refuses a node with children, and a base node. Also drops its cached snapshot.
    pub fn remove(&self, id: CheckpointId) -> io::Result<()> {
        if self.in_base(id) {
            return Err(Self::read_only(id));
        }
        let nodes = self.nodes()?;
        if !nodes.iter().any(|n| n.meta.id == id) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("no node {id}"),
            ));
        }
        let children: Vec<String> = nodes
            .iter()
            .filter(|n| n.trace.parent == Parent::Checkpoint(id))
            .map(|n| n.meta.id.to_string())
            .collect();
        if !children.is_empty() {
            return Err(io::Error::other(format!(
                "{id} has children: {}",
                children.join(", ")
            )));
        }
        fs::remove_file(self.node_path(id))?;
        match fs::remove_file(self.snapshot_path(id)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
        // The id may be reused: forget its verdict (a missing or unreadable status is fine).
        let status_path = self.root.join("status.ron");
        if let Ok(mut status) = read_ron::<BTreeMap<CheckpointId, Status>>(&status_path)
            && status.remove(&id).is_some()
        {
            write_atomic(&status_path, &to_ron(&status)?)?;
        }
        Ok(())
    }

    /// The cached snapshot; `None` if absent, of another schema, or undecodable (it is a
    /// cache: replay rebuilds it). A layered store falls back on the base's cache for a base
    /// node.
    pub fn snapshot(&self, id: CheckpointId) -> io::Result<Option<Checkpoint>> {
        match (Self::cached(&self.snapshot_path(id))?, &self.base) {
            (None, Some(base)) if self.in_base(id) => {
                Self::cached(&base.join("snapshots").join(format!("{id}.ron")))
            }
            (cp, _) => Ok(cp),
        }
    }

    fn cached(path: &Path) -> io::Result<Option<Checkpoint>> {
        let probe: SchemaProbe = match read_ron(path) {
            Ok(p) => p,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::InvalidData
                ) =>
            {
                return Ok(None);
            }
            Err(e) => return Err(e),
        };
        if probe.header.schema != SCHEMA {
            return Ok(None);
        }
        match read_ron(path) {
            Ok(cp) => Ok(Some(cp)),
            Err(e) if e.kind() == io::ErrorKind::InvalidData => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn put_snapshot(&self, id: CheckpointId, cp: &Checkpoint) -> io::Result<()> {
        write_atomic(&self.snapshot_path(id), &to_ron(cp)?)
    }

    /// Parents first (siblings by id); a missing parent or a cycle is an error.
    pub fn topo_order(&self) -> io::Result<Vec<Node>> {
        let mut pending: BTreeMap<CheckpointId, Node> =
            self.nodes()?.into_iter().map(|n| (n.meta.id, n)).collect();
        let mut children: BTreeMap<CheckpointId, Vec<CheckpointId>> = BTreeMap::new();
        let mut ready = BTreeSet::new();
        for (&id, n) in &pending {
            match n.trace.parent {
                Parent::NewGame => {
                    ready.insert(id);
                }
                Parent::Checkpoint(p) if pending.contains_key(&p) => {
                    children.entry(p).or_default().push(id)
                }
                Parent::Checkpoint(p) => {
                    return Err(invalid(format!("{id}: parent {p} is missing")));
                }
            }
        }
        let mut order = Vec::with_capacity(pending.len());
        while let Some(id) = ready.pop_first() {
            order.push(pending.remove(&id).expect("each node is ready once"));
            ready.extend(children.remove(&id).unwrap_or_default());
        }
        if !pending.is_empty() {
            let ids: Vec<String> = pending.keys().map(|id| id.to_string()).collect();
            return Err(invalid(format!("parent cycle through {}", ids.join(", "))));
        }
        Ok(order)
    }

    /// [`Store::verify_nodes`], as (id, status) pairs.
    pub fn verify(
        &self,
        replay: impl FnMut(Option<&Checkpoint>, &Trace) -> Result<Replayed, String>,
        accept: bool,
    ) -> io::Result<Vec<(CheckpointId, Status)>> {
        Ok(self
            .verify_nodes(replay, accept)?
            .into_iter()
            .map(|v| (v.id, v.status))
            .collect())
    }

    /// Replays every node in topological order and classifies it; also writes `status.ron`.
    ///
    /// Each trace replays from its parent's state *of this run* (`None` for a new game):
    /// - a Broken parent (or a replayer error) makes the node Broken, without replaying it;
    /// - `meta.schema != SCHEMA` is Stale;
    /// - a trace of another `TRACE_SCHEMA` is Broken, without replaying it;
    /// - an equal `state_digest` is Valid (its snapshot is cached), unless the node was recorded
    ///   on this build and the replay's world hash or periodic hashes differ from the recorded
    ///   ones (Broken: nondeterminism). A Valid node recorded on another build whose build,
    ///   world hash or trace hashes differ from this replay's is flagged [`Verdict::rehash`];
    /// - a different digest on the node's own build is Broken (nondeterminism), unless its
    ///   parent diverged (then the child is Diverged too, so `--accept` re-baselines the subtree);
    /// - otherwise it is Diverged, with the replay's first hash mismatch, and whether its
    ///   parent diverged too.
    ///
    /// With `accept`, Diverged and Stale nodes are re-baselined on the replaying build (meta,
    /// `trace.build`, re-recorded `trace.hashes` and snapshot), and so are the build, world hash
    /// and trace hashes of Valid nodes flagged `rehash`, so that "same build, different state
    /// is Broken" applies to the current build. Broken nodes never are. The returned list is
    /// the verdict of the run; `status.ron` records the tree after it, so accepted nodes are
    /// Valid there.
    pub fn verify_nodes(
        &self,
        mut replay: impl FnMut(Option<&Checkpoint>, &Trace) -> Result<Replayed, String>,
        accept: bool,
    ) -> io::Result<Vec<Verdict>> {
        // Each verified node's state in this run (`None`: Broken).
        let mut states: BTreeMap<CheckpointId, Option<Checkpoint>> = BTreeMap::new();
        let mut verdicts: BTreeMap<CheckpointId, Status> = BTreeMap::new();
        let mut out = Vec::new();
        let mut after = BTreeMap::new();
        for mut node in self.topo_order()? {
            let id = node.meta.id;
            let (parent, parent_diverged) = match node.trace.parent {
                Parent::NewGame => (Some(None), false),
                Parent::Checkpoint(p) => (
                    states[&p].as_ref().map(Some),
                    matches!(verdicts[&p], Status::Diverged { .. }),
                ),
            };
            let replayed = match parent {
                _ if node.trace.schema != TRACE_SCHEMA => {
                    Err(format!("trace schema {}", node.trace.schema))
                }
                None => Err("parent broken".to_string()),
                Some(parent) => replay(parent, &node.trace),
            };
            let mut rehash = false;
            let (status, state) = match replayed {
                Err(e) => (Status::Broken(e), None),
                Ok(r) => {
                    let build = r.checkpoint.header.build;
                    let same_build = node.meta.build == build && !parent_diverged;
                    let rebaselined = Node {
                        meta: NodeMeta::of(id, node.meta.label.clone(), &r.checkpoint),
                        trace: Trace {
                            build,
                            hashes: r.hashes,
                            ..node.trace.clone()
                        },
                    };
                    let status = if node.meta.schema != SCHEMA {
                        Status::Stale {
                            schema: node.meta.schema,
                        }
                    } else if digest(&r.checkpoint.world) == node.meta.digest {
                        let hashes_differ = rebaselined.meta.world_hash != node.meta.world_hash
                            || rebaselined.trace.hashes != node.trace.hashes;
                        if same_build && hashes_differ {
                            Status::Broken("nondeterministic: same build, different hashes".into())
                        } else {
                            rehash = rebaselined != node;
                            Status::Valid
                        }
                    } else if same_build {
                        Status::Broken("nondeterministic: same build, different state".into())
                    } else {
                        Status::Diverged {
                            hash: r.report.first_mismatch,
                            parent_diverged,
                        }
                    };
                    let rebaseline = accept && !matches!(status, Status::Broken(_));
                    if rebaseline && (status != Status::Valid || rehash) {
                        node = rebaselined;
                        self.write_node(&node)?;
                    }
                    if status == Status::Valid || rebaseline {
                        self.put_snapshot(id, &r.checkpoint)?;
                    }
                    let state = match status {
                        Status::Broken(_) => None,
                        _ => Some(r.checkpoint),
                    };
                    (status, state)
                }
            };
            let settled = match status {
                Status::Stale { .. } | Status::Diverged { .. } if accept => Status::Valid,
                ref s => s.clone(),
            };
            after.insert(id, settled);
            states.insert(id, state);
            verdicts.insert(id, status.clone());
            out.push(Verdict { id, status, rehash });
        }
        write_atomic(&self.root.join("status.ron"), &to_ron(&after)?)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_new_never_replaces_and_leaves_no_temp_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("nodes");
        let path = dir.join("c00001.ron");
        write_new(&path, "a").unwrap();
        let e = write_new(&path, "b").unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&path).unwrap(), "a");
        write_atomic(&path, "c").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "c");
        let names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["c00001.ron"]);
    }

    #[test]
    fn ids_parse_with_or_without_prefix() {
        assert_eq!(parse_id("c00012"), Some(CheckpointId(12)));
        assert_eq!(parse_id("12"), Some(CheckpointId(12)));
        assert_eq!(parse_id("c123456"), Some(CheckpointId(123456)));
        for s in ["", "c", "lab", "c12x", "-1", "+1", "c 1"] {
            assert_eq!(parse_id(s), None, "{s:?}");
        }
        assert_eq!(CheckpointId(42).to_string(), "c00042");
    }
}
