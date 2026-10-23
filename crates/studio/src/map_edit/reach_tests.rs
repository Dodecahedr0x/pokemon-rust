//! Reach unit tests: synthetic maps drawn in ASCII, behaviours set by hand (no build data).

use super::catalog::Catalog;
use super::model::{Doc, Editor, NewMap, Op, block};
use super::reach::{
    Cell, Exit, Grid, Hidden, Neighbor, Node, Walk, clear_reachable, facings, path_text,
};
use super::reach_lint::{Entries, Severity, cells_text, lint_map};
use super::schema::{GameConnection, GameObject, GameWarp, from_script_ron};
use engine::data::{Behavior, Dir};
use extract::schema::{Bg, BgKind, ConnectionDir, Coord};
use std::collections::BTreeSet;

/// A map from rows, one character per cell:
/// - `.` floor (elevation 3), `0` and `4` floor at elevation 0 and 4, `F` a bridge (15);
/// - `#` solid; `~` water (blocked, elevation 1);
/// - `v` `>` `<` `^` ledges (solid, jumped that way);
/// - `D` a door (solid), `W` a warp cell, `A` an arrow warp pushed down;
/// - `C` a counter (solid).
fn map(rows: &[&str]) -> (Doc, Vec<(i32, i32, char)>) {
    let (w, h) = (rows[0].chars().count() as u16, rows.len() as u16);
    let mut d = Doc::new(&NewMap {
        name: "Test".into(),
        layout: None,
        shared_layout: false,
        width: w,
        height: h,
        primary: "P".into(),
        secondary: "S".into(),
        fill: 1,
        fill_collision: 0,
        fill_elevation: 3,
        border: None,
    })
    .unwrap();
    let mut marks = Vec::new();
    for (y, row) in rows.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            let (coll, elev) = match ch {
                '.' | 'W' | 'A' => (0, 3),
                '0' => (0, 0),
                '4' => (0, 4),
                'F' => (0, 15),
                '~' => (0, 1),
                _ => (1, 3),
            };
            d.blocks[y * w as usize + x] = block(1, coll, elev);
            marks.push((x as i32, y as i32, ch));
        }
    }
    (d, marks)
}

/// The grid of `d` with the behaviours `marks` give.
fn grid(d: &Doc, marks: &[(i32, i32, char)]) -> Grid {
    let mut g = Grid::new(d, &Catalog::default());
    for &(x, y, ch) in marks {
        let c = g.cell_mut(x, y).unwrap();
        c.behavior = match ch {
            '~' => {
                c.water = true;
                Behavior::Blocked
            }
            'v' => Behavior::Ledge(Dir::Down),
            '^' => Behavior::Ledge(Dir::Up),
            '>' => Behavior::Ledge(Dir::Right),
            '<' => Behavior::Ledge(Dir::Left),
            'D' => Behavior::Door,
            'W' => Behavior::Warp,
            'A' => Behavior::ArrowWarp(Dir::Down),
            'C' => Behavior::Counter,
            _ => Behavior::Normal,
        };
    }
    g
}

fn obj(x: i16, y: i16) -> GameObject {
    GameObject {
        local_id: 1,
        id: None,
        gfx: "OBJ_EVENT_GFX_BOY_1".into(),
        x,
        y,
        elevation: 3,
        movement: "MOVEMENT_TYPE_FACE_DOWN".into(),
        range_x: 0,
        range_y: 0,
        trainer_type: "TRAINER_TYPE_NONE".into(),
        trainer_sight_or_berry_tree: 0,
        script: None,
        flag: None,
    }
}

fn trainer(x: i16, y: i16, movement: &str, sight: u16) -> GameObject {
    GameObject {
        id: Some("LOCALID_TRAINER".into()),
        movement: movement.into(),
        trainer_type: "TRAINER_TYPE_NORMAL".into(),
        trainer_sight_or_berry_tree: sight,
        ..obj(x, y)
    }
}

fn warp(x: i16, y: i16) -> GameWarp {
    GameWarp {
        x,
        y,
        elevation: 0,
        dest_map: "Other".into(),
        dest_warp: 0,
    }
}

fn coord(x: i16, y: i16) -> Coord {
    Coord {
        x,
        y,
        elevation: 0,
        var: Some("VAR_TEST".into()),
        value: 0,
        script: Some("Test_Sc01".into()),
    }
}

fn none(_: Node) -> bool {
    false
}

fn reached(walk: &Walk, from: (i32, i32)) -> BTreeSet<(i32, i32)> {
    walk.search(&[walk.start(from.0, from.1)], &none).cells()
}

#[test]
fn ledges_are_one_way() {
    let (d, m) = map(&["....", "#v##", "....", "#^##", "...."]);
    let g = grid(&d, &m);
    let w = Walk::new(&d, &g, &Hidden::None, false);
    let down = reached(&w, (0, 0));
    assert!(down.contains(&(1, 2)), "jumps down the ledge");
    assert!(!down.contains(&(1, 1)), "never stands on the ledge");
    assert!(
        !down.contains(&(0, 4)),
        "the up ledge is jumped from below only"
    );
    let s = w.search(&[w.start(1, 0)], &none);
    let p = s.path(s.at(1, 2).next().unwrap());
    assert_eq!(
        path_text(&p),
        "(1, 0) (1, 2)",
        "a jump is one step of two cells"
    );
    let up = reached(&w, (0, 4));
    assert!(up.contains(&(0, 2)), "jumps up the ledge from below");
    assert!(!up.contains(&(0, 0)), "but not the down ledge");
    let back = reached(&w, (0, 2));
    assert!(!back.contains(&(0, 0)), "no way back up the down ledge");
    assert!(!back.contains(&(0, 4)), "nor down the up ledge");
}

#[test]
fn elevation_follows_gen3() {
    let (d, m) = map(&[
        ".4#", "###", ".04", "###", ".F4", "###", ".F.", "###", "4F0",
    ]);
    let g = grid(&d, &m);
    let w = Walk::new(&d, &g, &Hidden::None, false);
    assert!(!reached(&w, (0, 0)).contains(&(1, 0)), "3 to 4 mismatches");
    let r = reached(&w, (0, 2));
    assert!(r.contains(&(2, 2)), "an elevation 0 cell frees the player");
    assert!(reached(&w, (2, 2)).contains(&(0, 2)), "both ways");
    let r = reached(&w, (0, 4));
    assert!(r.contains(&(1, 4)), "a bridge (15) takes any elevation");
    assert!(
        !r.contains(&(2, 4)),
        "and keeps the player's: 3 still mismatches 4"
    );
    assert!(
        reached(&w, (0, 6)).contains(&(2, 6)),
        "3 over a bridge to 3"
    );
    let s = w.search(&[w.start(0, 8)], &none);
    // 15 keeps 4 (and 0, coming back from the 0 cell).
    assert_eq!(s.at(1, 8).map(|n| n.e).collect::<Vec<_>>(), [0, 4]);
    assert!(s.cell_seen(2, 8), "to 0 from anything");
    // A 15 cell started on counts as 3.
    assert_eq!(w.start(1, 4).e, 3);
}

#[test]
fn water_needs_surf() {
    let (mut d, m) = map(&["..~~..", "######"]);
    let g = grid(&d, &m);
    let land = Walk::new(&d, &g, &Hidden::None, false);
    assert!(!reached(&land, (0, 0)).contains(&(2, 0)));
    let surf = Walk::new(&d, &g, &Hidden::None, true);
    let r = reached(&surf, (0, 0));
    assert!(
        r.contains(&(3, 0)) && r.contains(&(5, 0)),
        "surfs across and lands"
    );
    // An object on the landing cell (elevation 3) stops the landing, though the surfing
    // player is at elevation 1.
    d.map.objects.push(obj(4, 0));
    let g = grid(&d, &m);
    let surf = Walk::new(&d, &g, &Hidden::None, true);
    assert!(!reached(&surf, (0, 0)).contains(&(5, 0)));
    // And it cannot be talked to from the water (elevation 1 against 3).
    let s = surf.search(&[surf.start(0, 0)], &none);
    assert!(surf.talks_to(&s, 4, 0, 3).is_none());
}

#[test]
fn doors_warps_and_edges_are_exits() {
    let (mut d, m) = map(&["#D#W#", "....A", "....."]);
    d.map.warps = vec![warp(1, 0), warp(3, 0), warp(4, 1)];
    d.map.connections = vec![GameConnection {
        dir: ConnectionDir::South,
        offset: 0,
        map: "Below".into(),
    }];
    let g = grid(&d, &m);
    let w = Walk::new(&d, &g, &Hidden::None, false);
    let s = w.search(&[w.start(0, 1)], &none);
    let exits: Vec<Exit> = s.exits.keys().copied().collect();
    assert!(exits.contains(&Exit::Warp(0)), "pushing up into the door");
    assert!(exits.contains(&Exit::Warp(1)), "stepping on the warp cell");
    assert!(
        exits.contains(&Exit::Warp(2)),
        "pushing down on the arrow warp"
    );
    assert!(
        exits
            .iter()
            .any(|x| matches!(x, Exit::Edge { side: 0, .. })),
        "the south edge"
    );
    assert_eq!(s.exits[&Exit::Warp(0)], Node { x: 1, y: 1, e: 3 });
    // The warp cell is entered (and left by its warp): the search goes no further from it.
    assert!(s.cell_seen(3, 0));
    // Starting on a warp cell is an arrival: the player walks off it.
    let s = w.search(&[w.start(3, 0)], &none);
    assert!(s.cell_seen(0, 2));
    // A loaded neighbor whose cell is solid closes that part of the edge.
    let mut g = grid(&d, &m);
    g.sides[0].neighbor = Some(Neighbor {
        width: 5,
        height: 1,
        cells: vec![
            Cell::new(block(1, 1, 3)),
            Cell::new(block(1, 1, 3)),
            Cell::new(block(1, 1, 3)),
            Cell::new(block(1, 1, 3)),
            Cell::new(block(1, 0, 3)),
        ],
    });
    let w = Walk::new(&d, &g, &Hidden::None, false);
    let s = w.search(&[w.start(0, 1)], &none);
    let edges: Vec<Exit> = (s.exits.keys().copied())
        .filter(|x| matches!(x, Exit::Edge { .. }))
        .collect();
    assert_eq!(
        edges,
        [Exit::Edge {
            side: 0,
            x: 4,
            y: 2
        }]
    );
    // No connection: the border is solid.
    let (d, m) = map(&["..."]);
    let g = grid(&d, &m);
    let w = Walk::new(&d, &g, &Hidden::None, false);
    assert!(w.search(&[w.start(0, 0)], &none).exits.is_empty());
}

#[test]
fn objects_block_unless_hidden() {
    let (mut d, m) = map(&["...", "#.#", "..."]);
    d.map.objects.push(GameObject {
        flag: Some("FLAG_HIDE_GUARD".into()),
        ..obj(1, 1)
    });
    let g = grid(&d, &m);
    let shown = Walk::new(&d, &g, &Hidden::None, false);
    assert!(!reached(&shown, (0, 0)).contains(&(0, 2)));
    let flags = Hidden::Flags(BTreeSet::from(["FLAG_HIDE_GUARD".to_string()]));
    assert!(reached(&Walk::new(&d, &g, &flags, false), (0, 0)).contains(&(0, 2)));
    let all = Hidden::AllBut(BTreeSet::new());
    assert!(reached(&Walk::new(&d, &g, &all, false), (0, 0)).contains(&(0, 2)));
    let but = Hidden::AllBut(BTreeSet::from(["FLAG_HIDE_GUARD".to_string()]));
    assert!(!reached(&Walk::new(&d, &g, &but, false), (0, 0)).contains(&(0, 2)));
    // The fewest objects in the way name the guard.
    let goal = |n: Node| (n.x, n.y) == (0, 2);
    let found = shown.fewest_objects(&[shown.start(0, 0)], &goal, &|_| false);
    assert_eq!(found, Some(vec![0]));
    // Objects at an elevation the player is not at do not block.
    d.map.objects[0].elevation = 4;
    let (mut d2, m2) = map(&["...", "#4#", "..."]);
    d2.map.objects = d.map.objects.clone();
    let g2 = grid(&d2, &m2);
    let w = Walk::new(&d2, &g2, &Hidden::None, false);
    assert!(
        !reached(&w, (0, 0)).contains(&(0, 2)),
        "3 to 4 mismatches anyway"
    );
}

#[test]
fn trainer_sight_stops_at_collision_objects_and_elevation() {
    let (mut d, m) = map(&["......#.", ".....4..", "~~~~~~..", "........"]);
    d.map.objects = vec![
        trainer(0, 0, "MOVEMENT_TYPE_FACE_RIGHT", 8),
        trainer(0, 1, "MOVEMENT_TYPE_FACE_RIGHT", 3),
        trainer(0, 2, "MOVEMENT_TYPE_FACE_RIGHT", 6),
        trainer(0, 3, "MOVEMENT_TYPE_FACE_RIGHT", 6),
        obj(3, 3),
        obj(7, 0),
    ];
    let g = grid(&d, &m);
    let w = Walk::new(&d, &g, &Hidden::None, false);
    let row = |n: i32, y: i32| (1..=n).map(|x| (x, y)).collect::<Vec<_>>();
    assert_eq!(w.sight(0).unwrap(), row(5, 0), "up to the solid cell");
    assert_eq!(w.sight(1).unwrap(), row(3, 1), "its range");
    assert_eq!(
        w.sight(2).unwrap(),
        row(5, 2),
        "a swimmer sees along the water (elevation 1), not onto the land (3)"
    );
    assert_eq!(w.sight(3).unwrap(), row(2, 3), "up to an object");
    assert_eq!(w.sight(4), None, "not a trainer");
    // Elevation: a 4 cell stops a trainer at 3.
    d.map.objects[1].trainer_sight_or_berry_tree = 7;
    let w = Walk::new(&d, &g, &Hidden::None, false);
    assert_eq!(w.sight(1).unwrap(), row(4, 1));
    // Facing a wall: nothing; looking around: every way.
    let (mut d, m) = map(&["#.", ".."]);
    d.map.objects = vec![trainer(1, 0, "MOVEMENT_TYPE_FACE_RIGHT", 3)];
    let g = grid(&d, &m);
    let w = Walk::new(&d, &g, &Hidden::None, false);
    assert_eq!(w.sight(0).unwrap(), []);
    d.map.objects[0].movement = "MOVEMENT_TYPE_LOOK_AROUND".into();
    let w = Walk::new(&d, &g, &Hidden::None, false);
    assert_eq!(w.sight(0).unwrap(), [(1, 1)]);
    d.map.objects[0].trainer_type = "TRAINER_TYPE_BURIED".into();
    d.map.objects[0].movement = "MOVEMENT_TYPE_FACE_UP".into();
    let w = Walk::new(&d, &g, &Hidden::None, false);
    assert_eq!(
        w.sight(0).unwrap(),
        [(1, 1)],
        "buried trainers look all ways"
    );
}

#[test]
fn facings_follow_the_movement_name() {
    use Dir::*;
    assert_eq!(facings("MOVEMENT_TYPE_FACE_LEFT"), [Left]);
    assert_eq!(facings("MOVEMENT_TYPE_FACE_DOWN_AND_UP"), [Down, Up]);
    assert_eq!(
        facings("MOVEMENT_TYPE_FACE_DOWN_UP_AND_LEFT"),
        [Down, Up, Left]
    );
    assert_eq!(facings("MOVEMENT_TYPE_WALK_IN_PLACE_RIGHT"), [Right]);
    assert_eq!(facings("MOVEMENT_TYPE_WALK_UP_AND_DOWN"), [Down, Up]);
    assert_eq!(facings("MOVEMENT_TYPE_LOOK_AROUND"), Dir::ALL);
    assert_eq!(facings("MOVEMENT_TYPE_WANDER_AROUND"), Dir::ALL);
    assert_eq!(facings("MOVEMENT_TYPE_ROTATE_CLOCKWISE"), Dir::ALL);
    assert_eq!(facings("MOVEMENT_TYPE_INVISIBLE"), [Down]);
}

#[test]
fn bypass_found_and_not_found() {
    // A corridor whose only way east is the coord cell (3, 1).
    let (mut d, m) = map(&["#######", ".......", "#######"]);
    d.map.coords.push(coord(3, 1));
    let g = grid(&d, &m);
    let w = Walk::new(&d, &g, &Hidden::None, false);
    let on = |n: Node| (n.x, n.y) == (3, 1);
    let s = w.search(&[w.start(0, 1)], &on);
    assert!(!s.cell_seen(6, 1), "no way around the trigger");
    // A detour around it: found, and the witness never steps on it.
    let (mut d, m) = map(&["#.....#", "...#...", "#######"]);
    d.map.coords.push(coord(3, 0));
    d.map.coords.push(coord(2, 1));
    let g = grid(&d, &m);
    let w = Walk::new(&d, &g, &Hidden::None, false);
    let on = |n: Node| (n.x, n.y) == (2, 1);
    let s = w.search(&[w.start(0, 1)], &on);
    let p = s.path(s.at(6, 1).next().unwrap());
    assert!(p.iter().all(|n| (n.x, n.y) != (2, 1)));
    assert_eq!(
        path_text(&p),
        "(0, 1) (1, 1) (1, 0) (2, 0) (3, 0) (4, 0) (4, 1) (5, 1) (6, 1)"
    );
}

#[test]
fn trainer_sight_can_or_cannot_be_walked_past() {
    // One-wide corridor watched by a trainer: no bypass.
    let (mut d, m) = map(&["#####", ".....", "#####"]);
    d.map
        .objects
        .push(trainer(2, 0, "MOVEMENT_TYPE_FACE_DOWN", 3));
    d.blocks[2] = block(1, 0, 3);
    let g = grid(&d, &m);
    let w = Walk::new(&d, &g, &Hidden::None, false);
    let sight: BTreeSet<(i32, i32)> = w.sight(0).unwrap().into_iter().collect();
    assert_eq!(sight, BTreeSet::from([(2, 1)]));
    let s = w.search(&[w.start(0, 1)], &|n| sight.contains(&(n.x, n.y)));
    assert!(!s.cell_seen(4, 1));
    // Two wide, sight 1: the far row passes.
    let (mut d, m) = map(&["#####", ".....", ".....", "#####"]);
    d.map
        .objects
        .push(trainer(2, 0, "MOVEMENT_TYPE_FACE_DOWN", 1));
    d.blocks[2] = block(1, 0, 3);
    let g = grid(&d, &m);
    let w = Walk::new(&d, &g, &Hidden::None, false);
    let sight: BTreeSet<(i32, i32)> = w.sight(0).unwrap().into_iter().collect();
    let s = w.search(&[w.start(0, 1)], &|n| sight.contains(&(n.x, n.y)));
    assert!(s.cell_seen(4, 1));
}

#[test]
fn field_moves_clear_what_the_player_reaches() {
    let (mut d, m) = map(&["..#..", "..#..", ".....", "#####"]);
    let rock = |x, y| GameObject {
        gfx: "OBJ_EVENT_GFX_BREAKABLE_ROCK".into(),
        ..obj(x, y)
    };
    // The first rock is reached, the second only once the first is gone; the third, walled
    // in, stays.
    d.map.objects = vec![rock(2, 2), rock(3, 1)];
    d.blocks[3 * 5 + 4] = block(1, 1, 3);
    let g = grid(&d, &m);
    let mut w = Walk::new(&d, &g, &Hidden::None, false);
    let s = clear_reachable(&mut w, &[Node { x: 0, y: 0, e: 3 }]);
    assert!(w.obstacles.is_empty());
    assert!(s.cell_seen(4, 0));
    let (mut d, m) = map(&["...#.#", "...#.#", "...###"]);
    d.map.objects = vec![rock(4, 1)];
    let g = grid(&d, &m);
    let mut w = Walk::new(&d, &g, &Hidden::None, false);
    clear_reachable(&mut w, &[Node { x: 0, y: 0, e: 3 }]);
    assert_eq!(w.obstacles.len(), 1);
}

#[test]
fn talking_and_reading_need_a_reached_cell() {
    let (mut d, m) = map(&["#C.", "..."]);
    d.map.objects = vec![obj(0, 0)];
    d.map.bgs = vec![
        Bg {
            x: 2,
            y: 0,
            elevation: 0,
            kind: BgKind::Script {
                facing: 1,
                script: Some("Test_Sign".into()),
            },
        },
        Bg {
            x: 2,
            y: 0,
            elevation: 0,
            kind: BgKind::Script {
                facing: 3,
                script: Some("Test_Sign".into()),
            },
        },
    ];
    d.blocks[2] = block(1, 1, 3);
    let g = grid(&d, &m);
    let w = Walk::new(&d, &g, &Hidden::None, false);
    let s = w.search(&[w.start(0, 1)], &none);
    assert!(w.talks_to(&s, 0, 0, 3).is_some(), "from below");
    assert_eq!(
        w.reads(&s, 0),
        Some(Node { x: 2, y: 1, e: 3 }),
        "facing north"
    );
    assert_eq!(
        w.reads(&s, 1),
        None,
        "facing east: from (1, 0), the counter"
    );
    // Across the counter: an object behind it at (1, 0)'s far side.
    let (mut d, m) = map(&[".C.", "#.#"]);
    d.map.objects = vec![obj(2, 0)];
    let g = grid(&d, &m);
    let w = Walk::new(&d, &g, &Hidden::None, false);
    let s = w.search(&[w.start(0, 0)], &none);
    assert!(w.talks_to(&s, 2, 0, 3).is_some(), "across the counter");
}

fn subjects(found: &[super::reach_lint::Finding], sev: Severity) -> Vec<String> {
    (found.iter())
        .filter(|f| f.severity == sev)
        .map(|f| f.subject.clone())
        .collect()
}

#[test]
fn lint_finds_the_known_bug_shapes() {
    // Warp 0 the entrance (an arrow warp, south), warp 1 a door whose only cell in front is
    // a sealed alcove, a trainer facing a wall, an item ball walled in, a sign behind a guard
    // without a hide flag, and a trigger row the west column walks around.
    let (mut d, m) = map(&[
        "#####D##", "#...#.##", "#....###", "#.##..##", "#.....#.", "#..A..##",
    ]);
    d.map.warps = vec![warp(3, 5), warp(5, 0)];
    let named = |id: &str, local_id, o: GameObject| GameObject {
        id: Some(id.into()),
        local_id,
        ..o
    };
    d.map.objects = vec![
        named(
            "LOCALID_TRAINER",
            1,
            trainer(4, 3, "MOVEMENT_TYPE_FACE_LEFT", 3),
        ),
        named(
            "LOCALID_BALL",
            2,
            GameObject {
                gfx: "OBJ_EVENT_GFX_ITEM_BALL".into(),
                flag: Some("FLAG_ITEM".into()),
                ..obj(7, 4)
            },
        ),
        named("LOCALID_GUARD", 3, obj(3, 1)),
    ];
    d.map.bgs = vec![Bg {
        x: 3,
        y: 0,
        elevation: 0,
        kind: BgKind::Script {
            facing: 1,
            script: Some("Test_Sign".into()),
        },
    }];
    d.map.coords = vec![coord(2, 2), coord(3, 2), coord(4, 2)];
    let g = grid(&d, &m);
    let entries = Entries {
        arrivals: BTreeSet::from([0, 1]),
        points: Vec::new(),
    };
    let found = lint_map(&d, &g, &entries, None);
    for f in &found {
        println!("{f}");
    }
    assert_eq!(
        subjects(&found, Severity::Warning),
        [
            "warp 1",
            "object LOCALID_TRAINER",
            "object LOCALID_BALL",
            "bg 0"
        ]
    );
    assert_eq!(subjects(&found, Severity::Info), ["coords VAR_TEST=0"]);
    let alcove = &found[0];
    assert!(alcove.message.contains("an isolated area"), "{alcove}");
    let sign = found.iter().find(|f| f.subject == "bg 0").unwrap();
    assert!(sign.message.contains("only past LOCALID_GUARD"), "{sign}");
    let row = found
        .iter()
        .find(|f| f.subject.starts_with("coords"))
        .unwrap();
    assert!(
        row.message.contains("(2-4, 2)") && row.message.contains("warp 0"),
        "{row}"
    );
    // Closing the west column's top makes the row a barrier.
    d.blocks[8 + 1] = block(1, 1, 3);
    let g = grid(&d, &m);
    let found = lint_map(&d, &g, &entries, None);
    assert!(subjects(&found, Severity::Info).is_empty(), "{found:?}");
    // A map nothing leads to says so.
    let found = lint_map(&d, &g, &Entries::default(), None);
    assert_eq!(subjects(&found, Severity::Info), ["map"]);
}

#[test]
fn lint_reports_one_way_areas_and_surf() {
    // A ledge one way: arriving by warp 1, warp 0 cannot be reached.
    let (mut d, m) = map(&["W.>.W"]);
    d.map.warps = vec![warp(0, 0), warp(4, 0)];
    let g = grid(&d, &m);
    let entries = Entries {
        arrivals: BTreeSet::from([0, 1]),
        points: Vec::new(),
    };
    let found = lint_map(&d, &g, &entries, None);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].severity, Severity::Info);
    assert_eq!(found[0].subject, "warp 1");
    assert!(
        found[0].message.contains("cannot be reached"),
        "{}",
        found[0]
    );
    // Water: warp 1 (an exit only) needs Surf; a start past the water reaches no exit.
    let (mut d, m) = map(&["W.~~.W"]);
    d.map.warps = vec![warp(0, 0), warp(5, 0)];
    let g = grid(&d, &m);
    let entries = Entries {
        arrivals: BTreeSet::from([0]),
        points: vec![("start".into(), 4, 0)],
    };
    let found = lint_map(&d, &g, &entries, None);
    for f in &found {
        println!("{f}");
    }
    let w1 = found.iter().find(|f| f.subject == "warp 1").unwrap();
    assert_eq!(w1.severity, Severity::Info, "{w1}");
    assert!(w1.message.contains("Surf reaches it"), "{w1}");
    let start = found.iter().find(|f| f.subject == "start").unwrap();
    assert_eq!(start.severity, Severity::Warning, "{start}");
}

#[test]
fn cells_text_merges_rows() {
    let cells = BTreeSet::from([(2, 13), (3, 13), (4, 13), (7, 1), (9, 13)]);
    assert_eq!(cells_text(&cells), "(7, 1) (2-4, 13) (9, 13)");
}

#[test]
fn allow_reach_parses_and_needs_a_reason() {
    let ops: Vec<Op> =
        from_script_ron(r#"[AllowReach(subject: "warp 1", reason: "opened by a script")]"#)
            .unwrap();
    let (d, _) = map(&["."]);
    let mut ed = Editor::new(d.clone());
    ed.apply_all(&ops).unwrap();
    assert_eq!(ed.doc(), &d, "changes nothing");
    let bad: Vec<Op> = from_script_ron(r#"[AllowReach(subject: "warp 1", reason: " ")]"#).unwrap();
    assert!(Editor::new(d).apply_all(&bad).is_err());
}
