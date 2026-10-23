//! The ledger: every universe entry with the engine's status, its uses and reach, ranked.

use crate::decomp::{Decomp, MapInfo};
use crate::lowered::Lowering;
use crate::reach::Reach;
use crate::{Category, Engine, Status, owners, story};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

/// Where the explorer met an entry.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize)]
pub enum Reached {
    /// Not met (or no db).
    #[default]
    No,
    /// A branch stood on a map that uses it.
    Branch,
    /// The spine stood on a map that uses it.
    Spine,
    /// The engine reported it (an `unsupported`/`degraded` finding).
    Reported,
}

impl Reached {
    pub fn name(self) -> &'static str {
        match self {
            Reached::No => "-",
            Reached::Branch => "branch",
            Reached::Spine => "spine",
            Reached::Reported => "reported",
        }
    }
}

/// One feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub category: Category,
    pub name: String,
    pub status: Status,
    pub note: String,
    pub owner: String,
    /// Uses in the decomp: script sites (commands, specials), else maps (behaviours, battle,
    /// weathers, map types, field moves), items (item uses), openers' sites (screens).
    pub sites: usize,
    /// Maps that use it (folder names).
    pub maps: Vec<String>,
    /// The earliest story act among `maps` (0 = Act 1), if any.
    pub story_act: Option<usize>,
    /// On the parity plan's blocker list or bypassed by the spine.
    pub story_blocking: bool,
    pub reached: Reached,
    /// Maps where the sweep saw it reported (idling every map).
    pub sweep_maps: Option<u32>,
    /// The content build's IR sites (commands and specials), its reported and stale sites.
    pub lowered: Option<usize>,
    pub reports: Option<usize>,
    pub stale: Option<usize>,
}

impl Entry {
    /// The rank key: story-blocking, on a story map (earlier acts first), reached by the
    /// explorer, then site count.
    fn key(&self) -> impl Ord + '_ {
        (
            !self.story_blocking,
            self.story_act.unwrap_or(usize::MAX),
            std::cmp::Reverse(self.reached),
            std::cmp::Reverse(self.sites.max(self.lowered.unwrap_or(0))),
            self.category,
            &self.name,
        )
    }
}

/// Counts per status.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coverage {
    pub total: usize,
    pub by_status: BTreeMap<Status, usize>,
}

impl Coverage {
    pub fn count(&self, s: Status) -> usize {
        self.by_status.get(&s).copied().unwrap_or(0)
    }

    /// Native share, in percent.
    pub fn native_pct(&self) -> f64 {
        pct(self.count(Status::Native), self.total)
    }

    /// Native or degraded share, in percent.
    pub fn playable_pct(&self) -> f64 {
        pct(
            self.count(Status::Native) + self.count(Status::Degraded),
            self.total,
        )
    }
}

fn pct(n: usize, d: usize) -> f64 {
    if d == 0 {
        100.0
    } else {
        (n as f64) * 100.0 / (d as f64)
    }
}

/// The ledger.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Ledger {
    /// Per category, ranked: gaps first (in rank order), then the native entries by name.
    pub entries: BTreeMap<Category, Vec<Entry>>,
    pub coverage: BTreeMap<Category, Coverage>,
    /// What the ledger could not derive or read.
    pub issues: Vec<String>,
    /// One line on the explorer db and sweep read.
    pub sources: Vec<String>,
    /// The lowering's totals (sites by class, unsupported sites, unresolved `waitmovement`).
    pub lowering: BTreeMap<String, usize>,
}

/// What [`build`] needs besides the decomp and the engine.
#[derive(Clone, Debug, Default)]
pub struct Extra {
    /// The engine's own report names with their status (degraded or unsupported).
    pub engine_reports: BTreeMap<String, Status>,
    /// Entry names the spine bypasses (Task S): story-blocking.
    pub bypassed: Vec<String>,
}

/// The maps of `d` that use `name` of `cat`, and the entry's site count.
fn uses(d: &Decomp, cat: Category, name: &str) -> (Vec<String>, usize) {
    let filter = |f: &dyn Fn(&MapInfo) -> bool| -> Vec<String> {
        d.maps
            .iter()
            .filter(|m| f(m))
            .map(|m| m.name.clone())
            .collect()
    };
    match cat {
        Category::Command | Category::Special => (
            filter(&|m| m.script_sites.contains_key(name)),
            d.script_sites.get(name).copied().unwrap_or(0),
        ),
        Category::Behavior => {
            let v = filter(&|m| m.behaviors.contains_key(name));
            let n = v.len();
            (v, n)
        }
        Category::MoveEffect | Category::Ability | Category::HoldEffect => {
            let k = (cat, name.to_string());
            let v = filter(&|m| m.battle.contains(&k));
            let n = v.len();
            (v, n)
        }
        Category::ItemFieldUse | Category::ItemBattleUse => {
            let items: BTreeSet<&String> = d
                .items
                .iter()
                .filter(|(_, i)| {
                    let f = if cat == Category::ItemFieldUse {
                        &i.field_use
                    } else {
                        &i.battle_use
                    };
                    f.as_deref() == Some(name)
                })
                .map(|(k, _)| k)
                .collect();
            let v = filter(&|m| m.items.iter().any(|i| items.contains(i)));
            (v, items.len())
        }
        Category::Weather => {
            let v = filter(&|m| m.weather == name);
            let n = v.len();
            (v, n)
        }
        Category::MapType => {
            let v = filter(&|m| m.map_type == name);
            let n = v.len();
            (v, n)
        }
        Category::FieldMove => {
            let v = filter(&|m| field_move_used(m, name));
            let n = v.len();
            (v, n)
        }
        Category::BattleType => {
            // Trainer battles: every map with a trainer; double battles: maps with a double
            // trainer kind; the others have no map use the ledger can tell.
            let v = match name {
                "BATTLE_TYPE_TRAINER" | "BATTLE_TYPE_IS_MASTER" => {
                    filter(&|m| !m.trainers.is_empty())
                }
                "BATTLE_TYPE_DOUBLE" => filter(&|m| {
                    ["trainerbattle_double", "trainerbattle_rematch_double"]
                        .iter()
                        .any(|k| m.script_sites.contains_key(*k))
                        || m.script_sites.keys().any(|k| k.contains("double"))
                }),
                _ => Vec::new(),
            };
            let n = v.len();
            (v, n)
        }
        Category::Screen => {
            let openers = owners::SCREENS
                .iter()
                .find(|s| s.0 == name)
                .map_or(&[][..], |s| s.4);
            let v = filter(&|m| openers.iter().any(|o| m.script_sites.contains_key(*o)));
            let n = openers
                .iter()
                .map(|o| d.script_sites.get(*o).copied().unwrap_or(0))
                .sum();
            (v, n)
        }
        Category::EngineReport => (Vec::new(), 0),
    }
}

/// Field move `name` is needed on `m` (hand rules: the objects, behaviours and connections the
/// move acts on).
pub fn field_move_used(m: &MapInfo, name: &str) -> bool {
    let mb = |f: &dyn Fn(&str) -> bool| m.behaviors.keys().any(|b| f(b));
    match name {
        "FIELD_MOVE_CUT" => m.gfx.contains("OBJ_EVENT_GFX_CUTTABLE_TREE"),
        "FIELD_MOVE_ROCK_SMASH" => m.gfx.contains("OBJ_EVENT_GFX_BREAKABLE_ROCK"),
        "FIELD_MOVE_STRENGTH" => m.gfx.contains("OBJ_EVENT_GFX_PUSHABLE_BOULDER"),
        "FIELD_MOVE_FLASH" => m.requires_flash,
        "FIELD_MOVE_SURF" => mb(&|b| {
            b == "MB_POND_WATER"
                || b == "MB_DEEP_WATER"
                || b == "MB_OCEAN_WATER"
                || b == "MB_SEMI_DEEP_WATER"
                || b == "MB_INTERIOR_DEEP_WATER"
                || b == "MB_SOOTOPOLIS_DEEP_WATER"
                || b.ends_with("_CURRENT")
        }),
        "FIELD_MOVE_WATERFALL" => mb(&|b| b == "MB_WATERFALL"),
        "FIELD_MOVE_DIVE" => m.dive,
        _ => false,
    }
}

/// Joins the decomp, the engine's answers, the lowering and the reach into the ranked ledger.
pub fn build(
    d: &Decomp,
    engine: &dyn Engine,
    lowering: &Lowering,
    reach: &Reach,
    extra: &Extra,
) -> Ledger {
    let mut l = Ledger {
        issues: d.issues.clone(),
        ..Ledger::default()
    };
    let by_name: BTreeMap<&str, &MapInfo> = d.maps.iter().map(|m| (m.name.as_str(), m)).collect();
    // Reported names -> (degraded, maps).
    let mut reported: BTreeMap<&str, (bool, BTreeSet<u16>)> = BTreeMap::new();
    for r in &reach.reported {
        let e = reported.entry(&r.name).or_default();
        e.0 |= r.degraded;
        e.1.insert(r.map);
    }
    let blocking: BTreeSet<(Category, &str)> =
        story::BLOCKING.iter().map(|(c, n, _)| (*c, *n)).collect();
    let mut universe: BTreeMap<Category, Vec<String>> = BTreeMap::new();
    for cat in Category::ALL {
        let names: Vec<String> = match cat {
            Category::Screen => owners::SCREENS.iter().map(|s| s.0.to_string()).collect(),
            Category::EngineReport => {
                // The engine's names and what the explorer and the sweep saw reported, unless
                // another category has the name (`dofieldeffect`, `SPECIAL_…`).
                let known: BTreeSet<&String> = d.universe.values().flatten().collect();
                let s: BTreeSet<String> = extra
                    .engine_reports
                    .keys()
                    .cloned()
                    .chain(reported.keys().map(|k| k.to_string()))
                    .chain(reach.sweep.keys().cloned())
                    .filter(|k| !known.contains(k))
                    .collect();
                s.into_iter().collect()
            }
            _ => d.names(cat).to_vec(),
        };
        universe.insert(cat, names);
    }
    for (cat, names) in universe {
        let mut entries = Vec::new();
        for name in names {
            let answer = if cat == Category::EngineReport {
                match extra.engine_reports.get(&name) {
                    Some(&s) => crate::Answer::new(s, "the engine reports it"),
                    None => match reported.get(name.as_str()) {
                        Some((true, _)) => crate::Answer::new(Status::Degraded, "reported"),
                        Some((false, _)) => crate::Answer::new(Status::Unsupported, "reported"),
                        None => crate::Answer::new(Status::Unsupported, "reported by the sweep"),
                    },
                }
            } else {
                engine.answer(cat, &name, d)
            };
            let mut answer = answer;
            // A native entry the engine reports itself (`Report::Degraded("bufferitemnameplural")`
            // in the prelude, or seen by the explorer) runs simplified.
            if cat != Category::EngineReport && answer.status == Some(Status::Native) {
                let seen = reported.get(name.as_str()).map(|r| r.0);
                let said = extra.engine_reports.get(&name).copied();
                let worse = match (seen, said) {
                    (Some(false), _) | (_, Some(Status::Unsupported)) => Some(Status::Unsupported),
                    (Some(true), _) | (_, Some(Status::Degraded)) => Some(Status::Degraded),
                    _ => None,
                };
                if let Some(w) = worse {
                    answer.status = Some(w);
                    answer.note = format!("{}; the engine reports it {}", answer.note, w.name());
                }
            }
            let status = answer.status.unwrap_or(Status::Unknown);
            if answer.status.is_none() {
                l.issues
                    .push(format!("{} {name}: unknown: {}", cat.title(), answer.note));
            }
            let (maps, sites) = uses(d, cat, &name);
            let story_act = maps.iter().filter_map(|m| story::act_of(m)).min();
            let mut reached = maps
                .iter()
                .filter_map(|m| by_name.get(m.as_str()))
                .map(|m| match reach.map_level(m.id) {
                    2 => Reached::Spine,
                    1 => Reached::Branch,
                    _ => Reached::No,
                })
                .max()
                .unwrap_or_default();
            if reported.contains_key(name.as_str()) {
                reached = Reached::Reported;
            }
            let low = lowering.names.get(&name);
            let story_blocking = blocking.contains(&(cat, name.as_str()))
                || extra.bypassed.contains(&name);
            entries.push(Entry {
                category: cat,
                status,
                note: answer.note,
                owner: owners::owner(cat, &name).to_string(),
                sites,
                story_act,
                story_blocking,
                reached,
                sweep_maps: reach.sweep.get(&name).map(|s| s.0),
                lowered: low.map(|x| x.sites),
                reports: lowering.reports.get(&name).copied(),
                stale: lowering.stale.get(&name).copied(),
                maps,
                name,
            });
        }
        let mut cov = Coverage {
            total: entries.len(),
            ..Coverage::default()
        };
        for e in &entries {
            *cov.by_status.entry(e.status).or_default() += 1;
        }
        entries.sort_by(|a, b| {
            (!a.status.is_gap())
                .cmp(&!b.status.is_gap())
                .then_with(|| a.key().cmp(&b.key()))
        });
        l.coverage.insert(cat, cov);
        l.entries.insert(cat, entries);
    }
    l.lowering = lowering.stats.clone();
    l.lowering.insert(
        "unsupported_sites".into(),
        lowering.reports.values().sum::<usize>(),
    );
    l.lowering.insert(
        "stale_specialvar_sites".into(),
        lowering.stale.values().sum::<usize>(),
    );
    if lowering.is_empty() {
        l.issues
            .push("no lowering (content::PARITY is empty: a stub or Silice build)".into());
    }
    l.sources.push(if reach.explorer {
        let spine = reach
            .spine
            .as_ref()
            .map_or("no spine run".to_string(), |s| {
                format!(
                    "spine {}/{} steps{}",
                    s.0,
                    s.1,
                    s.2.as_ref()
                        .map_or(String::new(), |e| format!(" (stopped: {e})"))
                )
            });
        format!(
            "explorer db: {spine}; {} spine maps, {} more maps by branches; branches {:?}; {} \
             report findings",
            reach.spine_maps.len(),
            reach.branch_maps.len(),
            reach.branches,
            reach.reported.len()
        )
    } else {
        "explorer db: not read (reach is `-` everywhere)".to_string()
    });
    l.sources.push(if reach.swept {
        format!("sweep report: {} report kinds", reach.sweep.len())
    } else {
        "sweep report: not read".to_string()
    });
    l
}

impl Ledger {
    /// Every gap, ranked across categories.
    pub fn ranked_gaps(&self) -> Vec<&Entry> {
        let mut v: Vec<&Entry> = self
            .entries
            .values()
            .flatten()
            .filter(|e| e.status.is_gap())
            .collect();
        v.sort_by(|a, b| a.key().cmp(&b.key()));
        v
    }

    /// The `n` best-ranked gaps that Milestones 9 and 10 do not own (Wave C's input), without
    /// the `ignored` entries (documented no-ops: audio, cosmetic windows, plain-ground
    /// behaviours), which the per-category tables still rank.
    pub fn wave_c(&self, n: usize) -> Vec<&Entry> {
        self.ranked_gaps()
            .into_iter()
            .filter(|e| !owners::is_m9_m10(&e.owner) && e.status != Status::Ignored)
            .take(n)
            .collect()
    }

    pub fn entry(&self, cat: Category, name: &str) -> Option<&Entry> {
        self.entries.get(&cat)?.iter().find(|e| e.name == name)
    }

    pub fn ron(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .unwrap_or_else(|e| format!("// {e}"))
    }

    /// The coverage table (one row per category).
    pub fn coverage_table(&self) -> String {
        let mut o = String::from(
            "| category | entries | native | degraded | ignored | unsupported | missing | unknown | native % | native+degraded % |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n",
        );
        for (cat, c) in &self.coverage {
            let _ = writeln!(
                o,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {:.0}% | {:.0}% |",
                cat.title(),
                c.total,
                c.count(Status::Native),
                c.count(Status::Degraded),
                c.count(Status::Ignored),
                c.count(Status::Unsupported),
                c.count(Status::Missing),
                c.count(Status::Unknown),
                c.native_pct(),
                c.playable_pct()
            );
        }
        o
    }

    /// Gaps per act: entries used on each act's maps that are not native, per category.
    pub fn act_table(&self) -> String {
        let mut o = String::from("| act |");
        let cats: Vec<Category> = self.entries.keys().copied().collect();
        for c in &cats {
            let _ = write!(o, " {} |", c.title());
        }
        o.push_str("\n|---|");
        for _ in &cats {
            o.push_str("---:|");
        }
        o.push('\n');
        for (i, (act, _)) in story::ACTS.iter().enumerate() {
            let _ = write!(o, "| {act} |");
            for c in &cats {
                let n = self.entries[c]
                    .iter()
                    .filter(|e| e.status.is_gap() && e.story_act == Some(i))
                    .count();
                let _ = write!(o, " {n} |");
            }
            o.push('\n');
        }
        o
    }

    fn row(e: &Entry, rank: Option<usize>) -> String {
        let maps = if e.maps.is_empty() {
            "-".to_string()
        } else {
            let first: Vec<&str> = e.maps.iter().take(3).map(String::as_str).collect();
            format!(
                "{} ({}{})",
                e.maps.len(),
                first.join(", "),
                if e.maps.len() > 3 { ", …" } else { "" }
            )
        };
        let story = match (e.story_blocking, e.story_act) {
            (true, _) => "**blocking**".to_string(),
            (false, Some(a)) => story::ACTS[a].0.to_string(),
            (false, None) => "-".to_string(),
        };
        let lowered = match (e.lowered, e.reports, e.stale) {
            (None, None, None) => "-".to_string(),
            (l, r, s) => {
                let mut v = vec![l.map_or("0".into(), |l| l.to_string())];
                if let Some(r) = r {
                    v.push(format!("{r} rep"));
                }
                if let Some(s) = s {
                    v.push(format!("{s} stale"));
                }
                v.join(", ")
            }
        };
        format!(
            "| {} | `{}` | {} | {} | {} | {}{} | {} | {} | {} | {} |",
            rank.map_or(String::new(), |r| r.to_string()),
            e.name,
            e.status.name(),
            e.owner,
            story,
            e.reached.name(),
            e.sweep_maps
                .map_or(String::new(), |n| format!(" (sweep {n})")),
            e.sites,
            lowered,
            maps,
            e.note.replace('|', "/")
        )
    }

    const HEAD: &'static str = "| # | name | status | owner | story | reached | sites | lowered | maps | note |\n|---:|---|---|---|---|---|---:|---|---|---|\n";

    /// The whole ledger as Markdown.
    pub fn markdown(&self) -> String {
        let mut o = String::from(
            "# Emerald parity ledger\n\nGenerated by `cargo xtask ledger` (`crates/parity`). Ranked: story-blocking > on a story map (earlier act first) > reached by the explorer > site count. `reached`: `reported` (the engine reported it while exploring), `spine`/`branch` (the explorer stood on a map using it). `lowered`: the content build's IR sites, its `Report::Unsupported` sites and stale `specialvar` sites.\n\n",
        );
        for s in &self.sources {
            let _ = writeln!(o, "- {s}");
        }
        let _ = writeln!(o, "\n## Coverage\n\n{}", self.coverage_table());
        if !self.lowering.is_empty() {
            let _ = writeln!(o, "Lowering: {:?}\n", self.lowering);
        }
        let _ = writeln!(o, "## Gaps per act\n\n{}", self.act_table());
        let _ = writeln!(
            o,
            "## Top 50 gaps not owned by M9 or M10 (Wave C)\n\n| # | category | name | status | owner | story | reached | sites | note |\n|---:|---|---|---|---|---|---|---:|---|"
        );
        for (i, e) in self.wave_c(50).iter().enumerate() {
            let _ = writeln!(
                o,
                "| {} | {} | `{}` | {} | {} | {} | {} | {} | {} |",
                i + 1,
                e.category.title(),
                e.name,
                e.status.name(),
                e.owner,
                e.story_act.map_or("-", |a| story::ACTS[a].0),
                e.reached.name(),
                e.sites.max(e.lowered.unwrap_or(0)),
                e.note.replace('|', "/")
            );
        }
        let mut by_owner: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
        for e in self.ranked_gaps() {
            let x = by_owner.entry(&e.owner).or_default();
            x.0 += 1;
            if e.story_act.is_some() {
                x.1 += 1;
            }
        }
        let _ = writeln!(
            o,
            "\n## Gaps by owner\n\n| owner | gaps | on a story map |\n|---|---:|---:|"
        );
        for (k, (n, s)) in by_owner {
            let _ = writeln!(o, "| {k} | {n} | {s} |");
        }
        for (cat, entries) in &self.entries {
            let _ = write!(
                o,
                "\n## {} ({})\n\n{}",
                cat.title(),
                entries.len(),
                Self::HEAD
            );
            let mut rank = 0;
            for e in entries {
                let r = if e.status.is_gap() {
                    rank += 1;
                    Some(rank)
                } else {
                    None
                };
                let _ = writeln!(o, "{}", Self::row(e, r));
            }
        }
        if !self.issues.is_empty() {
            let _ = writeln!(o, "\n## Issues\n");
            for i in &self.issues {
                let _ = writeln!(o, "- {i}");
            }
        }
        o
    }
}
