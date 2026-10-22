use extract::export::{self, ExportOptions, SLICE_MAPS as MAPS};
use extract::schema::{
    BgKind, FontFile, Hex, Manifest, MapFile, SpriteFile, TilesetFile, WindowFile, from_rows,
};
use extract::{fixtures, gfx, maps, names};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A fresh, empty `export-<tag>` directory under cargo's integration-test tmp dir (in target/).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("export-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// One FR export shared by the tests that only read it.
fn fr_export() -> &'static (PathBuf, Manifest) {
    static OUT: OnceLock<(PathBuf, Manifest)> = OnceLock::new();
    OUT.get_or_init(|| {
        let dir = temp_dir("shared");
        let manifest = export::export(&fixtures::fr(), &dir, &ExportOptions::slice(true)).unwrap();
        (dir, manifest)
    })
}

/// Relative path -> contents of every file under `root`.
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let rel = path.strip_prefix(root).unwrap().to_string_lossy();
                out.insert(rel.replace('\\', "/"), std::fs::read(&path).unwrap());
            }
        }
    }
    out
}

/// The PNG's size and palette as BGR555 (8-bit channels divided by 8, as gbagfx).
fn png_info(path: &Path) -> (u32, u32, Vec<u16>) {
    let reader = png::Decoder::new(std::fs::File::open(path).unwrap())
        .read_info()
        .unwrap();
    let info = reader.info();
    let plte = info.palette.as_ref().unwrap();
    let pal = plte
        .chunks(3)
        .map(|c| (c[0] / 8) as u16 | ((c[1] / 8) as u16) << 5 | ((c[2] / 8) as u16) << 10)
        .collect();
    (info.width, info.height, pal)
}

/// BGR555 after the 8-bit round trip a PNG palette goes through.
fn via_png(c: u16) -> u16 {
    let [r, g, b, _] = gfx::rgb555_to_rgba(c);
    (r / 8) as u16 | ((g / 8) as u16) << 5 | ((b / 8) as u16) << 10
}

#[test]
fn export_is_deterministic() {
    let (a, _) = fr_export();
    let b = temp_dir("second");
    export::export(&fixtures::fr(), &b, &ExportOptions::slice(true)).unwrap();
    let (ta, tb) = (tree(a), tree(&b));
    assert_eq!(ta.keys().collect::<Vec<_>>(), tb.keys().collect::<Vec<_>>());
    for (path, bytes) in &ta {
        assert!(bytes == &tb[path], "{path} differs");
    }
    std::fs::remove_dir_all(&b).unwrap();
}

#[test]
fn writes_current_format_tree() {
    let (dir, m) = fr_export();
    assert_eq!(m.format, extract::schema::FORMAT_VERSION);
    assert_eq!((m.game_code.as_str(), m.language.as_str()), ("BPEF", "fr"));
    assert_eq!(m.sha1, fixtures::fr().sha1_hex());
    assert_eq!(m.maps, MAPS.map(String::from));
    let mut want = BTreeSet::from([
        "font/latin_normal.png".to_string(),
        "font/latin_normal.ron".to_string(),
        "window/message_box.png".to_string(),
        "window/message_box.ron".to_string(),
    ]);
    for map in MAPS {
        want.insert(format!("maps/{map}.ron"));
        want.insert(format!("previews/{map}.png"));
        want.insert(format!("scripts/{map}.rs"));
    }
    want.insert("scripts/legacy_index.ron".to_string());
    want.insert("text/fr.ron".to_string());
    // C-side tables (M8 Task C), whose contents tests/tables.rs checks.
    want.extend(extract::export::tables::FILES.map(String::from));
    // Battle data (M4 Task A).
    for f in [
        "species",
        "moves",
        "types",
        "trainers",
        "encounters",
        "heal_locations",
        "strings",
    ] {
        want.insert(format!("battle/{f}.ron"));
    }
    for t in &m.tilesets {
        want.insert(format!("tilesets/{t}.ron"));
        want.insert(format!("tilesets/{t}.png"));
    }
    for s in &m.sprites {
        want.insert(format!("sprites/{s}.ron"));
        want.insert(format!("sprites/{s}.png"));
    }
    // Every written file but the manifest is checksummed, and nothing else is written.
    let mut files: BTreeSet<String> = tree(dir).into_keys().collect();
    assert!(files.remove("manifest.ron"));
    // Battle graphics (M4 Task B), whose contents tests/battle_gfx.rs checks.
    const BATTLE_GFX: [&str; 6] = [
        "battle/gfx/",
        "battle/env/",
        "pokemon/",
        "trainers/",
        "starter/",
        "font/latin_small.",
    ];
    let battle_gfx: Vec<String> = files
        .iter()
        .filter(|f| BATTLE_GFX.iter().any(|p| f.starts_with(p)))
        .cloned()
        .collect();
    assert!(
        battle_gfx.len() > 2 * 440,
        "{} battle gfx files",
        battle_gfx.len()
    );
    want.extend(battle_gfx);
    assert_eq!(files, want);
    assert_eq!(m.files.keys().cloned().collect::<BTreeSet<_>>(), want);
    assert_eq!(
        m.tilesets,
        [
            "General",
            "Petalburg",
            "Building",
            "Shop",
            "PokemonCenter",
            "BrendansMaysHouse",
            "Lab",
            "GenericBuilding"
        ]
    );
    assert!(
        m.unknown_script_ops.is_empty(),
        "{:?}",
        m.unknown_script_ops
    );
    // Manifest round trip.
    let text = std::fs::read_to_string(dir.join("manifest.ron")).unwrap();
    assert_eq!(&ron::from_str::<Manifest>(&text).unwrap(), m);
}

#[test]
fn maps_and_tilesets_round_trip() {
    let (dir, m) = fr_export();
    let rom = fixtures::fr();
    let table = maps::locate_map_table(&rom).unwrap();
    let mut scripts = BTreeSet::new();
    for name in MAPS {
        let map = export::read_map(dir, name).unwrap();
        let text = std::fs::read_to_string(dir.join(format!("maps/{name}.ron"))).unwrap();
        assert_eq!(export::to_ron(&map).unwrap(), text, "{name}");
        assert_eq!(map.name, name);
        let n = names::map_by_dir(name).unwrap();
        assert_eq!((map.group, map.num), (n.group, n.num));
        // The layout rows rebuild the ROM layout; one map row per line.
        let id = maps::MapId {
            group: map.group,
            num: map.num,
        };
        let h = table.header(&rom, id).unwrap();
        let l = &map.layout;
        assert_eq!((l.width, l.height), (h.layout.width, h.layout.height));
        assert_eq!(l.blocks.len(), usize::from(l.height));
        assert!(l.blocks.iter().all(|r| r.0.len() == usize::from(l.width)));
        assert_eq!(from_rows(&l.blocks), h.layout.blocks);
        assert_eq!(from_rows(&l.border), h.layout.border);
        assert_eq!(
            m.provenance[&format!("map/{name}/header")],
            Hex(h.offset),
            "{name}"
        );
        for t in [&l.primary, &l.secondary] {
            assert!(m.tilesets.contains(t), "{name}: {t}");
        }
        // Names, not numbers.
        for o in &map.objects {
            assert!(names::OBJ_EVENT_GFX.value(&o.gfx).is_some(), "{}", o.gfx);
            assert!(o.movement.starts_with("MOVEMENT_TYPE_"), "{}", o.movement);
            if let Some(f) = &o.flag {
                assert!(names::FLAGS.value(f).is_some(), "{f}");
            }
        }
        for w in &map.warps {
            if let Some(d) = &w.dest_map {
                assert!(MAPS.contains(&d.as_str()), "{d}");
            }
        }
        for c in &map.connections {
            assert_eq!(c.map.is_some(), {
                let (g, n) = c.map_id.split_once('.').unwrap();
                let target = names::map(g.parse().unwrap(), n.parse().unwrap()).unwrap();
                MAPS.contains(&target.dir)
            });
        }
        // Script references are root names of an exported map.
        let s = &map.scripts;
        let mut refs: Vec<&String> = map
            .objects
            .iter()
            .filter_map(|o| o.script.as_ref())
            .collect();
        refs.extend(map.coords.iter().filter_map(|c| c.script.as_ref()));
        refs.extend(map.bgs.iter().filter_map(|b| match &b.kind {
            BgKind::Script { script, .. } => script.as_ref(),
            _ => None,
        }));
        refs.extend(
            [
                &s.on_load,
                &s.on_transition,
                &s.on_resume,
                &s.on_return_to_field,
                &s.on_dive_warp,
            ]
            .into_iter()
            .flatten(),
        );
        refs.extend(s.on_frame.iter().chain(&s.on_warp_into).map(|v| &v.script));
        for r in refs {
            assert!(
                m.provenance.contains_key(&format!("script/{r}")),
                "{name}: {r}"
            );
            assert!(
                MAPS.iter().any(|mm| r.starts_with(&format!("{mm}_"))),
                "{r}"
            );
            scripts.insert(r.clone());
        }
    }
    let roots: BTreeSet<String> = m
        .provenance
        .keys()
        .filter_map(|k| k.strip_prefix("script/"))
        .map(String::from)
        .collect();
    assert_eq!(scripts, roots, "every canonical root is referenced");

    for t in &m.tilesets {
        let ts = export::read_tileset(dir, t).unwrap();
        let text = std::fs::read_to_string(dir.join(format!("tilesets/{t}.ron"))).unwrap();
        assert_eq!(export::to_ron(&ts).unwrap(), text, "{t}");
        assert_eq!(&ts.name, t);
        assert_eq!(ts.palettes.len(), 16);
        assert_eq!(ts.metatiles.len(), ts.attributes.len());
        // The PNG holds the tiles with the first palette the tileset uses (0 or 6).
        let (w, h, pal) = png_info(&dir.join(format!("tilesets/{t}.png")));
        assert_eq!(w, 128, "{t}");
        assert_eq!(h, u32::from(ts.tile_count).div_ceil(16) * 8, "{t}");
        let first = if ts.secondary { 6 } else { 0 };
        let want: Vec<u16> = ts.palettes[first].iter().map(|&c| via_png(c)).collect();
        assert_eq!(pal, want, "{t}");
        let img = gfx::read_png_indexed(&dir.join(format!("tilesets/{t}.png"))).unwrap();
        assert_eq!(img.width, w);
    }
}

#[test]
fn littleroot_matches_known_offsets_and_content() {
    let (dir, m) = fr_export();
    assert_eq!(m.provenance["map/LittlerootTown/header"], Hex(0x4874a0));
    assert_eq!(m.provenance["map/LittlerootTown/layout"], Hex(0x3ef170));
    assert_eq!(m.provenance["tileset/General"], Hex(0x3e45f0));
    assert_eq!(m.provenance["tileset/Petalburg"], Hex(0x3e4608));
    let map: MapFile = export::read_map(dir, "LittlerootTown").unwrap();
    assert_eq!(
        (map.layout.primary.as_str(), map.layout.secondary.as_str()),
        ("General", "Petalburg")
    );
    assert_eq!(
        map.objects[0].script.as_deref(),
        Some("LittlerootTown_Obj1")
    );
    assert_eq!(map.objects[0].gfx, "OBJ_EVENT_GFX_TWIN");
    assert_eq!(
        map.objects[1].flag.as_deref(),
        Some("FLAG_HIDE_LITTLEROOT_TOWN_FAT_MAN")
    );
    assert_eq!(
        map.coords[0].var.as_deref(),
        Some("VAR_LITTLEROOT_TOWN_STATE")
    );
    assert_eq!(
        map.warps[0].dest_map.as_deref(),
        Some("LittlerootTown_MaysHouse_1F")
    );
    assert_eq!(map.warps[0].dest_map_id, "1.2");
    assert_eq!(
        map.scripts.on_transition.as_deref(),
        Some("LittlerootTown_OnTransition")
    );
    let north = map
        .connections
        .iter()
        .find(|c| c.map.as_deref() == Some("Route101"))
        .unwrap();
    assert_eq!(north.map_id, "0.16");
    // Route 103's east connection (Route 110) is not exported: no name, raw id kept.
    let r103 = export::read_map(dir, "Route103").unwrap();
    assert!(
        r103.connections
            .iter()
            .any(|c| c.map.is_none() && c.map_id == "0.25")
    );
    // Brendan's and May's 2F share their scripts: May's refer to Brendan's (exported first).
    let may = export::read_map(dir, "LittlerootTown_MaysHouse_2F").unwrap();
    let shared = may
        .bgs
        .iter()
        .filter_map(|b| match &b.kind {
            BgKind::Script { script, .. } => script.clone(),
            _ => None,
        })
        .filter(|s| s.starts_with("LittlerootTown_BrendansHouse_2F_"))
        .count();
    assert!(shared > 0);
}

#[test]
fn sprites_font_and_window() {
    let (dir, m) = fr_export();
    for s in [
        "BRENDAN_NORMAL",
        "MAY_NORMAL",
        "RIVAL_MAY_NORMAL",
        "RIVAL_BRENDAN_NORMAL",
    ] {
        assert!(m.sprites.contains(&s.to_string()), "{s}");
    }
    assert!(m.skipped_graphics.contains("OBJ_EVENT_GFX_VAR_0"));
    let brendan: SpriteFile = export::read_ron(&dir.join("sprites/BRENDAN_NORMAL.ron")).unwrap();
    assert_eq!(brendan.name, "BRENDAN_NORMAL");
    assert_eq!(brendan.gfx_id, 0);
    assert_eq!(brendan.frame_sizes.len(), 18);
    assert_eq!((brendan.width, brendan.height), (16, 32));
    assert_eq!((brendan.cell_width, brendan.cell_height), (16, 32));
    let (w, h, _) = png_info(&dir.join("sprites/BRENDAN_NORMAL.png"));
    assert_eq!((w, h), (16 * 18, 32));
    for s in &m.sprites {
        let f: SpriteFile = export::read_ron(&dir.join(format!("sprites/{s}.ron"))).unwrap();
        assert_eq!(&f.name, s);
        assert_eq!(
            names::gfx_name(f.gfx_id as u8),
            format!("OBJ_EVENT_GFX_{s}")
        );
    }

    let font: FontFile = export::read_ron(&dir.join("font/latin_normal.ron")).unwrap();
    assert_eq!((font.glyphs, font.glyphs_per_row), (512, 16));
    assert_eq!(font.widths.len(), 512);

    let win: WindowFile = export::read_ron(&dir.join("window/message_box.ron")).unwrap();
    assert_eq!(win.tile_count, 14);
    assert_eq!((win.text_fg, win.text_shadow, win.text_bg), (2, 3, 1));
    assert_eq!(win.palette[1], 0x7FFF, "white");
    let (w, h, pal) = png_info(&dir.join("window/message_box.png"));
    assert_eq!((w, h), (14 * 8, 8));
    assert_eq!(pal, win.palette.map(via_png));
}

#[test]
fn us_export_smoke() {
    let dir = temp_dir("us");
    let m = export::export(&fixtures::us(), &dir, &ExportOptions::slice(true)).unwrap();
    assert_eq!((m.game_code.as_str(), m.language.as_str()), ("BPEE", "en"));
    assert_eq!(m.provenance["map/LittlerootTown/header"], Hex(0x4825b4));
    assert_eq!(m.provenance["map/LittlerootTown/layout"], Hex(0x3ea284));
    assert_eq!(m.maps, MAPS.map(String::from));
    let fr = &fr_export().1;
    assert_eq!(m.tilesets, fr.tilesets);
    assert_eq!(m.sprites, fr.sprites);
    // Maps differ only in what the ROM is allowed to localize: nothing in these files.
    for name in MAPS {
        let us = export::read_map(&dir, name).unwrap();
        let fr = export::read_map(&fr_export().0, name).unwrap();
        assert_eq!(us, fr, "{name}");
    }
    let t: TilesetFile = export::read_tileset(&dir, "General").unwrap();
    assert!(!t.secondary);
    std::fs::remove_dir_all(&dir).unwrap();
}
