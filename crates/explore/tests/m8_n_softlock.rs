//! Milestone 8 Task N: the softlock probe tolerates objects that block the way for a while
//! (the wandering aide of Birch's lab, a false positive of Task L). A trapped start whose map
//! has an object that walks by itself is probed again after waits.

use content::Emerald;
use engine::Buttons;
use engine::checkpoint::Checkpoint;
use engine::data::{Dir, MapId, MovementType};
use explore::softlock::{self, Trap, walkers, walks};
use game_native::Session;
use std::cell::Cell;

/// A capturable new-game state.
fn start() -> Checkpoint {
    let mut s = Session::new(Emerald::for_shells());
    for _ in 0..3000 {
        if let Ok(cp) = s.checkpoint() {
            return cp;
        }
        s.step(Buttons::NONE);
    }
    panic!("no capturable frame")
}

/// `cp` with one object besides the player, moving as `m`.
fn with_object(cp: &Checkpoint, m: MovementType) -> Checkpoint {
    let mut cp = cp.clone();
    let mut o = cp.world.objects.objs[0];
    o.local_id = 1;
    o.movement = m;
    o.visible = true;
    o.frozen = false;
    cp.world.objects.objs.truncate(1);
    cp.world.objects.objs.push(o);
    cp
}

const PLACE: u64 = 7;

/// Probes `start` (k 2, breadth 8, 4 re-probes) where every macro stays in place until a wait
/// has run `free_after` times; then a macro leaves the place.
fn probe(start: &Checkpoint, free_after: u32) -> (Trap, u32, u32) {
    let map: MapId = start.world.field.map;
    let waits = Cell::new(0u32);
    let mut expand_all = |cp: &Checkpoint, _remaining: u32| {
        let place = if waits.get() >= free_after {
            PLACE + 1
        } else {
            PLACE
        };
        vec![(place, map, cp.clone())]
    };
    let mut wait = |cp: &Checkpoint| {
        waits.set(waits.get() + 1);
        let mut next = cp.clone();
        next.world.frame += 600;
        Some((PLACE, map, next))
    };
    let (trap, n) =
        softlock::probe_waiting(start, PLACE, 1, 2, 8, 4, &mut expand_all, &mut wait);
    (trap, n, waits.get())
}

#[test]
fn a_walking_object_gets_waits_and_re_probes() {
    let base = start();
    let wanderer = with_object(&base, MovementType::Wander);
    assert!(walkers(&wanderer.world));
    // Freed after two waits: two re-probes, then free.
    assert_eq!(probe(&wanderer, 2), (Trap::Free, 2, 2));
    // Never freed: every re-probe runs, and the verdict stays.
    let (trap, n, waits) = probe(&wanderer, u32::MAX);
    assert!(matches!(trap, Trap::Trapped { .. }), "{trap:?}");
    assert_eq!((n, waits), (4, 4));
    // A wait that leaves the place frees the start without another probe.
    let map = wanderer.world.field.map;
    let mut expand_all = |cp: &Checkpoint, _: u32| vec![(PLACE, map, cp.clone())];
    let mut wait = |cp: &Checkpoint| Some((PLACE + 1, map, cp.clone()));
    let r = softlock::probe_waiting(&wanderer, PLACE, 1, 2, 8, 4, &mut expand_all, &mut wait);
    assert_eq!(r, (Trap::Free, 1));
}

#[test]
fn objects_that_stay_put_get_no_wait() {
    let base = start();
    for m in [
        MovementType::Face(Dir::Down),
        MovementType::LookAround,
        MovementType::WalkInPlace(Dir::Up),
        MovementType::Rotate { clockwise: true },
    ] {
        assert!(!walks(m), "{m:?}");
        let still = with_object(&base, m);
        assert!(!walkers(&still.world));
        let (trap, n, waits) = probe(&still, u32::MAX);
        assert!(matches!(trap, Trap::Trapped { .. }), "{m:?}: {trap:?}");
        assert_eq!((n, waits), (0, 0));
    }
    // A frozen or invisible walker does not walk either.
    let mut frozen = with_object(&base, MovementType::Wander);
    frozen.world.objects.objs[1].frozen = true;
    assert!(!walkers(&frozen.world));
    let mut hidden = with_object(&base, MovementType::WanderVertical);
    hidden.world.objects.objs[1].visible = false;
    assert!(!walkers(&hidden.world));
}
