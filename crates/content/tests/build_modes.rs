//! Builds and tests `content` in its other modes, through a nested `cargo test -p content --lib`
//! with `PKR_EXTRACTED_DIR` set:
//! - stub mode: a missing extracted dir;
//! - generated scripts: a fixture extracted dir (the stub world written to disk, plus a
//!   hand-written `scripts/index.ron`, two script files and a text file), which exercises the
//!   `include!` of `scripts/<Map>.rs`, the prelude contract and script dispatch.

#[allow(dead_code)]
#[path = "../build/input.rs"]
mod input;
#[allow(dead_code)]
#[path = "../build/stub.rs"]
mod stub;

use extract::schema::{Object, ScriptEntry, ScriptIndex};
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

    let entry = |file: &str, root| ScriptEntry {
        file: file.into(),
        root,
    };
    let index = ScriptIndex {
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
        "scripts/index.ron",
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

    let fixture = tmp.join("extracted");
    write_fixture(&fixture);
    let out = content_tests(&fixture, &target, &["--skip", "extracted::"]);
    assert!(out.contains("FIXTURE SCRIPT OK"), "{out}");
    assert!(out.contains("every_script_id_dispatches ... ok"), "{out}");
}
