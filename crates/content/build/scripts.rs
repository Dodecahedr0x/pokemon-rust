//! Scripts (Milestone 8 Task D2): the script IR (`scripts/*.ron`, `scripts/index.ron`), the
//! committed support tables (`build/support/*.ron`) and the hand-written overrides
//! (`src/overrides/<Name>.rs`) go through [`extract::codegen::lower`]. The build writes:
//! - `$OUT_DIR/scripts/<Module>.rs`, one per IR module, and `$OUT_DIR/scripts.rs` (one
//!   `pub mod <Module>` each, `include!`d from `$OUT_DIR`; `content::scripts`), with the trainer
//!   table (`TRAINER_SCRIPTS`, `trainer_script`) behind `GameData::trainer_flag_of` and
//!   `GameData::trainer_battle_kind`;
//! - `$OUT_DIR/dispatch.rs` (`content::data`): `Flow`, `SCRIPT_COUNT`, `SCRIPT_NAMES` (sorted by
//!   name; the index is the `ScriptId`), `script_ids`, the `SCRIPTS` thunk table, `script()` and
//!   `SCRIPT_DIGESTS`;
//! - `$OUT_DIR/overrides.rs` (`content::overrides`): one `#[path]` module per applied override,
//!   and `OVERRIDES`;
//! - `$OUT_DIR/report.txt` (`content::REPORT`): the classification report, plus one
//!   `cargo::warning` summary line.
//!
//! **Checks.** A command or special of the IR missing from the support tables fails the build
//! with the list (`lower`); so does an override naming a script the IR does not define.
//!
//! **Overrides apply** when the extract has an IR named from the decomp (manifest provenance
//! `scripts/decomp_names` = 1): override names are decomp labels. Without an IR (the stub,
//! fixtures without scripts) or with positional names only (an extract made without
//! `PKR_DECOMP`), none applies, and the second case warns.

use crate::codegen::ident_ok;
use crate::input::{World, warn};
use anyhow::{Context, Result, bail, ensure};
use extract::codegen::{Lowered, Support};
use extract::export::read_ron;
use extract::schema::{IrArg, Manifest, ScriptFile, ScriptIndex};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// The script IR of an extracted dir.
#[derive(Clone, Debug, Default)]
pub struct Ir {
    /// Every module file, in module order.
    pub files: Vec<ScriptFile>,
    pub index: ScriptIndex,
    /// Whether the names come from the decomp (`scripts/decomp_names` provenance).
    pub decomp_names: bool,
}

/// Reads `scripts/index.ron` and every module it names; `None` without an index.
pub fn read_ir(dir: &Path, manifest: &Manifest) -> Result<Option<Ir>> {
    let index_path = dir.join("scripts/index.ron");
    if !index_path.exists() {
        return Ok(None);
    }
    let index: ScriptIndex = read_ron(&index_path)?;
    let modules: BTreeSet<&String> = index
        .scripts
        .values()
        .map(|e| &e.module)
        .chain(index.movements.values())
        .collect();
    let mut files = Vec::new();
    for m in modules {
        let f: ScriptFile = read_ron(&dir.join(format!("scripts/{m}.ron")))?;
        ensure!(&f.module == m, "scripts/{m}.ron holds module {}", f.module);
        files.push(f);
    }
    let decomp_names = manifest
        .provenance
        .get(extract::export::scripts::DECOMP_NAMES_KEY)
        .is_some_and(|h| h.0 == 1);
    Ok(Some(Ir {
        files,
        index,
        decomp_names,
    }))
}

/// The content crate (`crates/content`).
fn content_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The committed support tables (`build/support/{commands,specials,movements}.ron`), and
/// whether the prelude has Task H's item helpers ([`Support::item_helpers`]).
pub fn support(build_dir: &Path) -> Result<Support> {
    let read = |name: &str| -> Result<String> {
        let path = build_dir.join("support").join(name);
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))
    };
    let mut s = Support::parse(
        &read("commands.ron")?,
        Some(&read("specials.ron")?),
        Some(&read("movements.ron")?),
    )?;
    let items = build_dir.join("../src/prelude/items.rs");
    s.item_helpers = std::fs::read_to_string(&items)
        .with_context(|| format!("reading {}", items.display()))?
        .contains("pub async fn add_item(");
    Ok(s)
}

/// The override sources: `src/overrides/<Name>.rs` (every `.rs` but `mod.rs`), by name.
pub fn override_sources(overrides_dir: &Path) -> Result<BTreeMap<String, (PathBuf, String)>> {
    let mut out = BTreeMap::new();
    if !overrides_dir.is_dir() {
        return Ok(out);
    }
    for e in std::fs::read_dir(overrides_dir)
        .with_context(|| format!("reading {}", overrides_dir.display()))?
    {
        let p = e?.path();
        if p.extension().is_none_or(|x| x != "rs") {
            continue;
        }
        let Some(name) = p.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if name == "mod" {
            continue;
        }
        ensure!(ident_ok(name), "override file name {name:?}");
        let src = std::fs::read_to_string(&p).with_context(|| p.display().to_string())?;
        out.insert(name.to_string(), (p.clone(), src));
    }
    Ok(out)
}

/// The lowering of one world, with what the build needs besides [`Lowered`].
pub struct Scripted {
    pub lowered: Lowered,
    /// Script name -> module, for every lowered function (`SCRIPT_NAMES` order).
    pub module: BTreeMap<String, String>,
    /// The applied overrides: name -> file.
    pub overrides: BTreeMap<String, PathBuf>,
    /// Override names not applied, and why (for the report).
    pub skipped_overrides: Vec<String>,
    /// (script, trainer number, `TRAINER_BATTLE_*` kind) of every trainer script.
    pub trainers: Vec<(String, u16, u8)>,
}

/// Lowers `ir` with `support` and the override `sources` (see the module doc for which apply).
pub fn lower_ir(
    ir: Option<&Ir>,
    support: &Support,
    sources: &BTreeMap<String, (PathBuf, String)>,
) -> Result<Scripted> {
    let empty = Ir::default();
    let ir = ir.unwrap_or(&empty);
    let mut skipped = Vec::new();
    let applied: BTreeMap<String, (PathBuf, String)> = if ir.files.is_empty() {
        BTreeMap::new()
    } else if !ir.decomp_names {
        skipped.extend(
            sources
                .keys()
                .map(|n| format!("{n} (the IR has positional names only)")),
        );
        BTreeMap::new()
    } else {
        sources.clone()
    };
    let texts: BTreeMap<String, String> = applied
        .iter()
        .map(|(n, (_, s))| (n.clone(), s.clone()))
        .collect();
    let lowered = extract::codegen::lower(&ir.files, &ir.index, support, &texts)?;
    let mut module = BTreeMap::new();
    for name in lowered.script_digests.keys() {
        let e = ir
            .index
            .scripts
            .get(name)
            .with_context(|| format!("script {name} is not in scripts/index.ron"))?;
        ensure!(ident_ok(name), "script name {name:?}");
        ensure!(ident_ok(&e.module), "script module {:?}", e.module);
        module.insert(name.clone(), e.module.clone());
    }
    let labels: BTreeMap<&str, &extract::schema::IrLabel> = ir
        .files
        .iter()
        .flat_map(|f| &f.labels)
        .map(|l| (l.name.as_str(), l))
        .collect();
    let mut trainers = Vec::new();
    for (script, trainer) in &lowered.trainer_scripts {
        let Some(number) = extract::names::TRAINERS.value(trainer) else {
            continue;
        };
        let kind = labels
            .get(script.as_str())
            .and_then(|l| l.insns.first())
            .and_then(|i| i.args.first())
            .and_then(battle_kind)
            .with_context(|| format!("{script}: trainerbattle kind"))?;
        trainers.push((script.clone(), number as u16, kind));
    }
    Ok(Scripted {
        lowered,
        module,
        overrides: applied.into_iter().map(|(n, (p, _))| (n, p)).collect(),
        skipped_overrides: skipped,
        trainers,
    })
}

/// The `trainerbattle` type operand as its decomp number.
fn battle_kind(a: &IrArg) -> Option<u8> {
    match a {
        IrArg::Byte(b) => Some(*b),
        IrArg::Half(h) => u8::try_from(*h).ok(),
        IrArg::Const { name, .. } => extract::codegen::lower::TRAINER_BATTLE_KINDS
            .iter()
            .position(|k| k == name)
            .map(|p| p as u8),
        _ => None,
    }
}

/// The lowering of `w`, computed once per extracted dir and process: [`scripts`] (the data
/// generator's `ScriptId`s) and [`generate`] share it.
fn scripted(w: &World) -> Result<Arc<Scripted>> {
    type Cache = Mutex<BTreeMap<Option<PathBuf>, Arc<Scripted>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(s) = cache.lock().unwrap().get(&w.dir) {
        return Ok(s.clone());
    }
    let build = content_dir().join("build");
    let support = support(&build)?;
    let sources = override_sources(&content_dir().join("src/overrides"))?;
    let s = Arc::new(lower_ir(w.ir.as_ref(), &support, &sources)?);
    cache.lock().unwrap().insert(w.dir.clone(), s.clone());
    Ok(s)
}

/// What [`generate`] produces besides the module files it writes.
pub struct ScriptsOut {
    /// `scripts.rs`: the script modules and the trainer table.
    pub rs: String,
    /// `dispatch.rs`, part of `content::data`.
    pub dispatch: String,
    /// `overrides.rs`, part of `content::overrides`.
    pub overrides: String,
    /// `report.txt` (`content::REPORT`).
    pub report: String,
}

/// All script names, at their `ScriptId` (position).
pub struct Scripts {
    pub names: Vec<String>,
}

/// `ScriptId` numbering: every lowered function, sorted by name (`SCRIPT_NAMES`).
pub fn scripts(w: &World) -> Scripts {
    let s = scripted(w).unwrap_or_else(|e| panic!("lowering the scripts: {e:#}"));
    Scripts {
        names: s.module.keys().cloned().collect(),
    }
}

/// The scripts of `w`: writes `out/scripts/<Module>.rs` and returns the other files.
/// `support` is checked against the tables the lowering used (they are the same files).
pub fn generate(
    dir: Option<&Path>,
    out: &Path,
    w: &World,
    support: &Support,
) -> Result<ScriptsOut> {
    let _ = dir;
    ensure!(
        !support.commands.is_empty() && !support.specials.is_empty(),
        "empty support tables"
    );
    let s = scripted(w)?;
    let mods = out.join("scripts");
    std::fs::create_dir_all(&mods)?;
    for (m, body) in &s.lowered.modules {
        ensure!(ident_ok(m), "module name {m:?}");
        write_if_changed(&mods.join(format!("{m}.rs")), body)?;
    }
    let report = report(&s);
    warn(&format!("{}", s.lowered.stats.summary()));
    for o in &s.skipped_overrides {
        warn(&format!("override not applied: {o}"));
    }
    Ok(ScriptsOut {
        rs: scripts_rs(&s)?,
        dispatch: dispatch(&s),
        overrides: overrides_rs(&s)?,
        report,
    })
}

fn write_if_changed(path: &Path, s: &str) -> Result<()> {
    if std::fs::read(path).ok().as_deref() != Some(s.as_bytes()) {
        std::fs::write(path, s).with_context(|| path.display().to_string())?;
    }
    Ok(())
}

/// `scripts.rs`: one module per IR module, then the trainer table.
fn scripts_rs(s: &Scripted) -> Result<String> {
    let mut o = String::from("// @generated by content/build.rs (build/scripts.rs)\n");
    for m in s.lowered.modules.keys() {
        writeln!(
            o,
            "#[allow(non_snake_case, non_upper_case_globals, unused_mut, unused_variables, unused_assignments, unreachable_code, clippy::all)]\npub mod {m} {{\n    use crate::prelude::*;\n    include!(concat!(env!(\"OUT_DIR\"), \"/scripts/{m}.rs\"));\n}}"
        )?;
    }
    let id: BTreeMap<&str, usize> = s
        .module
        .keys()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();
    let mut rows: Vec<(usize, u16, u8, &str)> = Vec::new();
    for (script, trainer, kind) in &s.trainers {
        let Some(&i) = id.get(script.as_str()) else {
            bail!("trainer script {script} has no id");
        };
        rows.push((i, *trainer, *kind, script));
    }
    rows.sort();
    write!(
        o,
        "/// (`ScriptId`, trainer, `TRAINER_BATTLE_*` kind) of every script that starts with\n/// `trainerbattle` (Gen3 `GetTrainerFlagFromScriptPointer`), sorted by `ScriptId`.\npub static TRAINER_SCRIPTS: [(u16, u16, u8); {}] = [\n",
        rows.len()
    )?;
    for (i, t, k, name) in &rows {
        writeln!(o, "    ({i}, {t}, {k}), // {name}")?;
    }
    o.push_str("];\n");
    o.push_str(
        "/// The trainer and `trainerbattle` kind of script `id`, if it is a trainer script.\npub fn trainer_script(id: u16) -> Option<(u16, u8)> {\n    let i = TRAINER_SCRIPTS.binary_search_by_key(&id, |r| r.0).ok()?;\n    let (_, t, k) = TRAINER_SCRIPTS[i];\n    Some((t, k))\n}\n",
    );
    Ok(o)
}

/// `dispatch.rs`: the lowering's dispatch, then the digests.
fn dispatch(s: &Scripted) -> String {
    let mut o = s.lowered.dispatch.clone();
    o.push_str("/// Per script (decomp name): its lowered source, texts, movements, call-graph closure and\n/// override source (`extract::codegen::lower`).\npub static SCRIPT_DIGESTS: &[(&str, u64)] = &[\n");
    for (n, d) in &s.lowered.script_digests {
        let _ = writeln!(o, "    (\"{n}\", {d:#018x}),");
    }
    o.push_str("];\n");
    o
}

/// `overrides.rs`: the applied overrides' modules and `OVERRIDES`.
fn overrides_rs(s: &Scripted) -> Result<String> {
    let mut o = String::from("// @generated by content/build.rs (build/scripts.rs)\n");
    for (n, p) in &s.overrides {
        let path = p.canonicalize().with_context(|| p.display().to_string())?;
        let path = path.to_str().context("override path is not UTF-8")?;
        ensure!(!path.contains('"'), "override path {path:?}");
        writeln!(
            o,
            "#[allow(non_snake_case)]\n#[path = \"{path}\"]\nmod {n};\npub use {n}::{n};"
        )?;
    }
    write!(
        o,
        "/// The overridden scripts (decomp names).\npub static OVERRIDES: &[&str] = &["
    )?;
    for n in s.overrides.keys() {
        write!(o, "\"{n}\",")?;
    }
    o.push_str("];\n");
    Ok(o)
}

/// `report.txt`: sites by class and by reported op or special, the decomp-name share, the
/// movement actions with no step, `end` in callees and the overrides.
fn report(s: &Scripted) -> String {
    let st = &s.lowered.stats;
    let mut o = String::from("== scripts (build/scripts.rs) ==\n");
    let _ = writeln!(
        o,
        "functions {}, labels {}, texts {}, movements {}, tail-call groups {}",
        st.fns, st.labels, st.texts, st.movements, st.tail_call_groups
    );
    let _ = writeln!(o, "sites by class:");
    for (k, n) in &st.by_class {
        let _ = writeln!(o, "  {k} {n}");
    }
    let mut reports: Vec<(&String, &usize)> = st.reports.iter().collect();
    reports.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    let _ = writeln!(
        o,
        "unsupported sites by op or special ({}):",
        st.reports.values().sum::<usize>()
    );
    for (k, n) in reports {
        let _ = writeln!(o, "  {k} {n}");
    }
    let _ = writeln!(o, "decomp names:");
    for (k, p) in &st.named_decomp_pct {
        let _ = writeln!(o, "  {k} {p}%");
    }
    let _ = writeln!(o, "movement actions with no step:");
    for (k, n) in &st.unknown_movement_actions {
        let _ = writeln!(o, "  {k} {n}");
    }
    let _ = writeln!(
        o,
        "end in callee {}, unresolved waitmovement {}",
        st.end_in_callee, st.unresolved_waitmovement
    );
    let _ = writeln!(
        o,
        "overrides: {}",
        if s.overrides.is_empty() {
            "none".to_string()
        } else {
            s.overrides.keys().cloned().collect::<Vec<_>>().join(", ")
        }
    );
    for k in &s.skipped_overrides {
        let _ = writeln!(o, "  not applied: {k}");
    }
    let _ = writeln!(o, "trainer scripts {}", s.trainers.len());
    o
}
