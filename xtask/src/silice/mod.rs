//! `cargo xtask silice` (SIL1 Task B): the one-time importer of the Silice chapters into
//! `games/silice/`, and the permanent checker of that data. Formats: [`schema`].
//!
//! ```text
//! cargo xtask silice import <contenu-jeu> [--out DIR] [--force] [--report FILE] [--maps DIR]
//! cargo xtask silice check [--games DIR] [--extracted DIR] [--scripts DIR] [--allow RULE]...
//! cargo xtask silice tombstone [--games DIR] NAME...
//! ```
//!
//! `tombstone` marks ids as removed in their `ids/*.ron` file (they keep their numbers).

mod check;
mod ids;
mod import;
mod maps;
mod md;
mod names;
pub mod schema;

use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const USAGE: &str = "usage: cargo xtask silice import <contenu-jeu dir> [--out DIR] [--force] [--report FILE] [--maps DIR]\n       \
     cargo xtask silice check [--games DIR] [--extracted DIR] [--scripts DIR] [--allow RULE]...\n       \
     cargo xtask silice tombstone [--games DIR] NAME...";

pub fn main(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("import") => {
            let mut src = None;
            let mut out = crate::root().join("games/silice");
            let mut report = None;
            let mut maps = None;
            let mut force = false;
            let mut it = args[1..].iter();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--out" => out = PathBuf::from(it.next().context(USAGE)?),
                    "--report" => report = Some(PathBuf::from(it.next().context(USAGE)?)),
                    "--maps" => maps = Some(PathBuf::from(it.next().context(USAGE)?)),
                    "--force" => force = true,
                    s if !s.starts_with('-') && src.is_none() => src = Some(PathBuf::from(s)),
                    _ => bail!(USAGE),
                }
            }
            import::run(&import::Options {
                src: src.context(USAGE)?,
                out,
                force,
                report,
                maps,
            })
        }
        Some("check") => {
            let mut o = check::Options {
                games: crate::root().join("games/silice"),
                extracted: std::env::var_os("PKR_EXTRACTED_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| crate::root().join("content/extracted")),
                decomp: std::env::var_os("PKR_DECOMP").map(PathBuf::from),
                scripts: crate::root().join("crates/content/src/silice/scripts"),
                allow: Vec::new(),
            };
            let mut it = args[1..].iter();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--games" => o.games = PathBuf::from(it.next().context(USAGE)?),
                    "--extracted" => o.extracted = PathBuf::from(it.next().context(USAGE)?),
                    "--scripts" => o.scripts = PathBuf::from(it.next().context(USAGE)?),
                    "--allow" => o.allow.push(it.next().context(USAGE)?.clone()),
                    _ => bail!(USAGE),
                }
            }
            check::run(&o)
        }
        Some("tombstone") => {
            let mut games = crate::root().join("games/silice");
            let mut names = Vec::new();
            let mut it = args[1..].iter();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--games" => games = PathBuf::from(it.next().context(USAGE)?),
                    s if !s.starts_with('-') => names.push(s.to_string()),
                    _ => bail!(USAGE),
                }
            }
            if names.is_empty() {
                bail!(USAGE);
            }
            tombstone(&games, &names)
        }
        _ => bail!(USAGE),
    }
}

/// Marks `names` removed in whichever `ids/*.ron` holds them.
fn tombstone(games: &Path, names: &[String]) -> Result<()> {
    let mut left: Vec<String> = names.to_vec();
    let mut files = BTreeMap::new();
    for k in schema::IdKind::ALL {
        let rel = format!("ids/{}.ron", k.file());
        let path = games.join(&rel);
        if !path.exists() {
            continue;
        }
        let mut f: schema::IdsFile = read_ron(&path)?;
        let before = left.len();
        left = ids::tombstone(&mut f, &left);
        if left.len() != before {
            files.insert(rel, extract::export::to_ron(&f)?);
        }
    }
    if !left.is_empty() {
        bail!("not in any ids file: {}", left.join(", "));
    }
    write_files(games, &files, true)?;
    for n in names {
        println!("tombstoned {n}");
    }
    Ok(())
}

pub(crate) fn read_ron<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let s = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    ron::from_str(&s).with_context(|| format!("parsing {}", path.display()))
}

/// Writes `files` (relative path -> contents) under `out`. Without `force`, refuses (writing
/// nothing) when an existing file would change. Each write goes through a temporary file and a
/// rename. Returns the number of files written.
pub(crate) fn write_files(
    out: &Path,
    files: &BTreeMap<String, String>,
    force: bool,
) -> Result<usize> {
    let mut changed = Vec::new();
    let mut clobber = Vec::new();
    for (rel, text) in files {
        let path = out.join(rel);
        match std::fs::read(&path) {
            Ok(old) if old == text.as_bytes() => {}
            Ok(_) => {
                clobber.push(rel.as_str());
                changed.push(rel);
            }
            Err(_) => changed.push(rel),
        }
    }
    if !clobber.is_empty() && !force {
        bail!(
            "refusing to overwrite {} file(s) under {} without --force: {}",
            clobber.len(),
            out.display(),
            clobber.join(", ")
        );
    }
    for rel in &changed {
        let path = out.join(rel);
        std::fs::create_dir_all(path.parent().unwrap())
            .with_context(|| format!("creating {}", path.parent().unwrap().display()))?;
        let tmp = path.with_extension("ron.tmp");
        std::fs::write(&tmp, &files[*rel]).with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, &path).with_context(|| format!("renaming to {}", path.display()))?;
    }
    Ok(changed.len())
}
