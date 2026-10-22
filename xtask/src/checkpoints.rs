//! `cargo xtask checkpoints <list|verify|seed|label|rm|sav>` (Milestone 5 Task G).
//!
//! The store is `checkpoints/{stub|emerald}` after the content flavor (`content::STUB`), or
//! `$PKR_CHECKPOINTS`; `--root <dir>` overrides both.

use anyhow::{Context, Result, anyhow, bail};
use game_native::checkpoints::{materialize, replayer, seed};
use game_native::store::{Status, Store};
use game_native::trace::{CheckpointId, Parent};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const USAGE: &str = "usage: cargo xtask checkpoints [--root <dir>] \
<list | verify [--accept] | seed | label <id> <text> | rm <id> | sav <id> [-o <path>]>";

/// The outcome of [`verify`]: every node's status, in topological order.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct VerifySummary {
    pub nodes: Vec<(CheckpointId, Status)>,
}

impl VerifySummary {
    /// No node is `Diverged` or `Broken` (`Stale` nodes are only regenerated).
    pub fn ok(&self) -> bool {
        self.nodes
            .iter()
            .all(|(_, s)| !matches!(s, Status::Diverged { .. } | Status::Broken(_)))
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
    crate::fresh_extract()?;
    match args[..] {
        ["list"] => list(&root),
        ["verify"] => verify(&root, false).map(drop),
        ["verify", "--accept"] => verify(&root, true).map(drop),
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
        ["sav", id] => sav(&root, id, &crate::root().join("target/game.sav")),
        ["sav", id, "-o", out] => sav(&root, id, Path::new(out)),
        _ => bail!(USAGE),
    }
}

fn status_text(s: Option<&Status>) -> String {
    match s {
        None => "unverified".into(),
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

fn print_table(store: &Store, status: &BTreeMap<CheckpointId, Status>) -> Result<()> {
    println!(
        "{:<7} {:>6} {:>7}  {:<8} {:<28} status",
        "id", "frame", "map", "parent", "label"
    );
    for n in store.topo_order()? {
        let parent = match n.trace.parent {
            Parent::NewGame => "new game".to_string(),
            Parent::Checkpoint(p) => p.to_string(),
        };
        println!(
            "{:<7} {:>6} {:#07x}  {:<8} {:<28} {}",
            n.meta.id.to_string(),
            n.meta.frame,
            n.meta.map.0,
            parent,
            n.meta.label,
            status_text(status.get(&n.meta.id))
        );
    }
    Ok(())
}

/// The tree with the statuses of the last `verify` (`status.ron`).
fn list(root: &Path) -> Result<()> {
    let store = Store::open(root)?;
    let path = root.join("status.ron");
    let status: BTreeMap<CheckpointId, Status> = match std::fs::read_to_string(&path) {
        Ok(text) => ron::from_str(&text).with_context(|| format!("{}", path.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
        Err(e) => return Err(e.into()),
    };
    println!("{}", root.display());
    print_table(&store, &status)
}

/// Replays every node of the store at `root` on the current build (`accept` re-baselines
/// Diverged and Stale nodes) and prints the table. Without `accept`, a Diverged or Broken node
/// is an error (`cargo xtask parity` runs this before the ROM tests).
pub fn verify(root: &Path, accept: bool) -> Result<VerifySummary> {
    let store = Store::open(root)?;
    let game = content::Emerald::for_shells();
    let nodes = store.verify(replayer(&game), accept)?;
    let summary = VerifySummary { nodes };
    println!(
        "{}: {} nodes{}",
        root.display(),
        summary.nodes.len(),
        if accept { " (--accept)" } else { "" }
    );
    print_table(&store, &summary.nodes.iter().cloned().collect())?;
    if !summary.ok() && !accept {
        bail!(
            "checkpoint tree {} has diverged or broken nodes: replay the change with \
             `cargo xtask checkpoints verify --accept` if it is intended",
            root.display()
        );
    }
    Ok(summary)
}

/// Records the route and writes its `route:` nodes.
fn seed_route(root: &Path) -> Result<()> {
    let store = Store::open(root)?;
    let rec = game_native::scenario::record_route().map_err(anyhow::Error::msg)?;
    let ids = seed(&store, &content::Emerald::for_shells(), &rec).map_err(anyhow::Error::msg)?;
    println!("seeded {} route nodes in {}", ids.len(), root.display());
    print_table(&store, &BTreeMap::new())
}

/// Writes node `id`'s flash image (mGBA autoloads `target/game.sav` next to `target/game.gba`).
fn sav(root: &Path, id: &str, out: &Path) -> Result<()> {
    let store = Store::open(root)?;
    let id = store.find(id)?.meta.id;
    let cp = materialize(&store, &content::Emerald::for_shells(), id).map_err(|e| anyhow!(e))?;
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
    use game_native::store::Node;
    use game_native::trace::Run;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn temp_root(tag: &str) -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "pkr-xtask-cp-{}-{tag}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// A store holding the route's first node (from a new game).
    fn one_node_store(tag: &str) -> (PathBuf, Store) {
        let root = temp_root(tag);
        let store = Store::open(&root).unwrap();
        let mut rec = game_native::scenario::record_route().unwrap();
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
    fn verify_a_seeded_store_is_valid() {
        let (root, store) = one_node_store("valid");
        let s = verify(&root, false).unwrap();
        assert_eq!(s.nodes, [(CheckpointId(1), Status::Valid)]);
        assert!(s.ok());
        // `list` reads the verdicts back.
        list(&root).unwrap();
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
        let err = verify(&root, false).unwrap_err();
        assert!(err.to_string().contains("diverged"), "{err}");
        let s = verify(&root, true).unwrap();
        assert!(
            matches!(
                s.nodes[..],
                [(
                    CheckpointId(1),
                    Status::Diverged {
                        parent_diverged: false,
                        ..
                    }
                )]
            ),
            "{s:?}"
        );
        // Accepted: valid on this build from now on.
        let s = verify(&root, false).unwrap();
        assert_eq!(s.nodes, [(CheckpointId(1), Status::Valid)]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_forged_digest_on_the_same_build_is_broken() {
        let (root, _store) = one_node_store("broken");
        let mut node = read(&root);
        node.meta.digest ^= 1;
        write(&root, &node);
        let err = verify(&root, false).unwrap_err();
        assert!(err.to_string().contains("broken"), "{err}");
        let s = verify(&root, true).unwrap();
        assert!(
            matches!(&s.nodes[..], [(CheckpointId(1), Status::Broken(why))] if why.contains("nondeterministic")),
            "{s:?}"
        );
        assert!(!s.ok());
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
