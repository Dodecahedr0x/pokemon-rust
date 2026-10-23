//! The Silice script registry (SIL1 Wave 0 contract; per-map strictness and trainer scripts:
//! SIL1 Task F).
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
//! - **Digests.** `SCRIPT_DIGESTS` pairs each script name, in `SCRIPT_NAMES` order, with
//!   `TreeHash::file(name, source)`: FNV-1a 64 over the length-prefixed script name and the whole
//!   source of the file defining it (CRLF folded to LF). Any edit of a file moves the digest of
//!   every script in it; the module (file) name is not hashed. Calls into another file's script
//!   or helper are not followed.
//! - **Claimed maps** ([`Registry::claimed`]). A script file declares the maps whose scripts it
//!   owns: `pub const MAPS: &[&str] = &["BricoBourg", …];` (map file stems, string literals
//!   only). A map is claimed by at most one file, and a claimed map must be in `maps/`.
//! - **References: strict per map.** A **claimed** map is strict: every script it names must
//!   exist, else the build fails. An unclaimed map (one whose chapter has no script module yet,
//!   such as maps drafted ahead of their scenes) is in the **allowlist mode**: an unknown name
//!   is dropped from the map, listed in `content::REPORT` and summed up in one cargo warning.
//!   `PKR_SILICE_STRICT_SCRIPTS=1` ([`STRICT_ENV`]) makes every map strict. Every map is
//!   resolved first; a failing build then fails once, listing every unknown name a strict map
//!   uses, with the maps that use it ([`Resolver::finish`]).
//! - **Trainer scripts** ([`Registry::trainers`]). A script whose **first statement** calls the
//!   prelude's `trainer(&ctx, LOCALID_…, TRAINER_…, intro, defeat)` (under `if`, `!`, `.await`,
//!   `let` or parentheses) is a trainer script. The build lists it in `TRAINER_SCRIPTS` with the
//!   kind `TRAINER_BATTLE_SINGLE` (`dispatch.rs`), so the engine's sight
//!   (`GameData::trainer_flag_of`, `trainer_battle_kind`) knows a trainer object's trainer.

use crate::build_support::TreeHash;
use crate::codegen::ident_ok;

use anyhow::{Context, Result, bail, ensure};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// One registered script: `pub async fn <name>(ctx: Ctx)` in `scripts/<module>.rs`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptIn {
    pub name: String,
    pub module: String,
    /// Its `SCRIPT_DIGESTS` value.
    pub digest: u64,
}

/// Set to `1` to make every map strict, claimed or not.
pub const STRICT_ENV: &str = "PKR_SILICE_STRICT_SCRIPTS";

/// Whether [`STRICT_ENV`] makes every map strict (else only the claimed maps are).
pub fn strict() -> bool {
    std::env::var(STRICT_ENV).is_ok_and(|v| v == "1")
}

/// The const a script file claims its maps with: `pub const MAPS: &[&str] = &[…];`.
pub const MAPS_CONST: &str = "MAPS";

/// The prelude helper whose call, as a script's first statement, makes it a trainer script.
pub const TRAINER_FN: &str = "trainer";

/// Every script and script file.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    /// Sorted by name: the index is the `ScriptId`.
    pub scripts: Vec<ScriptIn>,
    /// (module, absolute path), sorted by module.
    pub modules: Vec<(String, PathBuf)>,
    /// The maps the script files claim (`pub const MAPS`), each with its module. A claimed map
    /// is strict.
    pub claimed: BTreeMap<String, String>,
    /// (script, trainer constant) of every trainer script, sorted by script name.
    pub trainers: Vec<(String, String)>,
    /// `TRAINER_SCRIPTS`: [`Self::trainers`] as (`ScriptId`, trainer number,
    /// `TRAINER_BATTLE_SINGLE`), sorted by `ScriptId`. Empty after [`scan`]; `silice::world`
    /// fills it once the trainer ids are loaded.
    pub trainer_table: Vec<(u16, u16, u8)>,
}

impl Registry {
    /// The `ScriptId` of script `name`.
    pub fn id(&self, name: &str) -> Option<u16> {
        let i = (self.scripts).binary_search_by(|s| s.name.as_str().cmp(name));
        i.ok().map(|i| i as u16)
    }
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
        for map in claimed_maps(&src).with_context(|| p.display().to_string())? {
            if let Some(old) = reg.claimed.insert(map.clone(), module.clone()) {
                ensure!(
                    old != module,
                    "{}: map {map} is listed twice in `{MAPS_CONST}`",
                    p.display()
                );
                bail!("map {map} is claimed by both scripts/{old}.rs and scripts/{module}.rs");
            }
        }
        reg.trainers
            .extend(trainer_fns(&src).with_context(|| p.display().to_string())?);
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
    reg.trainers.sort();
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

/// The maps a file claims: its top-level `pub const MAPS: &[&str] = &["A", "B"];` (none
/// without one). Anything but string literals in an array fails.
pub fn claimed_maps(src: &str) -> Result<Vec<String>> {
    let file = syn::parse_file(src).context("parsing")?;
    let mut out = Vec::new();
    for item in &file.items {
        if let syn::Item::Static(st) = item {
            ensure!(
                st.ident != MAPS_CONST,
                "`static {MAPS_CONST}`: the maps a file claims are a `pub const {MAPS_CONST}: \
                 &[&str] = &[\"Map\", …];`"
            );
        }
        let syn::Item::Const(c) = item else { continue };
        if c.ident != MAPS_CONST {
            continue;
        }
        ensure!(
            matches!(c.vis, syn::Visibility::Public(_)),
            "const {MAPS_CONST} must be `pub` (the maps whose scripts this file owns)"
        );
        let mut e = &*c.expr;
        if let syn::Expr::Reference(r) = e {
            e = &r.expr;
        }
        let syn::Expr::Array(a) = e else {
            bail!(
                "const {MAPS_CONST}: expected an array of string literals, `&[\"Map\", …]` (the \
                 build reads it without compiling the file: no other const, macro or path)"
            );
        };
        for el in &a.elems {
            match el {
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s),
                    ..
                }) => out.push(s.value()),
                _ => bail!(
                    "const {MAPS_CONST}: element {} is not a string literal (the build reads the \
                     map names without compiling the file)",
                    out.len() + 1
                ),
            }
        }
    }
    Ok(out)
}

/// The trainer scripts of a file: (script, trainer constant) for each top-level `pub async fn`
/// whose first statement calls [`TRAINER_FN`] with a `TRAINER_*` constant as its third argument.
pub fn trainer_fns(src: &str) -> Result<Vec<(String, String)>> {
    let file = syn::parse_file(src).context("parsing")?;
    let mut out = Vec::new();
    for item in &file.items {
        let syn::Item::Fn(f) = item else { continue };
        if !matches!(f.vis, syn::Visibility::Public(_)) || f.sig.asyncness.is_none() {
            continue;
        }
        let first = match f.block.stmts.first() {
            Some(syn::Stmt::Expr(e, _)) => Some(e),
            Some(syn::Stmt::Local(l)) => l.init.as_ref().map(|i| &*i.expr),
            _ => None,
        };
        if let Some(t) = first.and_then(trainer_call) {
            out.push((f.sig.ident.to_string(), t));
        }
    }
    Ok(out)
}

/// The trainer constant of a `trainer(_, _, TRAINER_X, ..)` call at the head of `e`.
fn trainer_call(e: &syn::Expr) -> Option<String> {
    match e {
        syn::Expr::If(i) => trainer_call(&i.cond),
        syn::Expr::Unary(u) => trainer_call(&u.expr),
        syn::Expr::Await(a) => trainer_call(&a.base),
        syn::Expr::Paren(p) => trainer_call(&p.expr),
        syn::Expr::Let(l) => trainer_call(&l.expr),
        syn::Expr::Call(c) => {
            let syn::Expr::Path(f) = &*c.func else {
                return None;
            };
            if !f.path.is_ident(TRAINER_FN) {
                return None;
            }
            let syn::Expr::Path(t) = c.args.iter().nth(2)? else {
                return None;
            };
            let t = t.path.get_ident()?.to_string();
            t.starts_with("TRAINER_").then_some(t)
        }
        _ => None,
    }
}

/// Resolves the script names the maps use against a [`Registry`]. Resolution never fails: an
/// unknown name is recorded with its map and dropped; [`Resolver::finish`] fails once, after
/// every map, when a strict map names an unknown script.
pub struct Resolver {
    known: BTreeSet<String>,
    /// [`strict`]: every map is strict.
    force: bool,
    /// The claimed (strict) maps, with their modules.
    claimed: BTreeMap<String, String>,
    /// Every map resolved so far.
    seen: BTreeSet<String>,
    /// The unknown script names, each with the maps that use it.
    pub unwritten: BTreeMap<String, BTreeSet<String>>,
}

impl Resolver {
    /// `force`: every map is strict ([`strict`]); else only `reg`'s claimed maps are.
    pub fn new(reg: &Registry, force: bool) -> Resolver {
        Resolver {
            known: reg.scripts.iter().map(|s| s.name.clone()).collect(),
            force,
            claimed: reg.claimed.clone(),
            seen: BTreeSet::new(),
            unwritten: BTreeMap::new(),
        }
    }

    /// Whether an unknown script of `map` fails the build.
    pub fn is_strict(&self, map: &str) -> bool {
        self.force || self.claimed.contains_key(map)
    }

    /// Records that `map` is in the build (`merge::to_map_file` calls it for every map).
    pub fn map(&mut self, map: &str) {
        self.seen.insert(map.to_string());
    }

    /// `Some(name)` for a registered script; `None` for an unknown one (the reference is
    /// dropped and recorded in [`Self::unwritten`] under `map`).
    pub fn script(&mut self, map: &str, name: &str) -> Option<String> {
        if self.known.contains(name) {
            return Some(name.to_string());
        }
        (self.unwritten.entry(name.to_string()).or_default()).insert(map.to_string());
        None
    }

    pub fn opt(&mut self, map: &str, name: &Option<String>) -> Option<String> {
        self.script(map, name.as_deref()?)
    }

    /// After every map: fails when a claimed map is not in the build (a typo in `MAPS`).
    /// `silice::world` runs it after the asset checks, so that a fixture games dir built with
    /// the committed scripts reports its own error first.
    pub fn check_claims(&self) -> Result<()> {
        let missing: Vec<String> = (self.claimed.iter())
            .filter(|(m, _)| !self.seen.contains(*m))
            .map(|(m, module)| format!("{m} (scripts/{module}.rs)"))
            .collect();
        ensure!(
            missing.is_empty(),
            "{} maps a script file claims (`pub const {MAPS_CONST}`) are not in maps/: {}",
            missing.len(),
            missing.join("; ")
        );
        Ok(())
    }

    /// After every map: fails when a strict map names an unknown script (listing each such
    /// name once, with all its maps).
    pub fn finish(&self) -> Result<()> {
        let list: Vec<String> = (self.unwritten.iter())
            .filter(|(_, maps)| maps.iter().any(|m| self.is_strict(m)))
            .map(|(name, maps)| {
                let maps: Vec<&str> = maps.iter().map(String::as_str).collect();
                format!("{name} (map {})", maps.join(", "))
            })
            .collect();
        if list.is_empty() {
            return Ok(());
        }
        bail!(
            "{} scripts the strict maps name are not defined by any src/silice/scripts/*.rs: {}",
            list.len(),
            list.join("; ")
        )
    }
}
