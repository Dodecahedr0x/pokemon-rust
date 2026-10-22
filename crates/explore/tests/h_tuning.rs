//! Task H: the fixes the slice runs called for.
//! - Block `WalkTo`s jitter inside their block (`macros::block_mates`), so the default 4x4
//!   blocks cover every stub cell (stub).
//! - The softlock probe resumes walks a battle interrupted and stops at the first state that
//!   leaves the start's region (real flavor: Route 101 by the tall grass, after the lab).

use content::Emerald;
use content::consts::MAP_ROUTE101;
use explore::config::Config;
use explore::softlock::Trap;
use explore::{macros, path, sim};
use game_native::Session;
use std::collections::BTreeSet;

/// The new game's first capturable state (stub).
fn first_capturable() -> engine::checkpoint::WorldSnapshot {
    let mut s = Session::new(Emerald::for_shells());
    loop {
        if let Ok(snap) = s.world().snapshot(s.game()) {
            return snap;
        }
        s.step(engine::Buttons::NONE);
    }
}

#[test]
fn block_mates_are_the_reachable_cells_of_the_target_block() {
    if !content::STUB {
        return;
    }
    let game = Emerald::for_shells();
    let snap = first_capturable();
    let reach = path::reach(&game, &snap);
    let player = snap.objects.objs[0].pos;
    let shift = Config::default().key.cell_shift;
    let block = |(x, y): (i16, i16)| (x >> shift, y >> shift);
    let mut covered = BTreeSet::new();
    for m in macros::candidates_with(&game, &snap, &reach, shift) {
        let explore::MacroKind::WalkTo { x, y } = m.kind else {
            continue;
        };
        let mates = macros::block_mates(&game, &snap, &reach, shift, (x, y));
        assert!(mates.contains(&(x, y)), "the candidate itself is a mate");
        for &c in &mates {
            assert_eq!(block(c), block((x, y)), "{c:?} is in {:?}'s block", (x, y));
            assert!(reach.dist(c.0, c.1).is_some() && c != player);
        }
        covered.extend(mates);
    }
    // Every reachable cell outside the player's block is a possible target.
    let want: BTreeSet<_> = reach
        .cells()
        .filter(|&c| block(c) != block(player))
        .collect();
    assert_eq!(covered, want);
    // Not a mate: an unreachable cell.
    assert!(macros::block_mates(&game, &snap, &reach, shift, (-1, -1)).is_empty());
}

/// The state of the first Route 101 softlock the slice run raised (before the probe resumed
/// walks): after the lab, next to the tall grass at (17, 13). `fixtures/route101_grass.runs` holds
/// only the button runs from a new game (no state, no hashes), 12,071 frames.
fn route101_grass() -> engine::checkpoint::Checkpoint {
    let text = include_str!("fixtures/route101_grass.runs");
    let runs = explore::runs::from_text(text).unwrap();
    let mut s = Session::new(Emerald::for_shells());
    for keys in explore::runs::keys(&runs) {
        s.step(keys);
    }
    let w = s.world();
    assert_eq!(w.map(), MAP_ROUTE101);
    assert!(!w.party().is_empty());
    assert_eq!(w.objects().player().pos(), (17, 13));
    s.checkpoint().expect("the node's end state is capturable")
}

#[test]
fn route101_grass_is_not_a_softlock() {
    if content::STUB {
        return;
    }
    let game = Emerald::for_shells();
    let cp = route101_grass();
    let cfg = Config::default();
    let probed = sim::trap(
        &game,
        &cp,
        cfg.detect.softlock_k,
        cfg.detect.softlock_breadth,
        &cfg,
    );
    assert_eq!(probed.trap, Trap::Free, "{probed:?}");
    // The probe stopped at the first state out of the region: far below a full first level.
    assert!(
        probed.frames < u64::from(cfg.limits.macro_frames) * 8,
        "{probed:?}"
    );
}
