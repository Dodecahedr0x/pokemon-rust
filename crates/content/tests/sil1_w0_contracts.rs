//! SIL1 Wave 0: the Silice build contracts, on the build's own code (`build/silice/`), in any
//! flavour and without the extract:
//! - id allocation: the committed ids load, and keep every number of the first import
//!   (`xtask/tests/fixtures/silice/ids.golden`); a renumbering, a dropped name or a reused number
//!   fails; tombstones keep their number and get no constant;
//! - the script registry: which functions are scripts, name-order ids, unique names, file names;
//! - an unknown script in a map fails when strict, and is dropped and listed in the Wave 0
//!   allowlist mode;
//! - the per-flavour source split of `BUILD_ID` and `GLOBAL_DIGEST`;
//! - `text/fr.ron` read entry by entry, a repeated key failing.

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
#[path = "../build/silice/mod.rs"]
mod silice;
#[allow(dead_code)]
#[path = "../build/stub.rs"]
mod stub;

use silice::ids;
use silice::registry::{self, Registry, Resolver};
use silice::schema::{IdEntry, IdKind, IdsFile};
use std::path::{Path, PathBuf};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn games() -> PathBuf {
    workspace().join("games/silice")
}

fn golden() -> String {
    std::fs::read_to_string(workspace().join("xtask/tests/fixtures/silice/ids.golden")).unwrap()
}

/// A fresh scratch dir under the test target dir.
fn scratch(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("sil1-w0-{name}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn put(root: &Path, rel: &str, s: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, s).unwrap();
}

// ---- ids ----

#[test]
fn committed_ids_load_and_keep_their_golden_numbers() {
    let ids = ids::load(&games()).unwrap();
    let n = ids::check_golden(&golden(), &ids).unwrap();
    assert!(n > 900, "{n} golden ids");
}

/// The first golden entry of `kind`: (name, num).
fn first_golden(kind: IdKind) -> (String, u32) {
    let g = golden();
    let l = g
        .lines()
        .find(|l| l.starts_with(&format!("{} ", kind.file())))
        .unwrap();
    let w: Vec<&str> = l.split(' ').collect();
    (w[1].to_string(), w[2].parse().unwrap())
}

#[test]
fn a_renumbered_or_dropped_id_fails_the_golden_check() {
    let ids = ids::load(&games()).unwrap();
    let (name, num) = first_golden(IdKind::Flags);
    let edit = |f: &dyn Fn(&mut IdsFile)| {
        let mut i = ids.clone();
        f(i.files.get_mut(&IdKind::Flags).unwrap());
        i
    };
    let renumbered = edit(&|f| {
        let e = f.ids.iter_mut().find(|e| e.name == name).unwrap();
        e.num = 0x4FF;
    });
    let e = ids::check_golden(&golden(), &renumbered).unwrap_err();
    assert!(format!("{e:#}").contains("never renumbered"), "{e:#}");
    let dropped = edit(&|f| f.ids.retain(|e| e.name != name));
    let e = ids::check_golden(&golden(), &dropped).unwrap_err();
    assert!(format!("{e:#}").contains("tombstone it instead"), "{e:#}");
    // A tombstone keeps the name and the number: still fine.
    let tombstoned = edit(&|f| f.ids.iter_mut().find(|e| e.name == name).unwrap().removed = true);
    ids::check_golden(&golden(), &tombstoned).unwrap();
    assert_eq!(tombstoned.get(IdKind::Flags, &name).unwrap().num, num);
    assert!(tombstoned.live(IdKind::Flags, &name).is_err());
    assert!(!tombstoned.consts().iter().any(|c| c.0 == name));
    assert!(ids.consts().iter().any(|c| c.0 == name && c.1 == "FlagId"));
}

#[test]
fn ids_files_reject_duplicates_out_of_range_and_reserved_numbers() {
    let entry = |name: &str, num: u32| IdEntry {
        name: name.into(),
        num,
        map: Some(name.into()),
        removed: false,
    };
    let file = |ids: Vec<IdEntry>| IdsFile { base: 0x113, ids };
    let ok = file(vec![entry("FLAG_SIL_A", 0x113), entry("FLAG_SIL_B", 0x114)]);
    ids::validate(IdKind::Flags, &ok).unwrap();
    for (bad, what) in [
        (
            file(vec![entry("FLAG_SIL_A", 0x113), entry("FLAG_SIL_A", 0x114)]),
            "listed twice",
        ),
        (
            file(vec![entry("FLAG_SIL_A", 0x113), entry("FLAG_SIL_B", 0x113)]),
            "reuses number",
        ),
        (file(vec![entry("FLAG_SIL_A", 0x500)]), "outside"),
    ] {
        let e = ids::validate(IdKind::Flags, &bad).unwrap_err();
        assert!(format!("{e:#}").contains(what), "{e:#}");
    }
    let dynamic = file(vec![entry("MAP_X", 0x7F7F)]);
    assert!(ids::validate(IdKind::Maps, &dynamic).is_err());
    let mut no_file = entry("MAP_X", 1);
    no_file.map = None;
    assert!(ids::validate(IdKind::Maps, &file(vec![no_file])).is_err());
}

#[test]
fn ids_constants_cover_live_maps_flags_vars_and_songs() {
    let ids = ids::load(&games()).unwrap();
    let consts = ids.consts();
    for (kind, ty) in [
        (IdKind::Maps, "MapId"),
        (IdKind::Flags, "FlagId"),
        (IdKind::Vars, "VarId"),
        (IdKind::Songs, "u16"),
    ] {
        let live = ids.files[&kind].ids.iter().filter(|e| !e.removed).count();
        assert_eq!(
            consts.iter().filter(|c| c.1 == ty).count(),
            live,
            "{kind:?}"
        );
    }
}

// ---- registry ----

#[test]
fn scripts_are_the_top_level_pub_async_fns() {
    let src = "use crate::silice::prelude::*;\n\
        pub async fn B_Sc01(ctx: Ctx) {}\n\
        pub async fn A_Sc01(_ctx: Ctx) { helper().await }\n\
        async fn helper() {}\n\
        pub fn not_async() {}\n\
        mod inner { pub async fn Nested(ctx: Ctx) {} }\n";
    assert_eq!(registry::script_fns(src).unwrap(), ["B_Sc01", "A_Sc01"]);
    for bad in [
        "pub async fn F(ctx: Ctx) -> u8 { 0 }",
        "pub async fn F(ctx: Ctx, n: u8) {}",
        "pub async fn F() {}",
        "pub async fn F<T>(ctx: T) {}",
    ] {
        let e = registry::script_fns(bad).unwrap_err();
        assert!(format!("{e:#}").contains("pub async fn F"), "{bad}: {e:#}");
    }
    assert!(registry::script_fns("pub async fn {").is_err());
}

#[test]
fn the_registry_numbers_scripts_by_name_across_files() {
    let d = scratch("registry");
    put(
        &d,
        "ch01_b.rs",
        "pub async fn Zed(ctx: Ctx) {}\npub async fn Mid(ctx: Ctx) {}\n",
    );
    put(&d, "ch00_a.rs", "pub async fn Alpha(ctx: Ctx) {}\n");
    put(&d, "notes.txt", "pub async fn Ignored(ctx: Ctx) {}\n");
    let reg = registry::scan(&d).unwrap();
    let names: Vec<_> = reg.scripts.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["Alpha", "Mid", "Zed"]);
    let modules: Vec<_> = reg.modules.iter().map(|m| m.0.as_str()).collect();
    assert_eq!(modules, ["ch00_a", "ch01_b"]);
    assert_eq!(reg.scripts[1].module, "ch01_b");
    // The digest follows the file: an edit of the file moves its scripts' digests only.
    let before: Vec<u64> = reg.scripts.iter().map(|s| s.digest).collect();
    put(
        &d,
        "ch01_b.rs",
        "pub async fn Zed(ctx: Ctx) {}\npub async fn Mid(ctx: Ctx) { }\n",
    );
    let after: Vec<u64> = (registry::scan(&d).unwrap().scripts.iter())
        .map(|s| s.digest)
        .collect();
    assert_eq!(after[0], before[0]);
    assert_ne!(after[1], before[1]);
    assert_ne!(after[2], before[2]);

    put(&d, "ch02_c.rs", "pub async fn Alpha(ctx: Ctx) {}\n");
    let e = registry::scan(&d).unwrap_err();
    assert!(format!("{e:#}").contains("defined in both"), "{e:#}");
    std::fs::remove_file(d.join("ch02_c.rs")).unwrap();
    put(&d, "mod.rs", "");
    assert!(registry::scan(&d).is_err());
    assert!(
        registry::scan(&d.join("missing"))
            .unwrap()
            .scripts
            .is_empty()
    );
}

#[test]
fn the_committed_scripts_register() {
    let reg = registry::scan(&workspace().join("crates/content/src/silice/scripts")).unwrap();
    assert!(!reg.scripts.is_empty());
    assert!(reg.scripts.windows(2).all(|w| w[0].name < w[1].name));
}

// ---- unknown scripts in maps ----

fn committed_maps(strict: bool, reg: &Registry) -> (anyhow::Result<()>, Resolver) {
    let ids = ids::load(&games()).unwrap();
    let mut r = Resolver::new(reg, strict);
    let mut res = Ok(());
    for m in silice::load::load_maps(&games()).unwrap() {
        if let Err(e) = silice::merge::to_map_file(&m, &ids, &mut r) {
            res = Err(e);
            break;
        }
    }
    (res, r)
}

#[test]
fn an_unknown_script_fails_when_strict_and_is_listed_in_the_allowlist_mode() {
    let reg = registry::scan(&workspace().join("crates/content/src/silice/scripts")).unwrap();
    let (strict, _) = committed_maps(true, &reg);
    let e = strict.unwrap_err();
    assert!(
        format!("{e:#}").contains("is not defined by any src/silice/scripts/*.rs"),
        "{e:#}"
    );
    let (allowed, r) = committed_maps(false, &reg);
    allowed.unwrap();
    assert!(!r.unwritten.is_empty());
    for s in &reg.scripts {
        assert!(!r.unwritten.contains(&s.name), "{} is registered", s.name);
    }
    // With every name registered, the strict build passes.
    let all = Registry {
        scripts: (r
            .unwritten
            .iter()
            .chain(reg.scripts.iter().map(|s| &s.name)))
        .map(|n| input::ScriptIn {
            name: n.clone(),
            module: "m".into(),
            digest: 0,
        })
        .collect(),
        modules: Vec::new(),
    };
    let (strict, r) = committed_maps(true, &all);
    strict.unwrap();
    assert!(r.unwritten.is_empty());
}

#[test]
fn maps_resolve_silice_names_to_numbers_and_keep_gen3_names() {
    let ids = ids::load(&games()).unwrap();
    let reg = Registry::default();
    let mut r = Resolver::new(&reg, false);
    let maps = silice::load::load_maps(&games()).unwrap();
    assert!(!maps.is_empty());
    for m in &maps {
        let f = silice::merge::to_map_file(m, &ids, &mut r).unwrap();
        let num = ids.map_file(&m.map.name).unwrap().num;
        assert_eq!(u32::from(f.group) << 8 | u32::from(f.num), num);
        assert_eq!(f.layout, m.layout);
        for o in &f.objects {
            if let Some(flag) = &o.flag {
                assert!(extract::names::FLAGS.value(flag).is_some(), "{flag}");
            }
        }
        for c in &f.coords {
            if let Some(v) = &c.var {
                assert!(extract::names::VARS.value(v).is_some(), "{v}");
            }
        }
        for w in &f.warps {
            assert!(w.dest_map.is_some() || w.dest_map_id == "127.127");
        }
    }
}

// ---- per-flavour sources ----

#[test]
fn each_flavour_hashes_only_its_own_sources() {
    use codegen::{build_source, extract_source, games_source, src_source};
    assert!(src_source(false, "lib.rs") && src_source(true, "lib.rs"));
    assert!(src_source(false, "prelude/mod.rs") && src_source(true, "prelude/mod.rs"));
    assert!(!src_source(false, "silice/mod.rs") && src_source(true, "silice/mod.rs"));
    assert!(src_source(false, "overrides/X.rs") && !src_source(true, "overrides/X.rs"));
    assert!(src_source(false, "new_game.rs") && !src_source(true, "new_game.rs"));
    assert!(!src_source(true, "silice/tests.rs"), "tests never count");
    assert!(!build_source(false, "silice/ids.rs") && build_source(true, "silice/ids.rs"));
    assert!(!extract_source(false, "games/data.rs") && extract_source(true, "games/data.rs"));
    assert!(games_source("ids/flags.ron") && games_source("maps/A.ron"));
    assert!(!games_source("edits/A/00_create.ron") && !games_source("IMPORT-REPORT.md"));

    let tmp = scratch("sources");
    let games = tmp.join("games");
    put(&tmp, "content/src/lib.rs", "a");
    put(&tmp, "content/build/main.rs", "b");
    put(&tmp, "games/ids/maps.ron", "m");
    put(&tmp, "extract/src/lib.rs", "e");
    let content = tmp.join("content");
    let emerald = || codegen::sources_digest(&content).unwrap();
    let silice = || codegen::sources_digest_for(&content, Some(&games)).unwrap();
    let id = |g: Option<&Path>| build_id::build_id_for(&content, None, g).unwrap();
    let (e0, s0, ide0, ids0) = (emerald(), silice(), id(None), id(Some(&games)));
    assert_ne!(e0, s0);
    // A Silice source: only the Silice digests move.
    put(&tmp, "content/src/silice/mod.rs", "s");
    put(&tmp, "content/build/silice/ids.rs", "s");
    assert_eq!((emerald(), id(None)), (e0, ide0));
    let (s1, ids1) = (silice(), id(Some(&games)));
    assert_ne!(s1, s0);
    assert_ne!(ids1, ids0);
    // A script, a map or an edit script: BUILD_ID only (a script has its SCRIPT_DIGESTS entry,
    // a map its MAP_DIGESTS entry), and never Emerald's.
    put(&tmp, "content/src/silice/scripts/ch00.rs", "x");
    put(&tmp, "games/maps/A.ron", "x");
    assert_eq!(silice(), s1);
    let ids2 = id(Some(&games));
    assert_ne!(ids2, ids1);
    put(&tmp, "games/edits/A/00_create.ron", "x");
    assert_eq!(id(Some(&games)), ids2);
    assert_eq!((emerald(), id(None)), (e0, ide0));
    // An Emerald-only source: only the Emerald digests move.
    put(&tmp, "content/src/overrides/X.rs", "o");
    assert_eq!((silice(), id(Some(&games))), (s1, ids2));
    assert_ne!(emerald(), e0);
}

// ---- texts ----

#[test]
fn texts_are_read_entry_by_entry_and_a_repeated_id_fails() {
    let d = scratch("texts");
    put(
        &d,
        "ok.ron",
        "{\n    \"B\": r\"b\\n\",\n    \"A\": \"a\",\n}\n",
    );
    let t = silice::text::read(&d.join("ok.ron")).unwrap();
    assert_eq!(t, [("B".into(), "b\\n".into()), ("A".into(), "a".into())]);
    put(
        &d,
        "dup.ron",
        "{\n    \"A\": \"a\",\n    \"A\": \"b\",\n}\n",
    );
    let e = silice::text::read(&d.join("dup.ron")).unwrap_err();
    assert!(
        format!("{e:#}").contains("text id A is defined twice"),
        "{e:#}"
    );
}
