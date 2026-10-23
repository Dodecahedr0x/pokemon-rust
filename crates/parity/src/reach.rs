//! What the explorer and the sweep reached: the explorer db (schema v4,
//! `crates/explore/src/schema.sql`), read-only, and `target/sweep/report.txt`.

use anyhow::{Context, Result, ensure};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// One `unsupported` finding (a `Report` the engine raised): `unsupported: NAME` or
/// `degraded: NAME`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reported {
    pub degraded: bool,
    pub name: String,
    pub map: u16,
    pub hits: u32,
    /// Raised by the spine itself (no branch).
    pub spine: bool,
}

/// The explorer's and the sweep's reach.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reach {
    /// Whether an explorer db was read.
    pub explorer: bool,
    /// Maps the spine stood on.
    pub spine_maps: BTreeSet<u16>,
    /// Maps branches stood on or entered (beyond the spine's).
    pub branch_maps: BTreeSet<u16>,
    pub reported: Vec<Reported>,
    /// `(done, steps, error)` of the last spine run.
    pub spine: Option<(i64, i64, Option<String>)>,
    /// Branches by outcome.
    pub branches: BTreeMap<String, i64>,
    /// Whether a sweep report was read.
    pub swept: bool,
    /// Report name (`dofieldeffect`, `SPECIAL_…`, `evolution scene`) -> (maps, events) while
    /// idling every map.
    pub sweep: BTreeMap<String, (u32, u32)>,
}

impl Reach {
    /// Reads the explorer db at `path` (schema 4).
    pub fn read_db(&mut self, path: &Path) -> Result<()> {
        let db =
            rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .with_context(|| format!("opening {}", path.display()))?;
        let schema: String = db
            .query_row("SELECT value FROM meta WHERE key = 'schema'", [], |r| {
                r.get(0)
            })
            .context("reading the explorer db's schema")?;
        ensure!(
            schema == "4",
            "{}: explorer db schema {schema}, expected 4 (`cargo xtask explore reset --yes`)",
            path.display()
        );
        let maps = |sql: &str| -> Result<BTreeSet<u16>> {
            let mut st = db.prepare(sql)?;
            let v = st
                .query_map([], |r| r.get::<_, i64>(0))?
                .collect::<rusqlite::Result<Vec<i64>>>()?;
            Ok(v.into_iter().map(|m| m as u16).collect())
        };
        self.spine_maps = maps("SELECT DISTINCT map FROM position")?;
        self.spine_maps
            .extend(maps("SELECT DISTINCT map FROM coverage WHERE spine > 0")?);
        let mut branch = maps("SELECT DISTINCT map FROM coverage WHERE hits > 0")?;
        branch.extend(maps("SELECT id FROM seen WHERE kind = 2")?);
        self.branch_maps = branch.difference(&self.spine_maps).copied().collect();
        let mut st = db.prepare(
            "SELECT message, map, hits, branch IS NULL FROM finding WHERE lower(kind) = 'unsupported'",
        )?;
        self.reported = st
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, bool>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .filter_map(|(m, map, hits, spine)| {
                let (degraded, name) = parse_report_message(&m)?;
                Some(Reported {
                    degraded,
                    name,
                    map: map as u16,
                    hits: hits as u32,
                    spine,
                })
            })
            .collect();
        self.spine = db
            .query_row(
                "SELECT done, steps, error FROM spine WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .ok();
        let mut st = db.prepare("SELECT outcome, COUNT(*) FROM branch GROUP BY outcome")?;
        self.branches = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        self.explorer = true;
        Ok(())
    }

    /// Reads a sweep report's "reports raised while idling" section.
    pub fn read_sweep(&mut self, text: &str) {
        let mut inside = false;
        for l in text.lines() {
            if l.starts_with("-- reports raised while idling") {
                inside = true;
                continue;
            }
            if !inside {
                continue;
            }
            if l.trim().is_empty() || l.starts_with("--") || l.starts_with("==") {
                break;
            }
            // `Unsupported("dofieldeffect"): 12 maps, 30 events`
            let Some((k, rest)) = l.rsplit_once("): ") else {
                continue;
            };
            let name = k.split_once("(\"").map(|(_, n)| n.trim_end_matches('"'));
            let nums: Vec<u32> = rest
                .split(|c: char| !c.is_ascii_digit())
                .filter_map(|n| n.parse().ok())
                .collect();
            if let (Some(name), &[maps, events]) = (name, &nums[..]) {
                self.sweep.insert(name.to_string(), (maps, events));
            }
        }
        self.swept = true;
    }

    /// Whether the explorer stood on map `id`: 2 the spine, 1 a branch, 0 no.
    pub fn map_level(&self, id: u16) -> u8 {
        if self.spine_maps.contains(&id) {
            2
        } else if self.branch_maps.contains(&id) {
            1
        } else {
            0
        }
    }
}

/// `unsupported: NAME` / `degraded: NAME` -> (degraded, NAME).
pub fn parse_report_message(m: &str) -> Option<(bool, String)> {
    if let Some(n) = m.strip_prefix("unsupported: ") {
        return Some((false, n.to_string()));
    }
    m.strip_prefix("degraded: ").map(|n| (true, n.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sweep_report_section() {
        let mut r = Reach::default();
        r.read_sweep(
            "x\n-- reports raised while idling (2 kinds): maps, events --\nUnsupported(\"dofieldeffect\"): 12 maps, 30 events\nDegraded(\"evolution scene\"): 1 maps, 1 events\n\nscripts started 3",
        );
        assert_eq!(r.sweep["dofieldeffect"], (12, 30));
        assert_eq!(r.sweep["evolution scene"], (1, 1));
    }
}
