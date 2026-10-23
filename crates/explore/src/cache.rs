//! Snapshot cache and state reach by replay (Task C).

use crate::db::{Db, DbError};
use crate::model::{NodeId, NodeRow};
use crate::runs;
use engine::checkpoint::{BuildHash, Checkpoint, build_hash, state_digest};
use engine::data::GameData;
use game_native::Session;
use std::collections::BTreeMap;
use std::sync::Arc;

/// In-memory LRU (by tick) of node end states, for one build: [`reach`] clears it when the
/// game's build differs from the one it was filled on.
pub struct SnapCache {
    cap: usize,
    /// The build the states were captured on (`None` until the first `reach`).
    build: Option<BuildHash>,
    tick: u64,
    map: BTreeMap<NodeId, (Arc<Checkpoint>, u64)>,
    /// tick -> id, the eviction order.
    order: BTreeMap<u64, NodeId>,
    hits: u64,
    misses: u64,
    /// Where the db's root node starts (`None`: a new game; a `--from` db's checkpoint).
    root: Option<Arc<Checkpoint>>,
}

impl SnapCache {
    /// A cache of at most `cap` states (0 keeps nothing).
    pub fn new(cap: usize) -> Self {
        SnapCache {
            cap,
            build: None,
            tick: 0,
            map: BTreeMap::new(),
            order: BTreeMap::new(),
            hits: 0,
            misses: 0,
            root: None,
        }
    }

    /// The root node's start state for [`reach`] (`None`: a new game).
    pub fn set_root(&mut self, root: Option<Arc<Checkpoint>>) {
        self.root = root;
    }

    pub fn root(&self) -> Option<&Arc<Checkpoint>> {
        self.root.as_ref()
    }

    fn touch(&mut self, id: NodeId) -> Option<Arc<Checkpoint>> {
        let (cp, t) = self.map.get_mut(&id)?;
        self.order.remove(t);
        self.tick += 1;
        *t = self.tick;
        self.order.insert(self.tick, id);
        Some(cp.clone())
    }

    /// Node `id`'s state, now the most recently used (counts a hit or a miss).
    pub fn get(&mut self, id: NodeId) -> Option<Arc<Checkpoint>> {
        let r = self.touch(id);
        if r.is_some() {
            self.hits += 1;
        } else {
            self.misses += 1;
        }
        r
    }

    /// Whether `id` is cached (no recency or hit change).
    pub fn contains(&self, id: NodeId) -> bool {
        self.map.contains_key(&id)
    }

    /// Inserts or replaces `id`, evicting the least recently used beyond `cap`.
    pub fn put(&mut self, id: NodeId, cp: Arc<Checkpoint>) {
        if self.cap == 0 {
            return;
        }
        if let Some((_, t)) = self.map.remove(&id) {
            self.order.remove(&t);
        }
        self.tick += 1;
        self.map.insert(id, (cp, self.tick));
        self.order.insert(self.tick, id);
        while self.map.len() > self.cap {
            let Some((_, old)) = self.order.pop_first() else {
                break;
            };
            self.map.remove(&old);
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// `(hits, misses)` of [`SnapCache::get`] (report-only).
    pub fn hit_stats(&self) -> (u64, u64) {
        (self.hits, self.misses)
    }

    /// Drops every state (after a build change).
    pub fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }

    /// Ties the cache to `build`, dropping every state of another build.
    pub fn set_build(&mut self, build: BuildHash) {
        if self.build != Some(build) {
            self.clear();
            self.build = Some(build);
        }
    }
}

#[derive(Debug)]
pub enum ReachError {
    Db(DbError),
    Missing(NodeId),
    Restore(String),
    /// Replaying `node`'s inputs did not reach its digest; `got` is `None` when the end frame
    /// was not capturable.
    Diverged {
        node: NodeId,
        want: u64,
        got: Option<u64>,
    },
}

impl std::fmt::Display for ReachError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReachError::Db(e) => write!(f, "{e}"),
            ReachError::Missing(id) => write!(f, "node {id} is not in the db"),
            ReachError::Restore(e) => write!(f, "restore: {e}"),
            ReachError::Diverged {
                node,
                want,
                got: Some(got),
            } => write!(
                f,
                "node {node} replays to digest {got:#018x}, want {want:#018x}"
            ),
            ReachError::Diverged {
                node,
                want,
                got: None,
            } => write!(
                f,
                "node {node} replays to a frame that is not capturable, want {want:#018x}"
            ),
        }
    }
}

impl std::error::Error for ReachError {}

impl From<DbError> for ReachError {
    fn from(e: DbError) -> Self {
        ReachError::Db(e)
    }
}

/// Steps `n`'s inputs on `s` and captures the end state, which must be capturable with
/// `n.digest`.
pub(crate) fn replay_node<G: GameData>(
    s: &mut Session<G>,
    n: &NodeRow,
) -> Result<Checkpoint, ReachError> {
    replay_node_observed(s, n, &mut |_| {})
}

/// [`replay_node`], calling `after` on the world after each frame.
pub(crate) fn replay_node_observed<G: GameData>(
    s: &mut Session<G>,
    n: &NodeRow,
    after: &mut dyn FnMut(&engine::World),
) -> Result<Checkpoint, ReachError> {
    for k in runs::keys(&n.inputs) {
        s.step(k);
        after(s.world());
    }
    let diverged = |got| ReachError::Diverged {
        node: n.id,
        want: n.digest,
        got,
    };
    let cp = s.checkpoint().map_err(|_| diverged(None))?;
    let got = state_digest(&cp.world);
    if got != n.digest {
        return Err(diverged(Some(got)));
    }
    Ok(cp)
}

/// A session at `start` (a new game for `None`).
pub(crate) fn session_at<G: GameData + Clone>(
    game: &G,
    start: Option<&Checkpoint>,
) -> Result<Session<G>, ReachError> {
    match start {
        None => Ok(Session::new(game.clone())),
        Some(cp) => {
            Session::restore(game.clone(), cp).map_err(|e| ReachError::Restore(format!("{e:?}")))
        }
    }
}

/// A db snapshot of `n`, trusted only on `build` and with `n`'s digest.
fn db_snapshot(db: &Db, n: &NodeRow, build: BuildHash) -> Result<Option<Checkpoint>, DbError> {
    Ok(db
        .snapshot(n.id, build)?
        .filter(|cp| state_digest(&cp.world) == n.digest))
}

/// Node `id`'s end state: the nearest ancestor in the cache, then in the db (same build), else
/// the root's start (the cache's [`SnapCache::set_root`], a new game by default); replays the
/// inputs (loaded from the db: they are not resident) down the lineage, checks each digest,
/// caches the result
/// (and every state replayed on the way). Counts one cache hit or one miss per call. The
/// cache is cleared first if it holds states of another build.
pub fn reach<G: GameData + Clone>(
    game: &G,
    db: &Db,
    cache: &mut SnapCache,
    id: NodeId,
) -> Result<Arc<Checkpoint>, ReachError> {
    let build = build_hash(game);
    cache.set_build(build);
    if cache.contains(id) {
        return Ok(cache.get(id).expect("contained"));
    }
    cache.misses += 1;
    let lineage = db.lineage(id)?;
    if lineage.last().map(|n| n.id) != Some(id) {
        return Err(ReachError::Missing(id));
    }
    // (index of the deepest node whose state we have, that state)
    let mut start: Option<(usize, Arc<Checkpoint>)> = None;
    for (i, n) in lineage.iter().enumerate().rev() {
        if let Some(cp) = cache.touch(n.id) {
            start = Some((i, cp));
            break;
        }
        if let Some(cp) = db_snapshot(db, n, build)? {
            let cp = Arc::new(cp);
            cache.put(n.id, cp.clone());
            start = Some((i, cp));
            break;
        }
    }
    let (from, start_cp) = match start {
        Some((i, cp)) if i + 1 == lineage.len() => return Ok(cp),
        Some((i, cp)) => (i + 1, Some(cp)),
        None => (0, cache.root.clone()),
    };
    let mut s = session_at(game, start_cp.as_deref())?;
    let mut last = None;
    for n in &lineage[from..] {
        let cp = Arc::new(replay_node(&mut s, n)?);
        cache.put(n.id, cp.clone());
        last = Some(cp);
    }
    Ok(last.expect("the lineage below the start is not empty"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A capturable state (the LRU never looks inside).
    fn cp() -> Arc<Checkpoint> {
        let mut s = Session::new(content::Emerald::for_shells());
        for _ in 0..3000 {
            if let Ok(cp) = s.checkpoint() {
                return Arc::new(cp);
            }
            s.step(engine::Buttons::NONE);
        }
        panic!("no capturable frame")
    }

    #[test]
    fn lru_evicts_the_least_recently_used() {
        let a = cp();
        let mut c = SnapCache::new(2);
        c.put(1, a.clone());
        c.put(2, a.clone());
        assert!(c.get(1).is_some()); // 1 is now newer than 2
        c.put(3, a.clone());
        assert!(c.contains(1) && !c.contains(2) && c.contains(3));
        c.put(1, a.clone()); // a replacement keeps the size
        assert_eq!(c.len(), 2);
        assert!(c.get(2).is_none());
        assert_eq!(c.hit_stats(), (1, 1));
        let mut z = SnapCache::new(0);
        z.put(1, a);
        assert!(z.is_empty());
    }
}
