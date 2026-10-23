//! Milestone 9 Wave 0 Step 0: the route-usage scan. It replays the parity route
//! (`scenario::cached_route`) and every node of the committed checkpoint tree of the current
//! flavor (`checkpoints/<flavor>/nodes`, each trace from its parent's replayed state), and counts
//! what each M9 task would change on them: START/SELECT pressed on the idle field, the
//! `pokemart`, white-fade and other reports executed, the scripts started (for the specials the
//! decomp reaches from them), field steps with a poisoned or Wurmple-line party, level
//! evolutions due after a battle, and trainer battles won with two or more usable mons (Gen3's
//! shift prompt). The table goes into the plan's "Wave 0 as built"; re-run it whenever the tree
//! gains nodes (M8 Task P's Act 1 trace).
//!
//! Slow (it records the route), so ignored by default:
//! `cargo test -p game-native --release --test m9_route_scan -- --ignored --nocapture`.

use engine::data::{EvoMethod, GameData};
use engine::input::Buttons;
use engine::mon::Status;
use engine::script::Report;
use engine::world::{Event, World};
use game_native::Session;
use game_native::scenario::cached_route;
use game_native::store::Store;
use game_native::trace::Parent;
use std::collections::{BTreeMap, BTreeSet};

/// Gen3 `SPECIES_WURMPLE` ..= `SPECIES_DUSTOX` (national order 265..=269 sits at 290..=294 in
/// the internal order).
const WURMPLE_LINE: std::ops::RangeInclusive<u16> = 290..=294;

#[derive(Default, Debug)]
struct Counts {
    frames: u64,
    start_on_field: u32,
    select_on_field: u32,
    reports: BTreeMap<String, u32>,
    scripts: BTreeSet<u16>,
    battles: u32,
    trainer_wins: u32,
    trainer_wins_two_usable: u32,
    /// Mons still at a level evolution after a won battle (none while the silent evolution
    /// runs: it already evolved them; the scene's own count is `evolved`).
    evolutions_due: u32,
    /// Level evolutions at a won battle's end (`Degraded("evolution scene")`, one per mon).
    evolved: u32,
    steps: u32,
    poisoned_steps: u32,
    wurmple_steps: u32,
    screens_opened: u32,
}

/// One replay's observer: call [`Scan::before`] before each step, [`Scan::after`] after it.
struct Scan<'g, G: GameData> {
    game: &'g G,
    c: Counts,
    prev_keys: Buttons,
    idle: bool,
    pos: Option<(u16, (i16, i16))>,
    trainer: bool,
}

impl<'g, G: GameData> Scan<'g, G> {
    fn new(game: &'g G) -> Self {
        Self {
            game,
            c: Counts::default(),
            prev_keys: Buttons::NONE,
            idle: false,
            pos: None,
            trainer: false,
        }
    }

    fn before(&mut self, w: &World) {
        self.idle = w.field_active()
            && !w.script_running()
            && !w.transition_active()
            && !w.in_battle()
            && w.screens_open() == 0;
    }

    fn after(&mut self, w: &World, keys: Buttons) {
        let c = &mut self.c;
        c.frames += 1;
        let pressed = Buttons(keys.0 & !self.prev_keys.0);
        self.prev_keys = keys;
        if self.idle && pressed.contains(Buttons::START) {
            c.start_on_field += 1;
        }
        if self.idle && pressed.contains(Buttons::SELECT) {
            c.select_on_field += 1;
        }
        for &e in w.events() {
            match e {
                Event::Report(r) => {
                    if r == Report::Degraded("evolution scene") {
                        c.evolved += 1;
                    }
                    let k = match r {
                        Report::Unsupported(n) => format!("Unsupported({n})"),
                        Report::Degraded(n) => format!("Degraded({n})"),
                    };
                    *c.reports.entry(k).or_default() += 1;
                }
                Event::ScriptStarted(id) => {
                    c.scripts.insert(id.0);
                }
                Event::BattleStarted => {
                    c.battles += 1;
                    self.trainer = w.battle().is_some_and(|(_, b, _)| {
                        matches!(b.setup().kind, engine::battle::BattleKind::Trainer { .. })
                    });
                }
                Event::BattleEnded(o) => {
                    let won = o == engine::battle::BattleOutcome::Won;
                    let usable = w.party().iter().filter(|m| m.hp > 0).count();
                    if won && self.trainer {
                        c.trainer_wins += 1;
                        if usable >= 2 {
                            c.trainer_wins_two_usable += 1;
                        }
                    }
                    for m in w.party().iter() {
                        let due = self.game.evolutions(m.species).iter().any(|e| {
                            matches!(
                                e.method,
                                EvoMethod::Level
                                    | EvoMethod::LevelAtkGtDef
                                    | EvoMethod::LevelAtkEqDef
                                    | EvoMethod::LevelAtkLtDef
                                    | EvoMethod::LevelSilcoon
                                    | EvoMethod::LevelCascoon
                                    | EvoMethod::LevelNinjask
                            ) && u16::from(m.level) >= e.param
                        });
                        c.evolutions_due += u32::from(due);
                    }
                }
                Event::ScreenOpened(_) => c.screens_opened += 1,
                _ => {}
            }
        }
        if w.field_active() {
            let p = (w.map().0, w.objects().player().pos());
            if self.pos.is_some_and(|q| q.0 == p.0 && q.1 != p.1) {
                c.steps += 1;
                let party = || w.party().iter();
                if party().any(|m| matches!(m.status, Status::Poison | Status::Toxic(_))) {
                    c.poisoned_steps += 1;
                }
                if party().any(|m| WURMPLE_LINE.contains(&m.species.0)) {
                    c.wurmple_steps += 1;
                }
            }
            self.pos = Some(p);
        }
    }
}

fn script_name(id: u16) -> &'static str {
    content::data::SCRIPT_NAMES
        .get(usize::from(id))
        .copied()
        .unwrap_or("?")
}

fn print(title: &str, c: &Counts) {
    println!("== {title}");
    println!(
        "frames {}  battles {}  screens opened {}",
        c.frames, c.battles, c.screens_opened
    );
    println!(
        "START on the idle field {}  SELECT {}",
        c.start_on_field, c.select_on_field
    );
    println!(
        "trainer wins {} (with 2+ usable mons: {})  level evolutions at a battle's end {} \
         (still due after it {})",
        c.trainer_wins, c.trainer_wins_two_usable, c.evolved, c.evolutions_due
    );
    println!(
        "field steps {} (poisoned party {}, Wurmple line {})",
        c.steps, c.poisoned_steps, c.wurmple_steps
    );
    for (r, n) in &c.reports {
        println!("report {r} x{n}");
    }
    let names: Vec<&str> = c.scripts.iter().map(|&i| script_name(i)).collect();
    println!("scripts started ({}): {}", names.len(), names.join(" "));
}

/// The decomp's script labels reachable from `roots` (every label named in a reachable body,
/// and fallthrough to the next label), and the M9-relevant commands they contain.
fn decomp_reach(roots: &BTreeSet<&str>) {
    let Some(decomp) = std::env::var_os("PKR_DECOMP").map(std::path::PathBuf::from) else {
        println!("PKR_DECOMP not set: no static reach");
        return;
    };
    let mut bodies: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut next: BTreeMap<String, String> = BTreeMap::new();
    let mut files = Vec::new();
    let mut stack = vec![decomp.join("data")];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "inc" || x == "s") {
                files.push(p);
            }
        }
    }
    for f in files {
        let Ok(text) = std::fs::read_to_string(&f) else {
            continue;
        };
        let mut cur: Option<String> = None;
        for line in text.lines() {
            let t = line.trim();
            if let Some(l) = t.strip_suffix("::").or_else(|| t.strip_suffix(':'))
                && !l.contains(char::is_whitespace)
                && !l.is_empty()
            {
                if let Some(c) = cur.replace(l.to_string()) {
                    next.insert(c, l.to_string());
                }
                bodies.entry(l.to_string()).or_default();
                continue;
            }
            if let Some(c) = &cur {
                bodies.get_mut(c).unwrap().push(t.to_string());
            }
        }
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut todo: Vec<String> = roots.iter().map(|s| s.to_string()).collect();
    while let Some(l) = todo.pop() {
        if !seen.insert(l.clone()) {
            continue;
        }
        let Some(body) = bodies.get(&l) else { continue };
        let mut ends = false;
        for line in body {
            for tok in line.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
                if bodies.contains_key(tok) && !seen.contains(tok) {
                    todo.push(tok.to_string());
                }
            }
            let op = line.split_whitespace().next().unwrap_or("");
            ends |= matches!(op, "end" | "return" | "goto" | "releaseall" | "step_end");
        }
        if !ends && let Some(n) = next.get(&l) {
            todo.push(n.clone());
        }
    }
    const NEEDLES: [&str; 8] = [
        "special SaveGame",
        "ChoosePartyMon",
        "ChooseMonForMoveTutor",
        "pokemart",
        "FADE_TO_WHITE",
        "FADE_FROM_WHITE",
        "PlayerPC",
        "ShowPokemonStorageSystemPC",
    ];
    println!(
        "== static reach: {} labels from the started scripts",
        seen.len()
    );
    for n in NEEDLES {
        let at: Vec<&str> = seen
            .iter()
            .filter(|l| {
                bodies
                    .get(*l)
                    .is_some_and(|b| b.iter().any(|line| line.contains(n)))
            })
            .map(String::as_str)
            .collect();
        let total = (bodies.values())
            .filter(|b| b.iter().any(|line| line.contains(n)))
            .count();
        println!("{n}: {} of {total} labels {:?}", at.len(), at);
    }
}

#[test]
#[ignore = "slow: records the route; run with --release -- --ignored --nocapture"]
fn m9_route_usage_scan() {
    let game = content::Emerald::for_shells();

    // The parity route.
    let rec = cached_route();
    let mut s = Session::new(content::Emerald::for_shells());
    let mut scan = Scan::new(&game);
    for f in 0..rec.frames() {
        let keys = rec.keys_at(f);
        scan.before(s.world());
        s.step(keys);
        scan.after(s.world(), keys);
    }
    print("parity route", &scan.c);
    let mut all: BTreeSet<u16> = scan.c.scripts.clone();

    // The committed tree, node by node from its parent's state.
    let store = Store::open(Store::default_root(content::FLAVOR)).unwrap();
    let nodes = store.topo_order().unwrap();
    let mut states = BTreeMap::new();
    let mut tree = Scan::new(&game);
    for n in &nodes {
        let mut w = match n.trace.parent {
            Parent::NewGame => World::new(&game),
            Parent::Checkpoint(p) => {
                let Some(cp) = states.get(&p) else {
                    println!("{}: parent {p:?} not replayed, skipped", n.meta.label);
                    continue;
                };
                World::restore(&game, cp).unwrap()
            }
        };
        tree.prev_keys = Buttons::NONE;
        tree.pos = None;
        for keys in n.trace.keys() {
            tree.before(&w);
            w.step(&game, keys);
            tree.after(&w, keys);
        }
        match w.checkpoint(&game) {
            Ok(cp) => {
                states.insert(n.meta.id, cp);
            }
            Err(e) => println!("{}: ends while {e:?}", n.meta.label),
        }
    }
    print(&format!("checkpoint tree ({} nodes)", nodes.len()), &tree.c);
    all.extend(&tree.c.scripts);

    let roots: BTreeSet<&str> = all.iter().map(|&i| script_name(i)).collect();
    decomp_reach(&roots);
}
