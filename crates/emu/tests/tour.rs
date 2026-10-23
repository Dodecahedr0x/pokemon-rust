//! The map tour (Milestone 8 Task M): the tour ROM (`game-gba --features tour`,
//! `target/game-tour.gba`, built by `cargo xtask parity --long`) driven in mGBA through every
//! map of `tour_logic::tour`, with a native World mirroring it in lockstep.
//!
//! The ROM boots a new game after the starter; each L+R warps it to the next map
//! (`tour_logic::Tour`). The test sends the combo when the native mirror is ready for a warp
//! (field, no transition; in a battle it presses A every other frame until the field is back),
//! then idles [`IDLE`] frames. On gated maps it first walks (`Walk`): [`WALK_STEPS`] steps
//! out and back in each direction whose cells are free (collision 0, no object template), once
//! the field is idle after the warp. The mirror steps with the inputs the ROM consumed (its input
//! ring), applies the same warp, and its hash must equal the ROM's at every sample.
//!
//! Per map (from the frame its warp was accepted to the next one), the ROM must show:
//! `late == 0`; every black-screen overrun (`late_black`) within `DISPLAY_LAG` steps after a
//! bulk step of the mirror; `loop_lines_max` ≤ [`LOOP_LINES_MAX`] (the shell resets its maxima
//! at each warp); no panic. Gated maps (the Act 1 maps and one per tileset family) fail the
//! test unless listed in [`KNOWN_OVER`] with a cause; the other maps (the stretch) are
//! reported. At the end, the memory gates (`emu::check_memory`). The table goes to stdout and
//! `target/tour/report.txt`.

extern crate alloc;

#[path = "../../game-gba/src/tour_logic.rs"]
mod tour_logic;

use emu::{DebugSnapshot, Emu, check_memory};
use engine::data::{GameData, MapId};
use engine::debug::input_slot;
use engine::{Buttons, World};
use game_native::scenario::DISPLAY_LAG;
use std::fmt::Write as _;
use std::path::Path;
use tour_logic::{ACT1_MAPS, Tour, tour};

/// Frames the tour stays on a map, from the step that accepted its warp.
const IDLE: u32 = 240;
/// Steps out (and back) in each free direction on a gated map.
const WALK_STEPS: i16 = 4;
/// Frames one leg of the walk may take before it is given up (a ledge, water, an NPC in the
/// way): an outward leg turns back, a leg back ends the walk.
const LEG_MAX: u32 = 90;
/// Frames after the warp by which the walk must have started (map scripts may hold the field).
const WALK_START_MAX: u32 = IDLE;
/// Frames the controller waits for the mirror to be ready for a warp (a battle to end).
const READY_MAX: u32 = 8000;
/// The shell loop's gate, as `parity.rs`'s `LOOP_LINES_MAX`.
const LOOP_LINES_MAX: u32 = 217;
/// A gated map over budget for a recorded cause.
struct KnownOver {
    map: &'static str,
    /// The frames (from the warp) of the late applies the cause explains; any other overrun
    /// (another late apply, a stray black overrun, the loop over its gate) fails the test.
    late_at: &'static [u32],
    cause: &'static str,
}

/// Gated maps over budget, with the cause. An entry whose map is back within budget fails the
/// test (remove it).
const KNOWN_OVER: &[KnownOver] = &[KnownOver {
    map: "EverGrandeCity_SidneysRoom",
    late_at: &[138],
    cause: "the door closing behind the player (`PokemonLeague_EliteFour_EventScript_\
            CloseDoor`) ends in `special DrawWholeMapView`, a whole-view redraw on a visible \
            frame; the Elite Four rooms are off the Act 1 route",
}];

/// After the starter, as the tour ROM (`game-gba --features tour`); Silice's new game there.
fn game() -> content::Emerald {
    game_native::scenario::after_starter().unwrap_or_else(content::Emerald::for_shells)
}

/// The decomp folder names `content/build/support/act1_maps.ron` lists.
fn act1_ron() -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../content/build/support/act1_maps.ron");
    let text = std::fs::read_to_string(&path).unwrap();
    text.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .filter_map(|l| l.split('"').nth(1).map(str::to_string))
        .collect()
}

#[test]
fn the_act1_list_matches_the_content_build() {
    assert_eq!(ACT1_MAPS.to_vec(), act1_ron());
}

#[test]
fn the_tour_visits_every_map_once_gated_first() {
    let g = game();
    let maps = &content::data::MAP_IDS;
    let (list, gated) = tour(&g, maps);
    assert_eq!(list.len(), maps.len());
    let mut sorted = list.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), maps.len(), "a map appears twice");
    assert!(gated <= list.len());
    // The gated part covers every tileset family.
    let family = |m: MapId| {
        let d = g.map(m);
        (d.primary as *const _, d.secondary as *const _)
    };
    for &m in maps.iter() {
        assert!(list[..gated].iter().any(|&t| family(t) == family(m)));
    }
    if content::FLAVOR == content::Flavor::Emerald {
        let names: Vec<&str> = list[..ACT1_MAPS.len()]
            .iter()
            .map(|&m| g.map(m).name)
            .collect();
        assert_eq!(names, ACT1_MAPS);
    }
}

/// `tour_logic::landing`'s search from the centre finds the cell a full scan in row order
/// would (`cargo xtask sweep`'s start cell).
#[test]
fn landing_is_the_nearest_free_cell_to_the_centre() {
    let g = game();
    for &m in content::data::MAP_IDS.iter() {
        let d = g.map(m);
        let want = if d.warps.is_empty() {
            let (w, h) = (i32::from(d.width), i32::from(d.height));
            let (cx, cy) = (w / 2, h / 2);
            let free = |x: i32, y: i32| {
                engine::data::block_collision(d.blocks[(y * w + x) as usize]) == 0
                    && !d
                        .objects
                        .iter()
                        .any(|o| (i32::from(o.x), i32::from(o.y)) == (x, y))
            };
            let (x, y) = (0..h)
                .flat_map(|y| (0..w).map(move |x| (x, y)))
                .filter(|&(x, y)| free(x, y))
                .min_by_key(|&(x, y)| (x - cx).abs() + (y - cy).abs())
                .unwrap_or((cx, cy));
            (None, x as i16, y as i16)
        } else {
            (Some(0), -1, -1)
        };
        assert_eq!(tour_logic::landing(&g, m), want, "{}", d.name);
    }
}

/// One map of the tour, as the ROM ran it.
struct Stop {
    map: MapId,
    gated: bool,
    /// The engine frame after the step that accepted the warp.
    from: u32,
    /// Where the mirror stood at the end of the stop.
    landed: MapId,
    battle: bool,
    late: u32,
    /// Frames (from the warp) at which `late` grew.
    late_at: Vec<u32>,
    late_black: u32,
    /// Black-screen overruns that followed no bulk step.
    stray_black: Vec<u32>,
    /// The ROM's maxima for the stop (the last sample before the next warp).
    hash_lines: u32,
    loop_lines: u32,
    /// The walk's (legs walked, legs planned), gated stops only.
    walk: (usize, usize),
}

impl Stop {
    fn over(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.late != 0 {
            out.push(format!("late {} (at +{:?})", self.late, self.late_at));
        }
        if !self.stray_black.is_empty() {
            out.push(format!(
                "black overruns after no bulk step at {:?}",
                self.stray_black
            ));
        }
        if self.loop_lines > LOOP_LINES_MAX {
            out.push(format!("loop {} lines", self.loop_lines));
        }
        out
    }
}

/// The walk on a gated stop: legs out to [`WALK_STEPS`] cells and back, per free direction.
#[derive(Default)]
struct Walk {
    plan: Option<Plan>,
    done: bool,
}

struct Plan {
    map: MapId,
    /// (held key, target cell) per leg.
    legs: Vec<(Buttons, (i16, i16))>,
    leg: usize,
    /// Legs that reached their target.
    walked: usize,
    /// The frame the current leg began.
    since: u32,
}

impl Walk {
    /// (legs that reached their target, legs planned).
    fn progress(&self) -> (usize, usize) {
        self.plan
            .as_ref()
            .map_or((0, 0), |p| (p.walked, p.legs.len()))
    }

    /// The keys for the ROM's next step while walking; `None` once the walk is over (done,
    /// given up, or interrupted: another map, a script, a battle, a transition).
    fn keys(&mut self, g: &impl GameData, world: &World, f: u32, from: u32) -> Option<Buttons> {
        if self.done {
            return None;
        }
        let idle = world.field_active() && !world.transition_active() && !world.script_running();
        let p = world.objects().player();
        let at = (p.x, p.y);
        let still = p.is_settled() && p.prev == at;
        let Some(plan) = &mut self.plan else {
            if f > from + WALK_START_MAX {
                self.done = true;
                return None;
            }
            if !(idle && still) {
                return Some(Buttons::NONE);
            }
            let map = world.map();
            let m = g.map(map);
            let (w, h) = (i32::from(m.width), i32::from(m.height));
            let free = |x: i16, y: i16| {
                let (x, y) = (i32::from(x), i32::from(y));
                (0..w).contains(&x)
                    && (0..h).contains(&y)
                    && engine::data::block_collision(m.blocks[(y * w + x) as usize]) == 0
                    && !(m.objects.iter()).any(|o| (i32::from(o.x), i32::from(o.y)) == (x, y))
            };
            let mut legs = Vec::new();
            for (out, back, (dx, dy)) in [
                (Buttons::UP, Buttons::DOWN, (0, -1)),
                (Buttons::DOWN, Buttons::UP, (0, 1)),
                (Buttons::LEFT, Buttons::RIGHT, (-1, 0)),
                (Buttons::RIGHT, Buttons::LEFT, (1, 0)),
            ] {
                if (1..=WALK_STEPS).all(|i| free(at.0 + dx * i, at.1 + dy * i)) {
                    legs.push((out, (at.0 + dx * WALK_STEPS, at.1 + dy * WALK_STEPS)));
                    legs.push((back, at));
                }
            }
            if legs.is_empty() {
                self.done = true;
                return None;
            }
            self.plan = Some(Plan {
                map,
                legs,
                leg: 0,
                walked: 0,
                since: f,
            });
            return Some(Buttons::NONE);
        };
        if world.map() != plan.map || !idle {
            self.done = true;
            return None;
        }
        let (key, target) = plan.legs[plan.leg];
        let arrived = at == target && still;
        let timed_out = f - plan.since > LEG_MAX;
        if arrived || timed_out {
            if timed_out && !arrived && plan.leg % 2 == 1 {
                // Not back at the origin: the next legs would start elsewhere.
                self.done = true;
                return None;
            }
            plan.walked += usize::from(arrived);
            plan.leg += 1;
            plan.since = f;
            if plan.leg == plan.legs.len() {
                self.done = true;
                return None;
            }
            return Some(plan.legs[plan.leg].0);
        }
        Some(key)
    }
}

/// What the controller does next.
enum Phase {
    /// Waiting for the mirror to accept a warp to stop `i`.
    Ready { since: u32 },
    /// On stop `i` since its warp; idles until `IDLE` frames passed.
    Idle,
}

#[test]
#[ignore = "needs target/game-tour.gba; run `cargo xtask parity --long`"]
fn every_tour_map_fits_the_frame() {
    let rom = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/game-tour.gba");
    let g = game();
    let maps = &content::data::MAP_IDS;
    let (list, gated) = tour(&g, maps);
    let mut mirror_tour = Tour::new(&g, maps);
    assert_eq!(mirror_tour.maps, list);
    let mut world = World::new(&g);
    let _ = world.initial_frame();
    let mut bulk: Vec<u32> = Vec::new();

    let mut emu = Emu::open(&rom).unwrap();
    let (mut snap, _) = emu.boot_published(240).unwrap();
    assert_eq!(snap.frame, 0, "the tour ROM restored a save");
    assert_eq!(snap.hash, world.hash(), "boot hash differs from native");

    // `PKR_TOUR_STOPS=n` ends the tour after its first n maps (the gated part is
    // `the_tour_visits_every_map_once_gated_first`'s count).
    let limit = std::env::var("PKR_TOUR_STOPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(usize::MAX);
    let mut stops: Vec<Stop> = Vec::new();
    let mut phase = Phase::Ready { since: 0 };
    let mut walk = Walk::default();
    let mut prev = Buttons::NONE;
    let mut last: Option<DebugSnapshot> = None;
    let (mut late, mut late_black) = (0u32, 0u32);
    loop {
        let f = snap.frame;
        // The controller: keys for the ROM's next step, from the mirror at the ROM's frame.
        let keys = match phase {
            Phase::Idle => {
                let (from, gated) = stops.last().map_or((0, false), |s| (s.from, s.gated));
                let walking = if gated {
                    walk.keys(&g, &world, f, from)
                } else {
                    None
                };
                if let Some(k) = walking {
                    k
                } else if f >= from + IDLE {
                    if stops.len() == list.len().min(limit) {
                        break;
                    }
                    phase = Phase::Ready { since: f };
                    Buttons::NONE
                } else {
                    Buttons::NONE
                }
            }
            Phase::Ready { .. } => Buttons::NONE,
        };
        let keys = match phase {
            Phase::Ready { since } => {
                assert!(
                    f < since + READY_MAX,
                    "stop {} ({}): not ready for a warp after {READY_MAX} frames",
                    stops.len(),
                    g.map(list[stops.len()]).name
                );
                let ready = world.field_active() && !world.transition_active();
                const LR: Buttons = Buttons(Buttons::L.0 | Buttons::R.0);
                if ready && !prev.contains(LR) {
                    LR
                } else if world.in_battle() && f % 2 == 0 {
                    Buttons::A
                } else {
                    Buttons::NONE
                }
            }
            Phase::Idle => keys,
        };
        emu.run_frame(keys).unwrap();
        let next = emu.debug().expect("debug region vanished");
        if next.panicked != 0 {
            panic!(
                "the ROM panicked (line {}) at engine frame {}, map {:#06x} ({}), last script {:?}",
                next.panicked,
                next.frame,
                next.map.0,
                g.map(next.map).name,
                next.last_script
            );
        }
        assert!(
            next.frame >= f,
            "the ROM went back from {f} to {}",
            next.frame
        );
        assert!(
            next.frame - f <= engine::debug::INPUT_RING as u32,
            "the ROM ran more steps than the input ring holds"
        );
        // The mirror replays what the ROM consumed, warps as the shell does.
        for n in f + 1..=next.frame {
            let k = Buttons(next.inputs[input_slot(n)]);
            let out = world.step(&g, k);
            if out.bulk {
                bulk.push(n);
            }
            let warped = mirror_tour.on_keys(&g, &mut world, prev, k);
            prev = k;
            if warped {
                if let Some(s) = stops.last_mut() {
                    // The last sample before this warp holds the previous stop's maxima.
                    if let Some(l) = &last {
                        s.hash_lines = l.hash_lines_max;
                        s.loop_lines = l.loop_lines_max;
                    }
                }
                let i = stops.len();
                stops.push(Stop {
                    map: list[i],
                    gated: i < gated,
                    from: n,
                    landed: list[i],
                    battle: false,
                    late: 0,
                    late_at: Vec::new(),
                    late_black: 0,
                    stray_black: Vec::new(),
                    hash_lines: 0,
                    loop_lines: 0,
                    walk: (0, 0),
                });
                phase = Phase::Idle;
                walk = Walk::default();
            }
            if let Some(s) = stops.last_mut() {
                s.walk = walk.progress();
                s.battle |= world.in_battle();
                s.landed = world.map();
            }
        }
        if next.frame > f {
            assert_eq!(
                next.hash,
                world.hash(),
                "World hash diverged at frame {} (stop {})",
                next.frame,
                stops.len()
            );
        }
        if let Some(s) = stops.last_mut() {
            if next.late > late {
                s.late += next.late - late;
                s.late_at.push(next.frame - s.from);
            }
            for _ in late_black..next.late_black {
                s.late_black += 1;
                let explained = bulk
                    .iter()
                    .rev()
                    .take_while(|&&b| b + DISPLAY_LAG >= next.frame)
                    .any(|&b| b < next.frame);
                if !explained {
                    s.stray_black.push(next.frame);
                }
            }
        }
        late = next.late;
        late_black = next.late_black;
        last = Some(next.clone());
        snap = next;
    }
    if let (Some(s), Some(l)) = (stops.last_mut(), &last) {
        s.hash_lines = l.hash_lines_max;
        s.loop_lines = l.loop_lines_max;
    }
    // The memory marks once the watch scanned everything the last step wrote.
    let settled = emu.settle(&snap, emu::SETTLE_FRAMES).unwrap();
    snap.heap_max = settled.heap_max;
    snap.stack_min = settled.stack_min;

    let mut report = String::new();
    let _ = writeln!(
        report,
        "{} maps ({gated} gated), {} engine frames, {} bulk steps",
        stops.len(),
        snap.frame,
        bulk.len()
    );
    let _ = writeln!(
        report,
        "{:<4} {:<5} {:<44} {:>5} {:>5} {:>4} {:>6}  notes",
        "#", "gate", "map", "hash", "loop", "late", "black"
    );
    let mut failures = Vec::new();
    let mut stretch_over = 0;
    for (i, s) in stops.iter().enumerate() {
        let name = g.map(s.map).name;
        let over = s.over();
        let mut notes = over.join("; ");
        if s.battle {
            notes.push_str(if notes.is_empty() {
                "battle"
            } else {
                "; battle"
            });
        }
        if s.gated {
            let _ = write!(
                notes,
                "{}walk {}/{} legs",
                if notes.is_empty() { "" } else { "; " },
                s.walk.0,
                s.walk.1
            );
        }
        if s.landed != s.map {
            let _ = write!(
                notes,
                "{}ended on {}",
                if notes.is_empty() { "" } else { "; " },
                g.map(s.landed).name
            );
        }
        let _ = writeln!(
            report,
            "{i:<4} {:<5} {name:<44} {:>5} {:>5} {:>4} {:>6}  {notes}",
            if s.gated { "yes" } else { "" },
            s.hash_lines,
            s.loop_lines,
            s.late,
            s.late_black
        );
        let known = KNOWN_OVER.iter().find(|k| k.map == name);
        if s.gated {
            match known {
                None if !over.is_empty() => {
                    failures.push(format!("{name}: {}", over.join("; ")));
                }
                None => {}
                Some(_) if over.is_empty() => {
                    failures.push(format!("{name}: within budget, remove it from KNOWN_OVER"))
                }
                Some(k) => {
                    let explained = s.late_at == k.late_at
                        && s.stray_black.is_empty()
                        && s.loop_lines <= LOOP_LINES_MAX;
                    if !explained {
                        failures.push(format!(
                            "{name}: {} (KNOWN_OVER explains only late at +{:?}: {})",
                            over.join("; "),
                            k.late_at,
                            k.cause
                        ));
                    }
                }
            }
        } else if !over.is_empty() {
            stretch_over += 1;
        }
    }
    let worst = |gate: bool| {
        stops
            .iter()
            .filter(|s| s.gated == gate)
            .map(|s| s.loop_lines)
            .max()
            .unwrap_or(0)
    };
    let walked = |f: fn(&(usize, usize)) -> usize| stops.iter().map(|s| f(&s.walk)).sum::<usize>();
    let _ = writeln!(
        report,
        "walk on gated maps: {} of {} legs planned reached their target, on {} maps",
        walked(|w| w.0),
        walked(|w| w.1),
        stops.iter().filter(|s| s.walk.0 > 0).count()
    );
    let _ = writeln!(
        report,
        "worst loop: gated {} lines, stretch {} lines; stretch maps over budget: {stretch_over}; \
         heap_max {} B, stack untouched {} B, reports {}, events dropped {}",
        worst(true),
        worst(false),
        snap.heap_max,
        snap.stack_min,
        snap.reports,
        snap.events_dropped
    );
    print!("{report}");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tour");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("report.txt"), &report).unwrap();
    assert!(failures.is_empty(), "gated maps over budget: {failures:#?}");
    check_memory("tour", &snap).unwrap();
}
