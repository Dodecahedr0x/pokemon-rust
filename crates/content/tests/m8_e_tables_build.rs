//! Milestone 8 Task E: Task C's tables and Task A's emotes compile. A fixture extracted dir (the
//! stub world written to disk, plus non-empty `tables/*.ron` and one emote) is built through a
//! nested `cargo test -p content --lib`, whose `tables_resolve_when_extracted` prints what
//! resolved.

#[allow(dead_code)]
#[path = "../build/battle.rs"]
mod battle;
#[allow(dead_code)]
#[path = "../build/battle_load.rs"]
mod battle_load;
#[allow(dead_code)]
#[path = "../build/blob_data.rs"]
mod blob_data;
#[allow(dead_code)]
#[path = "../../engine/build_support.rs"]
mod build_support;
#[allow(dead_code, unused_imports)]
#[path = "../build/classify/mod.rs"]
mod classify;
#[allow(dead_code)]
#[path = "../build/codegen.rs"]
mod codegen;
#[allow(dead_code)]
#[path = "../build/input.rs"]
mod input;
#[allow(dead_code)]
#[path = "../build/scripts.rs"]
mod scripts;
#[allow(dead_code)]
#[path = "../build/stub.rs"]
mod stub;

use extract::export::to_ron;
use extract::schema::{
    FieldEffectSpriteFile, Item, ItemPocket, ItemsFile, MapSectionsFile, MultichoiceFile,
    StdStringsFile,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The build script cannot link the engine: its copy of the Fairy matchup count must match.
#[test]
fn the_build_fairy_count_matches_the_engine() {
    assert_eq!(
        battle::engine_fairy_len(),
        engine::mon::FAIRY_MATCHUPS.len()
    );
}

fn put(dir: &Path, rel: &str, s: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, s).unwrap();
}

fn write_fixture(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
    input::write(&stub::world(), dir).unwrap();
    let item = |id: &str, name: &str, pocket| Item {
        id: id.into(),
        name: name.into(),
        pocket,
        price: 200,
        importance: 0,
    };
    let items = ItemsFile {
        items: vec![
            item("ITEM_NONE", "????????", None),
            item(
                "ITEM_MASTER_BALL",
                "MASTER BALL",
                Some(ItemPocket::PokeBalls),
            ),
            item("ITEM_ULTRA_BALL", "BALL B", Some(ItemPocket::PokeBalls)),
        ],
    };
    put(dir, "tables/items.ron", &to_ron(&items).unwrap());
    let multichoice = MultichoiceFile {
        lists: BTreeMap::from([
            ("MULTI_YESNO".into(), vec!["YES A".into(), "NO B".into()]),
            (
                "MULTI_ENTERINFO".into(),
                vec!["A".into(), "B".into(), "C".into()],
            ),
        ]),
    };
    put(
        dir,
        "tables/multichoice.ron",
        &to_ron(&multichoice).unwrap(),
    );
    let std_strings = StdStringsFile {
        strings: BTreeMap::from([("STDSTRING_COOL".into(), "COOL".into())]),
    };
    put(
        dir,
        "tables/std_strings.ron",
        &to_ron(&std_strings).unwrap(),
    );
    let sections = MapSectionsFile {
        sections: BTreeMap::from([("MAPSEC_LITTLEROOT_TOWN".into(), "TOWN A".into())]),
    };
    put(dir, "tables/map_sections.ron", &to_ron(&sections).unwrap());
    let emote = FieldEffectSpriteFile {
        name: "QuestionMark".into(),
        width: 16,
        height: 16,
        frames: 1,
        palette: [0; 16],
    };
    put(
        dir,
        "sprites/field_effects/QuestionMark.ron",
        &to_ron(&emote).unwrap(),
    );
    let img = extract::gfx::tiles_4bpp_to_image(&[0x11; 4 * 32], 2).unwrap();
    extract::gfx::write_png_indexed(
        &dir.join("sprites/field_effects/QuestionMark.png"),
        &img,
        &extract::gfx::Palette([0; 16]),
    )
    .unwrap();
}

#[test]
fn tables_and_emotes_compile_and_resolve() {
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("content-m8e-tables");
    let dir = tmp.join("extracted");
    write_fixture(&dir);
    let out = Command::new(env!("CARGO"))
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("CARGO_BUILD_TARGET_DIR")
        .env("PKR_EXTRACTED_DIR", &dir)
        .args(["test", "-p", "content", "--lib", "--target-dir"])
        .arg(tmp.join("target"))
        .args(["--", "--nocapture", "tables_resolve_when_extracted"])
        .output()
        .expect("spawning cargo");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("TABLES items=2 multichoice=2 std_strings=1 map_sections=1 emotes=1"),
        "{stdout}"
    );
}
