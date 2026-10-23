//! The map editor (SIL1 Task C) end to end: games dir files, renders against the game's own,
//! the CLI and the games dir hygiene. Runs on both worlds; the extracted maps and the fixture's
//! Emerald tilesets need the real extract and are skipped on the stub.
//!
//! The fixture (`tests/fixtures/games/fixture`) is self-contained: `FixtureTown` and
//! `FixtureRoute` warp and connect to each other only (and `MAP_DYNAMIC`). It paints with
//! Emerald's `General` + `Petalburg` tilesets, so it validates and renders on the extract only.

use content::Emerald;
use engine::data::GameData;
use extract::schema::{Layout, MapFile, from_rows, to_rows};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use studio::map_edit::catalog::extracted_dir;
use studio::map_edit::games::{COPY_MIN_DISTINCT, COPY_WINDOW, copies, hygiene};
use studio::map_edit::render::{PREVIEW, Preview, leak_map_data, render};
use studio::map_edit::schema::to_ron;
use studio::map_edit::validate::{Severity, errors};
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

fn new_map(name: &str, w: u16, h: u16, p: &str, s: &str, fill: u16) -> NewMap {
    NewMap {
        name: name.into(),
        layout: None,
        shared_layout: false,
        width: w,
        height: h,
        primary: p.into(),
        secondary: s.into(),
        fill,
        fill_collision: 0,
        fill_elevation: 3,
        border: None,
    }
}

fn extracted_map(name: &str) -> MapFile {
    let text = std::fs::read_to_string(extracted_dir().join(format!("maps/{name}.ron"))).unwrap();
    ron::from_str(&text).unwrap()
}

#[test]
fn the_fixture_round_trips_byte_for_byte() {
    let src = fixture();
    assert_eq!(src.map_names().unwrap(), ["FixtureRoute", "FixtureTown"]);
    let out = GamesDir::new(temp_dir("roundtrip"));
    for name in ["FixtureRoute", "FixtureTown"] {
        let doc = src.load(name).unwrap();
        assert_eq!((doc.width, doc.height), (8, 6));
        assert_eq!(
            (doc.primary.as_str(), doc.secondary.as_str()),
            ("General", "Petalburg")
        );
        out.save(&doc).unwrap();
        for (a, b) in [
            (src.map_path(name), out.map_path(name)),
            (src.layout_path(name), out.layout_path(name)),
        ] {
            assert_eq!(
                std::fs::read(&a).unwrap(),
                std::fs::read(&b).unwrap(),
                "{}",
                a.display()
            );
        }
    }
    let _ = std::fs::remove_dir_all(&out.root);
}

/// The fixture has no errors and no one-way links: its only warnings are its unwritten
/// scripts.
#[test]
fn the_fixture_validates_against_the_extract() {
    if !real() {
        return;
    }
    let games = fixture();
    let c = Catalog::for_games(&games).unwrap();
    assert_eq!(
        c.maps.keys().collect::<Vec<_>>(),
        ["FixtureRoute", "FixtureTown"],
        "a games dir's own maps only"
    );
    for name in ["FixtureRoute", "FixtureTown"] {
        let issues = validate(&games.load(name).unwrap(), &c);
        assert_eq!(errors(&issues), 0, "{name}: {issues:#?}");
        assert!(
            issues
                .iter()
                .all(|i| i.severity == Severity::Warning && i.message.contains("script")),
            "{name}: {issues:#?}"
        );
    }
    // An Emerald map is not a Silice map's neighbor.
    let mut d = games.load("FixtureTown").unwrap();
    d.map.warps[0].dest_map = "OldaleTown".into();
    let issues = validate(&d, &c);
    assert!(
        issues
            .iter()
            .any(|i| i.message == "unknown destination map \"OldaleTown\""),
        "{issues:#?}"
    );
}

/// A connected games dir map is drawn in the margin: `FixtureRoute`'s bottom row above
/// `FixtureTown`.
#[test]
fn renders_draw_connected_games_dir_maps() {
    if !real() {
        return;
    }
    let games = fixture();
    let c = Catalog::for_games(&games).unwrap();
    let town = games.load("FixtureTown").unwrap();
    let route = games.load("FixtureRoute").unwrap();
    let with = render(&town, &c, 1).unwrap();
    let route_img = render(&route, &c, 0).unwrap();
    for py in 0..16 {
        for px in 0..8 * 16 {
            assert_eq!(
                with.rgb555[py * with.width + 16 + px],
                route_img.rgb555[(5 * 16 + py) * route_img.width + px],
                "pixel ({px}, {py}) above the town"
            );
        }
    }
    let mut alone = town.clone();
    alone.map.connections.clear();
    assert!(render(&alone, &c, 1).unwrap().rgb555 != with.rgb555);
}

/// Maps whose Gen3 data has events outside the map (as in the decomp's `map.json`): the
/// editor flags them, the game never reaches them.
const OUT_OF_MAP_IN_GEN3: [&str; 13] = [
    "SlateportCity",
    "SlateportCity_Harbor",
    "LilycoveCity_DepartmentStore_1F",
    "MeteorFalls_1F_1R",
    "MeteorFalls_1F_2R",
    "UnusedContestHall1",
    "UnusedContestHall2",
    "UnusedContestHall3",
    "UnusedContestHall4",
    "UnusedContestHall5",
    "UnusedContestHall6",
    "BattleFrontier_BattleDomeCorridor",
    "BattleFrontier_BattleDomePreBattleRoom",
];

/// An editor error the extract may have: a warp to a map it lacks, a secret base bg event (the
/// editor does not edit secret bases; they render as the game does), and Gen3's own
/// out-of-map events ([`OUT_OF_MAP_IN_GEN3`]).
fn tolerated(map: &str, message: &str) -> bool {
    message.contains("UNEXPORTED_")
        || message == "secret bases are not supported"
        || (OUT_OF_MAP_IN_GEN3.contains(&map) && message.contains(" is outside the "))
}

/// Loads each extracted map into the editor; checks it has no errors but [`tolerated`] ones and
/// renders exactly as the game draws it. Every failing map is listed.
fn check_extracted_maps(only: Option<&[engine::data::MapId]>) {
    let game = Emerald::for_shells();
    let c = Catalog::from_content().unwrap();
    let mut n = 0;
    let mut failed = Vec::new();
    for (id, name) in map_list() {
        if only.is_some_and(|o| !o.contains(&id)) {
            continue;
        }
        n += 1;
        let doc = Doc::from_map_file(&extracted_map(name)).unwrap();
        let issues = validate(&doc, &c);
        let errs: Vec<_> = (issues.iter())
            .filter(|i| i.severity == Severity::Error && !tolerated(name, &i.message))
            .map(|i| format!("{}: {}", i.at, i.message))
            .collect();
        if !errs.is_empty() {
            failed.push(format!("{name}: {errs:?}"));
            continue;
        }
        let edited = render(&doc, &c, 3).unwrap();
        let want = render_map(&game, id, 3, &[]);
        if (edited.width, edited.height) != (want.width, want.height) {
            failed.push(format!("{name}: size"));
        } else if edited.rgb555 != want.rgb555 {
            failed.push(format!("{name}: pixels differ"));
        }
    }
    assert!(
        failed.is_empty(),
        "{} maps:\n{}",
        failed.len(),
        failed.join("\n")
    );
    assert!(n >= 15, "{n} maps checked");
}

/// The fixed sample (`content::samples`: the slice, later towns and routes, one interior per
/// tileset family).
#[test]
fn sample_extracted_maps_render_as_the_game() {
    if real() {
        check_extracted_maps(Some(&content::samples::map_ids()));
    }
}

/// Every map of the build (slow on the full import: `cargo xtask sweep --studio`).
#[test]
#[ignore]
fn every_extracted_map_renders_as_the_game() {
    if real() {
        check_extracted_maps(None);
    }
}

/// Extracted warps the editor cannot name stay, so later warp indices do not shift.
#[test]
fn extracted_warps_keep_their_indices() {
    if !real() {
        return;
    }
    let f = extracted_map("OldaleTown_PokemonCenter_2F");
    let doc = Doc::from_map_file(&f).unwrap();
    assert_eq!(doc.map.warps.len(), f.warps.len());
    let dests: Vec<&str> = doc.map.warps.iter().map(|w| w.dest_map.as_str()).collect();
    // The Union Room and the Trade Center (Cable Club maps: `UNEXPORTED_*` on an extract
    // without them, named on the full import), last as in the extract.
    let last = &dests[dests.len() - 2..];
    assert!(
        last == ["UnionRoom", "TradeCenter"] || last == ["UNEXPORTED_25_60", "UNEXPORTED_25_25"],
        "{last:?}"
    );
}

/// An edited map's render equals a render of a `MapData` built directly from the same layout
/// data; on the extract, also the game's render of the same map with the edits as overrides.
#[test]
fn an_edited_render_equals_a_direct_render() {
    let c = Catalog::from_content().unwrap();
    let (p, s, mts) = tilesets(&c);
    let mut ed = Editor::new(
        Doc::new(&NewMap {
            border: Some(vec![mts[1], mts[2], mts[1], mts[2]]),
            ..new_map("Edited", 17, 12, &p, &s, mts[0])
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
    let mut ed = Editor::new(Doc::from_map_file(&extracted_map(name)).unwrap());
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
    Object(x: 4, y: 4, gfx: "OBJ_EVENT_GFX_BOY_1", id: "LOCALID_CLITOWN_BOY"),
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
    let e = run(&["--games", g, "create", spec.to_str().unwrap()]).unwrap_err();
    assert!(
        format!("{e:#}").contains("exists"),
        "never replaces a map: {e:#}"
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
    let o = &doc.map.objects[0];
    assert_eq!(
        (o.x, o.local_id, o.id.as_deref()),
        (6, 2, Some("LOCALID_CLITOWN_BOY")),
        "local ids stay when an earlier object is deleted"
    );
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

    // Renumbering a named object is warned about.
    let renumber = d1.join("renumber.ron");
    let mut o = o.clone();
    o.local_id = 9;
    std::fs::write(
        &renumber,
        format!(
            "[Replace(index: 0, event: Object({}))]",
            to_ron(&o).unwrap().trim()
        ),
    )
    .unwrap();
    let out = run(&[
        "--games",
        g,
        "apply",
        "CliTown",
        renumber.to_str().unwrap(),
        "--dry-run",
    ])
    .unwrap();
    assert!(
        out.contains("warning: object 0: LOCALID_CLITOWN_BOY was local id 2, now 9"),
        "{out}"
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
    assert!(
        run(&["--game", "../elsewhere", "list"]).is_err(),
        "--game is a name"
    );
    for d in [d1, d2] {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// A new map never replaces a saved layout unless it shares it; a shared layout is loaded,
/// not refilled.
#[test]
fn new_maps_share_layouts_only_when_asked() {
    let c = Catalog::from_content().unwrap();
    let (p, s, mts) = tilesets(&c);
    let dir = temp_dir("shared");
    let games = GamesDir::new(dir.join("games"));
    let mut ed = Editor::new(
        games
            .new_doc(&new_map("First", 6, 5, &p, &s, mts[0]))
            .unwrap(),
    );
    ed.apply(&Op::Paint {
        x: 2,
        y: 2,
        metatile: Some(mts[1]),
        collision: None,
        elevation: None,
    })
    .unwrap();
    games.create(ed.doc()).unwrap();
    let layout = std::fs::read(games.layout_path("First")).unwrap();

    let second = NewMap {
        layout: Some("First".into()),
        ..new_map("Second", 3, 3, &p, &s, mts[2])
    };
    let e = games.new_doc(&second).unwrap_err();
    assert!(format!("{e:#}").contains("shared_layout"), "{e:#}");
    // The same through the CLI.
    let spec = dir.join("second.ron");
    std::fs::write(
        &spec,
        format!(
            "(name: \"Second\", layout: \"First\", width: 3, height: 3, primary: \"{p}\", \
             secondary: \"{s}\", fill: {})",
            mts[2]
        ),
    )
    .unwrap();
    let g = games.root.to_str().unwrap();
    assert!(run(&["--games", g, "create", spec.to_str().unwrap()]).is_err());
    // A hand-built doc with the same layout name but other blocks is refused too.
    let mut clash = Doc::new(&second).unwrap();
    clash.map.name = "Clash".into();
    assert!(games.create(&clash).is_err());
    assert_eq!(std::fs::read(games.layout_path("First")).unwrap(), layout);
    assert!(!games.exists("Clash"));

    // Shared: the saved layout, whatever the spec's size and fill.
    std::fs::write(
        &spec,
        "(name: \"Second\", layout: \"First\", shared_layout: true)",
    )
    .unwrap();
    let out = run(&["--games", g, "create", spec.to_str().unwrap()]).unwrap();
    assert!(out.starts_with("created "), "{out}");
    let doc = games.load("Second").unwrap();
    assert_eq!((doc.width, doc.height), (6, 5));
    assert_eq!(doc.block(2, 2).unwrap() & 0x3FF, mts[1]);
    assert_eq!(std::fs::read(games.layout_path("First")).unwrap(), layout);
    let e = games
        .new_doc(&NewMap {
            shared_layout: true,
            layout: Some("Missing".into()),
            ..new_map("Third", 0, 0, "", "", 0)
        })
        .unwrap_err();
    assert!(format!("{e:#}").contains("Missing"), "{e:#}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn validate_fails_on_errors_and_reports_every_map() {
    let c = Catalog::from_content().unwrap();
    let (p, s, _) = tilesets(&c);
    let dir = temp_dir("invalid");
    let games = GamesDir::new(dir.join("games"));
    let mut ed = Editor::new(Doc::new(&new_map("Broken", 2, 2, &p, &s, 0)).unwrap());
    ed.apply(&Op::Warp {
        x: 0,
        y: 0,
        dest_map: "NoSuchMap".into(),
        dest_warp: 0,
        elevation: 0,
    })
    .unwrap();
    games.save(ed.doc()).unwrap();
    // A map whose layout is gone, a layout no map uses, and a good map after them.
    let mut lost = Doc::new(&new_map("Lost", 2, 2, &p, &s, 0)).unwrap();
    lost.map.layout = "Gone".into();
    std::fs::write(games.map_path("Lost"), to_ron(&lost.map).unwrap()).unwrap();
    std::fs::write(games.layout_path("Unused"), to_ron(&lost.layout()).unwrap()).unwrap();
    games
        .save(&Doc::new(&new_map("Zed", 2, 2, &p, &s, 0)).unwrap())
        .unwrap();
    let g = games.root.to_str().unwrap();
    let e = format!("{:#}", run(&["--games", g, "validate"]).unwrap_err());
    for want in [
        "Broken: error: warp 0: unknown destination map \"NoSuchMap\"",
        "Lost: error: map: reading",
        "layout Unused: warning: no map uses it",
        "2 errors in 3 maps",
    ] {
        assert!(e.contains(want), "{want}: {e}");
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// Nothing under a games dir holds ROM data: only `maps/*.ron` and `layouts/*.ron` in their
/// canonical form (names, raw block indices, event data), and no layout copied from the
/// extract.
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

fn write_layout(g: &GamesDir, name: &str, l: &Layout) {
    std::fs::create_dir_all(g.layouts_dir()).unwrap();
    std::fs::write(g.layout_path(name), to_ron(l).unwrap()).unwrap();
}

/// A games dir layout that is an extracted one, or holds a varied square of one, is flagged;
/// repainted collision does not hide it; a uniform fill of the same metatile is not.
#[test]
fn layouts_copied_from_the_extract_are_flagged() {
    if !real() {
        return;
    }
    let route = extracted_map("Route101").layout;
    let dir = temp_dir("copies");
    let g = GamesDir::new(dir.join("games"));
    let mut planted = route.clone();
    let mut blocks = from_rows(&planted.blocks);
    for b in &mut blocks {
        *b ^= 1 << 10; // other collision
    }
    planted.blocks = to_rows(&blocks, planted.width as usize);
    write_layout(&g, "Planted", &planted);

    // The most varied 10x10 square of Route 101, pasted at (7, 5) of a 30x24 field.
    let cells = from_rows(&route.blocks);
    let w = route.width as usize;
    let distinct = |x: usize, y: usize| {
        let mut v: Vec<u16> = (y..y + COPY_WINDOW)
            .flat_map(|cy| (x..x + COPY_WINDOW).map(move |cx| (cx, cy)))
            .map(|(cx, cy)| cells[cy * w + cx] & 0x3FF)
            .collect();
        v.sort();
        v.dedup();
        v.len()
    };
    let (sx, sy) = (0..=route.height as usize - COPY_WINDOW)
        .flat_map(|y| (0..=w - COPY_WINDOW).map(move |x| (x, y)))
        .max_by_key(|&(x, y)| distinct(x, y))
        .unwrap();
    assert!(distinct(sx, sy) >= COPY_MIN_DISTINCT);
    let fill = cells[0];
    let (fw, fh) = (30usize, 24usize);
    let mut field = vec![fill; fw * fh];
    for dy in 0..COPY_WINDOW {
        for dx in 0..COPY_WINDOW {
            field[(5 + dy) * fw + 7 + dx] = cells[(sy + dy) * w + sx + dx];
        }
    }
    let sub = Layout {
        width: fw as u16,
        height: fh as u16,
        blocks: to_rows(&field, fw),
        ..route.clone()
    };
    write_layout(&g, "Patched", &sub);
    let uniform = Layout {
        blocks: to_rows(&vec![fill; w * route.height as usize], w),
        ..route.clone()
    };
    write_layout(&g, "Uniform", &uniform);

    let bad = copies(&g, &extracted_dir()).unwrap();
    assert_eq!(bad.len(), 2, "{bad:#?}");
    assert!(
        bad.iter()
            .any(|b| b.contains("Planted.ron: the blocks of extracted")),
        "{bad:#?}"
    );
    assert!(
        bad.iter()
            .any(|b| b.contains("Patched.ron: the 10x10 cells at")),
        "{bad:#?}"
    );
    let _ = std::fs::remove_dir_all(dir);
}
