//! Task H: the fixes the slice runs called for.
//! - Block `WalkTo`s jitter inside their block (`macros::block_mates`), so the default 4x4
//!   blocks cover every stub cell (stub).
//! - The softlock probe resumes walks a battle interrupted and stops at the first state that
//!   leaves the start's region (real flavor: Route 101 by the tall grass, after the lab).

use content::Emerald;
#[cfg(not(pkr_silice))]
use content::consts::MAP_ROUTE101;
use engine::data::GameData;
use explore::config::{Config, KeyConfig};
use explore::observe::NoObserver;
use explore::policy::Policy;
use explore::softlock::{self, Trap};
use explore::testing::GrassPocket;
use explore::{Macro, MacroKind, Opts, expand, key, macros, path, sim};
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
/// walks): after the lab, next to the tall grass at (19, 9). `fixtures/route101_grass.runs`
/// holds only the button runs from a new game (no state, no hashes), 16,057 frames
/// (regenerated after Milestone 8 Task G1, whose spread battle return and trainer prize
/// message shifted the route timing the Task F one relied on).
///
/// To regenerate it (for example after a content or engine change moves that state): run the
/// explorer with `probe_resumes: 0` until a Route 101 `Softlock` appears, e.g.
/// `echo "(detect: (probe_resumes: 0))" > /tmp/r0.ron` then
/// `cargo xtask explore run --secs 300 --config /tmp/r0.ron --db target/explore/r0.db`;
/// then `cargo xtask explore export <id> --db target/explore/r0.db --out target/explore/f.ron`
/// and copy the trace's `inputs` runs (joined by spaces, nothing else) into the fixture, and
/// update the position asserted below.
#[cfg(not(pkr_silice))]
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
    assert_eq!(w.objects().player().pos(), (19, 9));
    s.checkpoint().expect("the node's end state is capturable")
}

#[cfg(not(pkr_silice))]
#[test]
fn route101_grass_is_not_a_softlock() {
    if content::STUB {
        return;
    }
    let game = Emerald::for_shells();
    let cp = route101_grass();
    let mut cfg = Config::default();
    let probe = |cfg: &Config| {
        sim::trap(
            &game,
            &cp,
            cfg.detect.softlock_k,
            cfg.detect.softlock_breadth,
            cfg,
        )
    };
    let probed = probe(&cfg);
    assert_eq!(probed.trap, Trap::Free, "{probed:?}");
    // The probe stopped at the first state out of the place: far below a full first level.
    assert!(
        probed.frames < u64::from(cfg.limits.macro_frames) * 8,
        "{probed:?}"
    );
    // Without resumes, every walk ends in a wild battle by the grass: the false positive.
    cfg.detect.probe_resumes = 0;
    let probed = probe(&cfg);
    assert!(matches!(probed.trap, Trap::Trapped { .. }), "{probed:?}");
}

/// A sealed 3x3 pocket around the stub's tall-grass cell, with an exit elsewhere on the map:
/// the probe's walks meet wild battles, so the party levels up (every level counts in the
/// party bucket with `level_step` 1), yet nothing gets out. It must stay `Trapped`: the probe
/// compares places (map, flags, vars), not regions.
#[test]
fn a_grass_pocket_stays_trapped_whatever_the_party_does() {
    if !content::STUB {
        return;
    }
    let inner = Emerald::for_shells();
    let map = inner.new_game().map;
    let grass = (12, 3);
    // Walk next to the grass on the open map, then seal the pocket around it.
    let mut s = expand::new_game(inner);
    let to = Macro {
        kind: MacroKind::WalkTo { x: 12, y: 4 },
        opts: Opts::default(),
    };
    let cfg = Config {
        key: KeyConfig {
            level_step: 1,
            ..Config::default().key
        },
        ..Config::default()
    };
    expand::expand(
        &mut s,
        &to,
        &mut Policy::new(),
        &mut NoObserver,
        &cfg.limits,
    )
    .unwrap_or_else(|(e, _)| panic!("{e:?}"));
    let cp = s.checkpoint().unwrap();
    assert_eq!(cp.world.objects.objs[0].pos, (12, 4));
    let level = |cp: &engine::checkpoint::Checkpoint| key::key_parts(&cp.world, &cfg.key).party;
    let pocket = GrassPocket {
        inner,
        map,
        at: grass,
    };
    assert!(softlock::map_exits(&pocket, map) >= 1);
    let probed = sim::trap(&pocket, &cp, 3, 64, &cfg);
    assert!(matches!(probed.trap, Trap::Trapped { .. }), "{probed:?}");

    // The pocket does change the party: a few grass macros from the start level it up.
    let mut s = Session::restore(pocket, &cp).unwrap();
    let grass_walk = Macro {
        kind: MacroKind::Grass { x: 12, y: 3 },
        opts: Opts::default(),
    };
    let before = level(&cp);
    let mut changed = false;
    for _ in 0..6 {
        if expand::expand(
            &mut s,
            &grass_walk,
            &mut Policy::new(),
            &mut NoObserver,
            &cfg.limits,
        )
        .is_err()
        {
            break;
        }
        changed |= level(&s.checkpoint().unwrap()) != before;
    }
    assert!(changed, "the party bucket never changed in the pocket");
}
