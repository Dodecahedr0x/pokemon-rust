//! The checkpoint tree on disk (Milestone 5 Task D):
//!
//! ```text
//! nodes/c00001.ron       Node { meta, trace }            committed (no ROM data)
//! snapshots/c00001.ron   Checkpoint (RON, pretty)        git-ignored cache (ROM-derived nicknames)
//! status.ron             BTreeMap<CheckpointId, Status>  git-ignored, merged into by verify
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
use engine::checkpoint::{BuildHash, Checkpoint, SCHEMA, WorldSnapshot, schema_readable};
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

/// What a [`Store`] operation fails with.
#[derive(Debug)]
pub enum StoreError {
    /// No node with this id or label (as asked).
    NotFound(String),
    /// A label that is empty, reads as an id, or is taken (the message says which).
    Label(String),
    /// Node `id`'s parent is not in the store.
    Parent {
        id: CheckpointId,
        parent: CheckpointId,
    },
    /// A parent cycle through these nodes (ascending).
    Cycle(Vec<CheckpointId>),
    /// `remove` of a node other nodes start from.
    HasChildren(CheckpointId),
    /// A write to a node of a layered store's read-only base.
    ReadOnly(CheckpointId),
    /// A file that does not parse, or a node file whose name or content is not a node's.
    Parse {
        file: PathBuf,
        msg: String,
    },
    /// A request the store refuses: a trace or checkpoint of another schema, a trace that does
    /// not lead from its parent to its checkpoint, an id both local and in the base, no id left.
    Invalid(String),
    Io(io::Error),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::NotFound(q) => write!(f, "no node {q:?}"),
            StoreError::Label(msg) | StoreError::Invalid(msg) => f.write_str(msg),
            StoreError::Parent { id, parent } => write!(f, "{id}: parent {parent} is missing"),
            StoreError::Cycle(ids) => {
                let ids: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
                write!(f, "parent cycle through {}", ids.join(", "))
            }
            StoreError::HasChildren(id) => write!(f, "{id} has children"),
            StoreError::ReadOnly(id) => write!(f, "{id} is in the read-only base tree"),
            StoreError::Parse { file, msg } => write!(f, "{}: {msg}", file.display()),
            StoreError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StoreError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        StoreError::Io(e)
    }
}

impl StoreError {
    /// A missing file (an absent cache or status is not an error).
    fn is_not_found(&self) -> bool {
        matches!(self, StoreError::Io(e) if e.kind() == io::ErrorKind::NotFound)
    }

    /// A file that is there but unreadable as what it should hold (not RON, or not UTF-8).
    pub fn is_corrupt(&self) -> bool {
        match self {
            StoreError::Parse { .. } => true,
            StoreError::Io(e) => e.kind() == io::ErrorKind::InvalidData,
            _ => false,
        }
    }
}

/// Every node of a store, read once: by id, with their children and a topological order. Built
/// by [`Store::index`], which refuses a missing parent or a cycle.
#[derive(Clone, Debug, Default)]
pub struct Index {
    nodes: BTreeMap<CheckpointId, Node>,
    /// The nodes of a layered store's read-only base.
    base: BTreeSet<CheckpointId>,
    /// The nodes starting from a new game, ascending.
    roots: Vec<CheckpointId>,
    /// Each node's children, ascending.
    children: BTreeMap<CheckpointId, Vec<CheckpointId>>,
    /// Parents first, siblings by id.
    topo: Vec<CheckpointId>,
}

impl Index {
    fn build(loaded: Vec<(Node, bool)>) -> Result<Index, StoreError> {
        let mut index = Index::default();
        for (node, base) in loaded {
            if base {
                index.base.insert(node.meta.id);
            }
            index.nodes.insert(node.meta.id, node);
        }
        for (&id, n) in &index.nodes {
            match n.trace.parent {
                Parent::NewGame => index.roots.push(id),
                Parent::Checkpoint(p) if index.nodes.contains_key(&p) => {
                    index.children.entry(p).or_default().push(id)
                }
                Parent::Checkpoint(parent) => return Err(StoreError::Parent { id, parent }),
            }
        }
        let mut ready: BTreeSet<CheckpointId> = index.roots.iter().copied().collect();
        while let Some(id) = ready.pop_first() {
            index.topo.push(id);
            ready.extend(index.children.get(&id).into_iter().flatten().copied());
        }
        if index.topo.len() < index.nodes.len() {
            // Every node left is on a cycle or below one: walk up from the first to it.
            let placed: BTreeSet<CheckpointId> = index.topo.iter().copied().collect();
            let mut id = *index
                .nodes
                .keys()
                .find(|id| !placed.contains(id))
                .expect("left");
            let mut path = Vec::new();
            while !path.contains(&id) {
                path.push(id);
                id = match index.nodes[&id].trace.parent {
                    Parent::Checkpoint(p) => p,
                    Parent::NewGame => unreachable!("a node below a root is placed"),
                };
            }
            let start = path.iter().position(|&p| p == id).expect("on the path");
            let mut cycle = path.split_off(start);
            cycle.sort_unstable();
            return Err(StoreError::Cycle(cycle));
        }
        Ok(index)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn node(&self, id: CheckpointId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    /// Every node, by id.
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values()
    }

    /// Node `id` is in a layered store's read-only base.
    pub fn is_base(&self, id: CheckpointId) -> bool {
        self.base.contains(&id)
    }

    /// The nodes starting from `p`, ascending (`Parent::NewGame`: the roots).
    pub fn children(&self, p: Parent) -> &[CheckpointId] {
        match p {
            Parent::NewGame => &self.roots,
            Parent::Checkpoint(id) => self.children.get(&id).map_or(&[], Vec::as_slice),
        }
    }

    /// Parents first, siblings by id.
    pub fn topo(&self) -> &[CheckpointId] {
        &self.topo
    }

    /// `id` and its ancestors, root first.
    pub fn chain(&self, id: CheckpointId) -> Result<Vec<CheckpointId>, StoreError> {
        let mut chain = Vec::new();
        let mut at = Parent::Checkpoint(id);
        while let Parent::Checkpoint(id) = at {
            let node = self
                .node(id)
                .ok_or_else(|| StoreError::NotFound(id.to_string()))?;
            chain.push(id);
            at = node.trace.parent;
        }
        chain.reverse();
        Ok(chain)
    }

    /// The node labelled `label`.
    pub fn labelled(&self, label: &str) -> Option<&Node> {
        self.nodes().find(|n| n.meta.label == label)
    }
}

fn node_file(root: &Path, id: CheckpointId) -> PathBuf {
    root.join("nodes").join(format!("{id}.ron"))
}

fn status_file(root: &Path) -> PathBuf {
    root.join("status.ron")
}

/// The ids of `root/nodes/*.ron` (a missing directory has none).
fn node_ids(root: &Path) -> Result<Vec<CheckpointId>, StoreError> {
    let dir = root.join("nodes");
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
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
            .ok_or_else(|| StoreError::Parse {
                file: dir.join(&*name),
                msg: "not a node file name".into(),
            })?;
        ids.push(id);
    }
    Ok(ids)
}

/// Node `id` from `path`: a missing file is `NotFound`, a file holding another id `Parse`.
fn read_node_file(path: &Path, id: CheckpointId) -> Result<Node, StoreError> {
    let node: Node = read_ron(path).map_err(|e| match e {
        e if e.is_not_found() => StoreError::NotFound(id.to_string()),
        e => e,
    })?;
    if node.meta.id != id {
        return Err(StoreError::Parse {
            file: path.into(),
            msg: format!("holds node {}", node.meta.id),
        });
    }
    Ok(node)
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
fn to_ron<T: Serialize>(v: &T) -> Result<String, StoreError> {
    let mut s = ron::ser::to_string_pretty(v, ron_config()).map_err(io::Error::other)?;
    s.push('\n');
    Ok(s)
}

fn read_ron<T: DeserializeOwned>(path: &Path) -> Result<T, StoreError> {
    let text = fs::read_to_string(path)?;
    ron::from_str(&text).map_err(|e| StoreError::Parse {
        file: path.into(),
        msg: e.to_string(),
    })
}

/// `status.ron` at `path`; absent is empty.
fn read_statuses(path: &Path) -> Result<BTreeMap<CheckpointId, Status>, StoreError> {
    match read_ron(path) {
        Err(e) if e.is_not_found() => Ok(BTreeMap::new()),
        r => r,
    }
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

impl Store {
    /// The committed tree of `flavor`: `<workspace>/checkpoints/{stub|emerald|silice}`
    /// (`content::Flavor::name`; the shells pass `content::FLAVOR`).
    pub fn default_root(flavor: content::Flavor) -> PathBuf {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("crates/game-native is two levels below the workspace");
        workspace.join("checkpoints").join(flavor.name())
    }

    /// Where personal saves go (the `play` bin): `$PKR_CHECKPOINTS`, else
    /// `<default_root>/local` (git-ignored).
    pub fn local_root(flavor: content::Flavor) -> PathBuf {
        match std::env::var_os("PKR_CHECKPOINTS").filter(|d| !d.is_empty()) {
            Some(dir) => dir.into(),
            None => Self::default_root(flavor).join("local"),
        }
    }

    /// A store at `root`. Nothing is created until a node is added: a missing root is an
    /// empty store.
    pub fn open(root: impl Into<PathBuf>) -> Result<Store, StoreError> {
        let root = root.into();
        if root.exists() && !root.is_dir() {
            return Err(StoreError::Invalid(format!(
                "{} is not a directory",
                root.display()
            )));
        }
        Ok(Store { root, base: None })
    }

    /// A store writing at `local` over the read-only tree at `base`: `nodes`, `index`, `find`,
    /// `topo_order`, `statuses` and `materialize` see both, a local node may have a base
    /// parent, and `add`, `relabel`, `remove`, `status.ron` and the snapshot cache only touch
    /// `local`. A base node is never rewritten (`relabel`, `remove`, or a `verify --accept`
    /// that would change it fail with [`StoreError::ReadOnly`]).
    pub fn layered(
        base: impl Into<PathBuf>,
        local: impl Into<PathBuf>,
    ) -> Result<Store, StoreError> {
        let base = Store::open(base)?.root;
        Ok(Store {
            base: Some(base),
            ..Store::open(local)?
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The read-only base of a layered store.
    pub fn base(&self) -> Option<&Path> {
        self.base.as_deref()
    }

    fn node_path(&self, id: CheckpointId) -> PathBuf {
        node_file(&self.root, id)
    }

    /// Node `id` lives in the read-only base (not in this store's own root).
    pub fn is_base(&self, id: CheckpointId) -> bool {
        self.base
            .as_deref()
            .is_some_and(|base| !self.node_path(id).exists() && node_file(base, id).exists())
    }

    fn snapshot_path(&self, id: CheckpointId) -> PathBuf {
        self.root.join("snapshots").join(format!("{id}.ron"))
    }

    fn read_node(&self, id: CheckpointId) -> Result<Node, StoreError> {
        let path = match &self.base {
            Some(base) if self.is_base(id) => node_file(base, id),
            _ => self.node_path(id),
        };
        read_node_file(&path, id)
    }

    fn write_node(&self, node: &Node) -> Result<(), StoreError> {
        if self.is_base(node.meta.id) {
            return Err(StoreError::ReadOnly(node.meta.id));
        }
        Ok(write_atomic(&self.node_path(node.meta.id), &to_ron(node)?)?)
    }

    /// Every node file, by id, each read once, with whether it is a base node.
    fn load(&self) -> Result<Vec<(Node, bool)>, StoreError> {
        let mut files: BTreeMap<CheckpointId, (PathBuf, bool)> = node_ids(&self.root)?
            .into_iter()
            .map(|id| (id, (node_file(&self.root, id), false)))
            .collect();
        if let Some(base) = &self.base {
            for id in node_ids(base)? {
                if files.contains_key(&id) {
                    return Err(StoreError::Invalid(format!(
                        "{id} is both a local node and one of {}",
                        base.display()
                    )));
                }
                files.insert(id, (node_file(base, id), true));
            }
        }
        files
            .into_iter()
            .map(|(id, (path, base))| Ok((read_node_file(&path, id)?, base)))
            .collect()
    }

    /// Every node (with the base's, if layered), by id. Unlike [`Store::index`], parents are
    /// not checked.
    pub fn nodes(&self) -> Result<Vec<Node>, StoreError> {
        Ok(self.load()?.into_iter().map(|(n, _)| n).collect())
    }

    /// Every node, read once, with children and order; a missing parent or a cycle is an
    /// error.
    pub fn index(&self) -> Result<Index, StoreError> {
        Index::build(self.load()?)
    }

    /// By `c00012`, `12` (that one file) or label (every node once; parents are not checked,
    /// so a broken tree's nodes can still be found).
    pub fn find(&self, id_or_label: &str) -> Result<Node, StoreError> {
        if let Some(id) = parse_id(id_or_label) {
            return self.read_node(id);
        }
        self.nodes()?
            .into_iter()
            .find(|n| n.meta.label == id_or_label)
            .ok_or_else(|| StoreError::NotFound(id_or_label.into()))
    }

    /// The ids of every node file (with the base's), without reading them.
    fn ids(&self) -> Result<BTreeSet<CheckpointId>, StoreError> {
        let mut ids: BTreeSet<CheckpointId> = node_ids(&self.root)?.into_iter().collect();
        if let Some(base) = &self.base {
            ids.extend(node_ids(base)?);
        }
        Ok(ids)
    }

    /// Drops `id`'s entry from this store's own `status.ron` (a missing or unreadable status
    /// is left alone).
    fn forget_status(&self, id: CheckpointId) -> Result<(), StoreError> {
        let path = status_file(&self.root);
        if let Ok(mut status) = read_statuses(&path)
            && status.remove(&id).is_some()
        {
            write_atomic(&path, &to_ron(&status)?)?;
        }
        Ok(())
    }

    /// A label is non-empty, cannot be read as an id, and no other node has it.
    fn check_label(
        nodes: &[Node],
        label: &str,
        id: Option<CheckpointId>,
    ) -> Result<(), StoreError> {
        if label.is_empty() || parse_id(label).is_some() {
            return Err(StoreError::Label(format!(
                "label {label:?} is empty or reads as an id"
            )));
        }
        if let Some(other) = nodes
            .iter()
            .find(|n| n.meta.label == label && Some(n.meta.id) != id)
        {
            return Err(StoreError::Label(format!(
                "label {label:?} is taken by {}",
                other.meta.id
            )));
        }
        Ok(())
    }

    /// Adds a node (id: max + 1, above [`LOCAL_IDS`] in a layered store; removing the highest
    /// node frees its id for reuse) whose `trace` leads from its parent to `cp`, and caches
    /// `cp`.
    /// The parent must exist, the trace must start at the parent's frame (0 for a new game)
    /// and end at `cp`'s.
    pub fn add(
        &self,
        label: &str,
        trace: Trace,
        cp: &Checkpoint,
    ) -> Result<CheckpointId, StoreError> {
        let nodes = self.nodes()?;
        Self::check_label(&nodes, label, None)?;
        if trace.schema != TRACE_SCHEMA || cp.header.schema != SCHEMA {
            return Err(StoreError::Invalid(format!(
                "trace schema {} / checkpoint schema {} (this build: {TRACE_SCHEMA} / {SCHEMA})",
                trace.schema, cp.header.schema
            )));
        }
        let parent_frame = match trace.parent {
            Parent::NewGame => 0,
            Parent::Checkpoint(p) => match nodes.iter().find(|n| n.meta.id == p) {
                Some(n) => n.meta.frame,
                None => {
                    return Err(StoreError::Invalid(format!("parent {p} does not exist")));
                }
            },
        };
        let end = trace.start_frame.checked_add(trace.frames());
        if trace.start_frame != parent_frame || end != Some(cp.header.frame) {
            return Err(StoreError::Invalid(format!(
                "trace frames {}+{} do not lead from the parent (frame {parent_frame}) to the \
                 checkpoint (frame {})",
                trace.start_frame,
                trace.frames(),
                cp.header.frame
            )));
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
            let id = CheckpointId(
                next.ok_or_else(|| StoreError::Invalid("no checkpoint id left".into()))?,
            );
            node.meta.id = id;
            match write_new(&self.node_path(id), &to_ron(&node)?) {
                Ok(()) => {
                    self.put_snapshot(id, cp)?;
                    // A verdict of an earlier node with this id is not this node's.
                    self.forget_status(id)?;
                    return Ok(id);
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => next = id.0.checked_add(1),
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// Refuses a base node ([`StoreError::ReadOnly`]) and a label in use
    /// ([`StoreError::Label`]).
    pub fn relabel(&self, id: CheckpointId, label: &str) -> Result<(), StoreError> {
        if self.is_base(id) {
            return Err(StoreError::ReadOnly(id));
        }
        let nodes = self.nodes()?;
        Self::check_label(&nodes, label, Some(id))?;
        let mut node = self.read_node(id)?;
        node.meta.label = label.into();
        self.write_node(&node)
    }

    /// Moves node `id` under `parent` (`cargo xtask checkpoints promote`: a local child of a
    /// promoted node). The trace is kept, so the new parent must hold the old parent's state
    /// (the caller checks it); the new parent must exist at the trace's start frame. Refuses a
    /// base node ([`StoreError::ReadOnly`]). The node file is rewritten atomically.
    pub fn reparent(&self, id: CheckpointId, parent: CheckpointId) -> Result<(), StoreError> {
        if self.is_base(id) {
            return Err(StoreError::ReadOnly(id));
        }
        let mut node = self.read_node(id)?;
        let p = self.read_node(parent)?;
        if p.meta.frame != node.trace.start_frame {
            return Err(StoreError::Invalid(format!(
                "{id} starts at frame {}, {parent} is at frame {}",
                node.trace.start_frame, p.meta.frame
            )));
        }
        node.trace.parent = Parent::Checkpoint(parent);
        self.write_node(&node)
    }

    /// Refuses a node with children, and a base node. Also drops its cached snapshot and its
    /// `status.ron` entry. Parents are not checked, so a broken tree can be repaired.
    pub fn remove(&self, id: CheckpointId) -> Result<(), StoreError> {
        if self.is_base(id) {
            return Err(StoreError::ReadOnly(id));
        }
        let nodes = self.nodes()?;
        if !nodes.iter().any(|n| n.meta.id == id) {
            return Err(StoreError::NotFound(id.to_string()));
        }
        if nodes
            .iter()
            .any(|n| n.trace.parent == Parent::Checkpoint(id))
        {
            return Err(StoreError::HasChildren(id));
        }
        fs::remove_file(self.node_path(id))?;
        match fs::remove_file(self.snapshot_path(id)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
        // The id may be reused: forget its verdict.
        self.forget_status(id)
    }

    /// The cached snapshot; `None` if absent, of another schema, or undecodable (it is a
    /// cache: replay rebuilds it). One parse. A layered store falls back on the base's cache
    /// for a base node.
    pub fn snapshot(&self, id: CheckpointId) -> Result<Option<Checkpoint>, StoreError> {
        match (Self::cached(&self.snapshot_path(id))?, &self.base) {
            (None, Some(base)) if self.is_base(id) => {
                Self::cached(&base.join("snapshots").join(format!("{id}.ron")))
            }
            (cp, _) => Ok(cp),
        }
    }

    fn cached(path: &Path) -> Result<Option<Checkpoint>, StoreError> {
        match read_ron::<Checkpoint>(path) {
            Ok(cp) => Ok(schema_readable(cp.header.schema).then_some(cp)),
            Err(StoreError::Parse { .. }) => Ok(None),
            Err(StoreError::Io(e))
                if matches!(
                    e.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::InvalidData
                ) =>
            {
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    pub fn put_snapshot(&self, id: CheckpointId, cp: &Checkpoint) -> Result<(), StoreError> {
        Ok(write_atomic(&self.snapshot_path(id), &to_ron(cp)?)?)
    }

    /// The last verdicts (`status.ron`; none if absent). A layered store has two: the base's
    /// (from `cargo xtask checkpoints verify`) and its own. Both may hold a base node's verdict;
    /// the newer file (by modification time) wins, the local one on a tie or an unknown time.
    pub fn statuses(&self) -> Result<BTreeMap<CheckpointId, Status>, StoreError> {
        let local = status_file(&self.root);
        let Some(base) = self.base.as_deref().map(status_file) else {
            return read_statuses(&local);
        };
        let modified = |p: &Path| fs::metadata(p).and_then(|m| m.modified()).ok();
        let (older, newer) = match (modified(&base), modified(&local)) {
            (Some(b), Some(l)) if b > l => (local, base),
            _ => (base, local),
        };
        let mut all = read_statuses(&older)?;
        all.extend(read_statuses(&newer)?);
        Ok(all)
    }

    /// Merges `verdicts` into this store's own `status.ron` (read, modify, atomic rename):
    /// other entries stay, and only nodes that exist (here or in the base) are kept, so a
    /// verdict of a node removed meanwhile is dropped. An unreadable `status.ron` is replaced.
    ///
    /// Two concurrent merges are last-writer-wins (one's verdicts may be lost): acceptable,
    /// since the file is a cache that the next verify rewrites.
    pub fn merge_statuses(&self, verdicts: &[(CheckpointId, Status)]) -> Result<(), StoreError> {
        let path = status_file(&self.root);
        let mut all = match read_statuses(&path) {
            Err(e) if e.is_corrupt() => BTreeMap::new(),
            r => r?,
        };
        all.extend(verdicts.iter().cloned());
        let ids = self.ids()?;
        all.retain(|id, _| ids.contains(id));
        Ok(write_atomic(&path, &to_ron(&all)?)?)
    }

    /// Parents first (siblings by id); a missing parent or a cycle is an error.
    pub fn topo_order(&self) -> Result<Vec<Node>, StoreError> {
        let mut index = self.index()?;
        Ok(index
            .topo
            .iter()
            .map(|id| index.nodes.remove(id).expect("indexed"))
            .collect())
    }

    /// [`Store::verify_nodes`], as (id, status) pairs.
    pub fn verify(
        &self,
        replay: impl FnMut(Option<&Checkpoint>, &Trace) -> Result<Replayed, String>,
        accept: bool,
    ) -> Result<Vec<(CheckpointId, Status)>, StoreError> {
        Ok(self
            .verify_nodes(replay, accept)?
            .into_iter()
            .map(|v| (v.id, v.status))
            .collect())
    }

    /// Replays every node in topological order and classifies it; also merges the verdicts into `status.ron`
    /// ([`Store::merge_statuses`]: entries of other nodes stay).
    ///
    /// Each trace replays from its parent's state *of this run* (`None` for a new game):
    /// - a Broken parent (or a replayer error) makes the node Broken, without replaying it;
    /// - a `meta.schema` this build cannot read (`schema_readable`) is Stale;
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
    ) -> Result<Vec<Verdict>, StoreError> {
        self.verify_with(|_, parent, trace| replay(parent, trace), accept)
    }

    /// [`Store::verify_nodes`] on up to `jobs` threads, with the same verdicts and writes.
    ///
    /// Each node is first replayed speculatively, in parallel, from the state its parent will
    /// most likely have: a new game, or the parent's cached snapshot when it has the parent's
    /// recorded digest. The verification pass (in topological order, as `verify_nodes`) then
    /// takes a speculative replay only when it started from exactly (`==`) the parent's state
    /// of this run, and replays the node itself otherwise. A replay is a function of its start
    /// and its trace, so the outcome is `verify_nodes`'s: a chain whose snapshots are cached
    /// verifies in the time of its longest node.
    pub fn verify_nodes_par(
        &self,
        replay: impl Fn(Option<&Checkpoint>, &Trace) -> Result<Replayed, String> + Sync,
        accept: bool,
        jobs: usize,
    ) -> Result<Vec<Verdict>, StoreError> {
        let nodes = self.topo_order()?;
        let digests: BTreeMap<CheckpointId, u64> =
            nodes.iter().map(|n| (n.meta.id, n.meta.digest)).collect();
        // (node, the start it is replayed from).
        let mut guesses: Vec<(&Node, Option<Checkpoint>)> = Vec::new();
        for n in &nodes {
            if n.trace.schema != TRACE_SCHEMA {
                continue;
            }
            match n.trace.parent {
                Parent::NewGame => guesses.push((n, None)),
                Parent::Checkpoint(p) => {
                    if let Some(cp) = self.snapshot(p)?
                        && digests.get(&p) == Some(&digest(&cp.world))
                    {
                        guesses.push((n, Some(cp)));
                    }
                }
            }
        }
        let next = std::sync::atomic::AtomicUsize::new(0);
        let done = std::sync::Mutex::new(BTreeMap::new());
        std::thread::scope(|sc| {
            for _ in 0..jobs.max(1).min(guesses.len()) {
                sc.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some((n, start)) = guesses.get(i) else {
                            break;
                        };
                        let r = replay(start.as_ref(), &n.trace);
                        done.lock().expect("no panic holds it").insert(n.meta.id, r);
                    }
                });
            }
        });
        let mut done = done.into_inner().expect("no panic holds it");
        let starts: BTreeMap<CheckpointId, Option<Checkpoint>> = guesses
            .into_iter()
            .map(|(n, start)| (n.meta.id, start))
            .collect();
        self.verify_with(
            |id, parent, trace| match (starts.get(&id), done.remove(&id)) {
                (Some(start), Some(r)) if start.as_ref() == parent => r,
                _ => replay(parent, trace),
            },
            accept,
        )
    }

    /// [`Store::verify_nodes`] with `replay` also given the node's id.
    fn verify_with(
        &self,
        mut replay: impl FnMut(CheckpointId, Option<&Checkpoint>, &Trace) -> Result<Replayed, String>,
        accept: bool,
    ) -> Result<Vec<Verdict>, StoreError> {
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
                Some(parent) => replay(id, parent, &node.trace),
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
                    let status = if !schema_readable(node.meta.schema) {
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
        self.merge_statuses(&after.into_iter().collect::<Vec<_>>())?;
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
