//! View-window spawning (Milestone 8 Task F): Gen3 `TrySpawnObjectEvents`,
//! `RemoveObjectEventsOutsideView` and the `OBJECT_EVENTS_COUNT` cap.

use super::*;
use crate::data::{Dir, Gfx, GfxId, MovementType};
use alloc::vec::Vec;

const MAP: MapId = MapId(1);
const OTHER: MapId = MapId(2);
const HIDE: FlagId = FlagId(0x50);

fn ev(local_id: u8, x: i16, y: i16) -> ObjectEvent {
    ObjectEvent {
        local_id,
        gfx: Gfx::Fixed(GfxId(1)),
        x,
        y,
        elevation: 0,
        movement: MovementType::Face(Dir::Down),
        range: (0, 0),
        trainer_type: 0,
        trainer_sight: 0,
        script: None,
        hide_flag: None,
    }
}

/// A warp load onto `map` with the player on (x, y), then the template spawn.
fn loaded(map: MapId, x: i16, y: i16, templates: &[ObjectEvent], vars: &Vars) -> Objects {
    let mut o = Objects::new();
    o.enter_map(map, None);
    o.set_pos(PLAYER, x, y);
    o.spawn_templates(templates, vars);
    o
}

fn ids(o: &Objects) -> Vec<(MapId, u8)> {
    o.iter().skip(1).map(|x| (x.map, x.local_id)).collect()
}

fn locals(o: &Objects) -> Vec<u8> {
    o.iter().skip(1).map(|x| x.local_id).collect()
}

#[test]
fn a_warp_load_spawns_only_the_templates_in_the_view_window() {
    // Gen3 window around the player's cell: x - 9 ..= x + 10, y - 7 ..= y + 9.
    let t = [
        ev(1, 11, 20),
        ev(2, 10, 20),
        ev(3, 30, 20),
        ev(4, 31, 20),
        ev(5, 20, 13),
        ev(6, 20, 12),
        ev(7, 20, 29),
        ev(8, 20, 30),
    ];
    let o = loaded(MAP, 20, 20, &t, &Vars::new());
    assert_eq!(locals(&o), [1, 3, 5, 7]);
}

#[test]
fn hidden_templates_do_not_spawn() {
    let mut t = [ev(1, 20, 20), ev(2, 21, 20)];
    t[0].hide_flag = Some(HIDE);
    let mut vars = Vars::new();
    vars.set_flag(HIDE, true);
    let o = loaded(MAP, 20, 21, &t, &vars);
    assert_eq!(locals(&o), [2]);
}

#[test]
fn at_most_sixteen_objects_exist_player_included() {
    let t: Vec<ObjectEvent> = (1..=20).map(|i| ev(i, 10 + i as i16 % 5, 10)).collect();
    let o = loaded(MAP, 12, 12, &t, &Vars::new());
    assert_eq!(o.iter().count(), OBJECT_EVENTS_COUNT);
    assert_eq!(locals(&o), (1..=15).collect::<Vec<u8>>(), "template order");
}

#[test]
fn addobject_respects_the_cap() {
    let t: Vec<ObjectEvent> = (1..=15).map(|i| ev(i, 10, 10)).collect();
    let mut o = loaded(MAP, 10, 10, &t, &Vars::new());
    o.add(&ev(16, 11, 10), |_| 0);
    assert!(
        o.get(16).is_none(),
        "no free object slot (Gen3 `TrySpawnObjectEvent`)"
    );
    o.remove(3);
    o.add(&ev(16, 11, 10), |_| 0);
    assert!(o.get(16).is_some());
}

#[test]
fn moving_the_view_spawns_entering_templates_and_despawns_leaving_objects() {
    let t = [ev(1, 0, 0), ev(2, 30, 0), ev(3, 14, 0)];
    let vars = Vars::new();
    let mut o = loaded(MAP, 5, 0, &t, &vars);
    assert_eq!(locals(&o), [1, 3]);
    // Twelve cells east: 1 (at x - 17) leaves, 2 (x + 13) is still out.
    o.set_pos(PLAYER, 17, 0);
    o.view_moved(&t, &vars);
    assert_eq!(locals(&o), [3]);
    o.set_pos(PLAYER, 20, 0);
    o.view_moved(&t, &vars);
    assert_eq!(locals(&o), [3, 2], "2 enters at x + 10");
    // Back west: 1 respawns from its template.
    o.set_pos(PLAYER, 9, 0);
    o.view_moved(&t, &vars);
    assert_eq!(locals(&o), [3, 1]);
}

#[test]
fn an_object_whose_home_is_in_view_stays() {
    // Gen3 `RemoveObjectEventIfOutsideView` keeps an object if its current or initial cell is
    // in the window.
    let t = [ev(1, 10, 0)];
    let vars = Vars::new();
    let mut o = loaded(MAP, 10, 0, &t, &vars);
    o.set_pos(1, 40, 0);
    o.view_moved(&t, &vars);
    assert_eq!(o.get(1).map(|x| x.pos()), Some((40, 0)));
    o.set_pos(PLAYER, 40, 0);
    o.set_pos(1, 0, 0);
    o.view_moved(&t, &vars);
    assert!(o.get(1).is_none(), "neither cell in view");
}

#[test]
fn an_existing_object_is_not_respawned() {
    // Gen3 `GetAvailableObjectEventId`: a template whose object exists does not spawn again.
    let t = [ev(1, 10, 10)];
    let vars = Vars::new();
    let mut o = loaded(MAP, 10, 12, &t, &vars);
    o.set_pos(1, 11, 11);
    o.view_moved(&t, &vars);
    assert_eq!(o.iter().count(), 2);
    assert_eq!(o.get(1).unwrap().pos(), (11, 11));
}

#[test]
fn a_crossing_keeps_objects_and_spawns_the_new_map_by_view() {
    // Old map objects 1 (in view after the crossing) and 2 (out of view).
    let old = [ev(1, 5, 2), ev(2, 5, 30)];
    let vars = Vars::new();
    let mut o = loaded(OTHER, 5, 0, &old, &vars);
    assert_eq!(locals(&o), [1]);
    o.set_pos(PLAYER, 5, 28);
    o.view_moved(&old, &vars);
    assert_eq!(locals(&o), [2]);
    o.set_pos(PLAYER, 5, 23);
    o.view_moved(&old, &vars);
    assert_eq!(ids(&o), [(OTHER, 2)]);
    // The new map lies south: old coordinates move up by 25.
    o.enter_map(MAP, Some((0, -25)));
    assert_eq!(o.player().pos(), (5, -2));
    let new = [ev(1, 5, 3), ev(2, 5, 20)];
    o.spawn_templates(&new, &vars);
    assert_eq!(
        ids(&o),
        [(OTHER, 2), (MAP, 1)],
        "old 2 at (5, 5) stays in view"
    );
    // Crossing back keeps the leftover objects of the map entered (Gen3 does not respawn
    // them).
    o.set_pos(1, 6, 3);
    o.enter_map(OTHER, Some((0, 25)));
    o.spawn_templates(&old, &vars);
    assert_eq!(ids(&o), [(OTHER, 2), (MAP, 1)]);
    assert_eq!(o.get(2).unwrap().pos(), (5, 30));
}
