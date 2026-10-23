//! The map editor (SIL1 Task C) end to end: games dir files, renders against the game's own,
//! the CLI and the games dir hygiene. Runs on both worlds; the extracted maps and the fixture's
//! Emerald tilesets need the real extract and are skipped on the stub.

use content::Emerald;
use engine::data::GameData;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use studio::map_edit::catalog::extracted_dir;
use studio::map_edit::games::hygiene;
use studio::map_edit::render::{PREVIEW, Preview, leak_map_data, render};
use studio::map_edit::{Catalog, Doc, Editor, GamesDir, NewMap, Op, cli, validate};
use studio::map_view::{map_list, render_map};

fn temp_dir(tag: &str) -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pkr-map-edit-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn fixture() -> GamesDir {
    GamesDir::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/games/fixture"))
}

fn real() -> bool {
    if content::STUB {
        eprintln!("skipped on the stub world (needs the extract)");
    }
    !content::STUB
}

/// The catalog's first primary and secondary tilesets and a few metatiles they define.
fn tilesets(c: &Catalog) -> (String, String, Vec<u16>) {
    let p = c.tileset_names(false).remove(0);
    let s = c.tileset_names(true).remove(0);
    let (pr, sr) = c.metatile_ranges(&p, &s).unwrap();
    let mts = pr.take(6).chain(sr.take(3)).collect();
    (p, s, mts)
}

#[test]
fn the_fixture_round_trips_byte_for_byte() {
    let src = fixture();
    let doc = src.load("FixtureTown").unwrap();
    assert_eq!((doc.width, doc.height), (8, 6));
    assert_eq!(
        (doc.primary.as_str(), doc.secondary.as_str()),
        ("General", "Petalburg")
    );
    let out = GamesDir::new(temp_dir("roundtrip"));
    out.save(&doc).unwrap();
    for (a, b) in [
        (src.map_path("FixtureTown"), out.map_path("FixtureTown")),
        (
            src.layout_path("FixtureTown"),
            out.layout_path("FixtureTown"),
        ),
    ] {
        assert_eq!(
            std::fs::read(&a).unwrap(),
            std::fs::read(&b).unwrap(),
            "{}",
            a.display()
        );
    }
    let _ = std::fs::remove_dir_all(&out.root);
}

#[test]
fn the_fixture_validates_against_the_extract() {
    if !real() {
        return;
    }
    let games = fixture();
    let c = Catalog::for_games(&games).unwrap();
    let issues = validate(&games.load("FixtureTown").unwrap(), &c);
    assert_eq!(
        studio::map_edit::validate::errors(&issues),
        0,
        "{issues:#?}"
    );
    assert!(!issues.is_empty(), "its scripts are not written: warnings");
}

/// Every extracted map, loaded into the editor, renders exactly as the game draws it.
#[test]
fn extracted_maps_render_as_the_game() {
    if !real() {
        return;
    }
    let game = Emerald::for_shells();
    let c = Catalog::from_content().unwrap();
    for (id, name) in map_list() {
        let text =
            std::fs::read_to_string(extracted_dir().join(format!("maps/{name}.ron"))).unwrap();
        let file: extract::schema::MapFile = ron::from_str(&text).unwrap();
        let doc = Doc::from_map_file(&file).unwrap();
        assert_eq!(
            studio::map_edit::validate::errors(&validate(&doc, &c)),
            0,
            "{name}: {:#?}",
            validate(&doc, &c)
        );
        let edited = render(&doc, &c, 3).unwrap();
        let want = render_map(&game, id, 3, &[]);
        assert_eq!(
            (edited.width, edited.height),
            (want.width, want.height),
            "{name}"
        );
        assert!(edited.rgb555 == want.rgb555, "{name}: pixels differ");
    }
}

/// An edited map's render equals a render of a `MapData` built directly from the same layout
/// data; on the extract, also the game's render of the same map with the edits as overrides.
#[test]
fn an_edited_render_equals_a_direct_render() {
    let c = Catalog::from_content().unwrap();
    let (p, s, mts) = tilesets(&c);
    let mut ed = Editor::new(
        Doc::new(&NewMap {
            name: "Edited".into(),
            layout: None,
            width: 17,
            height: 12,
            primary: p,
            secondary: s,
            fill: mts[0],
            fill_collision: 0,
            fill_elevation: 3,
            border: Some(vec![mts[1], mts[2], mts[1], mts[2]]),
        })
        .unwrap(),
    );
    let blank = render(ed.doc(), &c, 2).unwrap();
    for (i, &m) in mts.iter().enumerate() {
        let i = i as i32;
        ed.apply(&Op::Rect {
            x: i,
            y: i % 5,
            w: 3,
            h: 4,
            metatile: Some(m),
            collision: None,
            elevation: None,
        })
        .unwrap();
    }
    let edited = render(ed.doc(), &c, 2).unwrap();
    let direct = render_map(
        &Preview::new(leak_map_data(ed.doc(), &c).unwrap()),
        PREVIEW,
        2,
        &[],
    );
    assert!(
        edited.rgb555 == direct.rgb555,
        "edited and direct renders differ"
    );
    assert!(edited.rgb555 != blank.rgb555, "the edits show");
    assert_eq!(edited.width, (17 + 4) * 16);

    if !real() {
        return;
    }
    // Route 101 with a few cells repainted: the game's render with them as overrides.
    let game = Emerald::for_shells();
    let (id, name) = map_list()
        .into_iter()
        .find(|(_, n)| *n == "Route101")
        .unwrap();
    let text = std::fs::read_to_string(extracted_dir().join(format!("maps/{name}.ron"))).unwrap();
    let file: extract::schema::MapFile = ron::from_str(&text).unwrap();
    let mut ed = Editor::new(Doc::from_map_file(&file).unwrap());
    let mut overrides = Vec::new();
    for (x, y, m) in [(0, 0, 1u16), (5, 7, 13), (19, 19, 468), (10, 3, 520)] {
        ed.apply(&Op::Paint {
            x,
            y,
            metatile: Some(m),
            collision: None,
            elevation: None,
        })
        .unwrap();
        overrides.push(((x as i16, y as i16), ed.doc().block(x, y).unwrap()));
    }
    let edited = render(ed.doc(), &c, 4).unwrap();
    let want = render_map(&game, id, 4, &overrides);
    assert!(edited.rgb555 == want.rgb555, "Route101 with overrides");
    let _ = game.map(id);
}

fn run(args: &[&str]) -> anyhow::Result<String> {
    cli::run(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
}

/// Creates a map and applies an edit script through the CLI in `dir`; returns the two files.
fn cli_session(dir: &Path, primary: &str, secondary: &str, mts: &[u16]) -> (Vec<u8>, Vec<u8>) {
    let spec = dir.join("spec.ron");
    std::fs::write(
        &spec,
        format!(
            "(name: \"CliTown\", width: 10, height: 8, primary: \"{primary}\", \
             secondary: \"{secondary}\", fill: {})",
            mts[0]
        ),
    )
    .unwrap();
    let script = dir.join("edits.ron");
    std::fs::write(
        &script,
        format!(
            r#"[
    Rect(x: 0, y: 0, w: 10, h: 1, metatile: {a}, collision: 1),
    Ascii(x: 1, y: 2, rows: ["ab", "ba"], legend: {{'a': (metatile: {b}), 'b': (elevation: 4)}}),
    Fill(x: 5, y: 5, metatile: {c}),
    Copy(x: 1, y: 2, w: 2, h: 2),
    Paste(x: 7, y: 6),
    Paint(x: 9, y: 7, metatile: {a}),
    Undo,
    Object(x: 3, y: 4, gfx: "OBJ_EVENT_GFX_BOY_1", script: "CliTown_EventScript_Boy"),
    Object(x: 4, y: 4, gfx: "OBJ_EVENT_GFX_BOY_1"),
    Delete(kind: Object, index: 0),
    Move(kind: Object, index: 0, x: 6, y: 6),
    Sign(x: 2, y: 1, script: "CliTown_EventScript_Sign"),
    Warp(x: 0, y: 7, dest_map: "CliTown", dest_warp: 0),
]
"#,
            a = mts[1],
            b = mts[2],
            c = mts[3],
        ),
    )
    .unwrap();
    let games = dir.join("games");
    let g = games.to_str().unwrap();
    let out = run(&["--games", g, "create", spec.to_str().unwrap()]).unwrap();
    assert!(out.starts_with("created "), "{out}");
    assert!(
        run(&["--games", g, "create", spec.to_str().unwrap()]).is_err(),
        "create never replaces a map"
    );
    let out = run(&["--games", g, "apply", "CliTown", script.to_str().unwrap()]).unwrap();
    assert!(out.contains("13 ops applied, saved"), "{out}");
    run(&["--games", g, "validate"]).unwrap();
    let gd = GamesDir::new(&games);
    (
        std::fs::read(gd.map_path("CliTown")).unwrap(),
        std::fs::read(gd.layout_path("CliTown")).unwrap(),
    )
}

#[test]
fn the_cli_applies_an_edit_script_deterministically() {
    let c = Catalog::from_content().unwrap();
    let (p, s, mts) = tilesets(&c);
    let (d1, d2) = (temp_dir("cli1"), temp_dir("cli2"));
    let first = cli_session(&d1, &p, &s, &mts);
    let second = cli_session(&d2, &p, &s, &mts);
    assert_eq!(first, second, "same script, same bytes");

    let games = GamesDir::new(d1.join("games"));
    let doc = games.load("CliTown").unwrap();
    let mt = |x, y| doc.block(x, y).unwrap() & 0x3FF;
    assert!((0..10).all(|x| mt(x, 0) == mts[1] && doc.block(x, 0).unwrap() >> 10 & 3 == 1));
    assert_eq!((mt(1, 2), mt(2, 3)), (mts[2], mts[2]));
    assert_eq!(doc.block(2, 2).unwrap() >> 12, 4, "elevation only");
    assert_eq!(mt(5, 5), mts[3], "filled");
    assert_eq!(mt(7, 6), mts[2], "pasted");
    assert_eq!(mt(9, 7), mts[3], "the paint was undone");
    assert_eq!(doc.map.objects.len(), 1);
    assert_eq!((doc.map.objects[0].x, doc.map.objects[0].local_id), (6, 1));
    assert_eq!(doc.map.warps[0].dest_map, "CliTown");

    // A failing script saves nothing.
    let bad = d1.join("bad.ron");
    std::fs::write(
        &bad,
        "[Paint(x: 1, y: 1, metatile: 2), Paint(x: 99, y: 0, metatile: 1)]",
    )
    .unwrap();
    let g = d1.join("games");
    let g = g.to_str().unwrap();
    let e = run(&["--games", g, "apply", "CliTown", bad.to_str().unwrap()]).unwrap_err();
    assert!(format!("{e:#}").contains("op 1"), "{e:#}");
    assert_eq!(std::fs::read(games.map_path("CliTown")).unwrap(), first.0);
    assert_eq!(
        std::fs::read(games.layout_path("CliTown")).unwrap(),
        first.1
    );

    // A render for review, outside the games dir.
    let png = d1.join("review/CliTown.png");
    let out = run(&[
        "--games",
        g,
        "render",
        "CliTown",
        "--out",
        png.to_str().unwrap(),
        "--margin",
        "1",
    ])
    .unwrap();
    assert!(out.contains("10x8 cells, margin 1"), "{out}");
    let bytes = std::fs::read(&png).unwrap();
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(run(&["--games", g, "list"]).unwrap(), "CliTown\n");
    for d in [d1, d2] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn validate_fails_on_errors() {
    let c = Catalog::from_content().unwrap();
    let (p, s, _) = tilesets(&c);
    let dir = temp_dir("invalid");
    let games = GamesDir::new(dir.join("games"));
    let mut ed = Editor::new(
        Doc::new(&NewMap {
            name: "Broken".into(),
            layout: None,
            width: 2,
            height: 2,
            primary: p,
            secondary: s,
            fill: 0,
            fill_collision: 0,
            fill_elevation: 3,
            border: None,
        })
        .unwrap(),
    );
    ed.apply(&Op::Warp {
        x: 0,
        y: 0,
        dest_map: "NoSuchMap".into(),
        dest_warp: 0,
        elevation: 0,
    })
    .unwrap();
    games.save(ed.doc()).unwrap();
    let g = games.root.to_str().unwrap();
    let e = run(&["--games", g, "validate"]).unwrap_err();
    assert!(
        format!("{e:#}").contains("unknown destination map \"NoSuchMap\""),
        "{e:#}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// Nothing under a games dir holds ROM data: only `maps/*.ron` and `layouts/*.ron` in their
/// canonical form (names, raw block indices, event data).
#[test]
fn games_dirs_hold_only_names_and_indices() {
    assert_eq!(hygiene(&fixture()).unwrap(), Vec::<String>::new());
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap();
    if let Ok(entries) = std::fs::read_dir(workspace.join("games")) {
        for e in entries {
            let g = GamesDir::new(e.unwrap().path());
            assert_eq!(
                hygiene(&g).unwrap(),
                Vec::<String>::new(),
                "{}",
                g.root.display()
            );
        }
    }
    // What the editor writes passes; planted files do not.
    let c = Catalog::from_content().unwrap();
    let (p, s, mts) = tilesets(&c);
    let dir = temp_dir("hygiene");
    cli_session(&dir, &p, &s, &mts);
    let g = GamesDir::new(dir.join("games"));
    assert_eq!(hygiene(&g).unwrap(), Vec::<String>::new());
    std::fs::write(g.maps_dir().join("tiles.png"), b"\x89PNG\r\n\x1a\n").unwrap();
    let layout = std::fs::read_to_string(g.layout_path("CliTown")).unwrap();
    std::fs::write(
        g.layout_path("Smuggled"),
        layout.replacen("    blocks:", "    tiles: [(1, 2, 3)],\n    blocks:", 1),
    )
    .unwrap();
    std::fs::write(g.layout_path("Reformatted"), layout.replace("    ", "  ")).unwrap();
    let bad = hygiene(&g).unwrap();
    assert_eq!(bad.len(), 3, "{bad:#?}");
    assert!(bad.iter().any(|b| b.contains("tiles.png: not a .ron file")));
    assert!(bad.iter().any(|b| b.contains("Smuggled.ron")), "{bad:#?}");
    assert!(
        bad.iter()
            .any(|b| b.contains("Reformatted.ron: not in the canonical format"))
    );
    let _ = std::fs::remove_dir_all(dir);
}
