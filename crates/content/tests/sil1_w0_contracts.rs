//! SIL1 Wave 0: the Silice build contracts, on the build's own code (`build/silice/`), in any
//! flavour and without the extract:
//! - id allocation: the committed ids load, and keep every number of the first import
//!   (`xtask/tests/fixtures/silice/ids.golden`); a renumbering, a dropped name or a reused number
//!   fails; tombstones keep their number and get no constant;
//! - the script registry: which functions are scripts, name-order ids, unique names, file names;
//! - an unknown script fails in a strict map (claimed by a script file's `MAPS`, or every map
//!   when forced), and is dropped and listed in an unclaimed one (the allowlist mode); claims
//!   and trainer scripts (SIL1 Task F);
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

    // A var on a number the shared code uses (VAR_STARTER_MON, 0x4023) collides; moved
    // (tombstoned, then appended live), it is fine, and the golden keeps both numbers.
    let vars = |ids: Vec<IdEntry>| IdsFile { base: 0x4020, ids };
    let on_starter = vars(vec![entry("VAR_SIL_A", 0x4022), entry("VAR_SIL_B", 0x4023)]);
    let e = ids::validate(IdKind::Vars, &on_starter).unwrap_err();
    assert!(format!("{e:#}").contains("is reserved"), "{e:#}");
    let mut moved = on_starter.clone();
    moved.ids[1].removed = true;
    moved.ids.push(entry("VAR_SIL_B", 0x4024));
    ids::validate(IdKind::Vars, &moved).unwrap();
    let mut both = ids::Ids::default();
    both.files.insert(IdKind::Vars, moved.clone());
    assert_eq!(both.live(IdKind::Vars, "VAR_SIL_B").unwrap(), Some(0x4024));
    let consts = both.consts();
    assert!(consts.contains(&("VAR_SIL_B".into(), "VarId", "VarId(0x4024)".into())));
    assert!(!consts.iter().any(|c| c.2 == "VarId(0x4023)"));
    let golden = "vars VAR_SIL_B 16419\nvars VAR_SIL_B 16420\n";
    assert_eq!(ids::check_golden(golden, &both).unwrap(), 2);
    // Twice live, or listed again before its tombstone: rejected.
    let mut twice = moved.clone();
    twice.ids[1].removed = false;
    assert!(ids::validate(IdKind::Vars, &twice).is_err());
    let mut before = moved;
    before.ids.swap(1, 2);
    assert!(ids::validate(IdKind::Vars, &before).is_err());
}

/// `VAR_SIL_EVALON_PARTENAIRE` sat on `VAR_STARTER_MON`'s number (0x4023): it was moved, its
/// tombstone keeps 0x4023, and no live Silice flag or var is on a shared Gen3 number.
#[test]
fn no_committed_silice_id_takes_a_shared_gen3_number() {
    let ids = ids::load(&games()).unwrap();
    let vars = &ids.files[&IdKind::Vars];
    let evalon: Vec<_> = (vars.ids.iter())
        .filter(|e| e.name == "VAR_SIL_EVALON_PARTENAIRE")
        .map(|e| (e.num, e.removed))
        .collect();
    assert_eq!(evalon, [(0x4023, true), (0x4034, false)]);
    for (kind, name, num) in extract::games::shared_gen3() {
        for e in ids.files[&kind].ids.iter().filter(|e| !e.removed) {
            assert_ne!(e.num, num, "{} takes {name}'s number", e.name);
        }
    }
}

/// `extract::games::SHARED_GEN3` is exactly the Gen3 flags and vars the shared code names: the
/// Gen3 prelude, the crate root, `samples.rs` and the Silice root (its `SpecialIds`, and
/// `game.ron`).
#[test]
fn the_reserved_gen3_names_are_the_ones_the_shared_code_uses() {
    let src = workspace().join("crates/content/src");
    let mut files = vec![
        src.join("lib.rs"),
        src.join("samples.rs"),
        src.join("silice/mod.rs"),
        src.join("silice/prelude.rs"),
        src.join("silice/new_game.rs"),
        // Its system ids and new-game flags (SIL1 Task E).
        games().join("game.ron"),
    ];
    for e in std::fs::read_dir(src.join("prelude")).unwrap() {
        files.push(e.unwrap().path());
    }
    let mut used = std::collections::BTreeSet::new();
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap();
        for w in words(&text) {
            let gen3 = if w.starts_with("FLAG_") {
                extract::names::FLAGS.value(w).is_some()
            } else {
                w.starts_with("VAR_") && extract::names::VARS.value(w).is_some()
            };
            // Silice's own names (`*_SIL_*`) and hex names are not Gen3 ones.
            if gen3 && !w.contains("_SIL_") && !w.contains("_0x") {
                used.insert(w.to_string());
            }
        }
    }
    let listed: std::collections::BTreeSet<String> = (extract::games::SHARED_GEN3.iter())
        .map(|s| s.to_string())
        .collect();
    assert_eq!(
        used, listed,
        "update extract::games::SHARED_GEN3 to the Gen3 flags and vars the shared code names"
    );
}

/// The identifier-like words of `text`.
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
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

/// Resolves every committed map, then the strict check (as `silice::world` does).
fn committed_maps(strict: bool, reg: &Registry) -> (anyhow::Result<()>, Resolver) {
    let ids = ids::load(&games()).unwrap();
    let mut r = Resolver::new(reg, strict);
    for m in silice::load::load_maps(&games()).unwrap() {
        silice::merge::to_map_file(&m, &ids, &mut r).unwrap();
    }
    (r.finish().and_then(|()| r.check_claims()), r)
}

/// The committed registry without one script file's scripts (and, if `keep_claims` is false,
/// without its claims): the shape of a chapter whose maps are drafted before its scenes.
fn without_module(reg: &Registry, module: &str, keep_claims: bool) -> Registry {
    let mut r = reg.clone();
    r.scripts.retain(|s| s.module != module);
    r.modules.retain(|m| m.0 != module);
    if !keep_claims {
        r.claimed.retain(|_, m| m != module);
    }
    r
}

/// The Ch0 maps, claimed by the `ch00_*.rs` files.
const CH0_MAPS: [&str; 6] = [
    "BricoBourg",
    "BricoBourg_Bar",
    "BricoBourg_FriendsHouse",
    "BricoBourg_PlayerHouse_1F",
    "BricoBourg_PlayerHouse_2F",
    "RouteS01",
];

#[test]
fn the_committed_maps_are_claimed_and_every_script_they_name_exists() {
    let reg = registry::scan(&workspace().join("crates/content/src/silice/scripts")).unwrap();
    let (strict, r) = committed_maps(false, &reg);
    strict.unwrap();
    // Only maps no file claims yet (chapters drafted before their scenes) name unwritten
    // scripts.
    for (n, maps) in &r.unwritten {
        for m in maps {
            assert!(!reg.claimed.contains_key(m), "{n} in claimed map {m}");
        }
    }
    // The Ch0 maps are claimed by the ch00 files.
    for n in CH0_MAPS {
        let module = reg.claimed.get(n).unwrap_or_else(|| panic!("{n}"));
        assert!(module.starts_with("ch00_"), "{n}: {module}");
    }
}

#[test]
fn an_unknown_script_fails_in_a_claimed_map_and_is_listed_in_an_unclaimed_one() {
    let reg = registry::scan(&workspace().join("crates/content/src/silice/scripts")).unwrap();
    // What the unclaimed maps already leave unwritten.
    let (_, base) = committed_maps(false, &reg);
    let base: Vec<&String> = base.unwritten.keys().collect();
    let module = &reg.claimed["RouteS01"];
    // Unclaimed (the allowlist mode): dropped and listed, the build goes on.
    let draft = without_module(&reg, module, false);
    let (allowed, r) = committed_maps(false, &draft);
    allowed.unwrap();
    let gone: Vec<&String> = (reg.scripts.iter())
        .filter(|s| &s.module == module)
        .map(|s| &s.name)
        .collect();
    assert!(gone.len() > 1);
    for n in &gone {
        let maps = r.unwritten.get(*n).unwrap_or_else(|| panic!("{n}"));
        assert!(maps.contains("RouteS01"), "{n}: {maps:?}");
    }
    for n in r.unwritten.keys() {
        assert!(gone.contains(&n) || base.contains(&n), "{n} is registered");
    }
    // Forced strict (`PKR_SILICE_STRICT_SCRIPTS=1`, which also fails on the unclaimed maps'
    // names), and claimed: one error after every map, naming every unknown script once, with
    // its maps.
    for (force, keep) in [(true, false), (false, true)] {
        let (strict, _) = committed_maps(force, &without_module(&reg, module, keep));
        let e = format!("{:#}", strict.unwrap_err());
        let count = gone.len() + if force { base.len() } else { 0 };
        assert!(
            e.starts_with(&format!(
                "{count} scripts the strict maps name are not defined by any src/silice/scripts/*.rs: "
            )),
            "{e}"
        );
        for n in &gone {
            assert_eq!(e.matches(&format!("{n} (map ")).count(), 1, "{n}: {e}");
        }
    }
}

#[test]
fn a_claimed_map_must_exist_and_be_claimed_once() {
    let reg = registry::scan(&workspace().join("crates/content/src/silice/scripts")).unwrap();
    let mut r = reg.clone();
    r.claimed.insert("NoSuchMap".into(), "ch00_town".into());
    let (e, _) = committed_maps(false, &r);
    let e = format!("{:#}", e.unwrap_err());
    assert!(e.contains("NoSuchMap (scripts/ch00_town.rs)"), "{e}");
    assert!(e.contains("are not in maps/"), "{e}");

    let d = scratch("claims");
    put(
        &d,
        "ch01_a.rs",
        "pub const MAPS: &[&str] = &[\"A\", \"B\"];\npub async fn S(ctx: Ctx) {}\n",
    );
    let reg = registry::scan(&d).unwrap();
    let claims: Vec<(&str, &str)> = (reg.claimed.iter())
        .map(|(m, f)| (m.as_str(), f.as_str()))
        .collect();
    assert_eq!(claims, [("A", "ch01_a"), ("B", "ch01_a")]);
    put(&d, "ch01_b.rs", "pub const MAPS: &[&str] = &[\"B\"];\n");
    let e = format!("{:#}", registry::scan(&d).unwrap_err());
    assert!(e.contains("map B is claimed by both"), "{e}");
    put(
        &d,
        "ch01_b.rs",
        "pub const MAPS: &[&str] = &[\"C\", \"C\"];\n",
    );
    let e = format!("{:#}", registry::scan(&d).unwrap_err());
    assert!(e.contains("map C is listed twice in `MAPS`"), "{e}");
    for (bad, why) in [
        ("const MAPS: &[&str] = &[\"A\"];", "must be `pub`"),
        (
            "pub const MAPS: &[&str] = &[\"A\", NAME];",
            "element 2 is not a string literal",
        ),
        (
            "pub const MAPS: &[&str] = X;",
            "expected an array of string literals",
        ),
        ("pub static MAPS: &[&str] = &[\"A\"];", "`static MAPS`"),
    ] {
        let e = format!("{:#}", registry::claimed_maps(bad).unwrap_err());
        assert!(e.contains(why), "{bad}: {e}");
    }
    assert!(
        registry::claimed_maps("pub const OTHER: u8 = 1;")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn trainer_scripts_are_found_by_their_first_statement() {
    let src = "\
pub async fn T1(ctx: Ctx) {
    if trainer(&ctx, LOCALID_A, TRAINER_SIL_A, I, D).await { say(&ctx, X).await; }
}
pub async fn T2(ctx: Ctx) {
    let _ = trainer(&ctx, 2, TRAINER_SIL_B, I, D).await;
}
pub async fn T3(ctx: Ctx) {
    trainer(&ctx, 3, TRAINER_SIL_C, I, D).await;
}
pub async fn NotFirst(ctx: Ctx) {
    say(&ctx, X).await;
    trainer(&ctx, 4, TRAINER_SIL_D, I, D).await;
}
pub async fn NotATrainer(ctx: Ctx) {
    trainer(&ctx, 5, OTHER, I, D).await;
}
async fn Private(ctx: Ctx) {
    trainer(&ctx, 6, TRAINER_SIL_E, I, D).await;
}
";
    let t = registry::trainer_fns(src).unwrap();
    let t: Vec<(&str, &str)> = t.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    assert_eq!(
        t,
        [
            ("T1", "TRAINER_SIL_A"),
            ("T2", "TRAINER_SIL_B"),
            ("T3", "TRAINER_SIL_C")
        ]
    );
    // Every trainer object of a strict map runs a trainer script, or the build fails.
    let reg = registry::scan(&workspace().join("crates/content/src/silice/scripts")).unwrap();
    let ids = ids::load(&games()).unwrap();
    let mut r = Resolver::new(&reg, false);
    let maps: Vec<_> = (silice::load::load_maps(&games()).unwrap().iter())
        .map(|m| silice::merge::to_map_file(m, &ids, &mut r).unwrap())
        .collect();
    silice::check_trainer_objects(&maps, &reg, &r).unwrap();
    let mut no_teo = reg.clone();
    no_teo
        .trainers
        .retain(|t| t.0 != "RouteS01_EventScript_Teo");
    let e = format!(
        "{:#}",
        silice::check_trainer_objects(&maps, &no_teo, &r).unwrap_err()
    );
    assert!(e.starts_with("1 trainer objects of strict maps"), "{e}");
    assert!(
        e.contains("RouteS01 object 3 (3, 24): RouteS01_EventScript_Teo"),
        "{e}"
    );
    // The committed route trainers.
    let reg = registry::scan(&workspace().join("crates/content/src/silice/scripts")).unwrap();
    let names: Vec<&str> = reg.trainers.iter().map(|t| t.1.as_str()).collect();
    for t in [
        "TRAINER_SIL_GAMIN_TEO",
        "TRAINER_SIL_SCOUT_NOAH",
        "TRAINER_SIL_FILLETTE_LILA",
        "TRAINER_SIL_CAMPEUR_YANIS",
    ] {
        assert!(names.contains(&t), "{t}: {names:?}");
    }
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
    put(&tmp, "content/src/new_game.rs", "n");
    assert_eq!((silice(), id(Some(&games))), (s1, ids2));
    assert_ne!(emerald(), e0);
    // An override: Emerald's BUILD_ID only (its source is in its script's SCRIPT_DIGESTS
    // entry, so not in GLOBAL_DIGEST: Task N).
    let (e1, ide1) = (emerald(), id(None));
    put(&tmp, "content/src/overrides/X.rs", "o");
    assert_eq!((silice(), id(Some(&games))), (s1, ids2));
    assert_eq!(emerald(), e1);
    assert_ne!(id(None), ide1);

    // The extracted dir: Silice hashes its base assets, not the Emerald maps, script IR or
    // previews; Emerald hashes all of it but the previews.
    let x = tmp.join("extracted");
    put(&x, "manifest.ron", "m");
    put(&x, "tilesets/t.ron", "t");
    put(&x, "maps/LittlerootTown.ron", "l");
    put(&x, "scripts/index.ron", "i");
    let ide = |x: &Path| build_id::build_id_for(&content, Some(x), None).unwrap();
    let ids = |x: &Path| build_id::build_id_for(&content, Some(x), Some(&games)).unwrap();
    let (xe0, xs0) = (ide(&x), ids(&x));
    assert_ne!(xe0, xs0);
    assert_ne!(xe0, id(None), "the stub hashes `stub`, not an extract");
    put(&x, "maps/LittlerootTown.ron", "l2");
    put(&x, "scripts/index.ron", "i2");
    assert_eq!(ids(&x), xs0, "the Emerald game is not a Silice input");
    let xe1 = ide(&x);
    assert_ne!(xe1, xe0);
    put(&x, "previews/LittlerootTown.png", "p");
    assert_eq!((ide(&x), ids(&x)), (xe1, xs0), "previews never count");
    put(&x, "tilesets/t.ron", "t2");
    assert_ne!(ids(&x), xs0, "a base asset is a Silice input");
    assert_ne!(ide(&x), xe1);
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
