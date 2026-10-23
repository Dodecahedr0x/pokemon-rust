//! Batch mode (`studio-map … build <edits dir>`): rebuilds maps from their edit files in one
//! process, as the step-by-step CLI would, byte for byte.
//!
//! An edits dir (by convention `games/<game>/edits/`) holds one dir per map, named after it,
//! with files `<phase>_<what>.ron` (`<phase>` is digits):
//!
//! ```text
//! edits/
//!   TownA/00_create.ron      a NewMap spec (`create`): at most one per map
//!   TownA/10_terrain.ron     edit scripts (a RON list of Op, `apply`)
//!   TownA/20_events.ron
//!   RouteB/00_create.ron
//!   RouteB/20_events.ron
//! ```
//!
//! The build runs, in this order:
//! 1. every create, in map order (a map with a create is rebuilt from scratch: its saved map
//!    file, and its layout unless the spec shares one, are ignored);
//! 2. every script, by phase number, then map name, then file name (a map without a create is
//!    a saved map, edited in place);
//! 3. one `validate` of the whole games dir.
//!
//! It is the same as `create` for each create file, in map order, then `apply` for each
//! script in that order, and writes the same bytes: each step reads the map and its layout as
//! the previous steps left them (in memory) and writes both back. Links between maps are
//! checked only by the final validation, so mutually linked maps need no passes. A failing op
//! (or a create that does not validate) stops the build before anything is written; the final
//! validation's errors fail it after writing, as `apply` saves drafts.

use super::catalog::Catalog;
use super::games::{GamesDir, case_clashes, check_name, write};
use super::model::{Doc, Editor, Env, NewMap, Op};
use super::schema::{GameMap, from_script_ron, to_ron};
use super::validate::{errors, validate};
use anyhow::{Context, Result, bail, ensure};
use extract::schema::Layout;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The file stem of a create spec, after its phase.
pub const CREATE: &str = "create";

/// One edit file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub phase: u32,
    pub map: String,
    pub path: PathBuf,
}

/// What an edits dir holds, in build order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// (map, spec file), in map order.
    pub creates: Vec<(String, PathBuf)>,
    /// Scripts by phase, map and file name.
    pub steps: Vec<Step>,
}

/// `<phase>_<what>.ron` as (phase, what).
fn split_name(file: &str) -> Option<(u32, &str)> {
    let stem = file.strip_suffix(".ron")?;
    let (phase, what) = stem.split_once('_')?;
    if phase.is_empty() || !phase.bytes().all(|b| b.is_ascii_digit()) || what.is_empty() {
        return None;
    }
    Some((phase.parse().ok()?, what))
}

fn sorted_entries(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        v.push(e?.path());
    }
    v.sort();
    Ok(v)
}

/// Reads an edits dir's layout (not the files' contents).
pub fn plan(edits: &Path) -> Result<Plan> {
    let mut p = Plan::default();
    let mut maps = Vec::new();
    for dir in sorted_entries(edits)? {
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        ensure!(
            dir.is_dir(),
            "{}: an edits dir holds only <Map>/ dirs",
            dir.display()
        );
        check_name(&name).with_context(|| dir.display().to_string())?;
        maps.push(name.clone());
        for f in sorted_entries(&dir)? {
            let file = f.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            let Some((phase, what)) = split_name(file).filter(|_| f.is_file()) else {
                bail!(
                    "{}: not an edit file (<phase>_<what>.ron, the phase in digits)",
                    f.display()
                );
            };
            if what == CREATE {
                ensure!(
                    !p.creates.iter().any(|(m, _)| *m == name),
                    "{}: a second create for {name}",
                    f.display()
                );
                p.creates.push((name.clone(), f.clone()));
            } else {
                p.steps.push(Step {
                    phase,
                    map: name.clone(),
                    path: f.clone(),
                });
            }
        }
    }
    if let Some((a, b)) = case_clashes(&maps).first() {
        bail!(
            "edits for {a} and {b}: map names that differ only in case are one file on a \
             case-insensitive file system"
        );
    }
    p.steps
        .sort_by(|a, b| (a.phase, &a.map, &a.path).cmp(&(b.phase, &b.map, &b.path)));
    Ok(p)
}

/// A games dir's maps and layouts as the steps leave them: in memory over the files.
struct Store<'a> {
    games: &'a GamesDir,
    maps: BTreeMap<String, Option<GameMap>>,
    layouts: BTreeMap<String, Option<Layout>>,
}

impl Store<'_> {
    fn names(&self, saved: Vec<String>, mem: &BTreeMap<String, Option<impl Sized>>) -> Vec<String> {
        let mut v: Vec<String> = saved
            .into_iter()
            .filter(|n| !mem.contains_key(n))
            .chain(mem.iter().filter(|e| e.1.is_some()).map(|e| e.0.clone()))
            .collect();
        v.sort();
        v
    }

    fn map_names(&self) -> Result<Vec<String>> {
        Ok(self.names(self.games.map_names()?, &self.maps))
    }

    fn layout_names(&self) -> Result<Vec<String>> {
        Ok(self.names(self.games.layout_names()?, &self.layouts))
    }

    fn check_case(&self, what: &str, name: &str) -> Result<()> {
        let names = match what {
            "map" => self.map_names()?,
            _ => self.layout_names()?,
        };
        if let Some(other) = super::games::case_clash(&names, name) {
            bail!(
                "{what} {name} differs from {what} {other} only in case (one file on a \
                 case-insensitive file system): use {other}, or another name"
            );
        }
        Ok(())
    }

    fn map(&self, name: &str) -> Result<Option<GameMap>> {
        match self.maps.get(name) {
            Some(m) => Ok(m.clone()),
            None if self.games.exists(name) => self.games.load_map(name).map(Some),
            None => Ok(None),
        }
    }

    fn layout(&self, name: &str) -> Result<Option<Layout>> {
        match self.layouts.get(name) {
            Some(l) => Ok(l.clone()),
            None if self.games.layout_path(name).exists() => self.games.load_layout(name).map(Some),
            None => Ok(None),
        }
    }

    /// `GamesDir::load`.
    fn load(&self, name: &str) -> Result<Doc> {
        self.check_case("map", name)?;
        let map = self
            .map(name)?
            .with_context(|| format!("no map {name} (create it first)"))?;
        let layout = self
            .layout(&map.layout)?
            .with_context(|| format!("{name}: no layout {}", map.layout))?;
        Doc::from_parts(map, layout)
    }

    /// `GamesDir::new_doc`.
    fn new_doc(&self, spec: &NewMap) -> Result<Doc> {
        let layout = spec.layout_name();
        check_name(&spec.name)?;
        check_name(&layout)?;
        self.check_case("map", &spec.name)?;
        self.check_case("layout", &layout)?;
        ensure!(self.map(&spec.name)?.is_none(), "map {} exists", spec.name);
        if spec.shared_layout {
            let saved = self
                .layout(&layout)?
                .with_context(|| format!("shared layout {layout}: none"))?;
            return Doc::from_parts(spec.blank_map(), saved);
        }
        ensure!(
            self.layout(&layout)?.is_none(),
            "layout {layout} exists (set shared_layout: true to share it, or name another \
             layout)"
        );
        Doc::new(spec)
    }

    /// `GamesDir::create`.
    fn create(&mut self, doc: &Doc) -> Result<()> {
        if let Some(l) = self.layout(&doc.map.layout)? {
            ensure!(
                l == doc.layout(),
                "layout {} exists with other contents (share it unchanged, or name another \
                 layout)",
                doc.map.layout
            );
        }
        self.save(doc)
    }

    /// `GamesDir::save`.
    fn save(&mut self, doc: &Doc) -> Result<()> {
        check_name(&doc.map.name)?;
        check_name(&doc.map.layout)?;
        self.check_case("map", &doc.map.name)?;
        self.check_case("layout", &doc.map.layout)?;
        self.layouts
            .insert(doc.map.layout.clone(), Some(doc.layout()));
        self.maps
            .insert(doc.map.name.clone(), Some(doc.map.clone()));
        Ok(())
    }

    /// Writes what the steps changed (only files whose bytes change).
    fn write(&self) -> Result<()> {
        for (name, l) in &self.layouts {
            if let Some(l) = l {
                write(&self.games.layout_path(name), &to_ron(l)?)?;
            }
        }
        for (name, m) in &self.maps {
            if let Some(m) = m {
                write(&self.games.map_path(name), &to_ron(m)?)?;
            }
        }
        Ok(())
    }
}

/// What a build did.
#[derive(Clone, Debug, Default)]
pub struct Built {
    pub created: usize,
    pub scripts: usize,
    pub ops: usize,
    /// The final validation's report, one line per issue.
    pub report: String,
    pub errors: usize,
    pub maps: usize,
}

/// Runs `edits` against `games` (module docs). `catalog` validates the creates; `env` gives
/// the ops their stamps and usual collisions.
pub fn build(
    games: &GamesDir,
    edits: &Path,
    catalog: &Catalog,
    env: Arc<dyn Env>,
) -> Result<Built> {
    let plan = plan(edits)?;
    let mut store = Store {
        games,
        maps: BTreeMap::new(),
        layouts: BTreeMap::new(),
    };
    let mut specs = Vec::new();
    for (map, path) in &plan.creates {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let spec: NewMap =
            from_script_ron(&text).with_context(|| format!("parsing {}", path.display()))?;
        ensure!(
            spec.name == *map,
            "{}: creates {:?}, not {map}",
            path.display(),
            spec.name
        );
        // Rebuilt from scratch: the saved files are not the starting point.
        store.maps.insert(map.clone(), None);
        if !spec.shared_layout {
            store.layouts.insert(spec.layout_name(), None);
        }
        specs.push((path, spec));
    }
    let mut built = Built::default();
    for (path, spec) in &specs {
        let doc = store
            .new_doc(spec)
            .with_context(|| format!("{}", path.display()))?;
        let issues = validate(&doc, catalog);
        if errors(&issues) > 0 {
            let lines: Vec<String> = issues
                .iter()
                .map(|i| format!("{}: {i}", spec.name))
                .collect();
            bail!("{}: not created:\n{}", path.display(), lines.join("\n"));
        }
        store
            .create(&doc)
            .with_context(|| format!("{}", path.display()))?;
        built.created += 1;
    }
    for step in &plan.steps {
        let path = &step.path;
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let ops: Vec<Op> =
            from_script_ron(&text).with_context(|| format!("parsing {}", path.display()))?;
        let doc = store
            .load(&step.map)
            .with_context(|| format!("{}", path.display()))?;
        let mut ed = Editor::new(doc).with_env(env.clone());
        ed.apply_all(&ops)
            .with_context(|| format!("{}", path.display()))?;
        store.save(ed.doc())?;
        built.scripts += 1;
        built.ops += ops.len();
    }
    store.write()?;
    let catalog = Catalog::for_games(games)?;
    let names = games.map_names()?;
    built.maps = names.len();
    let (report, n) = validate_all(games, &catalog, &names);
    built.report = report;
    built.errors = n;
    Ok(built)
}

/// The `validate` report of `names` (and the case clashes), and its error count.
pub fn validate_all(games: &GamesDir, catalog: &Catalog, names: &[String]) -> (String, usize) {
    let mut out = String::new();
    let mut total = 0;
    for (a, b) in case_clashes(names) {
        total += 1;
        out.push_str(&format!(
            "{a}: error: map: {b} differs only in case (one file on a case-insensitive file \
             system)\n"
        ));
    }
    for name in names {
        match games.load(name) {
            Ok(doc) => {
                let issues = validate(&doc, catalog);
                total += errors(&issues);
                for i in issues {
                    out.push_str(&format!("{name}: {i}\n"));
                }
            }
            Err(e) => {
                total += 1;
                out.push_str(&format!("{name}: error: map: {e:#}\n"));
            }
        }
    }
    (out, total)
}

/// Edit files that do not parse (`hygiene`): every `<Map>/<phase>_<what>.ron` must be a
/// create spec or an edit script.
pub fn check_edits(edits: &Path) -> Vec<String> {
    let plan = match plan(edits) {
        Ok(p) => p,
        Err(e) => return vec![format!("{e:#}")],
    };
    let mut bad = Vec::new();
    let read = |p: &Path| std::fs::read_to_string(p).with_context(|| p.display().to_string());
    for (_, p) in &plan.creates {
        if let Err(e) = read(p).and_then(|t| from_script_ron::<NewMap>(&t)) {
            bad.push(format!("{}: {e:#}", p.display()));
        }
    }
    for s in &plan.steps {
        if let Err(e) = read(&s.path).and_then(|t| from_script_ron::<Vec<Op>>(&t)) {
            bad.push(format!("{}: {e:#}", s.path.display()));
        }
    }
    bad
}
