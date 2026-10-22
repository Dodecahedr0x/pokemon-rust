//! Task C test helpers: node rows and a hand-built lineage (plain `Session`, no Task A code).
#![allow(dead_code)]

use content::Emerald;
use engine::Buttons;
use engine::checkpoint::{BuildHash, Checkpoint, state_digest};
use engine::data::MapId;
use explore::{AbstractKey, Fitness, Macro, MacroKind, NodeId, NodeRow, NodeStatus, Opts};
use game_native::Session;
use game_native::trace::rle;

pub fn walk(x: i16, y: i16) -> Macro {
    Macro {
        kind: MacroKind::WalkTo { x, y },
        opts: Opts::default(),
    }
}

/// A synthetic row (no game).
pub fn row(id: NodeId, parent: Option<NodeId>, key: u64, build: BuildHash) -> NodeRow {
    NodeRow {
        id,
        parent,
        depth: parent.map_or(0, |_| 1),
        macro_: walk(id as i16, 2),
        inputs: rle([Buttons::NONE, Buttons::A, Buttons::A]),
        frames: 3,
        total_frames: 3 * id as u64,
        end_frame: 100 + id as u32,
        key: AbstractKey(key),
        digest: u64::MAX - id as u64,
        world_hash: 1 << 63 | id as u64,
        map: MapId(1),
        pos: (id as i16, -1),
        progress: id as u32,
        fitness: Fitness {
            novelty: 10,
            progress: -2,
            bug: 0,
            balance: 0,
            total: 8,
        },
        generation: 1,
        expanded: 0,
        status: NodeStatus::Valid,
        merged_into: None,
        build,
    }
}

/// Steps NONE until capturable; after 300 frames alternates A and NONE (text). Panics after
/// 6000 frames.
fn settle(s: &mut Session<Emerald>, keys: &mut Vec<Buttons>) -> Checkpoint {
    for i in 0..6000u32 {
        if let Ok(cp) = s.checkpoint() {
            return cp;
        }
        let k = if i >= 300 && i % 2 == 0 {
            Buttons::A
        } else {
            Buttons::NONE
        };
        s.step(k);
        keys.push(k);
    }
    panic!("no capturable frame within 6000 frames");
}

/// One hand-built node: its inputs from the parent's end state and its end state.
pub struct Built {
    pub keys: Vec<Buttons>,
    pub cp: Checkpoint,
}

/// A lineage from a new game: the root (settled), then each direction held 16 frames and
/// settled again.
pub fn build_lineage(game: &Emerald, dirs: &[Buttons]) -> Vec<Built> {
    let mut s = Session::new(*game);
    let mut out = Vec::new();
    let mut keys = Vec::new();
    let cp = settle(&mut s, &mut keys);
    out.push(Built { keys, cp });
    for &d in dirs {
        let mut keys = vec![d; 16];
        for &k in &keys {
            s.step(k);
        }
        let cp = settle(&mut s, &mut keys);
        out.push(Built { keys, cp });
    }
    out
}

/// The rows of a built lineage (ids 1.., chained), keys 1000 + id, on `build`.
pub fn lineage_rows(built: &[Built], build: BuildHash) -> Vec<NodeRow> {
    let mut total = 0u64;
    built
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let id = i as NodeId + 1;
            total += b.keys.len() as u64;
            let w = &b.cp.world;
            NodeRow {
                id,
                parent: (id > 1).then_some(id - 1),
                depth: i as u32,
                macro_: walk(0, i as i16),
                inputs: rle(b.keys.iter().copied()),
                frames: b.keys.len() as u32,
                total_frames: total,
                end_frame: w.frame,
                key: AbstractKey(1000 + id as u64),
                digest: state_digest(w),
                world_hash: b.cp.header.world_hash,
                map: w.field.map,
                pos: w.objects.objs[0].pos,
                progress: 0,
                fitness: Fitness::default(),
                generation: 0,
                expanded: 0,
                status: NodeStatus::Valid,
                merged_into: None,
                build,
            }
        })
        .collect()
}
