//! Task E: the fault-injection wrappers forward everything they do not break.

use content::Emerald;
use engine::checkpoint::build_hash;
use engine::data::{GameData, MapId};
use explore::testing::{Drifting, FadeTrap, PanicOnScript, Rebuilt, WallAround};
use game_native::Session;
use game_native::scenario::cached_route;

/// Provided and required methods agree with `inner` (except `map` of the patched map).
fn same_as_inner(g: &impl GameData, inner: &Emerald) {
    let ng = inner.new_game();
    assert_eq!(build_hash(g), build_hash(inner));
    assert_eq!(g.special_ids(), inner.special_ids());
    assert!(std::ptr::eq(g.battle(), inner.battle()));
    assert!(std::ptr::eq(g.battle_gfx(), inner.battle_gfx()));
    assert!(std::ptr::eq(g.font(), inner.font()));
    assert_eq!(g.has_map(ng.map), inner.has_map(ng.map));
    assert_eq!(g.heal_location(1), inner.heal_location(1));
    assert_eq!(g.new_game().map, ng.map);
    assert_eq!((g.new_game().x, g.new_game().y), (ng.x, ng.y));
    let other = MapId(ng.map.0.wrapping_add(1));
    assert!(std::ptr::eq(g.map(other), inner.map(other)));
}

#[test]
fn wrappers_forward_the_rest() {
    let inner = Emerald::for_shells();
    let map = inner.new_game().map;
    same_as_inner(
        &PanicOnScript {
            inner,
            script: content::NEW_GAME_SCRIPT,
        },
        &inner,
    );
    same_as_inner(
        &WallAround {
            inner,
            map,
            at: (5, 5),
        },
        &inner,
    );
    same_as_inner(
        &FadeTrap {
            inner,
            map,
            at: (5, 5),
        },
        &inner,
    );
    let d = Drifting::new(inner);
    same_as_inner(&d, &inner);
    // Patched maps are leaked once and keep the base's size and tiles.
    let w = WallAround {
        inner,
        map,
        at: (5, 5),
    };
    assert!(std::ptr::eq(w.map(map), w.map(map)));
    assert_eq!(
        (w.map(map).width, w.map(map).height),
        (inner.map(map).width, inner.map(map).height)
    );
    let f = FadeTrap {
        inner,
        map,
        at: (5, 5),
    };
    assert_eq!(f.map(map).coords.len(), inner.map(map).coords.len() + 1);
    // A Drifting clone starts a fresh counter.
    let _ = d.wild_encounters(map, 0);
    assert_eq!(d.calls.get(), 1);
    assert_eq!(d.clone().calls.get(), 0);
}

#[test]
fn rebuilt_differs_in_build_only() {
    let inner = Emerald::for_shells();
    let rebuilt = Rebuilt {
        inner,
        build_id: inner.build_id() ^ 0x5A5A,
    };
    let (a, b) = (build_hash(&inner), build_hash(&rebuilt));
    assert_ne!(a, b);
    assert_eq!(a.engine, b.engine);
    assert_eq!(b.content, inner.build_id() ^ 0x5A5A);

    // Same behaviour: the route's hashes, frame by frame.
    let rec = cached_route();
    let n = rec.keys.len().min(5000);
    let mut s = Session::new(rebuilt);
    for i in 0..n {
        s.step(rec.keys[i]);
        assert_eq!(s.world().hash(), rec.hashes[i], "frame {}", i + 1);
    }
    // Its checkpoints carry its build (through the opening scene: Silice's bar).
    let mut s = Session::new(rebuilt);
    game_native::scenario::to_field(&mut s).unwrap_or_else(|e| panic!("{e}"));
    let cp = s.checkpoint().expect("capturable");
    assert_eq!(cp.header.build, b);
}
