//! Builds and tests `content` in its other modes, through a nested `cargo test -p content --lib`
//! with `PKR_EXTRACTED_DIR` set:
//! - stub mode: a missing extracted dir;
//! - generated scripts: a fixture extracted dir (the stub world written to disk, plus a
//!   hand-written `scripts/legacy_index.ron`, two script files and a text file), which exercises the
//!   `include!` of `scripts/<Map>.rs`, the prelude contract and script dispatch;
//! - a stale extract: that fixture with an older manifest `format`, which builds the stub and
//!   sets `content::STALE_EXTRACT`.

#[allow(dead_code)]
#[path = "../build/battle.rs"]
mod battle;
#[allow(dead_code)]
#[path = "../build/battle_load.rs"]
mod battle_load;
#[path = "../build/build_id.rs"]
mod build_id;
#[allow(dead_code)]
#[path = "../../engine/build_support.rs"]
mod build_support;
#[allow(dead_code)]
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

/// The id a nested build printed (`content::build_id_tests`).
fn printed_build_id(out: &str) -> u64 {
    let line = out
        .lines()
        .find_map(|l| l.split("CONTENT BUILD_ID=0x").nth(1))
        .unwrap_or_else(|| panic!("no CONTENT BUILD_ID in:\n{out}"));
    u64::from_str_radix(line.get(..16).unwrap(), 16).unwrap()
}

use extract::schema::{LegacyScriptEntry, LegacyScriptIndex, Object};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Runs the content lib tests against `extracted`; returns their stdout.
fn content_tests(extracted: &Path, target: &Path, extra: &[&str]) -> String {
    let out = Command::new(env!("CARGO"))
        .current_dir(workspace())
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("CARGO_BUILD_TARGET_DIR")
        .env("PKR_EXTRACTED_DIR", extracted)
        .args(["test", "-p", "content", "--lib", "--target-dir"])
        .arg(target)
        .args(["--", "--nocapture"])
        .args(extra)
        .output()
        .expect("spawning cargo");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "content tests failed with PKR_EXTRACTED_DIR={}:\n{stdout}\n{}",
        extracted.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    stdout
}

const STUB_TOWN_RS: &str = r#"// @generated fixture (hand-written)
pub async fn StubTown_Obj1(ctx: Ctx) {
    let mut cond = Ordering::Equal;
    msgbox(&ctx, StubTown_Obj1_T0, MsgBox::Npc).await;
    ctx.move_obj(PLAYER, &StubTown_Obj1_M0).await;
    ctx.wait_move(PLAYER).await;
    if ctx.flag(FLAG_0x1AB).await {
        cond = Ordering::Less;
    }
    if cond == Ordering::Equal {
        Box::pin(StubTown_Obj1_L0(ctx.clone())).await;
    }
    Box::pin(super::Other::Other_Main(ctx.clone())).await;
    let v = special(&ctx, SPECIAL_HealPlayerParty).await;
    ctx.set_var(VAR_0x4001, v).await;
    ctx.set_var(VAR_RESULT, check_player_gender(&ctx).await).await;
    callstd(&ctx, STD_FIND_ITEM).await;
    callstd(&ctx, STD_OBTAIN_ITEM).await;
    unsupported(&ctx, "playse").await;
    ctx.set_movement_type(1, MovementType::Face(Dir::Left)).await;
    ctx.fade(Fade::ToBlack).await;
    ctx.warp(MAP_PETALBURG_CITY, Some(0), 0, 0).await;
    ctx.log("FIXTURE SCRIPT DONE").await;
}

pub async fn StubTown_Obj1_L0(ctx: Ctx) {
    ctx.set_flag(FLAG_HIDE_LITTLEROOT_TOWN_RIVAL, true).await;
}

pub static StubTown_Obj1_M0: [Step; 3] = [
    Step::Walk(Dir::Up, Speed::Normal),
    Step::Emote(Emote::Exclamation),
    Step::Face(Dir::Down),
];
"#;

const OTHER_RS: &str = r#"// @generated fixture (hand-written)
pub async fn Other_Main(ctx: Ctx) {
    msgbox(&ctx, Other_Main_T0, MsgBox::Default).await;
}
"#;

fn write_fixture(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
    let mut w = stub::world();
    w.maps[0].objects.push(Object {
        local_id: 1,
        gfx: "OBJ_EVENT_GFX_BRENDAN_NORMAL".into(),
        x: 3,
        y: 3,
        elevation: 3,
        movement: "MOVEMENT_TYPE_WANDER_AROUND".into(),
        range_x: 1,
        range_y: 1,
        trainer_type: "TRAINER_TYPE_NONE".into(),
        trainer_sight_or_berry_tree: 0,
        script: Some("StubTown_Obj1".into()),
        flag: Some("FLAG_0x1AC".into()),
    });
    input::write(&w, dir).unwrap();

    let entry = |file: &str, root| LegacyScriptEntry {
        file: file.into(),
        root,
    };
    let index = LegacyScriptIndex {
        scripts: BTreeMap::from([
            ("StubTown_Obj1".into(), entry("StubTown.rs", true)),
            ("StubTown_Obj1_L0".into(), entry("StubTown.rs", false)),
            ("Other_Main".into(), entry("Other.rs", true)),
        ]),
        movements: BTreeMap::from([("StubTown_Obj1_M0".into(), "StubTown.rs".into())]),
    };
    let put = |rel: &str, s: &str| {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, s).unwrap();
    };
    put(
        "scripts/legacy_index.ron",
        &extract::export::to_ron(&index).unwrap(),
    );
    put("scripts/StubTown.rs", STUB_TOWN_RS);
    put("scripts/Other.rs", OTHER_RS);
    let texts: BTreeMap<String, String> = BTreeMap::from([
        (
            "StubTown_Obj1_T0".into(),
            "BONJOUR {PLAYER}!\\nÇa va?".into(),
        ),
        ("Other_Main_T0".into(), "AU REVOIR.".into()),
    ]);
    put("text/fr.ron", &extract::export::to_ron(&texts).unwrap());
}

#[test]
fn stub_and_generated_script_modes_build_and_pass() {
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("content-modes");
    let target = tmp.join("target");

    let out = content_tests(&tmp.join("missing"), &target, &[]);
    assert!(
        out.contains("the_stub_world_is_one_small_map ... ok"),
        "{out}"
    );
    assert!(!out.contains("STALE EXTRACT FLAGGED"), "{out}");
    let content = Path::new(env!("CARGO_MANIFEST_DIR"));
    let stub_id = printed_build_id(&out);
    assert_eq!(stub_id, build_id::build_id(content, None).unwrap());

    let fixture = tmp.join("extracted");
    write_fixture(&fixture);
    let out = content_tests(&fixture, &target, &["--skip", "extracted::"]);
    assert!(out.contains("FIXTURE SCRIPT OK"), "{out}");
    assert!(out.contains("every_script_id_dispatches ... ok"), "{out}");
    assert!(!out.contains("STALE EXTRACT FLAGGED"), "{out}");
    let fixture_id = printed_build_id(&out);
    assert_eq!(
        fixture_id,
        build_id::build_id(content, Some(&fixture)).unwrap()
    );
    assert_ne!(fixture_id, stub_id);

    let stale = tmp.join("stale");
    write_fixture(&stale);
    let manifest = stale.join("manifest.ron");
    let text = std::fs::read_to_string(&manifest).unwrap();
    let current = format!("format: {},", extract::schema::FORMAT_VERSION);
    assert!(text.contains(&current), "{text}");
    let old = format!("format: {},", extract::schema::FORMAT_VERSION - 1);
    std::fs::write(&manifest, text.replace(&current, &old)).unwrap();
    let out = content_tests(&stale, &target, &[]);
    assert!(out.contains("STALE EXTRACT FLAGGED"), "{out}");
    assert!(
        out.contains("the_stub_world_is_one_small_map ... ok"),
        "{out}"
    );
    // A stale extract builds the stub, so it has the stub's identity.
    assert_eq!(printed_build_id(&out), stub_id);
}

/// The content, build and extract sources are inputs; their tests are not.
#[test]
fn build_id_tracks_the_sources() {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("content-build-id-src");
    let _ = std::fs::remove_dir_all(&root);
    let put = |rel: &str, s: &str| {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, s).unwrap();
    };
    put("content/src/lib.rs", "// lib\n");
    put("content/build/main.rs", "// main\n");
    put("extract/src/lib.rs", "// extract\n");
    let content = root.join("content");
    let id = || build_id::build_id(&content, None).unwrap();
    let base = id();
    put("content/src/data_tests.rs", "// tests\n");
    put("extract/src/tests.rs", "// tests\n");
    assert_eq!(id(), base);
    for (rel, edit) in [
        ("content/src/lib.rs", "// lib 2\n"),
        ("content/build/main.rs", "// main 2\n"),
        ("extract/src/lib.rs", "// extract 2\n"),
        ("content/build/support/commands.ron", "{}\n"),
        ("content/build/support/commands.ron", "{\"end\": Native}\n"),
    ] {
        let before = id();
        put(rel, edit);
        assert_ne!(id(), before, "{rel}");
    }
}

/// The content build id: deterministic, stub vs extracted, and every extracted file but the
/// previews counts.
#[test]
fn build_id_tracks_the_extracted_files() {
    let content = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("content-build-id");
    let one = tmp.join("one");
    let two = tmp.join("two");
    write_fixture(&one);
    write_fixture(&two);
    let stub = build_id::build_id(content, None).unwrap();
    assert_eq!(build_id::build_id(content, None).unwrap(), stub);
    let id = build_id::build_id(content, Some(&one)).unwrap();
    assert_ne!(id, stub);
    assert_ne!(id, 0);
    // Another absolute path, same files.
    assert_eq!(build_id::build_id(content, Some(&two)).unwrap(), id);
    // Map renders, hidden files and backups are not build inputs.
    std::fs::create_dir_all(two.join("previews")).unwrap();
    std::fs::write(two.join("previews/StubTown.png"), b"png").unwrap();
    std::fs::write(two.join(".DS_Store"), b"finder").unwrap();
    std::fs::write(two.join("maps/.DS_Store"), b"finder").unwrap();
    std::fs::write(two.join("maps/StubTown.ron~"), b"backup").unwrap();
    std::fs::create_dir_all(two.join(".cache")).unwrap();
    std::fs::write(two.join(".cache/x"), b"x").unwrap();
    assert_eq!(build_id::build_id(content, Some(&two)).unwrap(), id);
    // A symlinked dir is followed (same files, same id).
    #[cfg(unix)]
    {
        let moved = tmp.join("moved-maps");
        let _ = std::fs::remove_dir_all(&moved);
        std::fs::rename(two.join("maps"), &moved).unwrap();
        std::os::unix::fs::symlink(&moved, two.join("maps")).unwrap();
        assert_eq!(build_id::build_id(content, Some(&two)).unwrap(), id);
    }
    // A hand edit that leaves the manifest as is still counts.
    std::fs::write(
        two.join("scripts/Other.rs"),
        format!("{OTHER_RS}// edited\n"),
    )
    .unwrap();
    assert_ne!(build_id::build_id(content, Some(&two)).unwrap(), id);
}
