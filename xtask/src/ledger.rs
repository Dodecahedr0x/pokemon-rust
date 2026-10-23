//! `cargo xtask ledger [--db <path>] [--sweep <path>] [--out <dir>]`: the Emerald feature-parity
//! ledger (`crates/parity`; `docs/plans/2026-10-23-emerald-parity.md` Task L), written to
//! `target/parity/ledger.{md,ron}`.
//!
//! The engine's answer for each entry comes from the code that dispatches it:
//! - commands and specials: the content build's support tables
//!   (`content/build/support/{commands,specials}.ron`, their classes and comments), and the
//!   `prelude::special` arms that report `Degraded`;
//! - behaviours, move effects, abilities, hold effects and map types: the build's classifiers
//!   (`content/build/classify/`, included here as the content tests include them);
//! - battle types: `content::prelude::{BATTLE_TYPES_HANDLED, BATTLE_TYPES_DEGRADED}`;
//! - item uses: `content::data::ITEM_USE` and the bag and battle-item stubs;
//! - screens: whether the engine's screen module still answers with the Wave 0 `stub_step`;
//! - weathers and field moves: nothing dispatches them (no field weather, no field-move use),
//!   so they are `ignored`/`missing` with that note;
//! - engine reports: every `Report::{Unsupported, Degraded}("…")` literal and `stub_step` name
//!   in `crates/{engine,content}/src` outside tests.
//!
//! The lowering's sites come from `content::PARITY`, the reach from the explorer db
//! (`target/explore/<flavor>/explore.db`) and the sweep report (`target/sweep/report.txt`)
//! when they exist.

#[allow(dead_code)]
#[path = "../../crates/content/build/classify/mod.rs"]
mod classify;

use anyhow::{Context, Result, bail};
use parity::decomp::Decomp;
use parity::ledger::Extra;
use parity::lowered::Lowering;
use parity::reach::Reach;
use parity::{Answer, Category, Engine, Status};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The engine's answers, read from this checkout.
struct Real {
    support: extract::codegen::Support,
    /// Support-table comment per command or special.
    notes: BTreeMap<String, String>,
    /// Specials whose `prelude::special` arm reports `Degraded`.
    degraded_specials: BTreeSet<String>,
    /// Screen module (`ui/bag.rs`) -> whether it is still the Wave 0 stub.
    stub_screens: BTreeMap<String, bool>,
    item_use_empty: bool,
    battle_items_stub: bool,
}

/// `"NAME": Class, // comment` lines of a support table -> comment.
fn support_notes(src: &str) -> BTreeMap<String, String> {
    src.lines()
        .filter_map(|l| {
            let l = l.trim();
            let rest = l.strip_prefix('"')?;
            let (name, rest) = rest.split_once('"')?;
            let note = rest.split_once("//").map_or("", |(_, c)| c.trim());
            Some((name.to_string(), note.to_string()))
        })
        .collect()
}

/// The specials whose arm in `prelude/special.rs`'s `special` calls `degraded(`: an arm starts
/// at `id if id == SPECIAL_…` (with `|| id == SPECIAL_…` continuations) and runs to the next.
pub fn degraded_specials(src: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut arm: Option<(Vec<String>, bool)> = None;
    let body = src.split("pub async fn special(").nth(1).unwrap_or("");
    let names = |l: &str| -> Vec<String> {
        l.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .filter(|w| w.starts_with("SPECIAL_"))
            .map(str::to_string)
            .collect()
    };
    let flush = |arm: &mut Option<(Vec<String>, bool)>, out: &mut BTreeSet<String>| {
        if let Some((n, true)) = arm.take() {
            out.extend(n);
        }
    };
    for l in body.lines() {
        let t = l.trim();
        if t.starts_with("id if id == ") {
            flush(&mut arm, &mut out);
            arm = Some((names(t), t.contains("degraded(")));
        } else if t.starts_with("_ =>") {
            flush(&mut arm, &mut out);
            break;
        } else if let Some((n, d)) = arm.as_mut() {
            if t.starts_with("|| id == ") {
                n.extend(names(t));
            }
            *d |= t.contains("degraded(");
        }
    }
    flush(&mut arm, &mut out);
    out
}

/// `Report::Unsupported("…")`/`Report::Degraded("…")` literals and `stub_step(cx, "…")` names
/// of the Rust sources under `dirs`, outside tests.
pub fn engine_reports(dirs: &[PathBuf]) -> Result<BTreeMap<String, Status>> {
    let re = regex::Regex::new(
        r#"Report::(Unsupported|Degraded)\("([^"]+)"\)|stub_step\(\s*\w+,\s*"([^"]+)"\)"#,
    )?;
    let mut out = BTreeMap::new();
    let mut stack: Vec<PathBuf> = dirs.to_vec();
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "tests" || name == "testing" {
                continue;
            }
            for e in std::fs::read_dir(&p)? {
                stack.push(e?.path());
            }
            continue;
        }
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.ends_with(".rs") || name.ends_with("tests.rs") || name.starts_with("test") {
            continue;
        }
        let src = std::fs::read_to_string(&p)?;
        // Test modules at the end of a file.
        let src = src.split("#[cfg(test)]").next().unwrap_or("");
        for c in re.captures_iter(src) {
            let (status, name) = match (c.get(1), c.get(2), c.get(3)) {
                (Some(k), Some(n), _) => (
                    if k.as_str() == "Degraded" {
                        Status::Degraded
                    } else {
                        Status::Unsupported
                    },
                    n.as_str(),
                ),
                (_, _, Some(n)) => (Status::Unsupported, n.as_str()),
                _ => continue,
            };
            out.entry(name.to_string()).or_insert(status);
        }
    }
    Ok(out)
}

impl Real {
    fn read(root: &Path) -> Result<Real> {
        let support_dir = root.join("crates/content/build/support");
        let read = |p: &Path| std::fs::read_to_string(p).with_context(|| p.display().to_string());
        let commands = read(&support_dir.join("commands.ron"))?;
        let specials = read(&support_dir.join("specials.ron"))?;
        let movements = read(&support_dir.join("movements.ron"))?;
        let support =
            extract::codegen::Support::parse(&commands, Some(&specials), Some(&movements))?;
        let mut notes = support_notes(&commands);
        notes.extend(support_notes(&specials));
        let degraded_specials =
            degraded_specials(&read(&root.join("crates/content/src/prelude/special.rs"))?);
        let mut stub_screens = BTreeMap::new();
        for s in parity::owners::SCREENS {
            if s.2.is_empty() {
                continue;
            }
            let p = root.join("crates/engine/src").join(s.2);
            let stub = std::fs::read_to_string(&p).map(|src| src.contains("stub_step("));
            stub_screens.insert(s.2.to_string(), stub.unwrap_or(true));
        }
        let battle_items = read(&root.join("crates/engine/src/battle/core/items.rs"))?;
        Ok(Real {
            support,
            notes,
            degraded_specials,
            stub_screens,
            item_use_empty: content::data::ITEM_USE.is_empty(),
            battle_items_stub: battle_items.contains("Wave 0: refused"),
        })
    }

    fn class(&self, c: Option<&extract::codegen::Class>, note: String) -> Answer {
        use extract::codegen::Class;
        match c {
            Some(Class::Native) => Answer::new(Status::Native, note),
            Some(Class::Ignore) => Answer::new(Status::Ignored, note),
            Some(Class::Default(v)) => {
                Answer::new(Status::Degraded, format!("Default({v}) {note}"))
            }
            Some(Class::Unsupported) => Answer::new(Status::Unsupported, note),
            None => Answer::new(
                Status::Unsupported,
                "not in the support tables (no IR site; the lowering would fail on one)",
            ),
        }
    }
}

impl Engine for Real {
    fn answer(&self, cat: Category, name: &str, d: &Decomp) -> Answer {
        let note = self.notes.get(name).cloned().unwrap_or_default();
        match cat {
            Category::Command => {
                // Macros that assemble to another command's opcode (`asm/macros/event.inc`):
                // `hideplayer` is `hideobjectat LOCALID_PLAYER, 0`, so the IR never names them.
                let op = match name {
                    "hideplayer" => "hideobjectat",
                    "showplayer" => "showobjectat",
                    n => n,
                };
                let note = if op == name {
                    note
                } else {
                    format!("the {op} opcode with LOCALID_PLAYER")
                };
                self.class(self.support.commands.get(op), note)
            }
            Category::Special => {
                if self.degraded_specials.contains(name) {
                    return Answer::new(Status::Degraded, note);
                }
                self.class(self.support.specials.get(name), note)
            }
            Category::Behavior => {
                let (v, exact) = classify::behavior_class(name);
                let s = match (exact, v.as_str()) {
                    (true, _) => Status::Native,
                    (false, "Normal") => Status::Ignored,
                    (false, "Blocked") => Status::Unsupported,
                    (false, _) => Status::Degraded,
                };
                Answer::new(s, format!("Behavior::{v}"))
            }
            Category::MoveEffect => match classify::move_effect(name) {
                Some(v) => Answer::new(Status::Native, v),
                None => Answer::new(Status::Unsupported, "MoveEffect::Unsupported"),
            },
            Category::Ability => {
                let v = classify::ability(name);
                if v != "Ability::Other" {
                    return Answer::new(Status::Native, v);
                }
                let silice = classify::ability_with(name, classify::Rules::SILICE);
                if silice != "Ability::Other" {
                    Answer::new(
                        Status::Missing,
                        format!("{silice} is modelled but off for Emerald (GEN3_EXTRA_ABILITIES)"),
                    )
                } else {
                    Answer::new(Status::Missing, "Ability::Other")
                }
            }
            Category::HoldEffect => {
                let items: Vec<&String> = d
                    .items
                    .iter()
                    .filter(|(_, i)| i.hold_effect.as_deref() == Some(name))
                    .map(|(k, _)| k)
                    .collect();
                let modelled = items
                    .iter()
                    .filter(|i| classify::hold_effect(i).is_some())
                    .count();
                let trainers = if classify::TRAINER_HELD_ITEMS {
                    ""
                } else {
                    "; trainer mons hold no item in Emerald (TRAINER_HELD_ITEMS)"
                };
                match (modelled, items.len()) {
                    (_, 0) => Answer::new(Status::Native, "no item has it"),
                    (0, n) => Answer::new(Status::Missing, format!("0 of {n} items")),
                    (m, n) if m == n => Answer::new(Status::Native, format!("{n} items{trainers}")),
                    (m, n) => Answer::new(
                        Status::Degraded,
                        format!("{m} of {n} items modelled{trainers}"),
                    ),
                }
            }
            Category::ItemFieldUse => {
                if self.item_use_empty || self.stub_screens.get("ui/bag.rs") == Some(&true) {
                    Answer::new(
                        Status::Missing,
                        "content::data::ITEM_USE is empty and the bag is the Wave 0 stub (M9 C, B)",
                    )
                } else {
                    Answer {
                        status: None,
                        note: "ITEM_USE is filled: per-function answers not derived yet".into(),
                    }
                }
            }
            Category::ItemBattleUse => {
                if self.battle_items_stub {
                    Answer::new(
                        Status::Missing,
                        "battle/core/items.rs refuses every item (Wave 0; M9 BM)",
                    )
                } else {
                    Answer {
                        status: None,
                        note: "battle items are implemented: per-function answers not derived yet"
                            .into(),
                    }
                }
            }
            // Wave C field moves: the A-press scripts on the map (`EventScript_UseSurf`, the
            // trees, rocks and boulders) and their field effects; using one from the party
            // menu is M9's.
            Category::FieldMove => match name {
                "FIELD_MOVE_SURF" => Answer::new(
                    Status::Native,
                    "A on water: EventScript_UseSurf, FldEff_UseSurf, surfing, water encounters",
                ),
                "FIELD_MOVE_CUT" => Answer::new(
                    Status::Native,
                    "EventScript_CutTree: FLDEFF_USE_CUT_ON_TREE (party-menu Cut on grass: M9)",
                ),
                "FIELD_MOVE_STRENGTH" => Answer::new(
                    Status::Native,
                    "EventScript_StrengthBoulder, FLDEFF_USE_STRENGTH, boulder pushing",
                ),
                "FIELD_MOVE_ROCK_SMASH" => Answer::new(
                    Status::Degraded,
                    "EventScript_RockSmash, FLDEFF_USE_ROCK_SMASH; SPECIAL_RockSmashWildEncounter \
                     unsupported",
                ),
                _ => Answer::new(
                    Status::Missing,
                    "no field use in the engine (GameData::field_move_gate only; M10)",
                ),
            },
            Category::Weather => {
                if name == "WEATHER_NONE" {
                    Answer::new(Status::Native, "no weather")
                } else {
                    Answer::new(
                        Status::Ignored,
                        "no field weather: the content build drops the map's weather",
                    )
                }
            }
            Category::BattleType => {
                if content::prelude::BATTLE_TYPES_HANDLED.contains(&name) {
                    Answer::new(Status::Native, "prelude::BATTLE_TYPES_HANDLED")
                } else if content::prelude::BATTLE_TYPES_DEGRADED.contains(&name) {
                    Answer::new(Status::Degraded, "prelude::BATTLE_TYPES_DEGRADED")
                } else {
                    Answer::new(Status::Missing, "")
                }
            }
            Category::MapType => match d.values.get(name).and_then(|v| u8::try_from(*v).ok()) {
                Some(raw) => Answer::new(
                    Status::Native,
                    format!("MapType::{}", classify::map_type(raw, "")),
                ),
                None => Answer {
                    status: None,
                    note: "no value".into(),
                },
            },
            Category::Screen => {
                let Some(s) = parity::owners::SCREENS.iter().find(|s| s.0 == name) else {
                    return Answer::default();
                };
                match self.stub_screens.get(s.2) {
                    _ if s.2 == "starter.rs" => Answer::new(Status::Native, "engine::starter"),
                    None => Answer::new(Status::Missing, "no engine screen"),
                    Some(true) => Answer::new(
                        Status::Missing,
                        format!("{} is the Wave 0 stub (reports Unsupported)", s.2),
                    ),
                    Some(false) => Answer::new(Status::Native, s.2),
                }
            }
            Category::EngineReport => Answer::default(),
        }
    }
}

/// `--flag <value>` of `args`.
fn flag(args: &[String], name: &str) -> Option<PathBuf> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
}

pub fn main(args: &[String]) -> Result<()> {
    let root = crate::root();
    if content::FLAVOR != content::Flavor::Emerald {
        bail!(
            "the ledger needs the Emerald content build (this one is {}): set PKR_EXTRACTED_DIR \
             or run `cargo xtask extract`",
            content::FLAVOR.name()
        );
    }
    let decomp_dir =
        std::env::var_os("PKR_DECOMP").map_or_else(|| root.join("../pokeemerald"), PathBuf::from);
    let d = Decomp::load(&decomp_dir)
        .with_context(|| format!("reading the decomp at {}", decomp_dir.display()))?;
    let engine = Real::read(&root)?;
    let lowering = Lowering::parse(content::PARITY);
    let mut reach = Reach::default();
    let db = flag(args, "--db").unwrap_or_else(|| root.join("target/explore/emerald/explore.db"));
    let mut issues = Vec::new();
    if db.exists() {
        if let Err(e) = reach.read_db(&db) {
            issues.push(format!("explorer db: {e:#}"));
        }
    } else {
        issues.push(format!(
            "no explorer db at {} (`cargo xtask explore branch`)",
            db.display()
        ));
    }
    let sweep = flag(args, "--sweep").unwrap_or_else(|| root.join("target/sweep/report.txt"));
    match std::fs::read_to_string(&sweep) {
        Ok(s) => reach.read_sweep(&s),
        Err(_) => issues.push(format!(
            "no sweep report at {} (`cargo xtask sweep`)",
            sweep.display()
        )),
    }
    let spine_src = std::fs::read_to_string(root.join("crates/explore/src/spine/emerald.rs"))
        .unwrap_or_default();
    let extra = Extra {
        engine_reports: engine_reports(&[
            root.join("crates/engine/src"),
            root.join("crates/content/src"),
        ])?,
        bypassed: parity::story::bypassed(&spine_src),
    };
    let mut ledger = parity::ledger::build(&d, &engine, &lowering, &reach, &extra);
    for (cat, set) in [
        (Category::Command, &engine.support.commands),
        (Category::Special, &engine.support.specials),
    ] {
        let known: BTreeSet<&String> = d.names(cat).iter().collect();
        for k in set.keys().filter(|k| !known.contains(k)) {
            ledger.issues.push(format!(
                "{}: {k} is in the support table but not in the decomp universe",
                cat.title()
            ));
        }
    }
    ledger.issues.extend(issues);
    let out = flag(args, "--out").unwrap_or_else(|| root.join("target/parity"));
    std::fs::create_dir_all(&out)?;
    std::fs::write(out.join("ledger.md"), ledger.markdown())?;
    std::fs::write(out.join("ledger.ron"), ledger.ron())?;
    print!("{}", ledger.coverage_table());
    println!("\nTop gaps not owned by M9/M10:");
    for (i, e) in ledger.wave_c(30).iter().enumerate() {
        println!(
            "{:>2}. {} {} ({}, {}, {}, {} sites)",
            i + 1,
            e.category.title(),
            e.name,
            e.status.name(),
            e.story_act.map_or("-", |a| parity::story::ACTS[a].0),
            e.reached.name(),
            e.sites.max(e.lowered.unwrap_or(0))
        );
    }
    for s in &ledger.sources {
        println!("{s}");
    }
    println!(
        "{} issues; wrote {}",
        ledger.issues.len(),
        out.join("ledger.{md,ron}").display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn degraded_arms_of_the_special_dispatch() {
        let src = "pub async fn special(ctx: &Ctx, id: u16) -> u16 {\n    match id {\n        id if id == SPECIAL_A => ctx.heal().await,\n        id if id == SPECIAL_B\n            || id == SPECIAL_C =>\n        {\n            degraded(ctx, id).await;\n        }\n        id if id == SPECIAL_D => degraded(ctx, id).await,\n        _ => {}\n    }\n}\n";
        let v: Vec<String> = degraded_specials(src).into_iter().collect();
        assert_eq!(v, ["SPECIAL_B", "SPECIAL_C", "SPECIAL_D"]);
    }

    /// The real dispatch: the specials the explorer reports degraded.
    #[test]
    fn the_real_degraded_specials() {
        let src =
            std::fs::read_to_string(crate::root().join("crates/content/src/prelude/special.rs"))
                .unwrap();
        let v = degraded_specials(&src);
        assert!(v.contains("SPECIAL_StartWallyTutorialBattle"));
        assert!(v.contains("SPECIAL_ChoosePartyMon"));
        assert!(!v.contains("SPECIAL_HealPlayerParty"));
    }

    #[test]
    fn engine_report_literals() {
        let r = engine_reports(&[crate::root().join("crates/engine/src")]).unwrap();
        assert_eq!(r.get("evolution scene"), Some(&Status::Degraded));
        assert_eq!(r.get("bag"), Some(&Status::Unsupported), "{r:?}");
        assert!(!r.contains_key("x"), "test literals are left out");
    }

    #[test]
    fn support_table_comments() {
        let n = support_notes("{\n    \"a\": Native, // why\n    \"b\": Ignore,\n}");
        assert_eq!(n["a"], "why");
        assert_eq!(n["b"], "");
    }
}
