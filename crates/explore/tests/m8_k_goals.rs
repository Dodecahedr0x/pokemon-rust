//! Milestone 8 Task K: the Act 1 goals are state predicates on decomp constants. The constants
//! are checked against the decomp's headers (`PKR_DECOMP`), the predicates on edited states.
//! Emerald's lists only: not built on Silice (`cfg(pkr_silice)`, SIL1 Task D).
#![cfg(not(pkr_silice))]

use content::Emerald;
use engine::checkpoint::WorldSnapshot;
use engine::data::{FlagId, MapId, VarId};
use explore::goals::{self, Act};
use game_native::Session;
use std::collections::BTreeMap;
use std::path::Path;

/// `#define NAME value` of `headers`, evaluated: literals, names and `+`/`-` (enough for
/// `flags.h`, `vars.h` and `opponents.h`).
fn defines(headers: &[&Path]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for h in headers {
        let text = std::fs::read_to_string(h).unwrap_or_else(|e| panic!("{}: {e}", h.display()));
        for line in text.lines() {
            let Some(rest) = line.trim().strip_prefix("#define ") else {
                continue;
            };
            let rest = rest.split("//").next().unwrap();
            let mut parts = rest.splitn(2, char::is_whitespace);
            let (Some(name), Some(value)) = (parts.next(), parts.next()) else {
                continue;
            };
            out.insert(name.to_string(), value.trim().to_string());
        }
    }
    out
}

fn eval(defs: &BTreeMap<String, String>, name: &str, depth: u32) -> Option<i64> {
    assert!(depth < 16, "{name}: recursive");
    let expr = defs.get(name)?.replace(['(', ')'], " ");
    let expr = expr.replace('+', " + ").replace('-', " - ");
    let mut total = 0i64;
    let mut sign = 1i64;
    for tok in expr.split_whitespace() {
        match tok {
            "+" => sign = 1,
            "-" => sign = -1,
            t => {
                let v = if let Some(hex) = t.strip_prefix("0x") {
                    i64::from_str_radix(hex, 16).ok()?
                } else if let Ok(v) = t.parse() {
                    v
                } else {
                    eval(defs, t, depth + 1)?
                };
                total += sign * v;
            }
        }
    }
    Some(total)
}

/// `MAP_*` of `data/maps/map_groups.json`: group number << 8 | index in the group.
fn map_constant(decomp: &Path, constant: &str) -> Option<u16> {
    let json = std::fs::read_to_string(decomp.join("data/maps/map_groups.json")).unwrap();
    let list = |key: &str| -> Vec<String> {
        let start = json.find(&format!("\"{key}\": [")).expect(key);
        let body = &json[start..];
        let body = &body[body.find('[').unwrap() + 1..body.find(']').unwrap()];
        body.split(',')
            .map(|s| s.trim().trim_matches('"').to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };
    // `PetalburgCity` -> `MAP_PETALBURG_CITY` (as `map_groups.h` is generated).
    let upper = |name: &str| {
        let mut s = String::from("MAP_");
        let mut prev_lower = false;
        for c in name.chars() {
            if c.is_ascii_uppercase() && prev_lower {
                s.push('_');
            }
            prev_lower = c.is_ascii_lowercase();
            s.push(c.to_ascii_uppercase());
        }
        s
    };
    for (g, group) in list("group_order").iter().enumerate() {
        if let Some(i) = list(group).iter().position(|m| upper(m) == constant) {
            return Some((g as u16) << 8 | i as u16);
        }
    }
    None
}

#[test]
fn act_constants_match_the_decomp_headers() {
    let Some(decomp) = std::env::var_os("PKR_DECOMP") else {
        eprintln!("skipped: PKR_DECOMP is not set");
        return;
    };
    let decomp = Path::new(&decomp);
    let inc = decomp.join("include/constants");
    let defs = defines(&[
        &inc.join("flags.h"),
        &inc.join("vars.h"),
        &inc.join("opponents.h"),
    ]);
    let constants = goals::constants();
    assert_eq!(constants.len(), 14, "{constants:?}");
    for (name, value) in constants {
        let want = if name.starts_with("MAP_") {
            map_constant(decomp, name).map(i64::from)
        } else {
            eval(&defs, name, 0)
        };
        assert_eq!(want, Some(i64::from(value)), "{name}");
    }
}

fn settled(game: Emerald) -> WorldSnapshot {
    let mut s = Session::new(game);
    game_native::scenario::to_field(&mut s).unwrap_or_else(|e| panic!("{e}"));
    s.world().snapshot(s.game()).unwrap()
}

fn set_flag(s: &mut WorldSnapshot, f: u16) {
    if let Err(i) = s.vars.flags.binary_search(&FlagId(f)) {
        s.vars.flags.insert(i, FlagId(f));
    }
}

fn set_var(s: &mut WorldSnapshot, v: u16, value: u16) {
    match s.vars.vars.binary_search_by_key(&VarId(v), |&(v, _)| v) {
        Ok(i) => s.vars.vars[i].1 = value,
        Err(i) => s.vars.vars.insert(i, (VarId(v), value)),
    }
}

#[test]
fn each_act1_goal_is_its_constant() {
    if content::STUB {
        return;
    }
    let c: BTreeMap<&str, u16> = goals::constants().into_iter().collect();
    let mut s = settled(Emerald::for_shells());
    let all = goals::goals_of(Act::Act1);
    // Satisfy goal i (and keep the earlier ones): progress from i reaches i + 1.
    let mut starter = settled(Emerald::after_starter()).party;
    for (i, g) in all.iter().enumerate() {
        assert!(!(g.reached)(&s), "{} before its edit", g.name);
        match g.name {
            "rescue_started" => set_var(&mut s, c["VAR_ROUTE101_STATE"], 1),
            "starter" => s.party = std::mem::take(&mut starter),
            "birch_rescued" => set_flag(&mut s, c["FLAG_RESCUED_BIRCH"]),
            "lab" => set_var(&mut s, c["VAR_BIRCH_LAB_STATE"], 3),
            "oldale" => set_flag(&mut s, c["FLAG_VISITED_OLDALE_TOWN"]),
            "route103" => s.field.map = MapId(c["MAP_ROUTE103"]),
            "may_defeated" => set_flag(&mut s, c["FLAG_DEFEATED_RIVAL_ROUTE103"]),
            "pokedex" => set_flag(&mut s, c["FLAG_RECEIVED_POKEDEX_FROM_BIRCH"]),
            "petalburg" => set_flag(&mut s, c["FLAG_VISITED_PETALBURG_CITY"]),
            "wally_tutorial" => set_var(&mut s, c["VAR_PETALBURG_GYM_STATE"], 2),
            "route104" => s.field.map = MapId(c["MAP_ROUTE104"]),
            "woods_grunt_defeated" => {
                set_flag(&mut s, 0x500 + c["TRAINER_GRUNT_PETALBURG_WOODS"]);
            }
            "rustboro" => set_flag(&mut s, c["FLAG_VISITED_RUSTBORO_CITY"]),
            "roxanne_defeated" => set_flag(&mut s, c["FLAG_DEFEATED_RUSTBORO_GYM"]),
            "badge1" => set_flag(&mut s, c["FLAG_BADGE01_GET"]),
            n => panic!("untested goal {n}"),
        }
        assert!((g.reached)(&s), "{} after its edit", g.name);
        // The route goals hold only on their map: progress is read from goal i on.
        assert_eq!(
            goals::progress(Act::Act1, i as u32, &s),
            i as u32 + 1,
            "{}",
            g.name
        );
    }
    // The slice stops at its seventh goal.
    assert_eq!(goals::progress(Act::Slice, 7, &s), 7);
    // `wally_tutorial` holds for every later gym state (2-6: 0-4 badges), not before.
    let w = all.iter().find(|g| g.name == "wally_tutorial").unwrap();
    for (v, want) in [(0, false), (1, false), (2, true), (6, true)] {
        set_var(&mut s, c["VAR_PETALBURG_GYM_STATE"], v);
        assert_eq!((w.reached)(&s), want, "gym state {v}");
    }
}
