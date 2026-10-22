//! `cargo xtask checkpoints <list|verify|seed|label|rm|sav|promote>` (Milestone 5 Task G;
//! `promote`: Milestone 8 Task K).
//!
//! The store is the committed tree, `checkpoints/{stub|emerald}` after the content flavor
//! (`content::STUB`); `--root <dir>` overrides it. The personal saves of `play` and the studio
//! (`local/`, `$PKR_CHECKPOINTS`) are not part of it, except for `list --local` and
//! `verify --local`, which open the local store layered over it (a local node may have a
//! committed parent), and `promote`, which moves a verified local node into it.

use anyhow::{Context, Result, anyhow, bail};
use game_native::checkpoints::{materialize, replayer, seed};
use game_native::store::{Node, Status, Store};
use game_native::trace::{CheckpointId, Parent};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const USAGE: &str = "usage: cargo xtask checkpoints [--root <dir>] \
<list [--local] | verify [--local | --accept] | seed | label <id> <text> | rm <id> | sav <id> [-o <path>] \
| promote <local id> --label <text>>";

/// The outcome of [`verify`]: every node's status, in topological order.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct VerifySummary {
    pub nodes: Vec<(CheckpointId, Status)>,
    /// The Valid nodes whose build and hashes `--accept` refreshes (or refreshed).
    pub rehash: BTreeSet<CheckpointId>,
}

impl VerifySummary {
    /// Every node is Valid ([`Status::healthy`], as the committed-tree test requires).
    pub fn ok(&self) -> bool {
        Status::healthy(self.nodes.iter().map(|(_, s)| s))
    }
}

pub fn main(args: &[String]) -> Result<()> {
    let mut args: Vec<&str> = args.iter().map(String::as_str).collect();
    let root = match args.iter().position(|&a| a == "--root") {
        Some(i) if i + 1 < args.len() => {
            let dir = PathBuf::from(args[i + 1]);
            args.drain(i..=i + 1);
            dir
        }
        Some(_) => bail!(USAGE),
        None => Store::default_root(content::STUB),
    };
    // Replaying needs the real content: a stale extract would replay the stub world.
    if matches!(args.first(), Some(&("verify" | "seed" | "sav"))) {
        crate::fresh_extract()?;
    }
    // `--local`: the local store over the committed tree (or `--root`), which stays read-only.
    let local = || Store::layered(&root, Store::local_root(content::STUB));
    match args[..] {
        ["list"] => list(&Store::open(&root)?),
        ["list", "--local"] => list(&local()?),
        ["verify"] => verify(&root, false).map(drop),
        ["verify", "--accept"] => verify(&root, true).map(drop),
        ["verify", "--local"] => verify_store(&local()?, false).map(drop),
        ["verify", "--local", "--accept"] | ["verify", "--accept", "--local"] => {
            verify_store(&local()?, true).map(drop)
        }
        ["seed"] => seed_route(&root),
        ["label", id, text] => {
            let store = Store::open(&root)?;
            let id = store.find(id)?.meta.id;
            store.relabel(id, text)?;
            println!("{id}: {text:?}");
            Ok(())
        }
        ["rm", id] => {
            let store = Store::open(&root)?;
            let id = store.find(id)?.meta.id;
            store.remove(id)?;
            println!("removed {id}");
            Ok(())
        }
        ["promote", id, "--label", label] | ["promote", "--label", label, id] => {
            let local = Store::local_root(content::STUB);
            let new = promote(&root, &local, id, label)?;
            println!(
                "{id} -> {new} {label:?} in {}: commit it (and `cargo xtask checkpoints verify`)",
                root.display()
            );
            Ok(())
        }
        ["sav", id] => sav(&root, id, &crate::root().join("target/game.sav")),
        ["sav", id, "-o", out] => sav(&root, id, Path::new(out)),
        _ => bail!(USAGE),
    }
}

fn status_text(s: Option<&Status>, rehash: bool) -> String {
    match s {
        None => "unverified".into(),
        Some(Status::Valid) if rehash => "valid (rehash)".into(),
        Some(Status::Valid) => "valid".into(),
        Some(Status::Stale { schema }) => format!("stale (schema {schema})"),
        Some(Status::Diverged {
            hash,
            parent_diverged,
        }) => {
            let mut t = "DIVERGED".to_string();
            if let Some(m) = hash {
                t += &format!(" from frame +{}", m.frame);
            }
            if *parent_diverged {
                t += " (parent diverged)";
            }
            t
        }
        Some(Status::Broken(why)) => format!("BROKEN: {why}"),
    }
}

fn print_table(
    store: &Store,
    status: &BTreeMap<CheckpointId, Status>,
    rehash: &BTreeSet<CheckpointId>,
) -> Result<()> {
    let index = store.index()?;
    println!("{} status", Node::row_header());
    for n in index.topo().iter().filter_map(|&id| index.node(id)) {
        let id = n.meta.id;
        let status = status_text(status.get(&id), rehash.contains(&id));
        println!("{} {status}", n.row());
    }
    Ok(())
}

/// Where `store` is, for messages: its root, and the base it is layered over.
fn place(store: &Store) -> String {
    match store.base() {
        Some(base) => format!("{} over {}", store.root().display(), base.display()),
        None => store.root().display().to_string(),
    }
}

/// The tree with the statuses of the last `verify` (`status.ron`).
fn list(store: &Store) -> Result<()> {
    let status = store.statuses()?;
    println!("{}", place(store));
    print_table(store, &status, &BTreeSet::new())
}

/// Replays every node of the store at `root` on the current build (`accept` re-baselines
/// Diverged and Stale nodes, and refreshes Valid ones flagged "rehash") and prints the table.
/// A Broken node is always an error; without `accept`, so is any node that is not Valid.
pub fn verify(root: &Path, accept: bool) -> Result<VerifySummary> {
    verify_store(&Store::open(root)?, accept)
}

/// [`verify`] on any store. A layered store (`--local`) is never accepted: re-baselining its
/// read-only base would fail half-way, and the committed tree is accepted on its own.
fn verify_store(store: &Store, accept: bool) -> Result<VerifySummary> {
    if accept && store.base().is_some() {
        bail!(
            "`verify --local --accept` is refused: the committed tree is read-only under the local \
             store (accept it alone with `verify --accept`)"
        );
    }
    let place = place(store);
    let summary = replay_all(store, accept)?;
    let broken = (summary.nodes.iter())
        .filter(|(_, s)| matches!(s, Status::Broken(_)))
        .count();
    if accept && broken > 0 {
        bail!("{broken} broken nodes were not accepted in {place}");
    }
    if !accept && !summary.ok() {
        bail!(
            "checkpoint tree {place} has stale, diverged or broken nodes: replay the change with \
             `cargo xtask checkpoints verify --accept` if it is intended"
        );
    }
    Ok(summary)
}

/// [`verify`] without the verdict: every node's status.
fn replay_all(store: &Store, accept: bool) -> Result<VerifySummary> {
    let game = content::Emerald::for_shells();
    let verdicts = store.verify_nodes(replayer(&game), accept)?;
    let summary = VerifySummary {
        rehash: verdicts.iter().filter(|v| v.rehash).map(|v| v.id).collect(),
        nodes: verdicts.into_iter().map(|v| (v.id, v.status)).collect(),
    };
    println!(
        "{}: {} nodes{}",
        place(store),
        summary.nodes.len(),
        if accept { " (--accept)" } else { "" }
    );
    let status = summary.nodes.iter().cloned().collect();
    print_table(store, &status, &summary.rehash)?;
    if !accept && !summary.rehash.is_empty() {
        println!(
            "{} valid nodes were recorded on another build: `verify --accept` refreshes their \
             build and hashes",
            summary.rehash.len()
        );
    }
    Ok(summary)
}

/// Moves the local node `id` (of the local store at `local` over the committed tree at
/// `committed`) into the committed tree as `label`, and returns its committed id. Refused
/// unless the node is local, its parent is committed (a new game, or a committed node: promote
/// a chain parents first), and its last verdict is Valid (`checkpoints verify --local`; an
/// `explore export --to-store` node is verified by the export). Its local children are
/// re-parented onto the committed node, so their traces and verdicts stay true.
fn promote(committed: &Path, local: &Path, id: &str, label: &str) -> Result<CheckpointId> {
    let layered = Store::layered(committed, local)?;
    let node = layered.find(id)?;
    let id = node.meta.id;
    if layered.is_base(id) {
        bail!("{id} is already in the committed tree");
    }
    if let Parent::Checkpoint(p) = node.trace.parent
        && !layered.is_base(p)
    {
        bail!("{id}'s parent {p} is local: promote it first");
    }
    match layered.statuses()?.get(&id) {
        Some(Status::Valid) => {}
        s => bail!(
            "{id} is {}: promote only a verified node (`cargo xtask checkpoints verify --local`)",
            status_text(s, false)
        ),
    }
    let game = content::Emerald::for_shells();
    let cp = match layered.snapshot(id)? {
        Some(cp) => cp,
        None => {
            materialize(&layered, &game, id)
                .map_err(|e| anyhow!(e))?
                .checkpoint
        }
    };
    if engine::checkpoint::state_digest(&cp.world) != node.meta.digest {
        bail!("{id}'s cached state is not its recorded one: verify it again");
    }
    let store = Store::open(committed)?;
    let new = store.add(label, node.trace.clone(), &cp)?;
    store.merge_statuses(&[(new, Status::Valid)])?;
    // Re-parent its local children (the same trace from the same state), then drop it.
    let own = Store::open(local)?;
    for mut child in own.nodes()? {
        if child.trace.parent == Parent::Checkpoint(id) {
            child.trace.parent = Parent::Checkpoint(new);
            let path = local.join("nodes").join(format!("{}.ron", child.meta.id));
            let text = ron::ser::to_string_pretty(&child, ron::ser::PrettyConfig::default())?;
            std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
        }
    }
    Store::layered(committed, local)?.remove(id)?;
    Ok(new)
}

/// Records the route and writes its `route:` nodes.
fn seed_route(root: &Path) -> Result<()> {
    let store = Store::open(root)?;
    let rec = game_native::scenario::record_route().map_err(anyhow::Error::msg)?;
    let ids = seed(&store, &content::Emerald::for_shells(), &rec).map_err(anyhow::Error::msg)?;
    println!("seeded {} route nodes in {}", ids.len(), root.display());
    print_table(&store, &BTreeMap::new(), &BTreeSet::new())
}

/// Writes node `id`'s flash image (mGBA autoloads `target/game.sav` next to `target/game.gba`).
fn sav(root: &Path, id: &str, out: &Path) -> Result<()> {
    let store = Store::open(root)?;
    let id = store.find(id)?.meta.id;
    let m = materialize(&store, &content::Emerald::for_shells(), id).map_err(|e| anyhow!(e))?;
    if m.diverged {
        eprintln!("warning: {id} diverged on this build");
    }
    let cp = m.checkpoint;
    let image = game_native::flash::encode_sav(&cp).map_err(|e| anyhow!(e))?;
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(out, image).with_context(|| format!("writing {}", out.display()))?;
    println!("{id} (frame {}) -> {}", cp.header.frame, out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::Buttons;
    use game_native::trace::Run;

    /// A fresh empty directory of its own; a passing test removes it.
    fn temp_root(tag: &str) -> PathBuf {
        tempfile::Builder::new()
            .prefix(&format!("pkr-xtask-{tag}-"))
            .tempdir()
            .unwrap()
            .keep()
    }

    /// A store holding the route's first node (from a new game).
    fn one_node_store(tag: &str) -> (PathBuf, Store) {
        let root = temp_root(tag);
        let store = Store::open(&root).unwrap();
        let mut rec = game_native::scenario::cached_route().clone();
        rec.nodes.truncate(1);
        seed(&store, &content::Emerald::for_shells(), &rec).unwrap();
        (root, store)
    }

    fn node_path(root: &Path) -> PathBuf {
        root.join("nodes/c00001.ron")
    }

    fn read(root: &Path) -> Node {
        ron::from_str(&std::fs::read_to_string(node_path(root)).unwrap()).unwrap()
    }

    fn write(root: &Path, n: &Node) {
        std::fs::write(node_path(root), ron::to_string(n).unwrap()).unwrap();
        // The cached snapshot would hide nothing from verify (it replays), but drop it anyway.
        let _ = std::fs::remove_file(root.join("snapshots/c00001.ron"));
    }

    /// The trace with its last frame's keys replaced by `keys`.
    fn with_last_keys(n: &Node, keys: Buttons) -> Node {
        let mut n = n.clone();
        let last = n.trace.inputs.pop().unwrap();
        if last.frames > 1 {
            n.trace.inputs.push(Run {
                keys: last.keys,
                frames: last.frames - 1,
            });
        }
        n.trace.inputs.push(Run { keys, frames: 1 });
        n
    }

    #[test]
    fn local_verifies_a_local_child_of_a_committed_node_and_never_accepts() {
        // The route's first two nodes, split: the first in the committed tree, its child local.
        let full = temp_root("split");
        let mut rec = game_native::scenario::cached_route().clone();
        rec.nodes.truncate(2);
        seed(
            &Store::open(&full).unwrap(),
            &content::Emerald::for_shells(),
            &rec,
        )
        .unwrap();
        let (base, local) = (temp_root("base"), temp_root("local"));
        for (dir, file) in [(&base, "c00001.ron"), (&local, "c00002.ron")] {
            std::fs::create_dir_all(dir.join("nodes")).unwrap();
            std::fs::copy(full.join("nodes").join(file), dir.join("nodes").join(file)).unwrap();
        }
        // Alone, the local store misses the parent.
        let alone = Store::open(&local).unwrap();
        list(&alone).unwrap_err();
        verify_store(&alone, false).unwrap_err();
        let layered = Store::layered(&base, &local).unwrap();
        let s = verify_store(&layered, false).unwrap();
        assert_eq!(
            s.nodes,
            [
                (CheckpointId(1), Status::Valid),
                (CheckpointId(2), Status::Valid)
            ]
        );
        list(&layered).unwrap();
        assert!(local.join("status.ron").exists());
        assert!(!base.join("status.ron").exists(), "the base is not written");
        assert!(!base.join("snapshots").exists(), "the base is not written");
        let err = verify_store(&layered, true).unwrap_err();
        assert!(err.to_string().contains("refused"), "{err}");
        for dir in [full, base, local] {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    /// The route's first three nodes: the first committed (`c00001`), the next two local, as
    /// a chain `c10001` -> `c10002` (local ids).
    fn committed_and_local_chain(tag: &str) -> (PathBuf, PathBuf) {
        let full = temp_root(&format!("{tag}-full"));
        let mut rec = game_native::scenario::cached_route().clone();
        rec.nodes.truncate(3);
        seed(
            &Store::open(&full).unwrap(),
            &content::Emerald::for_shells(),
            &rec,
        )
        .unwrap();
        let (base, local) = (temp_root(&format!("{tag}-base")), temp_root(tag));
        std::fs::create_dir_all(base.join("nodes")).unwrap();
        std::fs::create_dir_all(local.join("nodes")).unwrap();
        let file = |id: u32| format!("c{id:05}.ron");
        std::fs::copy(
            full.join("nodes").join(file(1)),
            base.join("nodes").join(file(1)),
        )
        .unwrap();
        for (old, new) in [(2, 10_001), (3, 10_002)] {
            let text = std::fs::read_to_string(full.join("nodes").join(file(old))).unwrap();
            let mut n: Node = ron::from_str(&text).unwrap();
            n.meta.id = CheckpointId(new);
            if n.trace.parent == Parent::Checkpoint(CheckpointId(2)) {
                n.trace.parent = Parent::Checkpoint(CheckpointId(10_001));
            }
            let text = ron::to_string(&n).unwrap();
            std::fs::write(local.join("nodes").join(file(new)), text).unwrap();
        }
        let _ = std::fs::remove_dir_all(full);
        (base, local)
    }

    #[test]
    fn promote_refuses_unverified_committed_and_orphaning_nodes() {
        let (base, local) = committed_and_local_chain("promote-refused");
        let err = |id: &str| promote(&base, &local, id, "x").unwrap_err().to_string();
        let e = err("c10001");
        assert!(e.contains("unverified"), "{e}");
        let e = err("c00001");
        assert!(e.contains("already in the committed tree"), "{e}");
        let e = err("c10002");
        assert!(e.contains("parent c10001 is local"), "{e}");
        // A node whose last verdict is not Valid.
        let layered = Store::layered(&base, &local).unwrap();
        layered
            .merge_statuses(&[(CheckpointId(10_001), Status::Broken("test".into()))])
            .unwrap();
        let e = err("c10001");
        assert!(e.contains("BROKEN"), "{e}");
        assert_eq!(Store::open(&base).unwrap().nodes().unwrap().len(), 1);
        assert_eq!(Store::open(&local).unwrap().nodes().unwrap().len(), 2);
        for dir in [base, local] {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn promote_moves_a_verified_node_and_reparents_its_children() {
        let (base, local) = committed_and_local_chain("promote");
        let layered = Store::layered(&base, &local).unwrap();
        verify_store(&layered, false).unwrap();
        let digest = layered.find("c10001").unwrap().meta.digest;
        let new = promote(&base, &local, "c10001", "promoted").unwrap();
        assert_eq!(new, CheckpointId(2));
        let committed = Store::open(&base).unwrap();
        let n = committed.find("promoted").unwrap();
        assert_eq!((n.meta.id, n.meta.digest), (new, digest));
        assert_eq!(n.trace.parent, Parent::Checkpoint(CheckpointId(1)));
        // The committed tree verifies on its own; the local child now hangs from it.
        assert!(verify(&base, false).unwrap().ok());
        let own = Store::open(&local).unwrap().nodes().unwrap();
        assert_eq!(own.len(), 1);
        assert_eq!(own[0].trace.parent, Parent::Checkpoint(new));
        let layered = Store::layered(&base, &local).unwrap();
        let s = verify_store(&layered, false).unwrap();
        assert_eq!(s.nodes.len(), 3);
        // The label is the committed tree's: a second promotion with it is refused.
        let e = promote(&base, &local, "c10002", "promoted").unwrap_err();
        assert!(e.to_string().contains("label"), "{e}");
        for dir in [base, local] {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn verify_a_seeded_store_is_valid() {
        let (root, store) = one_node_store("valid");
        let s = verify(&root, false).unwrap();
        assert_eq!(s.nodes, [(CheckpointId(1), Status::Valid)]);
        assert!(s.ok());
        // `list` reads the verdicts back.
        list(&store).unwrap();
        assert_eq!(store.nodes().unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_flipped_input_recorded_on_another_build_is_diverged() {
        let (root, _store) = one_node_store("diverged");
        let node = read(&root);
        let game = content::Emerald::for_shells();
        // A last-frame key that changes the end state and keeps it capturable.
        let mut replay = replayer(&game);
        let flipped = [Buttons::LEFT, Buttons::RIGHT, Buttons::UP, Buttons::DOWN]
            .into_iter()
            .map(|k| with_last_keys(&node, k))
            .find(|n| {
                replay(None, &n.trace).is_ok_and(|r| {
                    engine::checkpoint::state_digest(&r.checkpoint.world) != node.meta.digest
                })
            })
            .expect("some direction changes the end state");
        let mut old = flipped.clone();
        old.meta.build.engine ^= 1; // recorded on another build
        write(&root, &old);
        let diverged = |s: &VerifySummary| {
            matches!(
                s.nodes[..],
                [(
                    CheckpointId(1),
                    Status::Diverged {
                        hash: Some(_),
                        parent_diverged: false,
                    }
                )]
            )
        };
        let s = replay_all(&Store::open(&root).unwrap(), false).unwrap();
        assert!(diverged(&s), "{s:?}");
        assert!(!s.ok());
        verify(&root, false).unwrap_err();
        let s = verify(&root, true).unwrap();
        assert!(diverged(&s), "the run's verdict: {s:?}");
        // Accepted: re-baselined on this build, so a replay reproduces every recorded hash.
        let accepted = read(&root);
        assert_eq!(accepted.trace.inputs, flipped.trace.inputs);
        let r = replay(None, &accepted.trace).unwrap();
        assert_eq!(r.report.first_mismatch, None);
        assert_eq!(
            engine::checkpoint::state_digest(&r.checkpoint.world),
            accepted.meta.digest
        );
        let s = verify(&root, false).unwrap();
        assert_eq!(s.nodes, [(CheckpointId(1), Status::Valid)]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_valid_node_of_another_build_shows_rehash_and_accept_refreshes_it() {
        let (root, _store) = one_node_store("rehash");
        let fresh = read(&root);
        let mut old = fresh.clone();
        old.meta.build.engine ^= 1; // recorded on another build, with other hashes
        old.trace.build = old.meta.build;
        old.meta.world_hash ^= 1;
        old.trace.hashes[0].1 ^= 1;
        write(&root, &old);
        let s = verify(&root, false).unwrap();
        assert_eq!(s.nodes, [(CheckpointId(1), Status::Valid)]);
        assert_eq!(s.rehash, BTreeSet::from([CheckpointId(1)]));
        assert!(s.ok());
        assert_eq!(status_text(Some(&Status::Valid), true), "valid (rehash)");
        assert_eq!(read(&root), old, "left as is without --accept");
        verify(&root, true).unwrap();
        assert_eq!(read(&root), fresh, "refreshed to this build");
        let s = verify(&root, false).unwrap();
        assert!(s.rehash.is_empty() && s.ok(), "{s:?}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_forged_digest_on_the_same_build_is_broken() {
        let (root, _store) = one_node_store("broken");
        let mut node = read(&root);
        node.meta.digest ^= 1;
        write(&root, &node);
        let broken = |s: &VerifySummary| {
            matches!(&s.nodes[..], [(CheckpointId(1), Status::Broken(why))]
                if why.contains("nondeterministic"))
        };
        let s = replay_all(&Store::open(&root).unwrap(), false).unwrap();
        assert!(broken(&s), "{s:?}");
        verify(&root, false).unwrap_err();
        // `--accept` never accepts a Broken node, and fails.
        let err = verify(&root, true).unwrap_err();
        assert!(
            err.to_string().contains("1 broken nodes were not accepted"),
            "{err}"
        );
        assert_eq!(read(&root), node, "the node is left as it was");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sav_writes_a_flash_image_of_the_node() {
        let (root, _store) = one_node_store("sav");
        let out = root.join("game.sav");
        sav(&root, "route:", &out).unwrap_err();
        let label = read(&root).meta.label;
        sav(&root, &label, &out).unwrap();
        let (meta, cp) = game_native::flash::decode_sav(&std::fs::read(&out).unwrap()).unwrap();
        assert_eq!(meta.frame, read(&root).meta.frame);
        assert_eq!(
            engine::checkpoint::state_digest(&cp.world),
            read(&root).meta.digest
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
