//! The spine and the checkpoint store (`docs/plans/2026-10-23-explorer-spine.md` §2.4): the
//! spine's labelled positions export as chained local nodes `spine:<act>:<label>` (promotable
//! with `cargo xtask checkpoints promote`, parents first; Emerald's are committed up to
//! `spine:act1:badge1`), and compare with any committed `<act>:<label>` node of the same label.

use crate::findings;
use crate::goals::{self, Act};
use crate::spine::SpineRun;
use anyhow::{anyhow, bail};
use engine::checkpoint::{Checkpoint, state_digest};
use engine::data::GameData;
use game_native::store::{Status, Store, StoreError};
use game_native::trace::{CheckpointId, Trace};

/// The local node label of a spine checkpoint.
pub fn spine_label(act: Act, label: &str) -> String {
    format!("spine:{}:{label}", act.name())
}

/// The trace to the spine position labelled `label` (hashes by replay) from `from`, the node
/// and state of an earlier spine checkpoint and its spine frame (`None`: a new game), and the
/// check that it replays to that position's state.
pub fn position_trace<G: GameData + Clone>(
    game: &G,
    spine: &SpineRun,
    from: Option<(CheckpointId, &Checkpoint, u64)>,
    label: &str,
) -> anyhow::Result<Trace> {
    let p = spine
        .labelled(label)
        .ok_or_else(|| anyhow!("the spine has no checkpoint {label:?}"))?;
    let start = from.map_or(0, |(_, _, f)| f);
    let Some(len) = p.frame.checked_sub(start) else {
        bail!("{label}: frame {} is before its parent's {start}", p.frame);
    };
    let tail = crate::runs::split_at(&spine.inputs, start).1;
    let segment = crate::runs::split_at(&tail, len).0;
    let from = from.map(|(id, cp, _)| (id, cp));
    let trace = findings::repro_trace_from(game, from, &[], &segment)
        .map_err(|(_, at)| anyhow!("{label}: the replay panics at frame {at}"))?;
    let end = findings::end_state_from(game, from, &trace).map_err(|e| anyhow!("{label}: {e}"))?;
    if state_digest(&end.world) != state_digest(&p.end.world) {
        bail!("{label}: the replay ends in another state than the spine's position");
    }
    Ok(trace)
}

/// Exports the spine's labelled positions to `store`, a local store over the committed tree, as
/// `spine:<act>:<label>`, chained (§2.4): the first from a new game, each next one from the one
/// before, so they promote in order (`cargo xtask checkpoints promote`, parents first). With
/// `only`, the chain up to that label. A label already there with the same state is kept and
/// the chain goes on from it; with another state it is an error (remove the local node first).
/// A label past a `Step::Bypass` fails: inputs do not reproduce the spine there. Returns
/// `(label, id, new)` per checkpoint.
pub fn export<G: GameData + Clone>(
    game: &G,
    spine: &SpineRun,
    act: Act,
    store: &Store,
    only: Option<&str>,
) -> anyhow::Result<Vec<(String, String, bool)>> {
    if store.base().is_none() {
        bail!(
            "{} is not a local store over the committed tree: exports go to the local store \
             (`cargo xtask checkpoints promote` moves them into the committed tree)",
            store.root().display()
        );
    }
    let labels = spine.labels();
    if let Some(o) = only
        && !labels.contains(&o)
    {
        bail!("the spine has no checkpoint {o:?} (it has {labels:?})");
    }
    let mut out = Vec::new();
    // The chain so far: the last node, its state and its spine frame.
    let mut parent: Option<(CheckpointId, &Checkpoint, u64)> = None;
    for label in labels {
        let p = spine.labelled(label).expect("a label of the spine");
        let name = spine_label(act, label);
        let id = match store.find(&name) {
            Ok(n) => {
                let same = store
                    .snapshot(n.meta.id)?
                    .is_some_and(|cp| state_digest(&cp.world) == state_digest(&p.end.world));
                if !same {
                    bail!(
                        "{name} is already in {} ({}) with another state: remove it first",
                        store.root().display(),
                        n.meta.id
                    );
                }
                out.push((name, n.meta.id.to_string(), false));
                n.meta.id
            }
            Err(StoreError::NotFound(_)) => {
                let trace = position_trace(game, spine, parent, label)?;
                let id = store.add(&name, trace, &p.end)?;
                store.merge_statuses(&[(id, Status::Valid)])?;
                out.push((name, id.to_string(), true));
                id
            }
            Err(e) => return Err(e.into()),
        };
        parent = Some((id, &p.end, p.frame));
        if only == Some(label) {
            break;
        }
    }
    Ok(out)
}

/// A spine checkpoint against the committed node of the same label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shared {
    pub label: String,
    /// The committed node's label and id.
    pub node: String,
    /// Same map and same goal progress.
    pub equal: bool,
    pub detail: String,
}

/// The spine's labelled positions that a node `<act>:<label>` of `store` shares, compared on
/// their map and goal progress (`goals::state_progress`): the routes may differ, the progress
/// must not.
pub fn compare(spine: &SpineRun, act: Act, store: &Store) -> anyhow::Result<Vec<Shared>> {
    let mut out = Vec::new();
    for label in spine.labels() {
        let name = format!("{}:{label}", act.name());
        let n = match store.find(&name) {
            Ok(n) => n,
            Err(StoreError::NotFound(_)) => continue,
            Err(e) => return Err(e.into()),
        };
        let Some(cp) = store.snapshot(n.meta.id)? else {
            out.push(Shared {
                label: label.to_string(),
                node: format!("{name} {}", n.meta.id),
                equal: false,
                detail: "no snapshot (run `cargo xtask checkpoints verify`)".into(),
            });
            continue;
        };
        let p = spine.labelled(label).expect("a label of the spine");
        let (a, b) = (&p.end.world, &cp.world);
        let (pa, pb) = (goals::state_progress(act, a), goals::state_progress(act, b));
        let equal = a.field.map == b.field.map && pa == pb;
        out.push(Shared {
            label: label.to_string(),
            node: format!("{name} {}", n.meta.id),
            equal,
            detail: format!(
                "spine map {:#06x} goal {pa}, node map {:#06x} goal {pb}",
                a.field.map.0, b.field.map.0
            ),
        });
    }
    Ok(out)
}
