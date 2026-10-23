//! Incremental revalidation after a content-only change (Milestone 8 Task N).
//!
//! A db records the content digests of the build its nodes were checked on (`meta.digests`,
//! [`Digests::encode`]): the engine source hash, `content::data::GLOBAL_DIGEST`, and one
//! digest per script (`SCRIPT_DIGESTS`, in `ScriptId` order) and per map (`MAP_DIGESTS`, in
//! `MAP_IDS` order). On a build change, [`changes`] compares them with this build's:
//!
//! - the engine, `GLOBAL_DIGEST` (the prelude, the battle tables, the texts and every other
//!   non-generated or non-per-map/script input), or the list of script or map names differs:
//!   everything is revalidated (`None`), as before Task N;
//! - otherwise only the scripts and maps whose digest differs changed. A script's digest
//!   covers its call-graph closure, the std scripts it calls and its override source; a map's
//!   covers its tilesets and connected maps (Tasks D1, E).
//!
//! [`mark`] then marks Stale only the valid nodes whose own or any ancestor's `node_touch`
//! names a changed script or map (scripts started, maps entered or ended on), and moves every
//! other valid or merged node (and its db snapshot) to the new build without a replay. Merged
//! nodes under a change keep their old build, so the revalidation replays them as before. When
//! the start map of the db's root (a new game's, or the `--from` node's) changed, every node
//! is under it: everything is revalidated.

use crate::db::{Db, DbError};
use crate::model::{NodeId, NodeStatus, Touch};
use engine::checkpoint::{BuildHash, ENGINE_SRC_HASH};
use engine::data::{MapId, ScriptId};
use std::collections::{BTreeMap, BTreeSet};

/// The `meta` key of a db's [`Digests`].
pub const META_KEY: &str = "digests";

/// One build's content digests (see the module doc).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Digests {
    pub engine: u64,
    pub global: u64,
    /// Per script, in `ScriptId` order: (decomp name, digest).
    pub scripts: Vec<(String, u64)>,
    /// Per map, in `MAP_IDS` order: (id, decomp name, digest).
    pub maps: Vec<(MapId, String, u64)>,
}

impl Digests {
    /// This build's (the linked `content` and engine).
    pub fn current() -> Digests {
        Digests {
            engine: ENGINE_SRC_HASH,
            global: content::data::GLOBAL_DIGEST,
            scripts: content::data::SCRIPT_DIGESTS
                .iter()
                .map(|&(n, d)| (n.to_string(), d))
                .collect(),
            maps: content::data::MAP_DIGESTS
                .iter()
                .zip(content::data::MAP_IDS.iter())
                .map(|(&(n, d), &id)| (id, n.to_string(), d))
                .collect(),
        }
    }

    /// One line per entry: `e <engine>`, `g <global>`, `s <name> <digest>`, `m <id> <name>
    /// <digest>` (hex), in order.
    pub fn encode(&self) -> String {
        let mut o = format!("e {:x}\ng {:x}\n", self.engine, self.global);
        for (n, d) in &self.scripts {
            o.push_str(&format!("s {n} {d:x}\n"));
        }
        for (id, n, d) in &self.maps {
            o.push_str(&format!("m {:x} {n} {d:x}\n", id.0));
        }
        o
    }

    /// The inverse of [`Digests::encode`]; `None` on anything else.
    pub fn decode(text: &str) -> Option<Digests> {
        let hex = |s: &str| u64::from_str_radix(s, 16).ok();
        let mut d = Digests::default();
        let (mut engine, mut global) = (None, None);
        for line in text.lines() {
            let f: Vec<&str> = line.split(' ').collect();
            match f.as_slice() {
                ["e", v] => engine = Some(hex(v)?),
                ["g", v] => global = Some(hex(v)?),
                ["s", n, v] => d.scripts.push((n.to_string(), hex(v)?)),
                ["m", id, n, v] => d.maps.push((
                    MapId(u16::from_str_radix(id, 16).ok()?),
                    n.to_string(),
                    hex(v)?,
                )),
                _ => return None,
            }
        }
        d.engine = engine?;
        d.global = global?;
        Some(d)
    }
}

/// What changed between two builds whose digests differ only per script or map.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Changed {
    pub scripts: BTreeSet<ScriptId>,
    pub maps: BTreeSet<MapId>,
}

/// The scripts and maps that changed from `old` to `new`, or `None` when everything must be
/// revalidated: another engine or `GLOBAL_DIGEST`, or another list of script or map names (a
/// name in one build and not the other, or the same names in another order, which renumbers
/// the ids).
pub fn changes(old: &Digests, new: &Digests) -> Option<Changed> {
    if old.engine != new.engine || old.global != new.global {
        return None;
    }
    let same_names = old.scripts.len() == new.scripts.len()
        && old
            .scripts
            .iter()
            .zip(&new.scripts)
            .all(|(a, b)| a.0 == b.0)
        && old.maps.len() == new.maps.len()
        && old
            .maps
            .iter()
            .zip(&new.maps)
            .all(|(a, b)| a.0 == b.0 && a.1 == b.1);
    if !same_names {
        return None;
    }
    let scripts = old
        .scripts
        .iter()
        .zip(&new.scripts)
        .enumerate()
        .filter(|(_, (a, b))| a.1 != b.1)
        .map(|(i, _)| ScriptId(i as u16))
        .collect();
    let maps = old
        .maps
        .iter()
        .zip(&new.maps)
        .filter(|(a, b)| a.2 != b.2)
        .map(|(a, _)| a.0)
        .collect();
    Some(Changed { scripts, maps })
}

/// What [`mark`] did.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Marked {
    /// Valid nodes marked Stale (under a change).
    pub stale: u64,
    /// Valid or merged nodes moved to the new build without a replay.
    pub kept: u64,
}

/// On a build change to `build` (whose digests are `new`): marks Stale only the valid nodes
/// under a changed script or map and moves the others to `build` (see the module doc).
/// `root_map` is the map the db's root starts on. `None` (nothing written) when the db has no
/// digests, or [`changes`] says everything must be revalidated: the caller marks all stale.
pub fn mark(
    db: &mut Db,
    new: &Digests,
    build: BuildHash,
    root_map: MapId,
) -> Result<Option<Marked>, DbError> {
    let Some(old) = db.meta(META_KEY)?.and_then(|t| Digests::decode(&t)) else {
        return Ok(None);
    };
    let Some(changed) = changes(&old, new) else {
        return Ok(None);
    };
    if changed.maps.contains(&root_map) {
        return Ok(None);
    }
    let mut touched: BTreeSet<NodeId> = BTreeSet::new();
    for &s in &changed.scripts {
        touched.extend(db.touching(Touch::Script(s))?);
    }
    for &m in &changed.maps {
        touched.extend(db.touching(Touch::Map(m))?);
    }
    // Down the tree, parents first (a parent is always inserted before its children).
    let rows = db.nodes_light()?;
    let mut by_depth: BTreeMap<u32, Vec<(NodeId, Option<NodeId>, NodeStatus)>> = BTreeMap::new();
    for n in &rows {
        by_depth
            .entry(n.depth)
            .or_default()
            .push((n.id, n.parent, n.status));
    }
    drop(rows);
    let mut under: BTreeSet<NodeId> = BTreeSet::new();
    let (mut stale, mut kept) = (Vec::new(), Vec::new());
    for (_, level) in by_depth {
        for (id, parent, status) in level {
            let hit = touched.contains(&id) || parent.is_some_and(|p| under.contains(&p));
            if hit {
                under.insert(id);
            }
            match (hit, status) {
                (true, NodeStatus::Valid) => stale.push(id),
                (false, NodeStatus::Valid | NodeStatus::Merged) => kept.push(id),
                _ => {}
            }
        }
    }
    db.retag(&kept, &stale, build)?;
    Ok(Some(Marked {
        stale: stale.len() as u64,
        kept: kept.len() as u64,
    }))
}
