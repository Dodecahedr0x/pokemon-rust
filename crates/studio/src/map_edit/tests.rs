//! Model unit tests: no files, no build data (a made-up catalog).

use super::catalog::{Catalog, MapRef};
use super::model::{Brush, Doc, Editor, Event, EventKind, NewMap, Op, block};
use super::schema::{GameConnection, from_ron, from_script_ron, to_ron};
use super::validate::{Severity, errors, validate};
use engine::data::Tileset;
use extract::schema::{BgKind, ConnectionDir};
use std::collections::BTreeMap;

fn tileset(metatiles: usize) -> &'static Tileset {
    Box::leak(Box::new(Tileset {
        tiles: &[],
        palettes: &[],
        metatiles: Box::leak(vec![[0u16; 8]; metatiles].into_boxed_slice()),
        attributes: &[],
    }))
}

/// Tilesets `P` (10 metatiles) and `S` (4), maps `Other` (2 warps) and `Lonely` (none).
fn catalog() -> Catalog {
    let mut c = Catalog::default();
    c.add_tileset("P", tileset(10), false);
    c.add_tileset("S", tileset(4), true);
    c.maps.insert("Other".into(), MapRef { id: None, warps: 2 });
    c.maps
        .insert("Lonely".into(), MapRef { id: None, warps: 0 });
    c.scripts.insert("Known_Script".into());
    c
}

fn spec(w: u16, h: u16) -> NewMap {
    NewMap {
        name: "Test".into(),
        layout: None,
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
    let obj = |x, gfx: &str| Op::Object {
        x,
        y: 1,
        gfx: gfx.into(),
        movement: "MOVEMENT_TYPE_FACE_DOWN".into(),
        range_x: 0,
        range_y: 0,
        script: Some("Known_Script".into()),
        flag: None,
        trainer_type: "TRAINER_TYPE_NONE".into(),
        sight: 0,
        elevation: 3,
    };
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
    // Move, then delete the first object: the others are renumbered.
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
        [(1, "OBJ_EVENT_GFX_B"), (2, "OBJ_EVENT_GFX_C")]
    );
    // Replace keeps the index and renumbers.
    let Some(Event::Object(mut o)) = e.doc().event(EventKind::Object, 1) else {
        panic!("object 1");
    };
    o.trainer_type = "TRAINER_TYPE_NORMAL".into();
    o.trainer_sight_or_berry_tree = 4;
    o.local_id = 99;
    e.apply(&Op::Replace {
        index: 1,
        event: Event::Object(o),
    })
    .unwrap();
    let o = &e.doc().map.objects[1];
    assert_eq!((o.local_id, o.trainer_sight_or_berry_tree), (2, 4));
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
