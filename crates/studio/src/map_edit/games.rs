//! A games dir (`games/<game>/`): where edited maps are saved, as committed RON.

use super::model::Doc;
use super::schema::{GameMap, from_ron, to_ron};
use anyhow::{Context, Result, bail, ensure};
use extract::schema::Layout;
use std::path::{Path, PathBuf};

/// The game the editor works on by default.
pub const DEFAULT_GAME: &str = "silice";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GamesDir {
    pub root: PathBuf,
}

/// A map or layout name: an identifier, so it is a safe file name.
pub fn check_name(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !name.starts_with(|c: char| c.is_ascii_digit()),
        "{name:?} is not a map name (letters, digits and _)"
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
        let dir = self.maps_dir();
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        for e in std::fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
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

    /// Writes `maps/<name>.ron` and `layouts/<layout>.ron` (only the files whose bytes
    /// change).
    pub fn save(&self, doc: &Doc) -> Result<()> {
        check_name(&doc.map.name)?;
        check_name(&doc.map.layout)?;
        write(&self.map_path(&doc.map.name), &to_ron(&doc.map)?)?;
        write(&self.layout_path(&doc.map.layout), &to_ron(&doc.layout())?)
    }

    /// Writes a new map, refusing to replace a saved one.
    pub fn create(&self, doc: &Doc) -> Result<()> {
        if self.exists(&doc.map.name) {
            bail!("{} exists", self.map_path(&doc.map.name).display());
        }
        self.save(doc)
    }
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
    std::fs::write(p, text).with_context(|| format!("writing {}", p.display()))
}

/// Problems with what a games dir holds for the editor: anything but `maps/*.ron` and
/// `layouts/*.ron` under those two dirs, and any file that is not its schema's canonical RON
/// (re-serialized byte for byte), so the files hold nothing but names, raw block indices and
/// event data: no tiles, palettes or metatile graphics. Other top-level entries are not the
/// editor's and are not looked at.
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
    Ok(bad)
}
