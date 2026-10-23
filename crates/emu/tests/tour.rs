//! The map tour (Milestone 8 Task M): the tour ROM (`game-gba --features tour`,
//! `target/game-tour.gba`, built by `cargo xtask parity --long`) driven in mGBA through every
//! map of `tour_logic::tour`, with a native World mirroring it in lockstep.
//!
//! The ROM boots a new game after the starter; each L+R+START warps it to the next map
//! (`tour_logic::Tour`). The test sends the combo when the native mirror is ready for a warp
//! (field, no transition; in a battle it presses A every other frame until the field is back),
//! then idles [`IDLE`] frames. The mirror steps with the inputs the ROM consumed (its input
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
/// Frames the controller waits for the mirror to be ready for a warp (a battle to end).
const READY_MAX: u32 = 8000;
/// The shell loop's gate, as `parity.rs`'s `LOOP_LINES_MAX`.
const LOOP_LINES_MAX: u32 = 217;
/// Gated maps over budget, with the cause.
const KNOWN_OVER: &[(&str, &str)] = &[(
    "EverGrandeCity_SidneysRoom",
    "one late apply: the door closing behind the player (`PokemonLeague_EliteFour_EventScript_\
     CloseDoor`) ends in `special DrawWholeMapView`, a whole-view redraw on a visible frame; \
     the Elite Four rooms are off the Act 1 route",
)];

fn game() -> content::Emerald {
    content::Emerald::after_starter()
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
    if !content::STUB {
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
    let mut prev = Buttons::NONE;
    let mut last: Option<DebugSnapshot> = None;
    let (mut late, mut late_black) = (0u32, 0u32);
    loop {
        let f = snap.frame;
        // The controller: keys for the ROM's next step, from the mirror at the ROM's frame.
        let keys = match phase {
            Phase::Idle => {
                let from = stops.last().map_or(0, |s| s.from);
                if f >= from + IDLE {
                    if stops.len() == list.len().min(limit) {
                        break;
                    }
                    phase = Phase::Ready { since: f };
                }
                Buttons::NONE
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
                if ready && !prev.contains(Buttons::START) {
                    Buttons(Buttons::L.0 | Buttons::R.0 | Buttons::START.0)
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
                });
                phase = Phase::Idle;
            }
            if let Some(s) = stops.last_mut() {
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
        if !over.is_empty() {
            if s.gated {
                if !KNOWN_OVER.iter().any(|(n, _)| *n == name) {
                    failures.push(format!("{name}: {}", over.join("; ")));
                }
            } else {
                stretch_over += 1;
            }
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
