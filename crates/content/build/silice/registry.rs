//! The Silice script registry (SIL1 Wave 0 contract).
//!
//! - **Scripts.** Every top-level `pub async fn <Name>(ctx: Ctx)` (one argument, no return
//!   type, no generics) of `crates/content/src/silice/scripts/<module>.rs` is a script, found by
//!   parsing the file with `syn`. Helpers must be private or not `async`; a `pub async fn` of
//!   another shape fails the build. A script file starts with `use crate::silice::prelude::*;`.
//! - **Modules.** The file stem is the module (`content::scripts::<module>`, a `#[path]` module
//!   of the generated `scripts.rs`): an identifier, never `mod`.
//! - **Ids.** Script names are unique across files; `ScriptId`s are assigned in name order
//!   (`SCRIPT_NAMES`), so adding a script renumbers the later ones. That is fine: snapshots
//!   hold no `ScriptId` (M5), so a rename or an addition moves only `BUILD_ID`.
//! - **Digests.** `SCRIPT_DIGESTS[name]` is FNV-1a 64 over the name and its whole file (CRLF
//!   folded): any edit of a file revalidates its scripts. Calls into another file's script or
//!   helper are not followed yet (Task F, if scripts start sharing helpers across files).
//! - **References.** A map naming a script no file defines fails the build, except in the
//!   Wave 0 allowlist mode ([`ALLOW_UNWRITTEN`]): the Ch0 maps name about 68 scripts that do not
//!   exist yet, so they are dropped from the maps, listed in `content::REPORT` and summed up in
//!   one cargo warning. `PKR_SILICE_STRICT_SCRIPTS=1` makes the build strict now.

use crate::build_support::TreeHash;
use crate::codegen::ident_ok;
use crate::input::ScriptIn;
use anyhow::{Context, Result, bail, ensure};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Wave 0: a map may name a script that is not written yet (dropped, reported). **SIL1 Wave 1
/// Task F sets it to `false`** once the Ch0 scripts exist; from then on an unknown script fails
/// the build.
pub const ALLOW_UNWRITTEN: bool = true;

/// Set to `1` to fail on unwritten scripts while [`ALLOW_UNWRITTEN`] is still `true`.
pub const STRICT_ENV: &str = "PKR_SILICE_STRICT_SCRIPTS";

/// Whether an unknown script fails the build.
pub fn strict() -> bool {
    !ALLOW_UNWRITTEN || std::env::var(STRICT_ENV).is_ok_and(|v| v == "1")
}

/// Every script and script file.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    /// Sorted by name: the index is the `ScriptId`.
    pub scripts: Vec<ScriptIn>,
    /// (module, absolute path), sorted by module.
    pub modules: Vec<(String, PathBuf)>,
}

/// Scans `dir` (`src/silice/scripts`); a missing dir has no script.
pub fn scan(dir: &Path) -> Result<Registry> {
    let mut reg = Registry::default();
    if !dir.is_dir() {
        return Ok(reg);
    }
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| dir.display().to_string())?
        .map(|e| e.map(|e| e.path()))
        .collect::<std::io::Result<_>>()?;
    paths.retain(|p| p.extension().is_some_and(|x| x == "rs"));
    paths.sort();
    let mut by_name: BTreeMap<String, ScriptIn> = BTreeMap::new();
    for p in paths {
        let module = p
            .file_stem()
            .and_then(|s| s.to_str())
            .with_context(|| format!("{}: file name", p.display()))?
            .to_string();
        ensure!(
            ident_ok(&module) && module != "mod",
            "{}: a script file's stem must be a module name",
            p.display()
        );
        let src = std::fs::read_to_string(&p).with_context(|| p.display().to_string())?;
        let src = src.replace("\r\n", "\n");
        for name in script_fns(&src).with_context(|| p.display().to_string())? {
            let mut h = TreeHash::new();
            h.file(&name, src.as_bytes());
            let s = ScriptIn {
                name: name.clone(),
                module: module.clone(),
                digest: h.finish(),
            };
            if let Some(old) = by_name.insert(name.clone(), s) {
                bail!(
                    "script {name} is defined in both scripts/{}.rs and scripts/{module}.rs",
                    old.module
                );
            }
        }
        let abs = p.canonicalize().with_context(|| p.display().to_string())?;
        reg.modules.push((module, abs));
    }
    reg.scripts = by_name.into_values().collect();
    Ok(reg)
}

/// The script names of one file: its top-level `pub async fn`s, in file order.
pub fn script_fns(src: &str) -> Result<Vec<String>> {
    let file = syn::parse_file(src).context("parsing")?;
    let mut out = Vec::new();
    for item in &file.items {
        let syn::Item::Fn(f) = item else { continue };
        if !matches!(f.vis, syn::Visibility::Public(_)) || f.sig.asyncness.is_none() {
            continue;
        }
        let name = f.sig.ident.to_string();
        ensure!(
            f.sig.inputs.len() == 1
                && matches!(f.sig.output, syn::ReturnType::Default)
                && f.sig.generics.params.is_empty(),
            "pub async fn {name}: a script is `pub async fn {name}(ctx: Ctx)` (one argument, no \
             return type, no generics); make helpers private or not async"
        );
        ensure!(ident_ok(&name), "script name {name:?}");
        out.push(name);
    }
    Ok(out)
}

/// Resolves the script names the maps use against a [`Registry`].
pub struct Resolver {
    known: BTreeSet<String>,
    strict: bool,
    /// The unknown names dropped (allowlist mode).
    pub unwritten: BTreeSet<String>,
}

impl Resolver {
    pub fn new(reg: &Registry, strict: bool) -> Resolver {
        Resolver {
            known: reg.scripts.iter().map(|s| s.name.clone()).collect(),
            strict,
            unwritten: BTreeSet::new(),
        }
    }

    /// `Some(name)` for a registered script; for an unknown one, an error when strict, else
    /// `None` (the reference is dropped and recorded in [`Self::unwritten`]).
    pub fn script(&mut self, map: &str, name: &str) -> Result<Option<String>> {
        if self.known.contains(name) {
            return Ok(Some(name.to_string()));
        }
        ensure!(
            !self.strict,
            "map {map}: script {name} is not defined by any src/silice/scripts/*.rs"
        );
        self.unwritten.insert(name.to_string());
        Ok(None)
    }

    pub fn opt(&mut self, map: &str, name: &Option<String>) -> Result<Option<String>> {
        match name {
            Some(n) => self.script(map, n),
            None => Ok(None),
        }
    }
}
