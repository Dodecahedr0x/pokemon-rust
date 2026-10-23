//! The id allocation contract (SIL1 Wave 0): every Silice number comes from
//! `games/silice/ids/<kind>.ron` ([`IdsFile`], written by `cargo xtask silice import`).
//!
//! - **Append-only.** A name keeps its number forever; new names are appended after the highest
//!   number. A removed name becomes a tombstone (`removed: true`) and keeps its number, which is
//!   never handed out again. Saves and checkpoints hold numbers, so a renumbering would silently
//!   break them: [`check_golden`] (`crates/content/tests/sil1_w0_ids.rs`, over
//!   `xtask/tests/fixtures/silice/ids.golden`) fails on one.
//! - **Valid files.** [`load`] fails on a duplicate name or number, a number outside
//!   [`IdKind::range`] or [`IdKind::reserved`] (`MAP_DYNAMIC`), and a map entry without its map
//!   file name.
//! - **Constants.** [`Ids::consts`] emits `content::consts` for the live (non-tombstone) maps
//!   (`MAP_*: MapId`), flags (`FLAG_SIL_*: FlagId`), vars (`VAR_SIL_*: VarId`) and songs
//!   (`MUS_SIL_*`/`SE_SIL_*: u16`). A tombstone has no constant, so code naming one fails to
//!   compile. Trainers, items, species and moves get theirs with their data (Task A).
//! - **References.** [`Ids::live`] resolves a name in data files: a tombstone is an error.

use super::schema::{IdEntry, IdKind, IdsFile};
use anyhow::{Context, Result, bail, ensure};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Every ids file of a games dir.
#[derive(Clone, Debug, Default)]
pub struct Ids {
    pub files: BTreeMap<IdKind, IdsFile>,
}

/// Reads and checks `<games>/ids/<kind>.ron` for every [`IdKind`]; a missing file is empty.
pub fn load(games: &Path) -> Result<Ids> {
    let mut files = BTreeMap::new();
    for kind in IdKind::ALL {
        let path = games.join(format!("ids/{}.ron", kind.file()));
        let f: IdsFile = match path.exists() {
            true => extract::export::read_ron(&path)?,
            false => IdsFile {
                base: kind.range().0,
                ids: Vec::new(),
            },
        };
        validate(kind, &f).with_context(|| path.display().to_string())?;
        files.insert(kind, f);
    }
    Ok(Ids { files })
}

/// The rules of one ids file (see the module doc).
pub fn validate(kind: IdKind, f: &IdsFile) -> Result<()> {
    let (lo, hi) = kind.range();
    let mut names = BTreeSet::new();
    let mut nums = BTreeSet::new();
    for e in &f.ids {
        ensure!(names.insert(&e.name), "{} is listed twice", e.name);
        ensure!(
            nums.insert(e.num),
            "{} reuses number {} (numbers are never reused, even by a tombstone)",
            e.name,
            e.num
        );
        ensure!(
            (lo..=hi).contains(&e.num) && !kind.reserved(e.num),
            "{} = {:#X} is outside {lo:#X}..={hi:#X} or reserved",
            e.name,
            e.num
        );
        ensure!(
            kind != IdKind::Maps || e.map.is_some(),
            "{} has no map file name",
            e.name
        );
    }
    Ok(())
}

impl Ids {
    /// The entry named `name`, tombstone or not.
    pub fn get(&self, kind: IdKind, name: &str) -> Option<&IdEntry> {
        self.files.get(&kind)?.ids.iter().find(|e| e.name == name)
    }

    /// The number of `name` if the file lists it: an error for a tombstone.
    pub fn live(&self, kind: IdKind, name: &str) -> Result<Option<u32>> {
        match self.get(kind, name) {
            Some(e) if e.removed => {
                bail!("{name} is removed (a tombstone in ids/{}.ron)", kind.file())
            }
            Some(e) => Ok(Some(e.num)),
            None => Ok(None),
        }
    }

    /// The map entry whose map file is `maps/<file>.ron`.
    pub fn map_file(&self, file: &str) -> Option<&IdEntry> {
        self.files
            .get(&IdKind::Maps)?
            .ids
            .iter()
            .find(|e| e.map.as_deref() == Some(file))
    }

    /// Every entry, by kind, in file order.
    pub fn entries(&self) -> impl Iterator<Item = (IdKind, &IdEntry)> {
        self.files
            .iter()
            .flat_map(|(&k, f)| f.ids.iter().map(move |e| (k, e)))
    }

    /// The `content::consts` of the live maps, flags, vars and songs: (name, type, value).
    pub fn consts(&self) -> Vec<(String, &'static str, String)> {
        let mut out = Vec::new();
        for (kind, e) in self.entries().filter(|(_, e)| !e.removed) {
            let (ty, value) = match kind {
                IdKind::Maps => ("MapId", format!("MapId({:#X})", e.num)),
                IdKind::Flags => ("FlagId", format!("FlagId({:#X})", e.num)),
                IdKind::Vars => ("VarId", format!("VarId({:#X})", e.num)),
                IdKind::Songs => ("u16", format!("{:#X}", e.num)),
                _ => continue,
            };
            out.push((e.name.clone(), ty, value));
        }
        out
    }
}

/// Checks the append-only rule against a golden list (`# comment` lines, then one
/// `<file> <name> <num>` per line, as `xtask/tests/fixtures/silice/ids.golden`): every golden
/// name is still listed (a tombstone counts) with its number. Returns how many were checked.
#[allow(dead_code)] // used by the tests
pub fn check_golden(golden: &str, ids: &Ids) -> Result<usize> {
    let mut n = 0;
    for l in golden
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        let w: Vec<&str> = l.split_whitespace().collect();
        let [file, name, num] = w[..] else {
            bail!("golden line {l:?}");
        };
        let num: u32 = num.parse().with_context(|| format!("golden line {l:?}"))?;
        let kind = IdKind::ALL
            .into_iter()
            .find(|k| k.file() == file)
            .with_context(|| format!("golden line {l:?}: no ids file {file}"))?;
        match ids.get(kind, name) {
            Some(e) => ensure!(
                e.num == num,
                "ids/{file}.ron: {name} was {num}, now {}: ids are append-only, never renumbered",
                e.num
            ),
            None => bail!("ids/{file}.ron: {name} ({num}) is gone: tombstone it instead"),
        }
        n += 1;
    }
    Ok(n)
}
