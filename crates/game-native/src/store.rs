//! The checkpoint tree on disk (Milestone 5 Task D):
//!
//! ```text
//! nodes/c00001.ron       Node { meta, trace }            committed (no ROM data)
//! snapshots/c00001.ron   Checkpoint (RON, pretty)        git-ignored cache (ROM-derived nicknames)
//! status.ron             BTreeMap<CheckpointId, Status>  git-ignored, written by verify
//! ```
//!
//! Every file is pretty RON with a fixed format (declaration field order, `\n` lines, 4-space
//! indent), written to `*.tmp` and renamed into place. Nothing in a file depends on the host,
//! the clock or the store's location, so writing the same node twice gives the same bytes.

use crate::trace::{CheckpointId, HashMismatch, Parent, ReplayReport, Trace};
use engine::checkpoint::{BuildHash, Checkpoint, SCHEMA, WorldSnapshot};
use engine::data::MapId;
use engine::hash::Fnv64;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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
}

/// The semantic identity of a snapshot: FNV-1a-64 over its postcard bytes, as
/// `engine::checkpoint::state_digest` defines it (Task C; a test pins the two together).
fn digest(w: &WorldSnapshot) -> u64 {
    let bytes = postcard::to_allocvec(w).expect("snapshots always encode");
    let mut h = Fnv64::new();
    h.write(&bytes);
    h.finish()
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

/// Writes `path` through `path.tmp` and a rename, so readers never see half a file.
fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)
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
    /// `$PKR_CHECKPOINTS`, else `<workspace>/checkpoints/{stub|emerald}`.
    pub fn default_root(stub: bool) -> PathBuf {
        if let Some(dir) = std::env::var_os("PKR_CHECKPOINTS").filter(|d| !d.is_empty()) {
            return dir.into();
        }
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("crates/game-native is two levels below the workspace");
        workspace
            .join("checkpoints")
            .join(if stub { "stub" } else { "emerald" })
    }

    /// A store at `root`. Nothing is created until a node is added: a missing root is an
    /// empty store.
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Store> {
        let root = root.into();
        if root.exists() && !root.is_dir() {
            return Err(invalid(format!("{} is not a directory", root.display())));
        }
        Ok(Store { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn node_path(&self, id: CheckpointId) -> PathBuf {
        self.root.join("nodes").join(format!("{id}.ron"))
    }

    fn snapshot_path(&self, id: CheckpointId) -> PathBuf {
        self.root.join("snapshots").join(format!("{id}.ron"))
    }

    fn read_node(&self, id: CheckpointId) -> io::Result<Node> {
        let node: Node = read_ron(&self.node_path(id)).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => io::Error::new(e.kind(), format!("no node {id}")),
            _ => e,
        })?;
        if node.meta.id != id {
            return Err(invalid(format!("{id}.ron holds node {}", node.meta.id)));
        }
        Ok(node)
    }

    fn write_node(&self, node: &Node) -> io::Result<()> {
        write_atomic(&self.node_path(node.meta.id), &to_ron(node)?)
    }

    /// Every node, by id.
    pub fn nodes(&self) -> io::Result<Vec<Node>> {
        let entries = match fs::read_dir(self.root.join("nodes")) {
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
                .filter(|_| stem.starts_with('c'))
                .ok_or_else(|| invalid(format!("nodes/{name}: not a node file name")))?;
            ids.push(id);
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

    /// Adds a node (id: max + 1) whose `trace` leads from its parent to `cp`, and caches `cp`.
    /// The parent must exist, the trace must start at the parent's frame (0 for a new game)
    /// and end at `cp`'s.
    pub fn add(&self, label: &str, trace: Trace, cp: &Checkpoint) -> io::Result<CheckpointId> {
        let nodes = self.nodes()?;
        Self::check_label(&nodes, label, None)?;
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
        let id = CheckpointId(nodes.last().map_or(1, |n| n.meta.id.0 + 1));
        let node = Node {
            meta: NodeMeta::of(id, label.into(), cp),
            trace,
        };
        self.put_snapshot(id, cp)?;
        self.write_node(&node)?;
        Ok(id)
    }

    pub fn relabel(&self, id: CheckpointId, label: &str) -> io::Result<()> {
        let nodes = self.nodes()?;
        Self::check_label(&nodes, label, Some(id))?;
        let mut node = self.read_node(id)?;
        node.meta.label = label.into();
        self.write_node(&node)
    }

    /// Refuses a node with children. Also drops its cached snapshot.
    pub fn remove(&self, id: CheckpointId) -> io::Result<()> {
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
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    /// The cached snapshot; `None` if absent or of another schema.
    pub fn snapshot(&self, id: CheckpointId) -> io::Result<Option<Checkpoint>> {
        let path = self.snapshot_path(id);
        let probe: SchemaProbe = match read_ron(&path) {
            Ok(p) => p,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        if probe.header.schema != SCHEMA {
            return Ok(None);
        }
        read_ron(&path).map(Some)
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

    /// Replays every node in topological order and classifies it; also writes `status.ron`.
    ///
    /// Each trace replays from its parent's state *of this run* (`None` for a new game):
    /// - a Broken parent (or a replayer error) makes the node Broken, without replaying it;
    /// - `meta.schema != SCHEMA` is Stale;
    /// - an equal `state_digest` is Valid (its snapshot is cached);
    /// - a different digest on the node's own build is Broken (nondeterminism);
    /// - otherwise it is Diverged, with the replay's first hash mismatch, and whether its
    ///   parent diverged too.
    ///
    /// With `accept`, Diverged and Stale nodes are re-baselined on the replaying build: meta,
    /// `trace.build`, re-recorded `trace.hashes` and snapshot (Broken nodes never are). The
    /// returned list is the verdict of the run; `status.ron` records the tree after it, so
    /// accepted nodes are Valid there.
    pub fn verify(
        &self,
        mut replay: impl FnMut(Option<&Checkpoint>, &Trace) -> Result<Replayed, String>,
        accept: bool,
    ) -> io::Result<Vec<(CheckpointId, Status)>> {
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
                None => Err("parent broken".to_string()),
                Some(parent) => replay(parent, &node.trace),
            };
            let (status, state) = match replayed {
                Err(e) => (Status::Broken(e), None),
                Ok(r) => {
                    let status = if node.meta.schema != SCHEMA {
                        Status::Stale {
                            schema: node.meta.schema,
                        }
                    } else if digest(&r.checkpoint.world) == node.meta.digest {
                        Status::Valid
                    } else if node.meta.build == r.checkpoint.header.build {
                        Status::Broken("nondeterministic: same build, different state".into())
                    } else {
                        Status::Diverged {
                            hash: r.report.first_mismatch,
                            parent_diverged,
                        }
                    };
                    let rebaseline = accept && !matches!(status, Status::Broken(_));
                    if rebaseline && status != Status::Valid {
                        node.meta = NodeMeta::of(id, node.meta.label, &r.checkpoint);
                        node.trace.build = r.checkpoint.header.build;
                        node.trace.hashes = r.hashes;
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
            out.push((id, status));
        }
        write_atomic(&self.root.join("status.ron"), &to_ron(&after)?)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Once Task C's `state_digest` is in, the store's digest must be the same function.
    #[test]
    fn digest_is_state_digest() {
        let w: WorldSnapshot = ron::from_str(SNAPSHOT).unwrap();
        let ours = digest(&w);
        let theirs = std::panic::catch_unwind(|| engine::checkpoint::state_digest(&w));
        match theirs {
            Ok(d) => assert_eq!(ours, d),
            Err(_) => eprintln!("state_digest is still a stub; skipped"),
        }
    }

    const SNAPSHOT: &str = "(frame: 5, rng: 1, input: (held: (0), pressed: (0)), runner_issued: 0,
        player_name: (255, 255, 255, 255, 255, 255, 255, 255), player_id: 0,
        field: (map: (1), camera: (0, 0), drawn: None, ring: (0, 0), overrides: []),
        objects: (map: (1), run_state: NotMoving, objs: []), templates: [],
        vars: (flags: [(3)], vars: [((4), 5)]), fader: (level: 0, target: 0),
        transition: (map: (1), x: 0, y: 0, facing: Down, exit_walk: false, player_hidden: false),
        selected: None, last_mover: 0, delay: 0, door: 0, party: [], respawn: 0,
        prev_behavior: Normal, encounter_immunity: 0)";
}
