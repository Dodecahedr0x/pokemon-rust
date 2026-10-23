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
    assert!(out.contains("2 created, 4 scripts, 10 ops"), "{out}");
    assert!(!out.contains(": error:"), "{out}");
    assert!(out.ends_with("2 maps, no errors\n"), "{out}");
    assert_eq!(
        files(&g1),
        files(&g2),
        "build = create + apply, byte for byte"
    );

    // A populated dir is rebuilt from the edits, not edited again.
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
    put(&g.edits_dir().join("TownA/30_bad.ron"), "[Paint(x: 1)]");
    let e = format!("{:#}", run(&["--games", r, "hygiene"]).unwrap_err());
    for want in [
        "Loose.ron: not in the canonical format",
        "tiles.bin: not a .ron file",
        "30_bad.ron",
        "3 hygiene problems",
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
    let (w, _) = png_size(&pal);
    assert_eq!(w, 8 * 32);
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
    assert_eq!(png_size(&pal).0, 8 * 16);
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
            "Field: warning: cell (0, 0): solid (collision 1), but metatile 13 is walkable in all \
             288 extracted cells using it (AutoCollision sets it)"
        ),
        "{out}"
    );
    assert!(
        out.contains("Field: warning: cell (2, 3): walkable door metatile 584 (MB_ANIMATED_DOOR) has no warp event"),
        "{out}"
    );
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
