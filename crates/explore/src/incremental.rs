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
//! is under it: everything is revalidated. A changed trainer script also changes the maps
//! whose trainers run it (sight reads it unstarted), and a node whose engine events were
//! dropped is under any change.
//!
//! The touches stay true after a revalidation: every node it checks (replayed or repaired)
//! gets the touches of that replay or repair ([`TouchLog`], `Db::set_status_and_snapshots`).

use crate::db::{Db, DbError};
use crate::model::{NodeId, NodeStatus, SimSummary, Touch};
use engine::World;
use engine::checkpoint::{BuildHash, ENGINE_SRC_HASH};
use engine::data::{GameData, MapId, ScriptId};
use engine::world::Event;
use std::collections::{BTreeMap, BTreeSet};

/// A step's `node_touch` rows: the maps it entered or ended on, the scripts it started, and
/// [`Touch::EventsDropped`] when engine events were dropped (the lists may be incomplete).
pub fn touches(summary: &SimSummary, end_map: MapId) -> Vec<Touch> {
    let maps: BTreeSet<MapId> = summary.maps.iter().copied().chain([end_map]).collect();
    let mut t: Vec<Touch> = maps
        .into_iter()
        .map(Touch::Map)
        .chain(summary.scripts.iter().map(|&s| Touch::Script(s)))
        .collect();
    if summary.events_dropped > 0 {
        t.push(Touch::EventsDropped);
    }
    t
}

/// The touches of a replay, frame by frame (`World::events()`), as [`touches`] gives them for
/// an expansion: the revalidation rewrites a checked node's `node_touch` with them.
pub struct TouchLog {
    summary: SimSummary,
    dropped_start: u32,
    dropped_end: u32,
}

impl TouchLog {
    /// Before the first frame of the replay, on its start world.
    pub fn new(w: &World) -> TouchLog {
        TouchLog {
            summary: SimSummary::default(),
            dropped_start: w.events_dropped(),
            dropped_end: w.events_dropped(),
        }
    }

    /// After each frame.
    pub fn after(&mut self, w: &World) {
        for &e in w.events() {
            match e {
                Event::MapEntered(m) => {
                    if self.summary.maps.last() != Some(&m) {
                        self.summary.maps.push(m);
                    }
                }
                Event::ScriptStarted(s) => {
                    self.summary.scripts.insert(s);
                }
                _ => {}
            }
        }
        self.dropped_end = w.events_dropped();
    }

    /// The touches, with the map the replay ended on.
    pub fn finish(mut self, end_map: MapId) -> Vec<Touch> {
        self.summary.events_dropped = self.dropped_end.wrapping_sub(self.dropped_start);
        touches(&self.summary, end_map)
    }
}

/// Per script, the maps with an object that runs it and has a `trainer_type`: trainer sight
/// (`engine::world::sight`) reads the script's trainer and battle kind on those maps without
/// starting it, so a node there may depend on the script while touching only the map.
pub fn sighting_maps<G: GameData>(game: &G, maps: &[MapId]) -> BTreeMap<ScriptId, BTreeSet<MapId>> {
    let mut out: BTreeMap<ScriptId, BTreeSet<MapId>> = BTreeMap::new();
    for &m in maps {
        for o in game.map(m).objects {
            if let (Some(s), true) = (o.script, o.trainer_type != 0) {
                out.entry(s).or_default().insert(m);
            }
        }
    }
    out
}

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
///
/// Besides the nodes that started a changed script or entered or ended on a changed map:
/// - a changed trainer script changes every map where an object runs it with a
///   `trainer_type` ([`sighting_maps`]: sight reads it without starting it);
/// - a node whose events were dropped ([`Touch::EventsDropped`]) is under any change.
pub fn mark<G: GameData>(
    db: &mut Db,
    game: &G,
    new: &Digests,
    build: BuildHash,
    root_map: MapId,
) -> Result<Option<Marked>, DbError> {
    let Some(old) = db.meta(META_KEY)?.and_then(|t| Digests::decode(&t)) else {
        return Ok(None);
    };
    let Some(mut changed) = changes(&old, new) else {
        return Ok(None);
    };
    if !changed.scripts.is_empty() {
        let ids: Vec<MapId> = new.maps.iter().map(|m| m.0).collect();
        let sight = sighting_maps(game, &ids);
        for s in &changed.scripts {
            changed
                .maps
                .extend(sight.get(s).into_iter().flatten().copied());
        }
    }
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
    if !changed.scripts.is_empty() || !changed.maps.is_empty() {
        touched.extend(db.touching(Touch::EventsDropped)?);
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
