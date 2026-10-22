//! Decomp label transfer (M8 Task B) on both ROMs and every map: coverage on BPEF, precision on
//! BPEE (checked by content, since there is no built decomp symbol map), `map.json` roots and
//! independence from the order of the maps fed in.
use extract::inc::{Decomp, Label, LabelKind};
use extract::labels::{self, Kind, MapInput, NameOrigin, Transfer};
use extract::{Rom, export, fixtures, maps, movement, names, scripts, text};
use std::collections::{BTreeMap, BTreeSet};
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

fn fr_all() -> &'static (Rom, Transfer) {
    static T: OnceLock<(Rom, Transfer)> = OnceLock::new();
    T.get_or_init(|| {
        let rom = fixtures::fr();
        let t = labels::transfer(&rom, Some(decomp()), &inputs(&rom, &all_maps())).unwrap();
        (rom, t)
    })
}

fn us_all() -> &'static (Rom, Transfer) {
    static T: OnceLock<(Rom, Transfer)> = OnceLock::new();
    T.get_or_init(|| {
        let rom = fixtures::us();
        let t = labels::transfer(&rom, Some(decomp()), &inputs(&rom, &all_maps())).unwrap();
        (rom, t)
    })
}

fn pct(n: usize, of: usize) -> f64 {
    100.0 * n as f64 / of.max(1) as f64
}

/// `(decomp-named, total)` for functions, texts and movements.
type PerKind = [(usize, usize); 3];

/// Prints the transfer's errors, collisions, mismatches and per-module coverage (worst first).
fn report(what: &str, t: &Transfer) {
    eprintln!(
        "{what}: {} errors, {} collisions, {} mismatched blocks",
        t.errors.len(),
        t.collisions.len(),
        t.mismatches.len()
    );
    for e in &t.errors {
        eprintln!("  error: {} at {:x?}", e.label, e.offsets);
    }
    for c in &t.collisions {
        eprintln!(
            "  collision at {:#x}: kept {}, dropped {:?}",
            c.offset, c.kept, c.dropped
        );
    }
    for m in &t.mismatches {
        eprintln!("  mismatch: {} at {:#x}: {}", m.label, m.offset, m.reason);
    }
    // Per module: (named, total) per kind.
    let mut per: BTreeMap<&str, PerKind> = BTreeMap::new();
    for (i, set) in [&t.reached.scripts, &t.reached.texts, &t.reached.movements]
        .into_iter()
        .enumerate()
    {
        for off in set {
            let module = t.modules.get(off).map_or("?", String::as_str);
            let e = &mut per.entry(module).or_default()[i];
            e.1 += 1;
            if t.names[off].1 == NameOrigin::Decomp {
                e.0 += 1;
            }
        }
    }
    let mut rows: Vec<(usize, &str, PerKind)> = per
        .into_iter()
        .map(|(m, k)| (k.iter().map(|(n, of)| of - n).sum(), m, k))
        .collect();
    rows.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
    eprintln!("  worst modules (of {}):", rows.len());
    for (_, module, k) in rows.iter().take(10) {
        let s = t.stats.get(*module).copied().unwrap_or_default();
        eprintln!(
            "  {module}: scripts {}/{}, texts {}/{}, movements {}/{}; blocks aligned {}, mismatched {}",
            k[0].0, k[0].1, k[1].0, k[1].1, k[2].0, k[2].1, s.aligned, s.mismatched
        );
    }
}

#[test]
fn bpef_coverage_is_at_least_95_percent_per_kind() {
    let (_, t) = fr_all();
    report("BPEF", t);
    for (kind, set) in [
        ("functions", &t.reached.scripts),
        ("texts", &t.reached.texts),
        ("movements", &t.reached.movements),
    ] {
        let (n, of) = t.coverage(set);
        eprintln!("BPEF {kind}: {n}/{of} decomp ({:.2}%)", pct(n, of));
        assert!(of > 0 && pct(n, of) >= 95.0, "{kind}: {n}/{of}");
    }
    // Every reached offset has a name, and names are unique.
    let all: BTreeSet<u32> = t
        .reached
        .scripts
        .iter()
        .chain(&t.reached.texts)
        .chain(&t.reached.movements)
        .chain(&t.reached.braille)
        .copied()
        .collect();
    assert!(all.iter().all(|o| t.names.contains_key(o)));
    let unique: BTreeSet<&String> = t.names.values().map(|(n, _)| n).collect();
    assert_eq!(unique.len(), t.names.len());
}

/// Values of a decomp constant: a number, or every entry of `table` named `<prefix><s>` (a
/// special can be listed twice); empty when the source spells it some other way.
fn value(table: &names::Table, prefix: &str, s: &str) -> Vec<u32> {
    let num = match s.strip_prefix("0x") {
        Some(h) => u32::from_str_radix(h, 16).ok(),
        None => s.parse().ok(),
    };
    if let Some(n) = num {
        return vec![n];
    }
    let name = format!("{prefix}{s}");
    let mut out: Vec<u32> = table
        .iter()
        .filter(|(_, n)| *n == name)
        .map(|(v, _)| v)
        .collect();
    out.extend(table.value(&name));
    out
}

/// The `Flag`/`Var`/`Const` operands a statement's first command must carry: (operand index,
/// expected value) for the ones the source names.
fn named_operands(mac: &str, args: &[String]) -> Vec<(usize, Vec<u32>)> {
    let flag = |i: usize| value(&names::FLAGS, "", &args[i]);
    let var = |i: usize| value(&names::VARS, "", &args[i]);
    let cc = |p: &str| {
        ["lt", "eq", "gt", "le", "ge", "ne"]
            .iter()
            .any(|c| mac.strip_prefix(p) == Some(c))
    };
    match mac {
        "setflag" | "clearflag" | "checkflag" | "goto_if_set" | "goto_if_unset" | "call_if_set"
        | "call_if_unset" => vec![(0, flag(0))],
        "setvar" => vec![(0, var(0)), (1, value(&names::VARS, "", &args[1]))],
        "addvar" | "subvar" | "copyvar" => vec![(0, var(0))],
        "special" => vec![(0, value(&names::SPECIALS, "SPECIAL_", &args[0]))],
        "case" => vec![
            (0, value(&names::VARS, "", "VAR_0x8000")),
            (1, value(&names::VARS, "", &args[0])),
        ],
        _ if (cc("goto_if_") || cc("call_if_")) && args.len() == 3 => {
            vec![(0, var(0)), (1, value(&names::VARS, "", &args[1]))]
        }
        _ => vec![],
    }
}

/// Checks the commands at `off` against `l`'s body (`compare` either form, and the named
/// flag, var and special operands). Returns how many commands were checked and whether the
/// whole body was (bodies are checked up to a macro the reader cannot expand).
fn script_matches(d: &Decomp, rom: &Rom, l: &Label, off: u32) -> Result<(usize, bool), String> {
    let mut pc = off;
    let mut n = 0;
    for s in &l.body {
        let Some(ops) = d.expand_stmt(s) else {
            return Ok((n, false));
        };
        let want_args = named_operands(&s.mac, &s.args);
        for (k, w) in ops.iter().enumerate() {
            let insn = scripts::decode(rom, pc)
                .map_err(|e| e.to_string())?
                .ok_or(format!("invalid opcode at {pc:#x}"))?;
            let ok = match *w {
                "compare" => insn.name.starts_with("compare_var_to_"),
                w => insn.name == w,
            };
            if !ok {
                return Err(format!(
                    "line {}: {} in the ROM, {w} in the source",
                    s.line, insn.name
                ));
            }
            if k == 0 {
                for (i, want) in &want_args {
                    let got = match insn.args.get(*i) {
                        Some(scripts::Operand::Half(h)) => Some(u32::from(*h)),
                        _ => None,
                    };
                    if let Some(got) = got
                        && !want.is_empty()
                        && !want.contains(&got)
                    {
                        return Err(format!(
                            "line {}: {} operand {i} is {got:#x}, source {:?}",
                            s.line, insn.name, s.args
                        ));
                    }
                }
            }
            n += 1;
            pc += insn.len;
        }
        if scripts::opcode(&s.mac).is_some_and(scripts::is_terminator_op) {
            break;
        }
    }
    Ok((n, true))
}

#[test]
fn bpee_decomp_names_match_their_source_by_content() {
    let (rom, t) = us_all();
    report("BPEE", t);
    let d = decomp();
    let mut checked = BTreeMap::<Kind, usize>::new();
    let mut unchecked = BTreeMap::<Kind, usize>::new();
    let mut wrong = Vec::new();
    let mut partial = 0;
    let mut unexpanded = Vec::new();
    for (&off, (name, origin)) in &t.names {
        if *origin != NameOrigin::Decomp {
            continue;
        }
        let kind = t.kinds[&off];
        let Some(l) = d.label(name) else {
            *unchecked.entry(kind).or_default() += 1;
            continue;
        };
        let res = match (kind, l.kind) {
            (Kind::Script, LabelKind::Script) => match script_matches(d, rom, l, off) {
                Ok((0, false)) => {
                    *unchecked.entry(kind).or_default() += 1;
                    unexpanded.push(name.clone());
                    continue;
                }
                Ok((_, complete)) => {
                    if !complete {
                        partial += 1;
                    }
                    Ok(())
                }
                Err(e) => Err(e),
            },
            (Kind::Text, LabelKind::Text) => {
                let src = d.text(l).unwrap();
                let want = text::encode(&src).unwrap();
                match rom.slice(off, want.len()) {
                    Ok(got) if got == want.as_slice() => Ok(()),
                    _ => Err(format!("text differs: {:?}", rom.string_at(off).ok())),
                }
            }
            (Kind::Movement, LabelKind::Movement) => {
                let want = d.movement_actions(l).unwrap();
                let got = movement::decode(rom, off).unwrap();
                let ok = if d.terminates(l) {
                    got == want
                } else {
                    got.starts_with(&want)
                };
                if ok {
                    Ok(())
                } else {
                    Err(format!("actions {got:02x?}, source {want:02x?}"))
                }
            }
            (Kind::Braille, LabelKind::Braille) => {
                *unchecked.entry(kind).or_default() += 1;
                continue;
            }
            (k, lk) => Err(format!("{k:?} at a {lk:?} label")),
        };
        *checked.entry(kind).or_default() += 1;
        if let Err(e) = res {
            wrong.push(format!("{name} at {off:#x}: {e}"));
        }
    }
    eprintln!(
        "BPEE checked {checked:?} ({partial} scripts up to an unexpanded macro), unchecked \
         {unchecked:?} (scripts: {unexpanded:?})"
    );
    // Only braille goes unchecked (and scripts opening with a macro the reader cannot expand,
    // none today).
    assert!(unexpanded.is_empty(), "{unexpanded:?}");
    assert!(partial == 0, "{partial} scripts checked partially");
    assert!(
        unchecked.keys().all(|k| *k == Kind::Braille),
        "{unchecked:?}"
    );
    for (kind, set) in [
        ("functions", &t.reached.scripts),
        ("texts", &t.reached.texts),
        ("movements", &t.reached.movements),
    ] {
        let (n, of) = t.coverage(set);
        eprintln!("BPEE {kind}: {n}/{of} decomp ({:.2}%)", pct(n, of));
    }
    assert!(checked[&Kind::Script] > 3000 && checked[&Kind::Text] > 3000);
    assert!(
        wrong.is_empty(),
        "{} wrong:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
    assert!(t.errors.is_empty(), "{:?}", t.errors);
}

/// The `map.json` script labels of `map` (its events map), every kind.
fn json_labels(map: &str) -> Vec<String> {
    let j = decomp().events_map(map).unwrap();
    j.objects
        .iter()
        .chain(&j.coords)
        .chain(&j.bgs)
        .flatten()
        .cloned()
        .collect()
}

#[test]
fn every_map_json_event_label_is_a_root() {
    let (_, t) = fr_all();
    let roots: BTreeSet<&String> = t.root_labels.values().collect();
    let mut missing = Vec::new();
    for map in all_maps() {
        if decomp().events_map(map).is_none() {
            continue;
        }
        for l in json_labels(map) {
            if !roots.contains(&l) {
                missing.push(format!("{map}: {l}"));
            }
        }
    }
    assert!(missing.is_empty(), "{missing:?}");
    // Each root carries its label, unless a collision kept an earlier alias.
    let dropped: BTreeSet<&String> = t.collisions.iter().flat_map(|c| &c.dropped).collect();
    for (pos, label) in &t.root_labels {
        let (name, _) = &t.names[&t.roots[pos]];
        assert!(
            name == label || dropped.contains(label),
            "{pos}: {name}, map.json says {label}"
        );
    }
}

#[test]
fn slice_roots_get_their_map_json_labels() {
    let rom = fixtures::fr();
    let t = labels::transfer(&rom, Some(decomp()), &inputs(&rom, &export::SLICE_MAPS)).unwrap();
    let mut n = 0;
    for map in export::SLICE_MAPS {
        for l in json_labels(map) {
            let pos = t
                .root_labels
                .iter()
                .find(|(_, v)| **v == l)
                .map(|(k, _)| k)
                .unwrap_or_else(|| panic!("{map}: no root for {l}"));
            assert_eq!(t.names[&t.roots[pos]], (l.clone(), NameOrigin::Decomp));
            n += 1;
        }
        let ms = decomp().map_scripts(map);
        for l in ms
            .on_load
            .iter()
            .chain(&ms.on_transition)
            .chain(&ms.on_resume)
            .chain(&ms.on_frame)
        {
            assert!(t.names.values().any(|(name, _)| name == l), "{map}: {l}");
        }
    }
    assert!(n > 100, "{n}");
}

#[test]
fn decomp_names_do_not_depend_on_map_order() {
    let rom = fixtures::fr();
    let fwd: Vec<&str> = export::SLICE_MAPS.to_vec();
    let mut rev = fwd.clone();
    rev.reverse();
    let a = labels::transfer(&rom, Some(decomp()), &inputs(&rom, &fwd)).unwrap();
    let b = labels::transfer(&rom, Some(decomp()), &inputs(&rom, &rev)).unwrap();
    let decomp_names = |t: &Transfer| -> BTreeMap<u32, String> {
        t.names
            .iter()
            .filter(|(_, (_, o))| *o == NameOrigin::Decomp)
            .map(|(k, (n, _))| (*k, n.clone()))
            .collect()
    };
    assert_eq!(decomp_names(&a), decomp_names(&b));
    // A subset names its shared scripts as the whole set does.
    let (_, all) = fr_all();
    let full = decomp_names(all);
    for (off, name) in decomp_names(&a) {
        assert_eq!(full.get(&off), Some(&name), "{off:#x}");
    }
}

#[test]
fn without_the_decomp_every_name_is_positional() {
    let rom = fixtures::fr();
    let t = labels::transfer(&rom, None, &inputs(&rom, &["LittlerootTown", "Route101"])).unwrap();
    assert!(!t.names.is_empty());
    assert!(t.names.values().all(|(_, o)| *o == NameOrigin::Positional));
    assert!(t.names.values().any(|(n, _)| n == "LittlerootTown_Obj1"));
}

#[test]
fn reader_sees_every_source_and_map() {
    let d = decomp();
    let maps = d
        .files
        .iter()
        .filter(|f| f.path.starts_with("data/maps/"))
        .count();
    assert_eq!(maps, 468);
    assert!(d.files.iter().any(|f| f.module == "Common_event_scripts"));
    assert_eq!(d.maps.len(), names::MAPS.len());
    let whiteout = d.label("EventScript_WhiteOut").unwrap();
    assert_eq!(d.module_of(whiteout), "Common_event_scripts");
    assert_eq!(whiteout.kind, LabelKind::Script);
    let shared = d
        .maps
        .values()
        .filter(|m| m.shared_scripts_map.is_some())
        .count();
    assert_eq!(shared, 50);
    // Duplicate label names would make operand lookup ambiguous.
    let mut seen = BTreeSet::new();
    let dups: Vec<&str> = d
        .labels
        .iter()
        .filter(|l| !seen.insert(l.name.as_str()))
        .map(|l| l.name.as_str())
        .collect();
    assert!(dups.is_empty(), "{dups:?}");
}
