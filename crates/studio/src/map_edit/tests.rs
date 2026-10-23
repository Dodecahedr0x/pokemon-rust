//! Model unit tests: no files, no build data (a made-up catalog).

use super::catalog::{Catalog, MapRef};
use super::model::{Brush, Doc, Editor, Event, EventKind, NewMap, Op, UNDO_LIMIT, block};
use super::schema::{GameConnection, GameHeader, MAP_DYNAMIC, from_ron, from_script_ron, to_ron};
use super::validate::{Severity, errors, id_changes, validate};
use engine::data::Tileset;
use extract::schema::{BgKind, ConnectionDir, MapScripts};
use std::collections::BTreeMap;

fn tileset(metatiles: usize) -> &'static Tileset {
    Box::leak(Box::new(Tileset {
        tiles: &[],
        palettes: &[],
        metatiles: Box::leak(vec![[0u16; 8]; metatiles].into_boxed_slice()),
        attributes: &[],
    }))
}

fn conn(dir: ConnectionDir, offset: i32, map: &str) -> GameConnection {
    GameConnection {
        dir,
        offset,
        map: map.into(),
    }
}

/// Tilesets `P` (10 metatiles) and `S` (4). Maps: `Other` (10x10, 2 warps to `Test`, `Test`
/// south of it), `Lonely` (8x8, no warps, `Test` north of it at offset 2) and `OneWay` (1
/// warp, elsewhere).
fn catalog() -> Catalog {
    let mut c = Catalog::default();
    c.add_tileset("P", tileset(10), false);
    c.add_tileset("S", tileset(4), true);
    c.maps.insert(
        "Other".into(),
        MapRef {
            id: None,
            size: Some((10, 10)),
            warps: vec!["Test".into(), "Test".into()],
            connections: vec![conn(ConnectionDir::South, 0, "Test")],
        },
    );
    c.maps.insert(
        "Lonely".into(),
        MapRef {
            id: None,
            size: Some((8, 8)),
            warps: Vec::new(),
            connections: vec![conn(ConnectionDir::South, 2, "Test")],
        },
    );
    c.maps.insert(
        "OneWay".into(),
        MapRef {
            warps: vec!["Elsewhere".into()],
            ..MapRef::default()
        },
    );
    c.scripts.insert("Known_Script".into());
    c
}

fn spec(w: u16, h: u16) -> NewMap {
    NewMap {
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
    }
}

fn object(x: i16, y: i16, gfx: &str) -> Op {
    Op::Object {
        x,
        y,
        id: None,
        gfx: gfx.into(),
        movement: "MOVEMENT_TYPE_FACE_DOWN".into(),
        range_x: 0,
        range_y: 0,
        script: Some("Known_Script".into()),
        flag: None,
        trainer_type: "TRAINER_TYPE_NONE".into(),
        sight: 0,
        elevation: 3,
    }
}

fn warp(x: i16, y: i16, dest_map: &str, dest_warp: u8) -> Op {
    Op::Warp {
        x,
        y,
        dest_map: dest_map.into(),
        dest_warp,
        elevation: 0,
    }
}

fn texts(d: &Doc) -> Vec<String> {
    validate(d, &catalog())
        .iter()
        .map(|i| i.to_string())
        .collect()
}

fn editor(w: u16, h: u16) -> Editor {
    Editor::new(Doc::new(&spec(w, h)).unwrap())
}

fn mt(e: &Editor, x: i32, y: i32) -> u16 {
    e.doc().block(x, y).unwrap() & 0x3FF
}

fn paint(x: i32, y: i32, m: u16) -> Op {
    Op::Paint {
        x,
        y,
        metatile: Some(m),
        collision: None,
        elevation: None,
    }
}

#[test]
fn a_new_map_is_filled() {
    let e = editor(4, 3);
    let d = e.doc();
    assert_eq!((d.width, d.height, d.blocks.len()), (4, 3, 12));
    assert!(d.blocks.iter().all(|&b| b == block(1, 0, 3)));
    assert_eq!(d.border, [1; 4]);
    assert_eq!(d.map.layout, "Test");
    assert!(validate(d, &catalog()).is_empty());
}

#[test]
fn paint_keeps_the_other_bits_and_undo_redo_walk_back_and_forth() {
    let mut e = editor(4, 3);
    e.apply(&paint(1, 2, 5)).unwrap();
    assert_eq!(e.doc().block(1, 2), Some(block(5, 0, 3)));
    e.apply(&Op::Paint {
        x: 1,
        y: 2,
        metatile: None,
        collision: Some(1),
        elevation: None,
    })
    .unwrap();
    assert_eq!(e.doc().block(1, 2), Some(block(5, 1, 3)));
    assert!(e.undo());
    assert_eq!(e.doc().block(1, 2), Some(block(5, 0, 3)));
    assert!(e.undo());
    assert_eq!(e.doc().block(1, 2), Some(block(1, 0, 3)));
    assert!(!e.undo(), "nothing left to undo");
    assert!(e.redo() && e.redo());
    assert_eq!(e.doc().block(1, 2), Some(block(5, 1, 3)));
    assert!(!e.redo());
    // A new edit drops the redo history.
    e.apply(&Op::Undo).unwrap();
    e.apply(&paint(0, 0, 2)).unwrap();
    assert!(!e.can_redo());
}

#[test]
fn errors_leave_the_document_and_history_alone() {
    let mut e = editor(4, 3);
    let before = e.doc().clone();
    assert!(e.apply(&paint(4, 0, 2)).is_err(), "out of bounds");
    assert!(
        e.apply(&Op::Paint {
            x: 0,
            y: 0,
            metatile: None,
            collision: None,
            elevation: None,
        })
        .is_err(),
        "empty brush"
    );
    assert!(
        e.apply(&Op::Paste { x: 0, y: 0 }).is_err(),
        "empty clipboard"
    );
    assert_eq!(e.doc(), &before);
    assert!(!e.can_undo());
}

#[test]
fn rect_and_flood_fill() {
    let mut e = editor(5, 4);
    e.apply(&Op::Rect {
        x: 1,
        y: 1,
        w: 3,
        h: 2,
        metatile: Some(7),
        collision: None,
        elevation: None,
    })
    .unwrap();
    let grid = |e: &Editor| -> Vec<u16> {
        (0..4)
            .flat_map(|y| (0..5).map(move |x| (x, y)))
            .map(|(x, y)| mt(e, x, y))
            .collect()
    };
    #[rustfmt::skip]
    assert_eq!(grid(&e), [
        1, 1, 1, 1, 1,
        1, 7, 7, 7, 1,
        1, 7, 7, 7, 1,
        1, 1, 1, 1, 1,
    ]);
    // Fill the ring of 1s (4-connected around the rectangle) with 2s.
    e.apply(&Op::Fill {
        x: 0,
        y: 0,
        metatile: Some(2),
        collision: None,
        elevation: None,
    })
    .unwrap();
    #[rustfmt::skip]
    assert_eq!(grid(&e), [
        2, 2, 2, 2, 2,
        2, 7, 7, 7, 2,
        2, 7, 7, 7, 2,
        2, 2, 2, 2, 2,
    ]);
    // A fill stops at a different block: split the 7s with a wall and fill one side.
    e.apply(&paint(2, 1, 9)).unwrap();
    e.apply(&paint(2, 2, 9)).unwrap();
    e.apply(&Op::Fill {
        x: 1,
        y: 1,
        metatile: Some(3),
        collision: None,
        elevation: None,
    })
    .unwrap();
    #[rustfmt::skip]
    assert_eq!(grid(&e), [
        2, 2, 2, 2, 2,
        2, 3, 9, 7, 2,
        2, 3, 9, 7, 2,
        2, 2, 2, 2, 2,
    ]);
    assert!(e.undo());
    assert_eq!(mt(&e, 1, 1), 7);
    // A rectangle must fit.
    assert!(
        e.apply(&Op::Rect {
            x: 3,
            y: 3,
            w: 3,
            h: 1,
            metatile: Some(1),
            collision: None,
            elevation: None,
        })
        .is_err()
    );
}

#[test]
fn copy_paste_clips_at_the_edges() {
    let mut e = editor(4, 4);
    e.apply(&paint(0, 0, 5)).unwrap();
    e.apply(&paint(1, 1, 6)).unwrap();
    e.apply(&Op::Copy {
        x: 0,
        y: 0,
        w: 2,
        h: 2,
    })
    .unwrap();
    assert_eq!(e.clipboard().unwrap().blocks.len(), 4);
    e.apply(&Op::Paste { x: 3, y: 3 }).unwrap();
    assert_eq!(mt(&e, 3, 3), 5, "only the top-left fits");
    e.apply(&Op::Paste { x: 2, y: 0 }).unwrap();
    assert_eq!((mt(&e, 2, 0), mt(&e, 3, 1)), (5, 6));
}

#[test]
fn strokes_are_one_undo_step() {
    let mut e = editor(4, 1);
    e.begin_stroke();
    for x in 0..4 {
        e.apply(&paint(x, 0, 4)).unwrap();
    }
    e.end_stroke();
    assert!((0..4).all(|x| mt(&e, x, 0) == 4));
    assert!(e.undo());
    assert!((0..4).all(|x| mt(&e, x, 0) == 1));
    assert!(!e.can_undo());
}

#[test]
fn ascii_rows_paint_by_legend() {
    let mut e = editor(4, 3);
    let legend = BTreeMap::from([
        ('.', Brush::metatile(1)),
        (
            '#',
            Brush {
                metatile: Some(8),
                collision: Some(1),
                elevation: None,
            },
        ),
    ]);
    e.apply(&Op::Ascii {
        x: 0,
        y: 1,
        rows: vec!["#..#".into(), ".##.".into()],
        legend: legend.clone(),
    })
    .unwrap();
    assert_eq!(e.doc().block(0, 1), Some(block(8, 1, 3)));
    assert_eq!(e.doc().block(1, 2), Some(block(8, 1, 3)));
    assert_eq!(e.doc().block(1, 1), Some(block(1, 0, 3)));
    let bad = Op::Ascii {
        x: 0,
        y: 0,
        rows: vec!["?".into()],
        legend,
    };
    assert!(e.apply(&bad).is_err(), "not in the legend");
}

#[test]
fn event_crud_and_local_ids() {
    let mut e = editor(6, 6);
    let obj = |x, gfx: &str| object(x, 1, gfx);
    e.apply(&obj(1, "OBJ_EVENT_GFX_A")).unwrap();
    e.apply(&obj(2, "OBJ_EVENT_GFX_B")).unwrap();
    e.apply(&obj(3, "OBJ_EVENT_GFX_C")).unwrap();
    e.apply(&Op::Warp {
        x: 0,
        y: 5,
        dest_map: "Other".into(),
        dest_warp: 1,
        elevation: 0,
    })
    .unwrap();
    e.apply(&Op::Coord {
        x: 2,
        y: 2,
        var: "VAR_TEST".into(),
        value: 1,
        script: "Known_Script".into(),
        elevation: 3,
    })
    .unwrap();
    e.apply(&Op::Sign {
        x: 4,
        y: 0,
        script: "Known_Script".into(),
        facing: 0,
        elevation: 0,
    })
    .unwrap();
    e.apply(&Op::HiddenItem {
        x: 5,
        y: 5,
        item: "ITEM_POTION".into(),
        flag: "FLAG_HIDDEN_ITEM_TEST".into(),
        elevation: 0,
    })
    .unwrap();
    let ids: Vec<u8> = e.doc().map.objects.iter().map(|o| o.local_id).collect();
    assert_eq!(ids, [1, 2, 3]);
    assert_eq!(e.doc().event_at(2, 1), Some((EventKind::Object, 1)));
    assert_eq!(e.doc().event_at(5, 5), Some((EventKind::Bg, 1)));
    // Move, then delete the first object: the others keep their local ids.
    e.apply(&Op::Move {
        kind: EventKind::Object,
        index: 1,
        x: 4,
        y: 4,
    })
    .unwrap();
    assert_eq!(e.doc().event_at(4, 4), Some((EventKind::Object, 1)));
    assert!(
        e.apply(&Op::Move {
            kind: EventKind::Warp,
            index: 0,
            x: 6,
            y: 0
        })
        .is_err()
    );
    e.apply(&Op::Delete {
        kind: EventKind::Object,
        index: 0,
    })
    .unwrap();
    let objs = &e.doc().map.objects;
    assert_eq!(
        objs.iter()
            .map(|o| (o.local_id, o.gfx.as_str()))
            .collect::<Vec<_>>(),
        [(2, "OBJ_EVENT_GFX_B"), (3, "OBJ_EVENT_GFX_C")]
    );
    // A new object takes the highest id + 1; the gap stays.
    e.apply(&obj(0, "OBJ_EVENT_GFX_D")).unwrap();
    let ids: Vec<u8> = e.doc().map.objects.iter().map(|o| o.local_id).collect();
    assert_eq!(ids, [2, 3, 4]);
    e.apply(&Op::Delete {
        kind: EventKind::Object,
        index: 2,
    })
    .unwrap();
    // Replace keeps the index and stores the object as given.
    let Some(Event::Object(mut o)) = e.doc().event(EventKind::Object, 1) else {
        panic!("object 1");
    };
    o.trainer_type = "TRAINER_TYPE_NORMAL".into();
    o.trainer_sight_or_berry_tree = 4;
    o.local_id = 7;
    o.id = Some("LOCALID_TEST_C".into());
    e.apply(&Op::Replace {
        index: 1,
        event: Event::Object(o),
    })
    .unwrap();
    let o = &e.doc().map.objects[1];
    assert_eq!((o.local_id, o.trainer_sight_or_berry_tree), (7, 4));
    assert!(matches!(
        e.doc().map.bgs[0].kind,
        BgKind::Script { facing: 0, .. }
    ));
    // Connections.
    let c = |map: &str| GameConnection {
        dir: ConnectionDir::North,
        offset: 0,
        map: map.into(),
    };
    e.apply(&Op::Connect(c("Other"))).unwrap();
    e.apply(&Op::SetConnection {
        index: 0,
        connection: GameConnection {
            offset: -2,
            ..c("Lonely")
        },
    })
    .unwrap();
    assert_eq!(e.doc().map.connections[0].offset, -2);
    assert!(
        validate(e.doc(), &catalog()).is_empty(),
        "{:?}",
        validate(e.doc(), &catalog())
    );
    e.apply(&Op::Disconnect { index: 0 }).unwrap();
    assert!(e.doc().map.connections.is_empty());
    // Undo the whole history back to the empty map.
    while e.undo() {}
    assert_eq!(e.doc(), &Doc::new(&spec(6, 6)).unwrap());
}

#[test]
fn validation_reports_bad_cells_and_references() {
    let mut e = editor(3, 3);
    // Metatile 10 is past P's 10, 516 past S's 4; 512..516 are S's.
    e.apply(&paint(0, 0, 10)).unwrap();
    e.apply(&paint(1, 0, 513)).unwrap();
    e.apply(&paint(2, 0, 516)).unwrap();
    e.apply(&Op::Warp {
        x: 0,
        y: 0,
        dest_map: "Nowhere".into(),
        dest_warp: 0,
        elevation: 0,
    })
    .unwrap();
    e.apply(&Op::Warp {
        x: 1,
        y: 0,
        dest_map: "Other".into(),
        dest_warp: 2,
        elevation: 0,
    })
    .unwrap();
    e.apply(&Op::Object {
        x: 1,
        y: 1,
        id: None,
        gfx: "BOY".into(),
        movement: "MOVEMENT_TYPE_FACE_DOWN".into(),
        range_x: 0,
        range_y: 0,
        script: Some("Unwritten_Script".into()),
        flag: Some("HIDE".into()),
        trainer_type: "TRAINER_TYPE_SOMETIMES".into(),
        sight: 0,
        elevation: 3,
    })
    .unwrap();
    e.apply(&Op::Connect(GameConnection {
        dir: ConnectionDir::East,
        offset: 0,
        map: "Nowhere".into(),
    }))
    .unwrap();
    let mut d = e.doc().clone();
    // An event moved out of bounds by a hand edit of the file.
    d.map.coords.push(extract::schema::Coord {
        x: 7,
        y: 0,
        elevation: 0,
        var: Some("VAR_X".into()),
        value: 0,
        script: None,
    });
    let issues = validate(&d, &catalog());
    let text: Vec<String> = issues.iter().map(|i| i.to_string()).collect();
    let has = |s: &str| text.iter().any(|t| t.contains(s));
    assert!(has("error: cell (0, 0): metatile 10"), "{text:#?}");
    assert!(has("error: cell (2, 0): metatile 516"), "{text:#?}");
    assert!(!has("cell (1, 0)"), "513 is S's: {text:#?}");
    assert!(has("error: warp 0: unknown destination map \"Nowhere\""));
    assert!(has("error: warp 1: Other has 2 warps, not warp 2"));
    assert!(has("error: object 0: graphics \"BOY\""));
    assert!(has("error: object 0: flag \"HIDE\""));
    assert!(has("error: object 0: trainer type"));
    assert!(has("error: connection 0: unknown map \"Nowhere\""));
    assert!(has("error: coord 0: (7, 0) is outside the 3x3 map"));
    assert!(has("warning: object 0: script \"Unwritten_Script\""));
    assert_eq!(errors(&issues), issues.len() - 1, "one warning: {text:#?}");
    assert_eq!(issues.last().unwrap().severity, Severity::Warning);
    // Unknown tilesets: no metatile errors, one per tileset.
    let mut d = Doc::new(&spec(2, 2)).unwrap();
    d.primary = "S".into();
    d.secondary = "Q".into();
    let text: Vec<String> = validate(&d, &catalog())
        .iter()
        .map(|i| i.to_string())
        .collect();
    assert_eq!(
        text,
        [
            "error: layout: \"S\" is not a primary tileset",
            "error: layout: unknown secondary tileset \"Q\""
        ]
    );
}

#[test]
fn files_round_trip_byte_for_byte() {
    let mut e = editor(3, 2);
    e.apply(&paint(2, 1, 513)).unwrap();
    e.apply(&Op::Sign {
        x: 0,
        y: 0,
        script: "S".into(),
        facing: 2,
        elevation: 0,
    })
    .unwrap();
    let d = e.doc();
    let map = to_ron(&d.map).unwrap();
    let layout = to_ron(&d.layout()).unwrap();
    let back = Doc::from_parts(from_ron(&map).unwrap(), from_ron(&layout).unwrap()).unwrap();
    assert_eq!(&back, d);
    assert_eq!(to_ron(&back.map).unwrap(), map);
    assert_eq!(to_ron(&back.layout()).unwrap(), layout);
    assert!(
        layout.contains("        (12289, 12289, 12801),\n"),
        "raw blocks, one row per line: {layout}"
    );
}

#[test]
fn edit_scripts_take_bare_optionals() {
    let ops: Vec<Op> = from_script_ron(
        r#"[
            Paint(x: 1, y: 2, metatile: 5),
            Object(x: 1, y: 1, gfx: "OBJ_EVENT_GFX_BOY_1", script: "A_Script"),
            Fill(x: 0, y: 0, collision: 1),
            Undo,
        ]"#,
    )
    .unwrap();
    assert_eq!(ops[0], paint(1, 2, 5));
    let Op::Object {
        script, movement, ..
    } = &ops[1]
    else {
        panic!("{:?}", ops[1]);
    };
    assert_eq!(script.as_deref(), Some("A_Script"));
    assert_eq!(movement, "MOVEMENT_TYPE_FACE_DOWN");
    assert_eq!(ops[3], Op::Undo);
}

#[test]
fn resize_shifts_cells_events_and_connections() {
    let mut e = editor(4, 3);
    e.apply(&paint(0, 0, 5)).unwrap();
    e.apply(&paint(3, 2, 6)).unwrap();
    e.apply(&object(3, 2, "OBJ_EVENT_GFX_A")).unwrap();
    e.apply(&warp(0, 0, "Other", 0)).unwrap();
    e.apply(&Op::Connect(conn(ConnectionDir::North, 1, "Other")))
        .unwrap();
    e.apply(&Op::Connect(conn(ConnectionDir::East, 0, "Lonely")))
        .unwrap();
    // Grow by one cell on every side, new cells with collision 1 and elevation 4.
    e.apply(&Op::Resize {
        width: 6,
        height: 5,
        fill: 2,
        fill_collision: 1,
        fill_elevation: 4,
        dx: 1,
        dy: 1,
    })
    .unwrap();
    let d = e.doc();
    assert_eq!((d.width, d.height, d.blocks.len()), (6, 5, 30));
    assert_eq!(d.block(1, 1), Some(block(5, 0, 3)), "old (0, 0)");
    assert_eq!(d.block(4, 3), Some(block(6, 0, 3)), "old (3, 2)");
    assert_eq!(d.block(2, 2), Some(block(1, 0, 3)), "old (1, 1)");
    for (x, y) in [(0, 0), (5, 4), (5, 0), (0, 4), (3, 0)] {
        assert_eq!(d.block(x, y), Some(block(2, 1, 4)), "new ({x}, {y})");
    }
    assert_eq!((d.map.objects[0].x, d.map.objects[0].y), (4, 3));
    assert_eq!((d.map.warps[0].x, d.map.warps[0].y), (1, 1));
    let offsets: Vec<i32> = d.map.connections.iter().map(|c| c.offset).collect();
    assert_eq!(offsets, [2, 1], "north by dx, east by dy");
    // Shrink to the 2x2 at (3, 2): old cells (2..4, 1..3); the warp is left outside (an
    // error), the object kept.
    e.apply(&Op::Resize {
        width: 2,
        height: 2,
        fill: 2,
        fill_collision: 0,
        fill_elevation: 3,
        dx: -3,
        dy: -2,
    })
    .unwrap();
    let d = e.doc();
    assert_eq!(
        d.blocks,
        [
            block(1, 0, 3),
            block(1, 0, 3),
            block(1, 0, 3),
            block(6, 0, 3)
        ]
    );
    assert_eq!((d.map.objects[0].x, d.map.objects[0].y), (1, 1));
    let text = texts(d);
    assert!(
        text.iter()
            .any(|t| t == "error: warp 0: (-2, -1) is outside the 2x2 map"),
        "{text:#?}"
    );
    // Fill parts are range checked; sizes must fit the game.
    for (fill, collision, elevation) in [(1024, 0, 3), (1, 4, 3), (1, 0, 16)] {
        let op = Op::Resize {
            width: 3,
            height: 3,
            fill,
            fill_collision: collision,
            fill_elevation: elevation,
            dx: 0,
            dy: 0,
        };
        assert!(e.apply(&op).is_err(), "{op:?}");
    }
    let huge = Op::Resize {
        width: u16::MAX,
        height: u16::MAX,
        fill: 1,
        fill_collision: 0,
        fill_elevation: 3,
        dx: 0,
        dy: 0,
    };
    assert!(e.apply(&huge).is_err());
    assert!(e.undo() && e.undo());
    assert_eq!((e.doc().width, e.doc().height), (4, 3));
}

#[test]
fn border_tilesets_header_and_scripts_are_undo_steps() {
    let mut e = editor(2, 2);
    let start = e.doc().clone();
    e.apply(&Op::Border {
        blocks: vec![1, 2, 3, 4],
    })
    .unwrap();
    assert_eq!(e.doc().border, [1, 2, 3, 4]);
    assert!(
        e.apply(&Op::Border {
            blocks: vec![1, 2, 3]
        })
        .is_err()
    );
    e.apply(&Op::Tilesets {
        primary: "Q".into(),
        secondary: "R".into(),
    })
    .unwrap();
    assert_eq!(
        (e.doc().primary.as_str(), e.doc().secondary.as_str()),
        ("Q", "R")
    );
    let header = GameHeader {
        weather: "WEATHER_RAIN".into(),
        music: "MUS_DUMMY".into(),
        ..GameHeader::new_map()
    };
    e.apply(&Op::Header(header.clone())).unwrap();
    assert_eq!(e.doc().map.header, header);
    let scripts = MapScripts {
        on_load: Some("Known_Script".into()),
        ..MapScripts::default()
    };
    e.apply(&Op::Scripts(scripts.clone())).unwrap();
    assert_eq!(e.doc().map.scripts, scripts);
    for _ in 0..4 {
        assert!(e.undo());
    }
    assert_eq!(e.doc(), &start);
    assert!(!e.can_undo());
}

#[test]
fn undo_keeps_the_last_256_steps() {
    let mut e = editor(2, 1);
    for i in 0..UNDO_LIMIT + 10 {
        e.apply(&paint(0, 0, 2 + (i % 2) as u16)).unwrap();
    }
    let mut n = 0;
    while e.undo() {
        n += 1;
    }
    assert_eq!(n, UNDO_LIMIT);
    assert_ne!(mt(&e, 0, 0), 1, "the first steps are gone");
}

#[test]
fn fill_does_not_cross_diagonals() {
    let mut e = editor(3, 3);
    // A diamond of 9s around the centre: the corners touch it only diagonally.
    for (x, y) in [(1, 0), (0, 1), (2, 1), (1, 2)] {
        e.apply(&paint(x, y, 9)).unwrap();
    }
    e.apply(&Op::Fill {
        x: 0,
        y: 0,
        metatile: Some(4),
        collision: None,
        elevation: None,
    })
    .unwrap();
    let grid: Vec<u16> = (0..3)
        .flat_map(|y| (0..3).map(move |x| (x, y)))
        .map(|(x, y)| mt(&e, x, y))
        .collect();
    assert_eq!(grid, [4, 9, 1, 9, 1, 9, 1, 9, 1]);
}

#[test]
fn dirty_follows_the_saved_state() {
    let mut e = editor(2, 2);
    assert!(!e.is_dirty());
    e.apply(&paint(0, 0, 2)).unwrap();
    assert!(e.is_dirty());
    e.undo();
    assert!(!e.is_dirty(), "undone back to the saved state");
    e.redo();
    assert!(e.is_dirty());
    e.mark_saved();
    assert!(!e.is_dirty());
    e.undo();
    assert!(e.is_dirty(), "the saved state is the painted one now");
    let new = Editor::unsaved(Doc::new(&spec(2, 2)).unwrap());
    assert!(new.is_dirty() && new.saved().is_none());
}

#[test]
fn ranges_and_overflow_are_errors_not_panics() {
    let mut e = editor(4, 4);
    let rect = |x, w| Op::Rect {
        x,
        y: 0,
        w,
        h: 1,
        metatile: Some(1),
        collision: None,
        elevation: None,
    };
    for op in [
        rect(0, i32::MAX),
        rect(i32::MAX, 2),
        rect(i32::MIN, i32::MAX),
        rect(1, -3),
        Op::Copy {
            x: 0,
            y: 0,
            w: i32::MAX,
            h: i32::MAX,
        },
        Op::Ascii {
            x: i32::MAX,
            y: 0,
            rows: vec!["..".into()],
            legend: BTreeMap::from([('.', Brush::metatile(1))]),
        },
        Op::Paint {
            x: 0,
            y: 0,
            metatile: Some(1024),
            collision: None,
            elevation: None,
        },
    ] {
        assert!(e.apply(&op).is_err(), "{op:?}");
    }
    e.apply(&Op::Copy {
        x: 2,
        y: 2,
        w: 2,
        h: 2,
    })
    .unwrap();
    e.apply(&Op::Paste {
        x: i32::MAX,
        y: i32::MIN,
    })
    .unwrap();
    assert!(!e.can_undo(), "nothing landed on the map");
    // A new map's fill is checked like a brush.
    for (fill, collision, elevation) in [(1025, 0, 3), (1, 4, 3), (1, 0, 16)] {
        let s = NewMap {
            fill,
            fill_collision: collision,
            fill_elevation: elevation,
            ..spec(2, 2)
        };
        assert!(Doc::new(&s).is_err(), "{s:?}");
    }
    assert!(Doc::new(&spec(0, 2)).is_err());
    assert!(
        Doc::new(&spec(200, 200)).is_err(),
        "(200 + 15) * (200 + 14) > 10240"
    );
    assert!(
        Doc::new(&spec(100, 50)).is_ok(),
        "(100 + 15) * (50 + 14) = 7360"
    );
}

#[test]
fn dynamic_warps_take_warp_127() {
    let mut e = editor(3, 3);
    assert!(e.apply(&warp(0, 0, MAP_DYNAMIC, 0)).is_err());
    e.apply(&warp(0, 0, MAP_DYNAMIC, 127)).unwrap();
    assert_eq!(texts(e.doc()), Vec::<String>::new());
    let mut d = e.doc().clone();
    // `WARP_ID_SECRET_BASE` (the secret bases' exits on the full import).
    d.map.warps[0].dest_warp = 126;
    assert_eq!(texts(&d), Vec::<String>::new());
    d.map.warps[0].dest_warp = 3;
    assert_eq!(
        texts(&d),
        ["error: warp 0: a MAP_DYNAMIC warp has dest_warp 127"]
    );
}

#[test]
fn validation_checks_game_limits_and_warns_on_suspect_links() {
    let has = |text: &[String], s: &str| text.iter().any(|t| t.contains(s));
    // Limits: objects, size, warps, elevation, local ids.
    let mut d = Doc::new(&spec(10, 10)).unwrap();
    let mut e = Editor::new(d.clone());
    for i in 0..65 {
        e.apply(&object((i % 10) as i16, (i / 10) as i16, "OBJ_EVENT_GFX_A"))
            .unwrap();
    }
    let text = texts(e.doc());
    assert!(
        has(&text, "error: map: 65 objects (at most 64)"),
        "{text:#?}"
    );
    assert!(
        has(&text, "error: object 64: local id 65 (1..64)"),
        "{text:#?}"
    );
    d.map.warps = (0..256)
        .map(|i| super::schema::GameWarp {
            x: (i % 10) as i16,
            y: (i / 10 % 10) as i16,
            elevation: 0,
            dest_map: MAP_DYNAMIC.into(),
            dest_warp: 127,
        })
        .collect();
    let text = texts(&d);
    assert!(
        has(&text, "error: map: 256 warps (at most 255)"),
        "{text:#?}"
    );
    assert!(has(
        &text,
        "warning: warp 100: on the same cell (0, 0) as warp 0"
    ));
    let mut d = Doc::new(&spec(2, 2)).unwrap();
    (d.width, d.height) = (300, 30);
    d.blocks = vec![block(1, 0, 3); 9000];
    assert!(has(&texts(&d), "error: layout: 300x30 is too big"));
    // Objects: elevation, names, flags.
    let mut e = editor(6, 6);
    for x in 0..3 {
        e.apply(&object(x, 0, "OBJ_EVENT_GFX_A")).unwrap();
    }
    let mut d = e.doc().clone();
    d.map.objects[0].elevation = 16;
    d.map.objects[0].id = Some("LOCALID_A".into());
    d.map.objects[1].id = Some("LOCALID_A".into());
    d.map.objects[2].id = Some("BOB".into());
    d.map.objects[2].local_id = 1;
    d.map.objects[1].flag = Some("FLAG_HIDE_X".into());
    d.map.objects[2].flag = Some("FLAG_HIDE_X".into());
    let text = texts(&d);
    for s in [
        "error: object 0: elevation 16 is over 15",
        "error: object 1: id LOCALID_A is also object 0's",
        "error: object 2: id \"BOB\" is not a LOCALID_* name",
        "error: object 2: local id 1 is also object 0's",
        "warning: object 2: hide flag FLAG_HIDE_X is also object 1's",
    ] {
        assert!(has(&text, s), "{s}: {text:#?}");
    }
    // Links: one-way warps and connections, duplicate directions, misplaced neighbors;
    // duplicate hidden item flags.
    let mut e = editor(6, 6);
    e.apply(&warp(0, 0, "OneWay", 0)).unwrap();
    e.apply(&Op::Coord {
        x: 0,
        y: 0,
        var: "VAR_TEST".into(),
        value: 0,
        script: "Known_Script".into(),
        elevation: 3,
    })
    .unwrap();
    e.apply(&Op::Connect(conn(ConnectionDir::North, 3, "Other")))
        .unwrap();
    e.apply(&Op::Connect(conn(ConnectionDir::North, 2, "Lonely")))
        .unwrap();
    e.apply(&Op::Connect(conn(ConnectionDir::West, 9, "Lonely")))
        .unwrap();
    for x in [1, 2] {
        e.apply(&Op::HiddenItem {
            x,
            y: 1,
            item: "ITEM_POTION".into(),
            flag: "FLAG_HIDDEN_ITEM_TEST".into(),
            elevation: 0,
        })
        .unwrap();
    }
    let text = texts(e.doc());
    for s in [
        "warning: warp 0: OneWay has no warp back here",
        "warning: coord 0: on the same cell (0, 0) as warp 0",
        "warning: connection 0: Other has no South connection back here at offset -3",
        "warning: connection 1: a second North connection",
        "warning: connection 2: Lonely at offset 9 is outside the map's 6-cell edge",
        "warning: bg 1: hidden item flag FLAG_HIDDEN_ITEM_TEST is also bg 0's",
    ] {
        assert!(has(&text, s), "{s}: {text:#?}");
    }
    assert_eq!(errors(&validate(e.doc(), &catalog())), 0, "{text:#?}");
    // A map does not connect to itself.
    e.apply(&Op::Connect(conn(ConnectionDir::South, 0, "Test")))
        .unwrap();
    assert!(has(
        &texts(e.doc()),
        "error: connection 3: connects the map to itself"
    ));
}

#[test]
fn renumbered_named_objects_are_warned_about() {
    let mut e = editor(4, 4);
    let Op::Object { gfx, .. } = object(1, 1, "OBJ_EVENT_GFX_A") else {
        unreachable!()
    };
    e.apply(&Op::Object {
        x: 1,
        y: 1,
        id: Some("LOCALID_GUIDE".into()),
        gfx,
        movement: "MOVEMENT_TYPE_FACE_DOWN".into(),
        range_x: 0,
        range_y: 0,
        script: None,
        flag: None,
        trainer_type: "TRAINER_TYPE_NONE".into(),
        sight: 0,
        elevation: 3,
    })
    .unwrap();
    e.mark_saved();
    let Some(Event::Object(mut o)) = e.doc().event(EventKind::Object, 0) else {
        panic!("object 0");
    };
    o.local_id = 5;
    e.apply(&Op::Replace {
        index: 0,
        event: Event::Object(o),
    })
    .unwrap();
    let w = id_changes(e.saved().unwrap(), e.doc());
    assert_eq!(w.len(), 1);
    assert_eq!(
        w[0].to_string(),
        "warning: object 0: LOCALID_GUIDE was local id 1, now 5"
    );
    e.undo();
    assert!(id_changes(e.saved().unwrap(), e.doc()).is_empty());
}
