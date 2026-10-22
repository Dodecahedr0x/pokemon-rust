//! Milestone 8 Task D1: `build/support/commands.ron` classifies every script command of the
//! decomp, every `Native` one has a lowering arm, and every movement fallback lowers.

use extract::codegen::{Class, MoveFallback, NATIVE_COMMANDS, Support};
use extract::{movement, names};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn source() -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("build/support/commands.ron");
    std::fs::read_to_string(p).unwrap()
}

fn support() -> Support {
    Support::parse(&source(), None).unwrap()
}

fn decomp() -> Option<PathBuf> {
    std::env::var_os("PKR_DECOMP").map(PathBuf::from)
}

/// `gotostd` and `SCR_OP_GOTO_STD` name the same command.
fn norm(s: &str) -> String {
    s.to_ascii_lowercase().replace('_', "")
}

#[test]
fn commands_ron_classifies_227_commands() {
    let s = support();
    assert_eq!(
        s.commands.len(),
        227,
        "one entry per gScriptCmdTable command"
    );
    assert!(s.specials.is_empty(), "specials live in specials.ron");
    let count = |f: fn(&Class) -> bool| s.commands.values().filter(|c| f(c)).count();
    let native = count(|c| *c == Class::Native);
    let ignore = count(|c| *c == Class::Ignore);
    let default = count(|c| matches!(c, Class::Default(_)));
    let unsupported = count(|c| *c == Class::Unsupported);
    let header = format!(
        "// Counts: {native} Native, {ignore} Ignore, {default} Default, {unsupported} Unsupported."
    );
    assert!(source().contains(&header), "the header must say {header:?}");
}

#[test]
fn every_native_command_has_a_lowering_arm() {
    let s = support();
    let native: BTreeSet<&str> = s
        .commands
        .iter()
        .filter(|(_, c)| **c == Class::Native)
        .map(|(k, _)| k.as_str())
        .collect();
    let arms: BTreeSet<&str> = NATIVE_COMMANDS.iter().copied().collect();
    let missing: Vec<_> = native.difference(&arms).collect();
    assert!(
        missing.is_empty(),
        "Native without a lowering arm: {missing:?}"
    );
    let unused: Vec<_> = arms.difference(&native).collect();
    assert!(
        unused.is_empty(),
        "lowering arms not marked Native: {unused:?}"
    );
}

#[test]
fn every_ignore_and_default_has_a_reason() {
    for line in source().lines() {
        let l = line.trim();
        if l.starts_with('"') && (l.contains(": Ignore,") || l.contains(": Default(")) {
            let reason = l.split_once("//").map(|(_, r)| r.trim());
            assert!(reason.is_some_and(|r| !r.is_empty()), "no reason: {l}");
        }
    }
}

#[test]
fn commands_are_the_decomp_command_table() {
    let Some(dir) = decomp() else {
        return;
    };
    let table = std::fs::read_to_string(dir.join("data/script_cmd_table.inc")).unwrap();
    let ops: BTreeSet<String> = table
        .lines()
        .filter_map(|l| l.trim().strip_prefix("script_cmd_table_entry SCR_OP_"))
        .map(|l| norm(l.split_whitespace().next().unwrap()))
        .collect();
    assert_eq!(ops.len(), 227);
    let s = support();
    let keys: BTreeSet<String> = s.commands.keys().map(|k| norm(k)).collect();
    assert_eq!(keys, ops);
    // Keys are the `asm/macros/event.inc` macro names, or for the commands only a macro's
    // optional form emits (`applymovement` with a map is `applymovementat`), the `SCR_OP_`.
    let macros = std::fs::read_to_string(dir.join("asm/macros/event.inc")).unwrap();
    let defined: BTreeSet<&str> = macros
        .lines()
        .filter_map(|l| l.trim().strip_prefix(".macro "))
        .filter_map(|l| l.split([' ', '\t', ',']).next())
        .collect();
    let missing: Vec<_> = s
        .commands
        .keys()
        .filter(|k| {
            !defined.contains(k.as_str())
                && !macros.contains(&format!("SCR_OP_{} ", k.to_ascii_uppercase()))
                && !macros.contains(&format!("SCR_OP_{}\n", k.to_ascii_uppercase()))
        })
        .collect();
    assert!(missing.is_empty(), "not event.inc macros: {missing:?}");
}

#[test]
fn movement_fallbacks_name_actions_without_steps() {
    let step = |name: &str| {
        let v = names::MOVEMENT_ACTIONS
            .value(name)
            .unwrap_or_else(|| panic!("{name} is no MOVEMENT_ACTION_*"));
        movement::step(v as u8)
    };
    let s = support();
    assert!(!s.movements.is_empty());
    for (action, fb) in &s.movements {
        assert_eq!(
            step(action),
            None,
            "{action} has a Step: no fallback needed"
        );
        if let MoveFallback::As(list) = fb {
            assert!(!list.is_empty(), "{action}");
            for t in list {
                assert!(step(t).is_some(), "{action} -> {t}, which has no Step");
            }
        }
    }
    // Every action with no Step (but the list terminators) has a fallback.
    for (v, name) in names::MOVEMENT_ACTIONS.iter() {
        if v < 0xFE && movement::step(v as u8).is_none() {
            assert!(s.movements.contains_key(name), "{name} has no fallback");
        }
    }
}
