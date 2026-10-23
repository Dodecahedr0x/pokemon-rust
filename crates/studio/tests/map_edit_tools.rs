//! The map editor's productivity tools end to end: batch builds (byte-identical to the
//! step-by-step CLI), stamps, the hygiene command, overlays and labels, and behaviour-aware
//! collision. Runs on both worlds; what needs Emerald's tilesets or maps is skipped on the
//! stub.

use extract::schema::{from_rows, to_rows};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use studio::map_edit::batch::plan;
use studio::map_edit::catalog::extracted_dir;
use studio::map_edit::games::{COPY_MIN_DISTINCT, COPY_WINDOW, copies, hygiene};
use studio::map_edit::overlay;
use studio::map_edit::render::render;
use studio::map_edit::stamp::Stamp;
use studio::map_edit::{Catalog, GamesDir, cli};

fn temp_dir(tag: &str) -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pkr-map-tools-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn real() -> bool {
    if content::STUB {
        eprintln!("skipped on the stub world (needs the extract)");
    }
    !content::STUB
}

fn run(args: &[&str]) -> anyhow::Result<String> {
    cli::run(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
}

/// The catalog's first primary and secondary tilesets and a few metatiles they define.
fn tilesets() -> (String, String, Vec<u16>) {
    let c = Catalog::from_content().unwrap();
    let p = c.tileset_names(false).remove(0);
    let s = c.tileset_names(true).remove(0);
    let (pr, sr) = c.metatile_ranges(&p, &s).unwrap();
    let v: Vec<u16> = pr.take(6).chain(sr.take(3)).collect();
    // Nine, repeating on the stub world's small tilesets.
    let mts = (0..9).map(|i| v[i % v.len()]).collect();
    (p, s, mts)
}

fn put(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// Two maps that warp and connect to each other, as an edits dir: creates, terrain (phase 10,
/// with a stamp) and events (phase 20). Also writes the stamp.
fn write_edits(games: &Path, edits: &Path) {
    let (p, s, m) = tilesets();
    for (name, fill) in [("TownA", m[0]), ("RouteB", m[1])] {
        put(
            &edits.join(format!("{name}/00_create.ron")),
            &format!(
                "(name: \"{name}\", width: 9, height: 7, primary: \"{p}\", secondary: \
                 \"{s}\", fill: {fill})"
            ),
        );
    }
    let stamp = format!(
        "(\n    primary: \"{p}\",\n    secondary: \"{s}\",\n    width: 2,\n    height: 1,\n    \
         elevation: false,\n    blocks: [\n        ({}, {}),\n    ],\n)\n",
        m[2] | 1 << 10,
        m[6]
    );
    put(&games.join("stamps/Fence.ron"), &stamp);
    put(
        &edits.join("TownA/10_terrain.ron"),
        &format!(
            r##"[
    Rect(x: 0, y: 0, w: 9, h: 1, metatile: {a}, collision: 1),
    Ascii(x: 1, y: 2, rows: ["#\"#"], legend: {{'#': (metatile: {b}), '"': (metatile: {c}, collision: 1)}}),
    Stamp(x: 5, y: 4, name: "Fence"),
    AutoCollision(rect: (x: 0, y: 0, w: 9, h: 3)),
]"##,
            a = m[3],
            b = m[4],
            c = m[5]
        ),
    );
    put(
        &edits.join("TownA/20_events.ron"),
        r#"[
    Warp(x: 4, y: 6, dest_map: "RouteB", dest_warp: 0),
    Object(x: 2, y: 3, gfx: "OBJ_EVENT_GFX_BOY_1", id: "LOCALID_TOWNA_BOY"),
    Connect((dir: North, offset: 0, map: "RouteB")),
]"#,
    );
    put(
        &edits.join("RouteB/10_terrain.ron"),
        &format!(
            "[Fill(x: 0, y: 0, metatile: {}), Stamp(x: 0, y: 6, name: \"Fence\")]",
            m[7]
        ),
    );
    put(
        &edits.join("RouteB/20_events.ron"),
        r#"[
    Warp(x: 4, y: 0, dest_map: "TownA", dest_warp: 0),
    Connect((dir: South, offset: 0, map: "TownA")),
]"#,
    );
}

fn files(g: &GamesDir) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for dir in [g.maps_dir(), g.layouts_dir()] {
        let mut names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        names.sort();
        for p in names {
            out.push((
                p.strip_prefix(&g.root).unwrap().display().to_string(),
                std::fs::read(&p).unwrap(),
            ));
        }
    }
    out
}

/// `build` gives the bytes of `create` for each spec then `apply` for each script in phase
/// order, defers link checks to its one validation, and rebuilds a populated dir the same.
#[test]
fn build_equals_the_step_by_step_cli() {
    let (d1, d2) = (temp_dir("steps"), temp_dir("build"));
    let (g1, g2) = (
        GamesDir::new(d1.join("games")),
        GamesDir::new(d2.join("games")),
    );
    write_edits(&g1.root, &g1.edits_dir());
    write_edits(&g2.root, &g2.edits_dir());
    put(&g2.edits_dir().join(".DS_Store"), "finder");
    put(&g2.edits_dir().join("TownA/.DS_Store"), "finder");

    let p = plan(&g1.edits_dir()).unwrap();
    let order: Vec<String> = (p.steps.iter())
        .map(|s| {
            s.path
                .strip_prefix(g1.edits_dir())
                .unwrap()
                .display()
                .to_string()
        })
        .collect();
    assert_eq!(
        order,
        [
            "RouteB/10_terrain.ron",
            "TownA/10_terrain.ron",
            "RouteB/20_events.ron",
            "TownA/20_events.ron"
        ]
    );
    let r1 = g1.root.to_str().unwrap();
    for map in ["RouteB", "TownA"] {
        let spec = g1.edits_dir().join(map).join("00_create.ron");
        run(&["--games", r1, "create", spec.to_str().unwrap()]).unwrap();
    }
    let mut transient = String::new();
    for s in &p.steps {
        transient += &run(&["--games", r1, "apply", &s.map, s.path.to_str().unwrap()]).unwrap();
    }
    assert!(
        transient.contains("error: warp 0: TownA has 0 warps"),
        "step by step, RouteB's warp is an error until TownA has its own: {transient}"
    );

    let r2 = g2.root.to_str().unwrap();
    let out = run(&["--games", r2, "build"]).unwrap();
    assert!(out.contains("2 created, 4 scripts, 11 ops"), "{out}");
    assert!(!out.contains(": error:"), "{out}");
    assert!(out.ends_with("2 maps, no errors\n"), "{out}");
    assert_eq!(
        files(&g1),
        files(&g2),
        "build = create + apply, byte for byte"
    );

    // A populated dir is rebuilt from the edits, not edited again: a hand edit of a saved
    // file is replaced.
    let script = d2.join("hand.ron");
    std::fs::write(
        &script,
        "[Paint(x: 0, y: 6, metatile: 1), Delete(kind: Warp, index: 0)]",
    )
    .unwrap();
    run(&["--games", r2, "apply", "TownA", script.to_str().unwrap()]).unwrap();
    assert_ne!(files(&g1), files(&g2));
    let out = run(&["--games", r2, "build", g2.edits_dir().to_str().unwrap()]).unwrap();
    assert!(out.contains("2 created"), "{out}");
    assert_eq!(files(&g1), files(&g2));

    let town = g2.load("TownA").unwrap();
    let (_, _, m) = tilesets();
    assert_eq!(
        town.block(2, 2).unwrap(),
        m[5] | 1 << 10 | 3 << 12,
        "'\"' is a key"
    );
    assert_eq!(
        town.block(5, 4).unwrap(),
        m[2] | 1 << 10 | 3 << 12,
        "stamped, map elevation"
    );
    assert_eq!(town.map.objects[0].local_id, 1);
    for d in [d1, d2] {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// A failing op, a create that names another map or a stray file stops the build before
/// anything is written.
#[test]
fn failed_builds_write_nothing() {
    let dir = temp_dir("fail");
    let g = GamesDir::new(dir.join("games"));
    write_edits(&g.root, &g.edits_dir());
    let r = g.root.to_str().unwrap();
    put(
        &g.edits_dir().join("TownA/30_broken.ron"),
        "[Paint(x: 1, y: 1, metatile: 1), Paint(x: 99, y: 0, metatile: 1)]",
    );
    let e = format!("{:#}", run(&["--games", r, "build"]).unwrap_err());
    assert!(e.contains("30_broken.ron") && e.contains("op 1"), "{e}");
    assert!(!g.maps_dir().exists() && !g.layouts_dir().exists());
    std::fs::remove_file(g.edits_dir().join("TownA/30_broken.ron")).unwrap();

    put(&g.edits_dir().join("TownA/notes.txt"), "plan");
    let e = format!("{:#}", run(&["--games", r, "build"]).unwrap_err());
    assert!(e.contains("notes.txt: not an edit file"), "{e}");
    std::fs::remove_file(g.edits_dir().join("TownA/notes.txt")).unwrap();

    let spec = g.edits_dir().join("RouteB/00_create.ron");
    let text = std::fs::read_to_string(&spec).unwrap();
    std::fs::write(&spec, text.replace("\"RouteB\"", "\"RouteC\"")).unwrap();
    let e = format!("{:#}", run(&["--games", r, "build"]).unwrap_err());
    assert!(e.contains("creates \"RouteC\", not RouteB"), "{e}");
    assert!(!g.maps_dir().exists());
    let _ = std::fs::remove_dir_all(dir);
}

/// `stamp-from` captures a region (never over a saved stamp without `--force`), `Stamp`
/// paints it, and the hygiene command accepts what the editor wrote but not planted files.
#[test]
fn stamps_and_the_hygiene_command() {
    let dir = temp_dir("stamps");
    let g = GamesDir::new(dir.join("games"));
    write_edits(&g.root, &g.edits_dir());
    let r = g.root.to_str().unwrap();
    run(&["--games", r, "build"]).unwrap();
    let out = run(&[
        "--games",
        r,
        "stamp-from",
        "TownA",
        "1",
        "2",
        "3",
        "1",
        "Wall",
    ])
    .unwrap();
    assert!(out.contains("stamp Wall: 3x1 cells of TownA"), "{out}");
    let e = run(&[
        "--games",
        r,
        "stamp-from",
        "TownA",
        "0",
        "0",
        "2",
        "2",
        "Wall",
    ])
    .unwrap_err();
    assert!(format!("{e:#}").contains("--force"), "{e:#}");
    run(&[
        "--games",
        r,
        "stamp-from",
        "TownA",
        "1",
        "2",
        "3",
        "1",
        "Wall",
        "--no-elevation",
        "--force",
    ])
    .unwrap();
    let st = g.load_stamp("Wall").unwrap();
    assert!(!st.elevation);
    let e = run(&[
        "--games",
        r,
        "stamp-from",
        "TownA",
        "8",
        "0",
        "2",
        "1",
        "Edge",
    ])
    .unwrap_err();
    assert!(format!("{e:#}").contains("not inside"), "{e:#}");
    let script = dir.join("wall.ron");
    std::fs::write(&script, "[Stamp(x: 0, y: 4, name: \"Wall\")]").unwrap();
    run(&["--games", r, "apply", "RouteB", script.to_str().unwrap()]).unwrap();
    let (town, route) = (g.load("TownA").unwrap(), g.load("RouteB").unwrap());
    for x in 0..3 {
        assert_eq!(route.block(x, 4), town.block(x + 1, 2));
    }

    assert_eq!(hygiene(&g).unwrap(), Vec::<String>::new());
    let out = run(&["--games", r, "hygiene"]).unwrap();
    assert!(out.ends_with(": clean\n"), "{out}");
    let wall = std::fs::read_to_string(g.stamp_path("Wall")).unwrap();
    std::fs::write(g.stamp_path("Loose"), wall.replace("    ", "  ")).unwrap();
    std::fs::write(g.stamps_dir().join("tiles.bin"), b"\0").unwrap();
    let mut short = g.load_stamp("Wall").unwrap();
    short.width = 4;
    std::fs::write(
        g.stamp_path("Short"),
        studio::map_edit::schema::to_ron(&short).unwrap(),
    )
    .unwrap();
    put(&g.edits_dir().join("TownA/30_bad.ron"), "[Paint(x: 1)]");
    let e = format!("{:#}", run(&["--games", r, "hygiene"]).unwrap_err());
    for want in [
        "Loose.ron: not in the canonical format",
        "tiles.bin: not a .ron file",
        "30_bad.ron",
        "Short.ron: the stamp's blocks are not 4x1",
        "4 hygiene problems",
    ] {
        assert!(e.contains(want), "{want}: {e}");
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// PNG width and height from the IHDR chunk.
fn png_size(p: &Path) -> (u32, u32) {
    let b = std::fs::read(p).unwrap();
    assert_eq!(&b[..8], b"\x89PNG\r\n\x1a\n");
    let n = |i: usize| u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    (n(16), n(20))
}

/// Labels and overlays: scaled 2x by default, drawn over the picture, and the plain render is
/// unchanged.
#[test]
fn overlays_and_labels() {
    let dir = temp_dir("overlay");
    let g = GamesDir::new(dir.join("games"));
    write_edits(&g.root, &g.edits_dir());
    let r = g.root.to_str().unwrap();
    run(&["--games", r, "build"]).unwrap();
    let png = |n: &str| dir.join(n);
    let plain = png("plain.png");
    run(&[
        "--games",
        r,
        "render",
        "TownA",
        "--out",
        plain.to_str().unwrap(),
    ])
    .unwrap();
    assert_eq!(png_size(&plain), ((9 + 4) * 16, (7 + 4) * 16));
    let over = png("over.png");
    let out = run(&[
        "--games",
        r,
        "render",
        "TownA",
        "--out",
        over.to_str().unwrap(),
        "--events",
        "--collision",
        "--margin",
        "1",
    ])
    .unwrap();
    assert!(
        out.contains("scale 2") && out.contains("warps magenta"),
        "{out}"
    );
    assert_eq!(png_size(&over), ((9 + 2) * 32, (7 + 2) * 32));

    let (p, s, _) = tilesets();
    let pal = png("pal.png");
    let out = run(&[
        "palette",
        &p,
        &s,
        "--labels",
        "--out",
        pal.to_str().unwrap(),
    ])
    .unwrap();
    assert!(out.contains("behaviour codes"), "{out}");
    // Labels sit in a gutter: a column is a 4-digit id and a code wide (46 px at scale 2,
    // wider than the 32 px art) plus a 2 px gap.
    let (w, _) = png_size(&pal);
    assert_eq!(w, 8 * 48);
    run(&[
        "palette",
        &p,
        &s,
        "--labels",
        "--scale",
        "1",
        "--out",
        pal.to_str().unwrap(),
    ])
    .unwrap();
    // 23 px of labels, a 1 px gap.
    assert_eq!(png_size(&pal).0, 8 * 24);
    assert!(
        run(&["palette", &p, &s, "--events"]).is_err(),
        "a render option"
    );

    // The overlays draw over the cells they mark.
    let c = Catalog::for_games(&g).unwrap();
    let doc = g.load("TownA").unwrap();
    let base = overlay::scale(&render(&doc, &c, 0).unwrap(), 2);
    let mut ev = base.clone();
    overlay::draw_events(&mut ev, &doc, 2);
    let px = |img: &studio::map_view::MapImage, x: usize, y: usize| img.rgb555[y * img.width + x];
    // The object at (2, 3): its yellow frame's top-left pixel.
    assert_eq!(px(&ev, 2 * 32, 3 * 32), overlay::YELLOW);
    assert_eq!(px(&ev, 0, 0), px(&base, 0, 0), "elsewhere untouched");
    let mut col = base.clone();
    overlay::draw_collision(&mut col, &doc, &c, 2);
    assert_ne!(px(&col, 4, 4), px(&base, 4, 4), "row 0 is solid: tinted");
    assert_eq!(px(&col, 4, 36), px(&base, 4, 36), "(0, 1) walkable");
    let _ = std::fs::remove_dir_all(dir);
}

/// On Emerald's tilesets: tall grass painted solid is warned about and `AutoCollision` makes
/// it walkable, and a door without a warp is warned about.
#[test]
fn auto_collision_on_emerald_tilesets() {
    if !real() {
        return;
    }
    let dir = temp_dir("auto");
    let g = GamesDir::new(dir.join("games"));
    let r = g.root.to_str().unwrap();
    put(
        &g.edits_dir().join("Field/00_create.ron"),
        "(name: \"Field\", width: 6, height: 4, primary: \"General\", secondary: \
         \"Petalburg\", fill: 1)",
    );
    // 13: tall grass; 584: an animated door.
    put(
        &g.edits_dir().join("Field/10_terrain.ron"),
        "[Rect(x: 0, y: 0, w: 6, h: 2, metatile: 13, collision: 1), Paint(x: 2, y: 3, metatile: 584, collision: 0)]",
    );
    let out = run(&["--games", r, "build"]).unwrap();
    assert!(
        out.contains(
            "Field: warning: cell (0, 0): solid (collision 1), but metatile 13 is walkable in "
        ) && out.contains("extracted cells using it (AutoCollision sets it)"),
        "{out}"
    );
    assert!(
        out.contains("Field: warning: cell (2, 3): walkable door metatile 584 (MB_ANIMATED_DOOR) has no warp event"),
        "{out}"
    );
    put(
        &g.edits_dir().join("Field/30_allow.ron"),
        r#"[AllowCollision(cells: [(2, 3)], reason: "a door that opens in a later chapter")]"#,
    );
    let out = run(&["--games", r, "build"]).unwrap();
    assert!(!out.contains("cell (2, 3)"), "allowed: {out}");
    put(
        &g.edits_dir().join("Field/20_collision.ron"),
        "[AutoCollision()]",
    );
    let out = run(&["--games", r, "build"]).unwrap();
    assert!(!out.contains("AutoCollision sets it"), "{out}");
    let d = g.load("Field").unwrap();
    assert!((0..6).all(|x| d.block(x, 1).unwrap() >> 10 & 3 == 0));
    let _ = std::fs::remove_dir_all(dir);
}

/// A stamp holding a varied square of an extracted map is flagged like a layout.
#[test]
fn stamps_copied_from_the_extract_are_flagged() {
    if !real() {
        return;
    }
    let text = std::fs::read_to_string(extracted_dir().join("maps/Route101.ron")).unwrap();
    let route: extract::schema::MapFile = ron::from_str(&text).unwrap();
    let l = route.layout;
    let cells = &from_rows(&l.blocks);
    let w = l.width as usize;
    let distinct = |x: usize, y: usize| {
        let mut v: Vec<u16> = (y..y + COPY_WINDOW)
            .flat_map(|cy| (x..x + COPY_WINDOW).map(move |cx| cells[cy * w + cx] & 0x3FF))
            .collect();
        v.sort();
        v.dedup();
        v.len()
    };
    let (sx, sy) = (0..=l.height as usize - COPY_WINDOW)
        .flat_map(|y| (0..=w - COPY_WINDOW).map(move |x| (x, y)))
        .max_by_key(|&(x, y)| distinct(x, y))
        .unwrap();
    assert!(distinct(sx, sy) >= COPY_MIN_DISTINCT);
    let square: Vec<u16> = (sy..sy + COPY_WINDOW)
        .flat_map(|y| (sx..sx + COPY_WINDOW).map(move |x| (x, y)))
        .map(|(x, y)| cells[y * w + x])
        .collect();
    let dir = temp_dir("stampcopy");
    let g = GamesDir::new(dir.join("games"));
    let st = Stamp {
        primary: l.primary.clone(),
        secondary: l.secondary.clone(),
        width: COPY_WINDOW as u16,
        height: COPY_WINDOW as u16,
        elevation: true,
        blocks: to_rows(&square, COPY_WINDOW),
    };
    g.save_stamp("Lifted", &st, false).unwrap();
    let small = Stamp {
        width: 3,
        height: 3,
        blocks: to_rows(&square[..9], 3),
        ..st.clone()
    };
    g.save_stamp("Small", &small, false).unwrap();
    let bad = copies(&g, &extracted_dir()).unwrap();
    assert_eq!(bad.len(), 1, "{bad:#?}");
    assert!(
        bad[0].contains("Lifted.ron: the 10x10 cells at (0, 0)"),
        "{bad:#?}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// `render --extract` and `dump --extract` read the extract's own maps (reference for
/// authors); without the extract they fail with a hint.
#[test]
fn extract_maps_render_and_dump() {
    let dir = temp_dir("extract");
    let png = dir.join("ref.png");
    let args = |map: &'static str| {
        [
            "render",
            map,
            "--extract",
            "--collision",
            "--events",
            "--out",
            png.to_str().unwrap(),
        ]
        .map(String::from)
    };
    if !real() {
        let err = cli::run(&args("LittlerootTown")).unwrap_err();
        assert!(format!("{err:#}").contains("--extract"), "{err:#}");
        return;
    }
    let out = cli::run(&args("LittlerootTown")).unwrap();
    assert!(out.contains("LittlerootTown: 20x20 cells"), "{out}");
    assert_eq!(png_size(&png), ((20 + 4) * 32, (20 + 4) * 32));
    let text = run(&["dump", "LittlerootTown", "--extract", "--out", "-"]).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines[0],
        "LittlerootTown: 20x20, General + Petalburg; metatile ids, * = solid"
    );
    // A header line, the border, the x indices, then 20 rows of 20 cells.
    assert_eq!(lines.len(), 3 + 20);
    assert!(lines[3..].iter().all(|l| l.len() == 3 + 20 * 5), "{text}");
    assert!(text.contains('*'), "solid cells are marked");
    let file = dir.join("ref.txt");
    let out = run(&[
        "dump",
        "LittlerootTown",
        "--extract",
        "--out",
        file.to_str().unwrap(),
    ])
    .unwrap();
    assert!(out.contains("20x20 cells"), "{out}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
    for bad in ["NoSuchMap", "../maps/LittlerootTown"] {
        assert!(cli::run(&args(bad)).is_err(), "{bad}");
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// `palette --range` keeps a slice of the pair, `--zoom` scales the art apart from the
/// labels, and labels sit in a gutter above the art, never over it.
#[test]
fn palette_ranges_zoom_and_gutter_labels() {
    use studio::map_edit::render::{palette, palette_of};
    let dir = temp_dir("palette");
    let (p, s, _) = tilesets();
    let c = Catalog::from_content().unwrap();
    let (pr, sr) = c.metatile_ranges(&p, &s).unwrap();
    let pal = dir.join("pal.png");
    let out_ = pal.to_str().unwrap();
    // The stub world's secondary has no metatiles.
    let first = if sr.is_empty() { pr.start } else { sr.start };
    let range = format!("{first}..={}", first + 1);
    let out = run(&["palette", &p, &s, "--range", &range, "--out", out_]).unwrap();
    assert!(
        out.contains(&format!("metatiles {range}: 8 cells")),
        "{out}"
    );
    assert_eq!(png_size(&pal), (8 * 16, 16));
    run(&[
        "palette", &p, &s, "--range", &range, "--zoom", "3", "--out", out_,
    ])
    .unwrap();
    assert_eq!(png_size(&pal), (8 * 48, 48));
    // Labels at scale 1 over art at zoom 4: 64 px cells plus a 1 px gap; rows add a 7 px band.
    let out = run(&[
        "palette", &p, &s, "--range", &range, "--labels", "--scale", "1", "--zoom", "4", "--out",
        out_,
    ])
    .unwrap();
    assert!(out.contains("behaviour codes"), "{out}");
    assert_eq!(png_size(&pal), (8 * 65, 7 + 64 + 1));
    let err = run(&["palette", &p, &s, "--range", "2000..=2001"]).unwrap_err();
    assert!(format!("{err:#}").contains("no metatile"), "{err:#}");
    assert!(run(&["palette", &p, &s, "--range", "9..2"]).is_err());
    assert!(run(&["palette", &p, &s, "--zoom", "9"]).is_err());

    // The art under the labels is the plain palette's, pixel for pixel.
    let both = format!("{}..={}", pr.start, sr.start);
    let r = studio::map_edit::cli::metatile_range(&both).unwrap();
    let part = palette_of(&c, &p, &s, Some(r)).unwrap();
    assert_eq!(part.metatiles[0], Some(pr.start));
    assert_eq!(part.metatiles.contains(&Some(sr.start)), !sr.is_empty());
    let full = palette(&c, &p, &s).unwrap();
    let (ts, zoom) = (2, 3);
    let img = overlay::label_palette(&full, &c, &p, &s, ts, zoom, true);
    let art = overlay::scale(&full.image, zoom);
    let (cell, band) = (16 * zoom, overlay::text_box("0", ts).1);
    let col_w = cell.max(overlay::text_box("0000", ts).0 + ts + overlay::text_box("D", ts).0) + ts;
    let row_h = band + cell + ts;
    for i in [0usize, 1, 9] {
        let Some(_) = full.metatiles.get(i).copied().flatten() else {
            continue;
        };
        let (cx, cy) = (i % 8, i / 8);
        for (x, y) in [(0, 0), (cell - 1, cell - 1), (cell / 2, 3)] {
            assert_eq!(
                img.rgb555[(cy * row_h + band + y) * img.width + cx * col_w + x],
                art.rgb555[(cy * cell + y) * art.width + cx * cell + x],
                "cell {i} ({x}, {y})"
            );
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// Gen3 draws `Building`'s metatile 1 black whatever the secondary (its tiles are the
/// primary's), so it is a safe out-of-map border for any interior; `palette` lists each
/// pair's black metatiles.
#[test]
fn building_metatile_1_is_void_with_every_secondary() {
    if !real() {
        return;
    }
    use studio::map_edit::render::palette_of;
    let c = Catalog::from_content().unwrap();
    let secondaries = c.tileset_names(true);
    // Silice's content ships the secondaries its maps use (34 today), Emerald all of them.
    let least = if content::FLAVOR == content::Flavor::Silice {
        30
    } else {
        40
    };
    assert!(secondaries.len() > least, "{secondaries:?}");
    for s in &secondaries {
        let pal = palette_of(&c, "Building", s, Some(1..=1)).unwrap();
        assert_eq!(pal.void_metatiles(), [1], "Building + {s}");
    }
    let out = run(&[
        "palette",
        "Building",
        "Lab",
        "--out",
        temp_dir("void").join("p.png").to_str().unwrap(),
    ])
    .unwrap();
    assert!(
        out.contains("void (all-black) metatiles, for an interior's border: 1 513"),
        "{out}"
    );
}

/// The new op forms through `build`: `Object(local_id: N)` and `ScriptsSet`.
#[test]
fn local_ids_and_scripts_set_through_build() {
    let dir = temp_dir("ops");
    let g = GamesDir::new(dir.join("games"));
    write_edits(&g.root, &g.edits_dir());
    put(
        &g.edits_dir().join("TownA/30_more.ron"),
        r#"[
    Scripts((on_load: None, on_transition: Some("TownA_OnTransition"), on_resume: None, on_return_to_field: None, on_dive_warp: None, on_frame: [], on_warp_into: [])),
    ScriptsSet(on_load: "TownA_OnLoad"),
    Object(x: 3, y: 3, gfx: "OBJ_EVENT_GFX_BOY_1", id: "LOCALID_TOWNA_GUIDE", local_id: 20),
]"#,
    );
    let r = g.root.to_str().unwrap();
    run(&["--games", r, "build"]).unwrap();
    let d = g.load("TownA").unwrap();
    assert_eq!(d.map.scripts.on_load.as_deref(), Some("TownA_OnLoad"));
    assert_eq!(
        d.map.scripts.on_transition.as_deref(),
        Some("TownA_OnTransition")
    );
    let ids: Vec<u8> = d.map.objects.iter().map(|o| o.local_id).collect();
    assert_eq!(ids, [1, 20]);
    put(
        &g.edits_dir().join("TownA/40_dup.ron"),
        r#"[Object(x: 4, y: 3, gfx: "OBJ_EVENT_GFX_BOY_1", local_id: 1)]"#,
    );
    let err = run(&["--games", r, "build"]).unwrap_err();
    assert!(
        format!("{err:#}").contains("local id 1 is already object 0's"),
        "{err:#}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// `build` on a copy of the committed Silice edits gives the committed maps and layouts,
/// byte for byte (the op changes are backward compatible).
#[test]
fn committed_silice_edits_rebuild_byte_identical() {
    if !real() {
        return;
    }
    // The edits name the games dir's own tilesets and sprites, which only the Silice build has
    // (`PKR_GAME=silice cargo test -p studio`).
    if content::FLAVOR != content::Flavor::Silice {
        eprintln!("skipped: needs the Silice build");
        return;
    }
    let ws = studio::paths::StudioPaths::detect().workspace;
    let committed = GamesDir::new(ws.join("games/silice"));
    let dir = temp_dir("silice");
    let g = GamesDir::new(dir.join("games"));
    for e in std::fs::read_dir(committed.edits_dir()).unwrap() {
        let map = e.unwrap().path();
        if !map.is_dir() {
            continue;
        }
        for f in std::fs::read_dir(&map).unwrap() {
            let f = f.unwrap().path();
            let to = g
                .edits_dir()
                .join(map.file_name().unwrap())
                .join(f.file_name().unwrap());
            put(&to, &std::fs::read_to_string(&f).unwrap());
        }
    }
    // The stamps the edits place.
    if let Ok(stamps) = std::fs::read_dir(committed.stamps_dir()) {
        for f in stamps {
            let f = f.unwrap().path();
            let to = g.stamps_dir().join(f.file_name().unwrap());
            std::fs::create_dir_all(g.stamps_dir()).unwrap();
            std::fs::copy(&f, &to).unwrap();
        }
    }
    run(&["--games", g.root.to_str().unwrap(), "build"]).unwrap();
    let rel = |g: &GamesDir| {
        files(g)
            .into_iter()
            .filter(|(p, _)| {
                let name = Path::new(p)
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string();
                g.edits_dir().join(&name).is_dir()
            })
            .collect::<Vec<_>>()
    };
    let built = files(&g);
    assert!(!built.is_empty());
    let want = rel(&committed);
    assert_eq!(
        built.iter().map(|f| &f.0).collect::<Vec<_>>(),
        want.iter().map(|f| &f.0).collect::<Vec<_>>()
    );
    for ((p, a), (_, b)) in built.iter().zip(&want) {
        assert!(a == b, "{p} differs from the committed file");
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// `lint-reach --all` on the committed Silice maps: records the current findings (printed; the
/// triage list is in the Silice plan, "Reachability checker as built") without failing on
/// them, and checks the output is stable. `reach`, `bypass` and `render --reach` run on the
/// first gym, whose review the bypass reproduces.
#[test]
fn reach_tools_on_the_committed_silice_maps() {
    if !real() {
        return;
    }
    // Since the Silice redraws (SilBoisEther, SilBoisEtherGym, …) the maps name the games dir's
    // own tilesets, which only the Silice build has.
    if content::FLAVOR != content::Flavor::Silice {
        eprintln!("skipped: needs the Silice build");
        return;
    }
    let ws = studio::paths::StudioPaths::detect().workspace;
    let g = ws.join("games/silice");
    let g = g.to_str().unwrap();
    let out = run(&["--games", g, "lint-reach", "--all"]).unwrap();
    println!("{out}");
    let last = out.lines().last().unwrap();
    assert!(
        last.contains(" maps: ") && last.contains(" warnings, "),
        "{last}"
    );
    assert_eq!(out, run(&["--games", g, "lint-reach", "--all"]).unwrap());
    for line in out.lines().filter(|l| *l != last) {
        assert!(
            line.contains(": warning: ") || line.contains(": info: "),
            "{line}"
        );
    }

    let r = run(&["--games", g, "reach", "BoisEther_Gym", "--from", "7,30"]).unwrap();
    assert!(r.contains("cells reachable"), "{r}");
    assert!(r.contains("LOCALID_GYM1_JULES: sees 1 cells"), "{r}");
    // The review's claim: with the spore grates avoided, every path from the shower to the
    // door's front cell crosses Jules's line of sight.
    let base = [
        "--games",
        g,
        "bypass",
        "BoisEther_Gym",
        "--from",
        "7,30",
        "--to",
        "8,23",
        "--avoid",
        "coord:BoisEther_Gym_EventScript_Spores",
    ];
    let b = run(&base).unwrap();
    assert!(b.contains("bypass: yes"), "{b}");
    let mut jules = base.to_vec();
    jules.extend(["--avoid", "trainer:LOCALID_GYM1_JULES"]);
    let b = run(&jules).unwrap();
    assert!(b.contains("bypass: no"), "{b}");

    let dir = temp_dir("reach");
    let png = dir.join("gym.png");
    let r = run(&[
        "--games",
        g,
        "render",
        "BoisEther_Gym",
        "--reach",
        "7,30",
        "--sight",
        "--out",
        png.to_str().unwrap(),
    ])
    .unwrap();
    assert!(r.contains("reach: ") && r.contains("sight: "), "{r}");
    let plain = dir.join("plain.png");
    run(&[
        "--games",
        g,
        "render",
        "BoisEther_Gym",
        "--scale",
        "2",
        "--out",
        plain.to_str().unwrap(),
    ])
    .unwrap();
    assert_eq!(png_size(&png), png_size(&plain));
    assert_ne!(std::fs::read(&png).unwrap(), std::fs::read(&plain).unwrap());
    assert!(run(&["--games", g, "render", "BoisEther_Gym", "--reach", "7"]).is_err());
    assert!(run(&["--games", g, "reach", "BoisEther_Gym"]).is_err());
    let _ = std::fs::remove_dir_all(dir);
}
