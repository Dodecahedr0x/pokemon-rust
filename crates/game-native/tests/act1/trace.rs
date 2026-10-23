//! The flattened Act 1 trace (Milestone 8 Task P): the committed `act1:*` chain, from a new
//! game to `FLAG_BADGE01_GET`, as one list of keys. Shared by `game-native/tests/act1.rs` and
//! `emu/tests/parity.rs` (`#[path]`).

use engine::Buttons;
use game_native::store::Store;

/// The chain's labels, root first (decomp names: the goals of `explore::goals::emerald::ACT1`).
pub const LABELS: [&str; 5] = [
    "act1:pokedex",
    "act1:petalburg",
    "act1:woods",
    "act1:rustboro",
    "act1:badge1",
];

pub struct Act1 {
    /// The keys of every frame from a new game.
    pub keys: Vec<Buttons>,
    /// Each node of the chain as (label, frame), root first; the last one ends the trace.
    pub nodes: Vec<(String, u32)>,
}

impl Act1 {
    pub fn frames(&self) -> u32 {
        self.keys.len() as u32
    }

    /// The keys of step `f` (nothing past the end).
    pub fn keys_at(&self, f: u32) -> Buttons {
        self.keys.get(f as usize).copied().unwrap_or(Buttons::NONE)
    }
}

/// The committed chain ending at `act1:badge1`, or `None` on the stub build (its tree has no
/// Act 1).
pub fn load() -> Option<Act1> {
    if content::STUB {
        return None;
    }
    let store = Store::open(Store::default_root(content::FLAVOR)).expect("the committed tree");
    let index = store.index().expect("the committed tree's index");
    let last = index
        .labelled(LABELS[LABELS.len() - 1])
        .expect("no act1:badge1 node in the committed tree");
    let mut keys = Vec::new();
    let mut nodes = Vec::new();
    for id in index.chain(last.meta.id).expect("act1:badge1's chain") {
        let n = index.node(id).expect("a chain node");
        assert_eq!(
            n.trace.start_frame as usize,
            keys.len(),
            "{id} starts at another frame than its parent's"
        );
        keys.extend(n.trace.keys());
        assert_eq!(n.meta.frame as usize, keys.len(), "{id}'s frame");
        nodes.push((n.meta.label.clone(), n.meta.frame));
    }
    let labels: Vec<&str> = nodes.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(labels, LABELS, "the act1 chain");
    Some(Act1 { keys, nodes })
}
