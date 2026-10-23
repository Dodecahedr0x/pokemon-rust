//! A games dir (`games/<game>/`): where edited maps are saved, as committed RON.

use super::model::{Doc, NewMap};
use super::schema::{GameMap, from_ron, to_ron};
use anyhow::{Context, Result, bail, ensure};
use engine::data::BLOCK_METATILE_MASK;
use extract::schema::{Layout, LayoutFile, MapFile, from_rows};
use std::collections::{BTreeSet, HashMap};
use std::hash::{DefaultHasher, Hasher};
use std::path::{Path, PathBuf};

/// The game the editor works on by default.
pub const DEFAULT_GAME: &str = "silice";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GamesDir {
    pub root: PathBuf,
}

/// A map, layout or game name: an identifier, so it is a safe file name.
pub fn check_name(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !name.starts_with(|c: char| c.is_ascii_digit()),
        "{name:?} is not a name (letters, digits and _, not starting with a digit)"
    );
    Ok(())
}

impl GamesDir {
    pub fn new(root: impl Into<PathBuf>) -> GamesDir {
        GamesDir { root: root.into() }
    }

    /// `<workspace>/games/<game>`.
    pub fn for_game(workspace: &Path, game: &str) -> GamesDir {
        GamesDir::new(workspace.join("games").join(game))
    }

    pub fn maps_dir(&self) -> PathBuf {
        self.root.join("maps")
    }

    pub fn layouts_dir(&self) -> PathBuf {
        self.root.join("layouts")
    }

    pub fn map_path(&self, name: &str) -> PathBuf {
        self.maps_dir().join(format!("{name}.ron"))
    }

    pub fn layout_path(&self, name: &str) -> PathBuf {
        self.layouts_dir().join(format!("{name}.ron"))
    }

    /// The saved maps' names, sorted (none when the dir does not exist).
    pub fn map_names(&self) -> Result<Vec<String>> {
        ron_stems(&self.maps_dir())
    }

    /// The saved layouts' names, sorted.
    pub fn layout_names(&self) -> Result<Vec<String>> {
        ron_stems(&self.layouts_dir())
    }

    pub fn exists(&self, name: &str) -> bool {
        self.map_path(name).is_file()
    }

    pub fn load_map(&self, name: &str) -> Result<GameMap> {
        let p = self.map_path(name);
        let map: GameMap = read(&p)?;
        ensure!(
            map.name == name,
            "{}: names the map {:?}",
            p.display(),
            map.name
        );
        Ok(map)
    }

    pub fn load_layout(&self, name: &str) -> Result<Layout> {
        read(&self.layout_path(name))
    }

    pub fn load(&self, name: &str) -> Result<Doc> {
        let map = self.load_map(name)?;
        let layout = self.load_layout(&map.layout)?;
        Doc::from_parts(map, layout)
    }

    /// A new map's document, refused when `maps/<name>.ron` exists. With `shared_layout` it
    /// is the saved layout's (which must exist); without, a fresh one, refused when
    /// `layouts/<layout>.ron` exists (a new map never replaces a saved layout).
    pub fn new_doc(&self, spec: &NewMap) -> Result<Doc> {
        let layout = spec.layout_name();
        check_name(&spec.name)?;
        check_name(&layout)?;
        ensure!(
            !self.exists(&spec.name),
            "{} exists",
            self.map_path(&spec.name).display()
        );
        if spec.shared_layout {
            let saved = self
                .load_layout(&layout)
                .with_context(|| format!("shared layout {layout}"))?;
            return Doc::from_parts(spec.blank_map(), saved);
        }
        ensure!(
            !self.layout_path(&layout).exists(),
            "{} exists (set shared_layout: true to share it, or name another layout)",
            self.layout_path(&layout).display()
        );
        Doc::new(spec)
    }

    /// Writes `maps/<name>.ron` and `layouts/<layout>.ron` (only the files whose bytes
    /// change; each through a temporary file and a rename, so a failed write leaves the old
    /// file whole).
    pub fn save(&self, doc: &Doc) -> Result<()> {
        check_name(&doc.map.name)?;
        check_name(&doc.map.layout)?;
        write(&self.layout_path(&doc.map.layout), &to_ron(&doc.layout())?)?;
        write(&self.map_path(&doc.map.name), &to_ron(&doc.map)?)
    }

    /// Writes a new map. Refuses to replace a saved map, or a saved layout with other
    /// contents (an identical one is shared: only the map file is new).
    pub fn create(&self, doc: &Doc) -> Result<()> {
        check_name(&doc.map.name)?;
        check_name(&doc.map.layout)?;
        if self.exists(&doc.map.name) {
            bail!("{} exists", self.map_path(&doc.map.name).display());
        }
        let lp = self.layout_path(&doc.map.layout);
        if lp.exists() {
            let same = (self.load_layout(&doc.map.layout)).is_ok_and(|l| l == doc.layout());
            ensure!(
                same,
                "{} exists with other contents (share it unchanged, or name another layout)",
                lp.display()
            );
        }
        self.save(doc)
    }

    /// Layouts no map uses.
    pub fn orphan_layouts(&self) -> Result<Vec<String>> {
        let used: BTreeSet<String> = (self.map_names()?.iter())
            .filter_map(|n| self.load_map(n).ok())
            .map(|m| m.layout)
            .collect();
        Ok((self.layout_names()?.into_iter())
            .filter(|l| !used.contains(l))
            .collect())
    }
}

fn ron_stems(dir: &Path) -> Result<Vec<String>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for e in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let p = e?.path();
        if p.extension().is_some_and(|x| x == "ron")
            && let Some(stem) = p.file_stem().and_then(|s| s.to_str())
        {
            names.push(stem.to_string());
        }
    }
    names.sort();
    Ok(names)
}

fn read<T: serde::de::DeserializeOwned>(p: &Path) -> Result<T> {
    let text = std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))?;
    from_ron(&text).with_context(|| format!("parsing {}", p.display()))
}

fn write(p: &Path, text: &str) -> Result<()> {
    if std::fs::read_to_string(p).is_ok_and(|old| old == text) {
        return Ok(());
    }
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let tmp = p.with_extension("ron.tmp");
    std::fs::write(&tmp, text).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, p)
        .with_context(|| format!("renaming {} to {}", tmp.display(), p.display()))
}

/// Problems with what a games dir holds for the editor: anything but `maps/*.ron` and
/// `layouts/*.ron` under those two dirs, and any file that is not its schema's canonical RON
/// (re-serialized byte for byte), so the files hold nothing but names, raw block indices and
/// event data: no tiles, palettes or metatile graphics. When the extract is present
/// ([`super::catalog::extracted_dir`]), also every layout copied from an extracted one
/// ([`copies`]). Other top-level entries are not the editor's and are not looked at.
pub fn hygiene(games: &GamesDir) -> Result<Vec<String>> {
    let mut bad = Vec::new();
    for (dir, is_map) in [(games.maps_dir(), true), (games.layouts_dir(), false)] {
        if !dir.is_dir() {
            continue;
        }
        for e in std::fs::read_dir(&dir)? {
            let p = e?.path();
            if !p.is_file() || p.extension().is_none_or(|x| x != "ron") {
                bad.push(format!("{}: not a .ron file", p.display()));
                continue;
            }
            let bytes = std::fs::read(&p)?;
            let Ok(text) = String::from_utf8(bytes) else {
                bad.push(format!("{}: not UTF-8 text", p.display()));
                continue;
            };
            let again = if is_map {
                from_ron::<GameMap>(&text).and_then(|m| to_ron(&m))
            } else {
                from_ron::<Layout>(&text).and_then(|l| to_ron(&l))
            };
            match again {
                Ok(again) if again == text => {}
                Ok(_) => bad.push(format!("{}: not in the canonical format", p.display())),
                Err(e) => bad.push(format!("{}: {e:#}", p.display())),
            }
        }
    }
    let extracted = super::catalog::extracted_dir();
    if extracted.join("maps").is_dir() {
        bad.extend(copies(games, &extracted)?);
    }
    Ok(bad)
}

/// Side of the squares [`copies`] compares.
pub const COPY_WINDOW: usize = 10;
/// Distinct metatiles a square needs to count (uniform fills and plain patterns match
/// everywhere and say nothing).
pub const COPY_MIN_DISTINCT: usize = 12;

/// A layout's metatile grid (collision and elevation dropped: repainting them does not make a
/// copy original).
struct Grid {
    name: String,
    w: usize,
    h: usize,
    cells: Vec<u16>,
}

impl Grid {
    fn new(name: String, l: &Layout) -> Grid {
        Grid {
            name,
            w: l.width as usize,
            h: l.height as usize,
            cells: (from_rows(&l.blocks).iter())
                .map(|b| b & BLOCK_METATILE_MASK)
                .collect(),
        }
    }

    fn square(&self, x: usize, y: usize) -> impl Iterator<Item = u16> + '_ {
        (y..y + COPY_WINDOW)
            .flat_map(move |cy| self.cells[cy * self.w + x..cy * self.w + x + COPY_WINDOW].iter())
            .copied()
    }

    fn squares(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        let (xs, ys) = (
            (self.w + 1).saturating_sub(COPY_WINDOW),
            (self.h + 1).saturating_sub(COPY_WINDOW),
        );
        (0..ys).flat_map(move |y| (0..xs).map(move |x| (x, y)))
    }

    fn hash(&self, x: usize, y: usize) -> Option<u64> {
        let mut seen = BTreeSet::new();
        let mut h = DefaultHasher::new();
        for c in self.square(x, y) {
            seen.insert(c);
            h.write_u16(c);
        }
        (seen.len() >= COPY_MIN_DISTINCT).then(|| h.finish())
    }
}

/// The extract's layouts: `layouts/*.ron` and the maps' own.
fn extracted_grids(extracted: &Path) -> Result<Vec<Grid>> {
    let mut grids = Vec::new();
    for (sub, is_map) in [("layouts", false), ("maps", true)] {
        let dir = extracted.join(sub);
        for name in ron_stems(&dir)? {
            let p = dir.join(format!("{name}.ron"));
            let text =
                std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
            let layout = match is_map {
                true => ron::from_str::<MapFile>(&text).map(|m| m.layout),
                false => ron::from_str::<LayoutFile>(&text).map(|l| l.layout),
            };
            let layout = layout.with_context(|| format!("parsing {}", p.display()))?;
            grids.push(Grid::new(format!("{sub}/{name}"), &layout));
        }
    }
    Ok(grids)
}

/// Every games dir layout whose metatiles equal an extracted layout's, or that holds a
/// [`COPY_WINDOW`]-square of one (with at least [`COPY_MIN_DISTINCT`] metatiles): authored maps
/// may use the extract's tilesets, not its maps.
pub fn copies(games: &GamesDir, extracted: &Path) -> Result<Vec<String>> {
    let theirs = extracted_grids(extracted)?;
    let mut index = HashMap::<u64, Vec<(usize, usize, usize)>>::new();
    for (i, g) in theirs.iter().enumerate() {
        for (x, y) in g.squares() {
            if let Some(h) = g.hash(x, y) {
                index.entry(h).or_default().push((i, x, y));
            }
        }
    }
    let mut bad = Vec::new();
    for name in games.layout_names()? {
        let Ok(l) = games.load_layout(&name) else {
            continue; // `hygiene` reports it
        };
        let ours = Grid::new(name.clone(), &l);
        let path = games.layout_path(&name);
        if let Some(t) = (theirs.iter()).find(|t| t.w == ours.w && t.cells == ours.cells) {
            bad.push(format!(
                "{}: the blocks of extracted {}",
                path.display(),
                t.name
            ));
            continue;
        }
        let found = ours.squares().find_map(|(x, y)| {
            let hits = index.get(&ours.hash(x, y)?)?;
            hits.iter()
                .find(|&&(i, tx, ty)| theirs[i].square(tx, ty).eq(ours.square(x, y)))
                .map(|&(i, tx, ty)| (x, y, i, tx, ty))
        });
        if let Some((x, y, i, tx, ty)) = found {
            bad.push(format!(
                "{}: the {COPY_WINDOW}x{COPY_WINDOW} cells at ({x}, {y}) are extracted {}'s at \
                 ({tx}, {ty})",
                path.display(),
                theirs[i].name
            ));
        }
    }
    Ok(bad)
}
