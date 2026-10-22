//! World tests of Milestone 8 Task F (field at scale: view-window spawning, stale-view
//! crossings, warp slots).

use super::*;

#[test]
fn old_map_objects_despawn_once_out_of_view_and_respawn_on_return() {
    let mut w = started();
    walk(&mut w, Dir::Up, 5);
    run(&mut w, Buttons::UP, 1);
    run(&mut w, Buttons::NONE, 16);
    assert_eq!((w.map(), pos(&w)), (ROUTE, (5, 9)));
    let town = |w: &World| w.objects().iter().filter(|o| o.map == TOWN).count();
    assert!(town(&w) > 0, "the town's NPCs stay in view");
    // The town's NPC 1 is on (6, 15) in route coordinates: out of the window from y = 5.
    walk(&mut w, Dir::Up, 3);
    assert!(town(&w) > 0);
    walk(&mut w, Dir::Up, 1);
    assert_eq!(town(&w), 0, "despawned by the view window");
    walk(&mut w, Dir::Down, 4);
    run(&mut w, Buttons::DOWN, 1);
    assert_eq!(w.map(), TOWN);
    let npc = w.objects().get(1).expect("respawned from its template");
    assert_eq!(npc.pos(), (6, 5));
}
