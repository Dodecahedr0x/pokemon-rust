use extract::export::{self, MapFile};
use extract::fixtures;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A fresh, empty directory under the system temp dir, unique to this process and `tag`.
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pkr-export-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// One FR export shared by the tests that only read it.
fn fr_export() -> &'static (PathBuf, export::Manifest) {
    static OUT: OnceLock<(PathBuf, export::Manifest)> = OnceLock::new();
    OUT.get_or_init(|| {
        let dir = temp_dir("shared");
        let manifest = export::export(&fixtures::fr(), &dir).unwrap();
        (dir, manifest)
    })
}

/// Relative path -> contents of every file under `root`.
fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let rel = path.strip_prefix(root).unwrap().to_path_buf();
                out.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    out
}

#[test]
fn export_is_deterministic() {
    let (a, _) = fr_export();
    let b = temp_dir("second");
    export::export(&fixtures::fr(), &b).unwrap();
    let (ta, tb) = (tree(a), tree(&b));
    assert_eq!(ta.keys().collect::<Vec<_>>(), tb.keys().collect::<Vec<_>>());
    for (path, bytes) in &ta {
        assert!(bytes == &tb[path], "{} differs", path.display());
    }
    std::fs::remove_dir_all(&b).unwrap();
}

#[test]
fn writes_expected_tree() {
    let (dir, manifest) = fr_export();
    for f in [
        "manifest.ron",
        "font/latin_normal.png",
        "font/latin_normal.ron",
        "text/fr.ron",
        "sprites/0.png",
        "sprites/0.ron",
        "sprites/89.png",
        "sprites/89.ron",
    ] {
        assert!(dir.join(f).is_file(), "missing {f}");
    }
    for name in ["LittlerootTown", "OldaleTown", "Route101"] {
        assert!(dir.join(format!("maps/{name}/map.ron")).is_file());
        assert!(dir.join(format!("previews/{name}.png")).is_file());
    }
    assert!(!manifest.tilesets.is_empty());
    for key in manifest.tilesets.keys() {
        for f in [
            "tiles.png",
            "palettes.ron",
            "metatiles.ron",
            "attributes.ron",
        ] {
            assert!(
                dir.join("tilesets").join(key).join(f).is_file(),
                "{key}/{f}"
            );
        }
    }
    // Every written file (but the manifest itself) is checksummed.
    let files: Vec<_> = tree(dir)
        .into_keys()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .filter(|p| p != "manifest.ron")
        .collect();
    assert_eq!(files, manifest.files.keys().cloned().collect::<Vec<_>>());
    assert_eq!(manifest.game_code, "BPEF");
    assert_eq!(manifest.sha1, fixtures::fr().sha1_hex());
}

#[test]
fn map_ron_round_trips() {
    let (dir, _) = fr_export();
    for name in ["LittlerootTown", "OldaleTown", "Route101"] {
        let text = std::fs::read_to_string(dir.join(format!("maps/{name}/map.ron"))).unwrap();
        let map: MapFile = ron::from_str(&text).unwrap();
        assert_eq!(map.name, name);
        assert_eq!(export::to_ron(&map).unwrap(), text);
        let w = map.header.layout.width as usize;
        assert_eq!(
            map.header.layout.blocks.len(),
            w * map.header.layout.height as usize
        );
    }
}

#[test]
fn littleroot_header_matches_known_offsets() {
    let (dir, manifest) = fr_export();
    let text = std::fs::read_to_string(dir.join("maps/LittlerootTown/map.ron")).unwrap();
    let map: MapFile = ron::from_str(&text).unwrap();
    assert_eq!(map.header.offset, 0x4874a0);
    assert_eq!(map.header.layout.offset, 0x3ef170);
    assert!(manifest.tilesets.contains_key("3e45f0"));
    assert!(manifest.tilesets.contains_key("3e4608"));
}

#[test]
fn french_text_is_exported_and_referenced() {
    let (dir, _) = fr_export();
    let text = std::fs::read_to_string(dir.join("text/fr.ron")).unwrap();
    let strings: BTreeMap<String, String> = ron::from_str(&text).unwrap();
    assert!(strings.len() > 20, "only {} texts", strings.len());
    assert!(strings.values().any(|s| s.contains("BOURG-EN-VOL")));
    // Every key referenced by a map exists, and every key belongs to a map.
    let mut referenced = Vec::new();
    for name in ["LittlerootTown", "OldaleTown", "Route101"] {
        let map: MapFile = ron::from_str(
            &std::fs::read_to_string(dir.join(format!("maps/{name}/map.ron"))).unwrap(),
        )
        .unwrap();
        assert!(!map.scripts.texts.is_empty(), "{name} has no texts");
        for key in &map.scripts.texts {
            assert!(key.starts_with(&format!("{name}.")), "{key}");
            assert!(strings.contains_key(key), "{key} missing from fr.ron");
        }
        referenced.extend(map.scripts.texts);
    }
    referenced.sort();
    assert_eq!(referenced, strings.keys().cloned().collect::<Vec<_>>());
}

#[test]
fn sprite_sheets_match_frame_counts() {
    let (dir, manifest) = fr_export();
    let brendan = &manifest.sprites[&0];
    assert_eq!(brendan.frames, 18);
    assert_eq!((brendan.width, brendan.height), (16, 32));
    let png = png::Decoder::new(std::fs::File::open(dir.join("sprites/0.png")).unwrap());
    let info = png.read_info().unwrap();
    assert_eq!((info.info().width, info.info().height), (16 * 18, 32));
    assert!(manifest.sprites.contains_key(&89));
}
