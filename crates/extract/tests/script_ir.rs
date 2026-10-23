//! The script IR (M8 Task B): built for every map on both ROMs, it covers exactly the
//! instructions today's walk decodes, every name it uses resolves, and it round-trips through
//! RON; the export writes it and no Rust (Task D2).
use extract::export::scripts::{Ir, build_ir};
use extract::export::{self, ExportOptions, to_ron};
use extract::inc::Decomp;
use extract::labels::MapInput;
use extract::schema::{IrArg, Manifest, NameOrigin, RootKind, ScriptFile, ScriptIndex, TextFile};
use extract::{Rom, fixtures, maps, names, roots, scripts};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::OnceLock;

fn decomp() -> &'static Decomp {
    static D: OnceLock<Decomp> = OnceLock::new();
    D.get_or_init(|| Decomp::load(&fixtures::decomp()).unwrap())
}

fn inputs(rom: &Rom, dirs: &[&str]) -> Vec<MapInput> {
    let table = maps::locate_map_table(rom).unwrap();
    dirs.iter()
        .map(|dir| {
            let m = names::map_by_dir(dir).unwrap();
            let id = maps::MapId {
                group: m.group,
                num: m.num,
            };
            MapInput {
                map: dir.to_string(),
                header: table.header(rom, id).unwrap(),
            }
        })
        .collect()
}

fn all_maps() -> Vec<&'static str> {
    names::MAPS.iter().map(|m| m.dir).collect()
}

/// Today's walk over every root of `maps`: the decoded offsets and the unknown opcodes.
fn walk(rom: &Rom, maps: &[MapInput]) -> (BTreeSet<u32>, usize) {
    let mut offs = Vec::new();
    for m in maps {
        offs.extend(
            roots::named_roots(rom, &m.header, &m.map)
                .unwrap()
                .into_iter()
                .map(|(_, o)| o),
        );
    }
    // The std scripts are roots too (Task D2).
    offs.extend(roots::std_scripts(rom).unwrap().unwrap());
    // So are the field scripts C starts (Wave C field moves: `EventScript_UseSurf`).
    offs.extend(roots::field_scripts(rom).unwrap().into_iter().flatten());
    let scan = scripts::walk(rom, &offs).unwrap();
    (scan.visited, scan.unknown.len())
}

/// The offset of every IR instruction, re-decoded from each label's offset.
fn ir_offsets(rom: &Rom, ir: &Ir) -> BTreeSet<u32> {
    let mut out = BTreeSet::new();
    for l in ir.files.values().flat_map(|f| &f.labels) {
        let mut pc = l.offset.0;
        for i in &l.insns {
            out.insert(pc);
            if i.op == "unknown" {
                break;
            }
            let insn = scripts::decode(rom, pc).unwrap().unwrap();
            assert_eq!(insn.name, i.op, "{} at {pc:#x}", l.name);
            pc += insn.len;
        }
    }
    out
}

fn unknown_count(ir: &Ir) -> usize {
    ir.files
        .values()
        .flat_map(|f| &f.labels)
        .flat_map(|l| &l.insns)
        .filter(|i| i.op == "unknown")
        .count()
}

/// Every Script/Text/Movement operand and every `next` names something the IR defines.
fn check_names(ir: &Ir) {
    let movements: BTreeSet<&String> = ir
        .files
        .values()
        .flat_map(|f| &f.movements)
        .map(|m| &m.name)
        .collect();
    for f in ir.files.values() {
        for l in &f.labels {
            assert_eq!(ir.index.scripts[&l.name].module, f.module, "{}", l.name);
            if let Some(n) = &l.next {
                assert!(ir.index.scripts.contains_key(n), "{}: next {n}", l.name);
            }
            for i in &l.insns {
                for a in &i.args {
                    match a {
                        IrArg::Script(s) => assert!(ir.index.scripts.contains_key(s), "{s}"),
                        IrArg::Text(t) => assert!(ir.texts.contains_key(t), "{t}"),
                        IrArg::Movement(m) => assert!(movements.contains(m), "{m}"),
                        _ => {}
                    }
                }
            }
        }
    }
    assert_eq!(
        ir.texts.keys().collect::<Vec<_>>(),
        ir.index.texts.keys().collect::<Vec<_>>()
    );
}

fn check(rom: &Rom, what: &str) -> Ir {
    let maps = inputs(rom, &all_maps());
    let ir = build_ir(rom, Some(decomp()), &maps).unwrap();
    let (visited, unknown) = walk(rom, &maps);
    eprintln!(
        "{what}: {} modules, {} labels, {} insns (walk {}), {} texts",
        ir.files.len(),
        ir.index.scripts.len(),
        ir.insn_count(),
        visited.len(),
        ir.texts.len()
    );
    assert_eq!(ir.insn_count(), visited.len(), "{what}: instructions");
    assert_eq!(ir_offsets(rom, &ir), visited, "{what}: instruction offsets");
    assert_eq!(unknown_count(&ir), unknown, "{what}: unknown opcodes");
    check_names(&ir);
    ir
}

#[test]
fn bpef_ir_covers_the_walk_and_resolves_every_name() {
    let rom = fixtures::fr();
    let ir = check(&rom, "BPEF");
    let decomp_named = ir
        .index
        .scripts
        .values()
        .filter(|e| e.origin == NameOrigin::Decomp)
        .count();
    assert!(decomp_named * 100 >= ir.index.scripts.len() * 95);
    // The map script root of a slice map is a root of its map's module.
    let lt = &ir.files["LittlerootTown"];
    assert_eq!(
        lt.source.as_deref(),
        Some("data/maps/LittlerootTown/scripts.inc")
    );
    assert!(
        lt.labels
            .iter()
            .any(|l| l.root == Some(RootKind::MapScript("OnTransition".into())))
    );
    assert!(ir.files.keys().any(|m| m.starts_with("Common_")));
}

#[test]
fn bpee_ir_covers_the_walk_and_resolves_every_name() {
    check(&fixtures::us(), "BPEE");
}

#[test]
fn ir_round_trips_through_ron() {
    let rom = fixtures::fr();
    let ir = build_ir(&rom, Some(decomp()), &inputs(&rom, &export::SLICE_MAPS)).unwrap();
    for f in ir.files.values() {
        let s = to_ron(f).unwrap();
        assert_eq!(&ron::from_str::<ScriptFile>(&s).unwrap(), f, "{}", f.module);
    }
    let s = to_ron(&ir.index).unwrap();
    assert_eq!(ron::from_str::<ScriptIndex>(&s).unwrap(), ir.index);
}

#[test]
fn without_the_decomp_the_ir_is_positional() {
    let rom = fixtures::fr();
    let maps = inputs(&rom, &export::SLICE_MAPS);
    let ir = build_ir(&rom, None, &maps).unwrap();
    assert!(
        ir.index
            .scripts
            .values()
            .all(|e| e.origin == NameOrigin::Positional)
    );
    let visited = walk(&rom, &maps).0;
    assert_eq!(ir.insn_count(), visited.len());
    assert_eq!(ir_offsets(&rom, &ir), visited);
    check_names(&ir);
}

#[test]
fn export_writes_the_ir_and_no_rust() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("export-script-ir");
    let _ = std::fs::remove_dir_all(&dir);
    let m: Manifest = export::export(&fixtures::fr(), &dir, &ExportOptions::slice(false)).unwrap();
    let index: ScriptIndex = export::read_ron(&dir.join("scripts/index.ron")).unwrap();
    let texts: TextFile = export::read_ron(&dir.join("text/fr.ron")).unwrap();
    assert!(index.scripts.len() > 300 && texts.len() > 300);
    for module in index
        .scripts
        .values()
        .map(|e| &e.module)
        .collect::<BTreeSet<_>>()
    {
        let f: ScriptFile = export::read_ron(&dir.join(format!("scripts/{module}.ron"))).unwrap();
        assert_eq!(&f.module, module);
        assert!(m.files.contains_key(&format!("scripts/{module}.ron")));
    }
    // Task D2: no Rust, no legacy index, no second text file.
    assert!(
        !m.files.keys().any(|f| f.ends_with(".rs")),
        "{:?}",
        m.files.keys()
    );
    assert!(!dir.join("scripts/legacy_index.ron").exists());
    assert!(!dir.join("text/ir_fr.ron").exists());
    // The map files name their scripts by the IR's names.
    for map in export::SLICE_MAPS {
        let f = export::read_map(&dir, map).unwrap();
        let refs = f
            .objects
            .iter()
            .filter_map(|o| o.script.as_ref())
            .chain(f.coords.iter().filter_map(|c| c.script.as_ref()));
        for s in refs {
            assert!(index.scripts.contains_key(s), "{map}: {s}");
        }
    }
    // The std scripts are in the IR, with the texts the native std helpers print.
    for s in extract::roots::STD_SCRIPTS {
        assert!(index.scripts.contains_key(s), "{s}");
    }
    for t in [
        "gText_ObtainedTheItem",
        "gText_PutItemInPocket",
        "gText_PlayerFoundOneItem",
        "gText_TooBadBagIsFull",
    ] {
        assert!(texts.contains_key(t), "{t}");
    }
    assert_eq!(m.format_minor, extract::schema::FORMAT_MINOR);
    // The files a stale extract is told it lacks are ones every extract writes.
    assert!(!extract::schema::MINOR_FILES.is_empty());
    assert!(extract::schema::missing_since(0, |p| m.files.contains_key(p)).is_empty());
    assert_eq!(
        m.provenance[extract::export::scripts::DECOMP_NAMES_KEY],
        extract::schema::Hex(1)
    );
}
