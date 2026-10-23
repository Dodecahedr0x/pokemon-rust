//! World tests of Milestone 8 Task M (the map tour's `World::debug_warp`).

use super::*;

/// Runs frames until the transition ends.
fn settle(w: &mut World) {
    for _ in 0..200 {
        run(w, Buttons::NONE, 1);
        if !w.transition_active() {
            return;
        }
    }
    panic!("the warp never ended");
}

#[test]
fn debug_warp_lands_on_the_warp_or_the_cell() {
    let mut w = started();
    assert!(!w.debug_warp(&Game, MapId(77), None, 1, 1), "a missing map");
    assert!(!w.transition_active());
    assert!(w.debug_warp(&Game, HOUSE, Some(0), -1, -1));
    assert!(
        !w.debug_warp(&Game, ROUTE, None, 4, 6),
        "refused while a transition runs"
    );
    settle(&mut w);
    assert_eq!((w.map(), pos(&w)), (HOUSE, (2, 5)), "on the house's warp 0");
    assert!(w.debug_warp(&Game, ROUTE, None, 4, 6));
    settle(&mut w);
    assert_eq!((w.map(), pos(&w)), (ROUTE, (4, 6)));
}

#[test]
fn debug_warp_ends_a_running_script() {
    let mut w = started();
    tap(&mut w, Buttons::RIGHT, 10);
    tap(&mut w, Buttons::A, 4);
    assert!(w.script_running());
    assert!(w.debug_warp(&Game, HOUSE, Some(0), -1, -1));
    assert!(!w.script_running());
    settle(&mut w);
    assert_eq!(w.map(), HOUSE);
    assert!(!w.text().is_open(), "the dialogue went with the old map");
}
