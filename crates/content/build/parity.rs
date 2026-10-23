//! `$OUT_DIR/parity.tsv` (`content::PARITY`), the lowering's side of the parity ledger
//! (`crates/parity`, `cargo xtask ledger`; owned by the Emerald parity plan's Task L).
//!
//! One tab-separated row per fact, decomp names and counts only:
//! - `site <name> <class> <module> <n>`: `n` IR sites of command or special `name` (a special
//!   is its `SPECIAL_*` name, its `special`/`specialvar` site counted under both) in script
//!   module `module` (a map, or `Common_<stem>`), with its support-table class (`Native`,
//!   `Ignore`, `Default`, `Unsupported`). Overridden scripts' own labels are left out.
//! - `report <name> <n>`: the lowering's `Report::Unsupported` sites (`Stats::reports`).
//! - `stale <name> <n>`: stale `specialvar` sites (`Stats::stale_specialvars`).
//! - `stat <key> <n>`: `unresolved_waitmovement` and the sites by class.
//!
//! Empty for the stub and Silice (no IR).

use crate::scripts::Ir;
use extract::codegen::{Class, Stats, Support};
use extract::schema::{IrArg, IrInsn};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

/// The `SPECIAL_*` name of a `special`/`specialvar` site.
fn special_of(i: &IrInsn) -> Option<String> {
    let a = match (i.op.as_str(), &i.args[..]) {
        ("special", [s]) | ("specialvar", [_, s]) => s,
        _ => return None,
    };
    match a {
        IrArg::Const { name, .. } => Some(name.clone()),
        IrArg::Half(n) => Some(extract::names::special_name(*n)),
        _ => None,
    }
}

fn class_name(c: Option<&Class>) -> &'static str {
    match c {
        Some(Class::Native) => "Native",
        Some(Class::Ignore) => "Ignore",
        Some(Class::Default(_)) => "Default",
        Some(Class::Unsupported) | None => "Unsupported",
    }
}

/// The rows (see the module doc), sorted.
pub fn emit(
    ir: Option<&Ir>,
    support: &Support,
    overrides: &BTreeSet<String>,
    stats: &Stats,
) -> String {
    let mut sites: BTreeMap<(String, &'static str, String), usize> = BTreeMap::new();
    for f in ir.map(|ir| &ir.files[..]).unwrap_or_default() {
        for l in f.labels.iter().filter(|l| !overrides.contains(&l.name)) {
            for i in &l.insns {
                if i.op == "unknown" {
                    continue;
                }
                let class = class_name(support.commands.get(&i.op));
                *sites
                    .entry((i.op.clone(), class, f.module.clone()))
                    .or_default() += 1;
                if let Some(s) = special_of(i) {
                    let class = class_name(support.specials.get(&s));
                    *sites.entry((s, class, f.module.clone())).or_default() += 1;
                }
            }
        }
    }
    let mut o = String::new();
    for ((name, class, module), n) in &sites {
        let _ = writeln!(o, "site\t{name}\t{class}\t{module}\t{n}");
    }
    for (name, n) in &stats.reports {
        let _ = writeln!(o, "report\t{name}\t{n}");
    }
    for (name, n) in &stats.stale_specialvars {
        let _ = writeln!(o, "stale\t{name}\t{n}");
    }
    let _ = writeln!(
        o,
        "stat\tunresolved_waitmovement\t{}",
        stats.unresolved_waitmovement
    );
    for (k, n) in &stats.by_class {
        let _ = writeln!(o, "stat\tclass_{k}\t{n}");
    }
    o
}
