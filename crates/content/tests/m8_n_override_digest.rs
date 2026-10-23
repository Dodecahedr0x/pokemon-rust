//! Milestone 8 Task N: an override file is not a global source. `GLOBAL_DIGEST`'s sources
//! digest leaves out `src/overrides/<Name>.rs` (each is in its script's `SCRIPT_DIGESTS`
//! entry, `extract/tests/codegen.rs`), so an override edit revalidates only the nodes under
//! that script and its callers. `BUILD_ID` still covers every override.

#[path = "../build/battle.rs"]
#[allow(dead_code)]
mod battle;
#[path = "../build/battle_load.rs"]
#[allow(dead_code)]
mod battle_load;
#[path = "../build/blob_data.rs"]
#[allow(dead_code)]
mod blob_data;
#[path = "../../engine/build_support.rs"]
#[allow(dead_code)]
mod build_support;
#[path = "../build/build_id.rs"]
#[allow(dead_code)]
mod build_id;
#[path = "../build/classify/mod.rs"]
#[allow(dead_code)]
mod classify;
#[path = "../build/codegen.rs"]
#[allow(dead_code)]
mod codegen;
#[path = "../build/input.rs"]
#[allow(dead_code)]
mod input;
#[path = "../build/scripts.rs"]
#[allow(dead_code)]
mod scripts;
#[path = "../build/stub.rs"]
#[allow(dead_code)]
mod stub;

#[test]
fn an_override_edit_is_not_a_global_change() {
    // `<root>/content` and `<root>/extract/src`, as the workspace lays them out.
    let root = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("m8n-overrides");
    let _ = std::fs::remove_dir_all(&root);
    let tmp = root.join("content");
    std::fs::create_dir_all(root.join("extract/src")).unwrap();
    std::fs::write(root.join("extract/src/lib.rs"), "e").unwrap();
    let put = |rel: &str, s: &str| {
        let p = tmp.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, s).unwrap();
    };
    put("src/lib.rs", "a");
    put("build/main.rs", "b");
    put("src/overrides/M_Callee.rs", "1");
    let base = codegen::sources_digest(&tmp).unwrap();
    let id = build_id::build_id(&tmp, None).unwrap();
    put("src/overrides/M_Callee.rs", "2");
    put("src/overrides/M_New.rs", "3");
    assert_eq!(codegen::sources_digest(&tmp).unwrap(), base);
    assert_ne!(build_id::build_id(&tmp, None).unwrap(), id, "the build id covers it");
    put("src/overrides/mod.rs", "m");
    assert_ne!(codegen::sources_digest(&tmp).unwrap(), base);
    std::fs::remove_file(tmp.join("src/overrides/mod.rs")).unwrap();
    put("src/prelude.rs", "p");
    assert_ne!(codegen::sources_digest(&tmp).unwrap(), base);
}
