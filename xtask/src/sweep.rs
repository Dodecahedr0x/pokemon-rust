//! `cargo xtask sweep` (Milestone 8 Task L): every map of the content build entered and idled,
//! natively, in release.
//!
//! For every map (`content::data::MAP_IDS`), a new game ([`Emerald::after_starter`]: the
//! story flags of the Milestone 3 start, one Pokémon) starts there through [`StartAt`], on the
//! first warp's cell, else on the walkable cell nearest the centre. Then:
//! - [`IDLE`] frames with no input, with no panic; when the World is not capturable then, up
//!   to [`MASH`] frames mashing A (B while a yes/no question waits), until it is (Task N: so
//!   the maps with an entry cutscene or question reach the checks below). A map still not
//!   capturable is listed with its verdict, its last script and whether the map changed; the
//!   scripts that loop on a facility's challenge status are listed as known defects
//!   ([`KNOWN_LOOPS`]);
//! - when capturable: a checkpoint round trip (postcard within the flash budget, decoded equal),
//!   restored with an equal hash, then [`SHADOW`] frames stepped beside the live session with
//!   equal hashes;
//! - a render of the last frame (the PPU panics on a command or object it rejects).
//!
//! `Report` events are aggregated per op or special (sites are maps), and the build's
//! classification report (`content::REPORT`) is appended. Everything goes to
//! `target/sweep/report.txt`. Exit 1 on a panic, a round-trip or shadow mismatch, or dropped
//! events (`World::events_dropped`).
//!
//! `cargo xtask sweep --studio` runs the studio's `#[ignore]`d whole-game tests (map renders,
//! cells, overlays, encounters, the map editor's renders) in release. `cargo xtask sweep
//! --repro` builds `content` twice (native debug, its `$OUT_DIR` cleaned each time) and fails
//! on any byte of difference.

use anyhow::{Context, Result, bail, ensure};
use content::Emerald;
use engine::data::{GameData, MapId, NewGame, block_collision};
use engine::world::Event;
use engine::{Buttons, World};
use game_native::Session;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Idle frames per map.
const IDLE: u32 = 600;
/// Frames of mashing (A pressed every other frame, B instead while a yes/no question waits)
/// after the idle frames, for a map not capturable yet (Task N: cutscenes and questions).
const MASH: u32 = 3000;
/// Frames a restored session runs beside the live one.
const SHADOW: u32 = 60;
/// A script started at least this often over the idle frames spins (restarts every few
/// frames): a map script whose condition never changes.
const SPIN_STARTS: u32 = IDLE / 4;
/// The suffix of the facility scripts known to spin (known defects, not verdicts): a
/// facility lobby's `OnFrame` table runs `<Lobby>_EventScript_GetChallengeStatus` while the
/// challenge status (`CallFrontierUtilFunc`, unsupported, answers a default) says a challenge
/// is under way, so it restarts every frame. The rooms warp the player back to their lobby on
/// entry, so they spin there too.
const KNOWN_SPIN: &str = "_EventScript_GetChallengeStatus";
/// Set on the release re-run, so it does not re-run itself.
const INNER: &str = "PKR_SWEEP_INNER";

pub fn main(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        None => {}
        Some("--studio") => return studio(),
        Some("--repro") => return repro(),
        Some(a) => bail!("usage: cargo xtask sweep [--studio | --repro] (got {a:?})"),
    }
    if cfg!(debug_assertions) && std::env::var_os(INNER).is_none() {
        // The sweep runs the content scripts: in release.
        return crate::run(
            Command::new("cargo")
                .current_dir(crate::root())
                .env(INNER, "1")
                .args(["run", "--release", "-p", "xtask", "--", "sweep"]),
        );
    }
    sweep()
}

/// A game that starts its new game on `(map, x, y)`.
#[derive(Clone, Copy)]
struct StartAt {
    inner: Emerald,
    map: MapId,
    x: i16,
    y: i16,
}

impl GameData for StartAt {
    explore::forward_game_data!(inner; map has_map special_ids sprite player_sprite font window
        text ui_text behavior script build_id battle wild_encounters trainer battle_text
        battle_gfx heal_location m8);

    fn new_game(&self) -> NewGame {
        NewGame {
            map: self.map,
            x: self.x,
            y: self.y,
            ..self.inner.new_game()
        }
    }
}

/// The start cell of `map`: its first warp's, else the walkable cell (collision 0, no object
/// template) nearest its centre (ties in row order).
fn start_cell(game: &impl GameData, map: MapId) -> (i16, i16) {
    let m = game.map(map);
    if let Some(w) = m.warps.first() {
        return (w.x, w.y);
    }
    let (w, h) = (i32::from(m.width), i32::from(m.height));
    let (cx, cy) = (w / 2, h / 2);
    let occupied = |x: i32, y: i32| {
        m.objects
            .iter()
            .any(|o| (i32::from(o.x), i32::from(o.y)) == (x, y))
    };
    (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| block_collision(m.blocks[(y * w + x) as usize]) == 0 && !occupied(x, y))
        .min_by_key(|&(x, y)| (x - cx).abs() + (y - cy).abs())
        .map_or((cx as i16, cy as i16), |(x, y)| (x as i16, y as i16))
}

/// What one map did.
struct MapRun {
    map: MapId,
    name: &'static str,
    start: (i16, i16),
    /// A panic: its message, the frame and the last script started.
    panic: Option<String>,
    /// Why the World was not capturable after the idle frames.
    not_capturable: Option<String>,
    /// A checkpoint round trip, restore or shadow failure.
    round_trip: Option<String>,
    events_dropped: u32,
    /// Report name -> count over the run.
    reports: BTreeMap<String, u32>,
    maps_entered: u32,
    scripts_started: u32,
    /// Starts per script over the run.
    starts: BTreeMap<engine::data::ScriptId, u32>,
    /// Idle frames whose world hash differs from the previous frame's.
    changed: u32,
    /// Frames mashed until capturable (`None`: capturable after the idle frames).
    mashed: Option<u32>,
    /// The map at the end (after a warp from a cutscene or answer, not the start map).
    end_map: MapId,
    /// The last script started, by name.
    last_script: String,
}

thread_local! {
    static PANIC: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn script_name(id: Option<engine::data::ScriptId>) -> String {
    match id {
        Some(s) => content::data::SCRIPT_NAMES
            .get(usize::from(s.0))
            .map_or(format!("script {}", s.0), |n| n.to_string()),
        None => "no script".to_string(),
    }
}

/// Why a World is still busy after the idle and mashed frames, in words (the sweep's triage).
fn verdict(w: &World) -> &'static str {
    use engine::script::Request;
    match w.script_waiting() {
        Some(Request::WaitMessage) => "a message never finishes printing, even mashed",
        Some(Request::WaitButton | Request::YesNo | Request::Multichoice { .. }) => {
            "still waits for the player after mashing"
        }
        Some(_) => "script waits on the engine",
        None if w.script_running() => {
            "script loops without waiting (a facility's challenge state, unsupported specials)"
        }
        None => "not capturable",
    }
}

fn run_map(map: MapId) -> MapRun {
    let base = Emerald::after_starter();
    let start = start_cell(&base, map);
    let game = StartAt {
        inner: base,
        map,
        x: start.0,
        y: start.1,
    };
    let mut r = MapRun {
        map,
        name: base.map(map).name,
        start,
        panic: None,
        not_capturable: None,
        round_trip: None,
        events_dropped: 0,
        reports: BTreeMap::new(),
        maps_entered: 0,
        scripts_started: 0,
        starts: BTreeMap::new(),
        changed: 0,
        mashed: None,
        end_map: map,
        last_script: script_name(None),
    };
    let frame = std::cell::Cell::new(0u32);
    let last_script = std::cell::Cell::new(None);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let mut s = Session::new(game);
        let collect = |w: &World, r: &mut MapRun| {
            for e in w.events() {
                match e {
                    Event::Report(rep) => *r.reports.entry(format!("{rep:?}")).or_default() += 1,
                    Event::MapEntered(_) => r.maps_entered += 1,
                    Event::ScriptStarted(id) => {
                        r.scripts_started += 1;
                        *r.starts.entry(*id).or_default() += 1;
                    }
                    _ => {}
                }
            }
            r.events_dropped = w.events_dropped();
        };
        collect(s.world(), &mut r);
        let mut hash = s.world().hash();
        for n in 1..=IDLE {
            frame.set(n);
            s.step(Buttons::default());
            let h = s.world().hash();
            r.changed += u32::from(h != hash);
            hash = h;
            last_script.set(s.world().last_script());
            collect(s.world(), &mut r);
        }
        if s.world().capturable().is_err() {
            let mut n = 0;
            while n < MASH && s.world().capturable().is_err() {
                n += 1;
                frame.set(IDLE + n);
                let key = if n % 2 == 0 {
                    Buttons::default()
                } else if s.world().script_waiting() == Some(&engine::script::Request::YesNo) {
                    Buttons::B
                } else {
                    Buttons::A
                };
                s.step(key);
                last_script.set(s.world().last_script());
                collect(s.world(), &mut r);
            }
            r.mashed = Some(n);
        }
        r.end_map = s.world().map();
        r.last_script = script_name(s.world().last_script());
        s.framebuffer();
        let cp = match s.checkpoint() {
            Ok(cp) => cp,
            Err(e) => {
                let w = s.world();
                r.not_capturable = Some(format!(
                    "{}: {e:?}; waiting on {:?}",
                    verdict(w),
                    w.script_waiting()
                ));
                return;
            }
        };
        let bytes = match engine::checkpoint::encode_checked(&cp) {
            Ok(b) => b,
            Err(e) => return r.round_trip = Some(format!("encode: {e:?}")),
        };
        match engine::checkpoint::decode(&bytes) {
            Ok(d) if d == cp => {}
            Ok(_) => return r.round_trip = Some("decoded checkpoint differs".into()),
            Err(e) => return r.round_trip = Some(format!("decode: {e:?}")),
        }
        let mut back = match Session::restore(game, &cp) {
            Ok(b) => b,
            Err(e) => return r.round_trip = Some(format!("restore: {e:?}")),
        };
        if back.world().hash() != s.world().hash() {
            return r.round_trip = Some("restored hash differs".into());
        }
        let at = frame.get();
        for n in 1..=SHADOW {
            frame.set(at + n);
            s.step(Buttons::default());
            back.step(Buttons::default());
            if back.world().hash() != s.world().hash() {
                r.round_trip = Some(format!(
                    "shadow: hashes differ {n} frames after the restore"
                ));
                return;
            }
        }
        back.framebuffer();
    }));
    if outcome.is_err() {
        let msg = PANIC.with(|p| p.borrow_mut().take()).unwrap_or_default();
        r.panic = Some(format!(
            "frame {}, last script {}: {msg}",
            frame.get(),
            script_name(last_script.get())
        ));
    }
    r
}

fn sweep() -> Result<()> {
    ensure!(
        content::STUB || content::scripts::DECOMP_NAMES,
        "the script IR has positional names only (scripts/decomp_names = 0): set PKR_DECOMP and \
         re-run `cargo xtask extract` before `cargo xtask sweep`"
    );
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|info| {
        let loc = info
            .location()
            .map_or(String::new(), |l| format!(" at {}:{}", l.file(), l.line()));
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        PANIC.with(|p| *p.borrow_mut() = Some(format!("{msg}{loc}")));
    }));
    let maps: &[MapId] = &content::data::MAP_IDS;
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let t = std::time::Instant::now();
    let mut runs: Vec<MapRun> = std::thread::scope(|sc| {
        let handles: Vec<_> = (0..threads)
            .map(|k| {
                sc.spawn(move || {
                    maps.iter()
                        .skip(k)
                        .step_by(threads)
                        .map(|&m| run_map(m))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("sweep thread"))
            .collect()
    });
    std::panic::set_hook(hook);
    let order: BTreeMap<MapId, usize> = maps.iter().enumerate().map(|(i, &m)| (m, i)).collect();
    runs.sort_by_key(|r| order[&r.map]);
    let elapsed = t.elapsed();

    let mut out = String::new();
    let panics: Vec<&MapRun> = runs.iter().filter(|r| r.panic.is_some()).collect();
    let trips: Vec<&MapRun> = runs.iter().filter(|r| r.round_trip.is_some()).collect();
    let dropped: Vec<&MapRun> = runs.iter().filter(|r| r.events_dropped > 0).collect();
    let stuck: Vec<&MapRun> = runs
        .iter()
        .filter(|r| r.not_capturable.is_some() && !r.known_loop())
        .collect();
    let known: Vec<&MapRun> = runs.iter().filter(|r| r.known_loop()).collect();
    let spins: Vec<&MapRun> = runs
        .iter()
        .filter(|r| r.spin().is_some() && !r.known_loop())
        .collect();
    let mashed: Vec<&MapRun> = runs
        .iter()
        .filter(|r| r.mashed.is_some() && r.not_capturable.is_none() && !r.known_loop())
        .collect();
    let passed = runs
        .iter()
        .filter(|r| {
            r.panic.is_none()
                && r.round_trip.is_none()
                && r.events_dropped == 0
                && r.not_capturable.is_none()
        })
        .count();
    writeln!(
        out,
        "== sweep: {} maps, {IDLE} idle frames (then up to {MASH} mashed) + {SHADOW} shadowed, \
         {threads} threads, {:.2} s ==",
        runs.len(),
        elapsed.as_secs_f64()
    )?;
    writeln!(
        out,
        "passed {passed}/{} ({} after mashing); panics {}; round-trip failures {}; events \
         dropped on {} maps; not capturable {}; other spinning scripts {}; known defects {}",
        runs.len(),
        mashed.len(),
        panics.len(),
        trips.len(),
        dropped.len(),
        stuck.len(),
        spins.len(),
        known.len()
    )?;
    let label = |r: &MapRun| format!("{:#06x} {} at {:?}", r.map.0, r.name_key(), r.start);
    // Where a map's run ended: its last script, and the map when a warp changed it.
    let end = |r: &MapRun| {
        let moved = if r.end_map == r.map {
            "same map".to_string()
        } else {
            format!("map changed to {}", map_key(r.end_map))
        };
        format!("last script {}, {moved}", r.last_script)
    };
    for (title, list, what) in [
        ("panics", &panics, 0),
        ("round-trip failures", &trips, 1),
        ("events dropped", &dropped, 2),
        ("capturable after mashing (then checked)", &mashed, 3),
        ("not capturable after mashing", &stuck, 4),
        ("spinning scripts (restart every few frames)", &spins, 5),
        (
            "known defects: a facility's challenge-status script spins (unsupported facility \
             specials; the checks ran when capturable)",
            &known,
            5,
        ),
    ] {
        if list.is_empty() {
            continue;
        }
        writeln!(out, "\n-- {title} --")?;
        for r in list.iter() {
            let why = match what {
                0 => r.panic.clone().unwrap_or_default(),
                1 => r.round_trip.clone().unwrap_or_default(),
                2 => format!("{} events dropped", r.events_dropped),
                3 => format!("{} frames mashed; {}", r.mashed.unwrap_or(0), end(r)),
                4 => format!(
                    "{}; {}",
                    r.not_capturable.clone().unwrap_or_default(),
                    end(r)
                ),
                _ => {
                    let (name, n) = r.spin().unwrap_or_default();
                    let checked = if r.not_capturable.is_some() {
                        "not capturable"
                    } else {
                        "checked"
                    };
                    format!("{name} started {n} times; {checked}; {}", end(r))
                }
            };
            writeln!(out, "{}: {why}", label(r))?;
        }
    }
    // Reports: per name, the maps raising it and the total count.
    let mut reports: BTreeMap<&str, (u32, u32)> = BTreeMap::new();
    for r in &runs {
        for (k, &n) in &r.reports {
            let e = reports.entry(k).or_default();
            e.0 += 1;
            e.1 += n;
        }
    }
    let mut by_maps: Vec<(&str, (u32, u32))> = reports.into_iter().collect();
    by_maps.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(b.0)));
    writeln!(
        out,
        "\n-- reports raised while idling ({} kinds): maps, events --",
        by_maps.len()
    )?;
    for (k, (m, n)) in &by_maps {
        writeln!(out, "{k}: {m} maps, {n} events")?;
    }
    writeln!(
        out,
        "\nscripts started {}, maps entered {}, idle frames with a new world hash {}",
        runs.iter().map(|r| r.scripts_started).sum::<u32>(),
        runs.iter().map(|r| r.maps_entered).sum::<u32>(),
        runs.iter().map(|r| r.changed).sum::<u32>()
    )?;
    writeln!(out, "\n== content::REPORT ==\n{}", content::REPORT)?;
    let dir = crate::root().join("target/sweep");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("report.txt");
    std::fs::write(&path, &out)?;
    let head: String = out
        .split("\n== content::REPORT")
        .next()
        .unwrap_or_default()
        .to_string();
    println!("{head}");
    println!("written to {}", path.display());
    if !panics.is_empty() || !trips.is_empty() || !dropped.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}

/// `map`'s decomp name, else its id.
fn map_key(map: MapId) -> String {
    extract::names::MAPS
        .iter()
        .find(|m| (u16::from(m.group) << 8 | u16::from(m.num)) == map.0)
        .map_or_else(|| format!("{:#06x}", map.0), |m| m.dir.to_string())
}

impl MapRun {
    /// The map's decomp name when the build has one, else its display name.
    fn name_key(&self) -> String {
        extract::names::MAPS
            .iter()
            .find(|m| (u16::from(m.group) << 8 | u16::from(m.num)) == self.map.0)
            .map_or_else(|| self.name.to_string(), |m| m.dir.to_string())
    }

    /// The script that started most often, if it spins ([`SPIN_STARTS`]): name and starts.
    fn spin(&self) -> Option<(String, u32)> {
        let (&id, &n) = self
            .starts
            .iter()
            .max_by_key(|&(id, n)| (*n, std::cmp::Reverse(*id)))?;
        (n >= SPIN_STARTS).then(|| (script_name(Some(id)), n))
    }

    /// A known defect: a facility's challenge-status script spins ([`KNOWN_SPIN`]).
    fn known_loop(&self) -> bool {
        self.spin()
            .is_some_and(|(name, _)| name.ends_with(KNOWN_SPIN))
    }
}

/// The studio's whole-game tests, in release.
fn studio() -> Result<()> {
    crate::run(Command::new("cargo").current_dir(crate::root()).args([
        "test",
        "--release",
        "-p",
        "studio",
        "--test",
        "map_view",
        "--test",
        "data_views",
        "--test",
        "map_edit",
        "--",
        "--ignored",
    ]))
}

/// The `$OUT_DIR` of the native debug build of `content`, after `cargo clean -p content` and a
/// build: the one build dir holding a `report.txt`.
fn build_content_out() -> Result<PathBuf> {
    let root = crate::root();
    crate::run(
        Command::new("cargo")
            .current_dir(&root)
            .args(["clean", "-p", "content"]),
    )?;
    crate::run(
        Command::new("cargo")
            .current_dir(&root)
            .args(["build", "-p", "content"]),
    )?;
    let build = root.join("target/debug/build");
    let mut outs = Vec::new();
    for e in std::fs::read_dir(&build).with_context(|| format!("{}", build.display()))? {
        let p = e?.path().join("out");
        let content = p
            .parent()
            .and_then(|d| d.file_name())
            .is_some_and(|n| n.to_string_lossy().starts_with("content-"));
        if content && p.join("report.txt").exists() {
            outs.push(p);
        }
    }
    ensure!(
        outs.len() == 1,
        "expected one content $OUT_DIR, found {outs:?}"
    );
    Ok(outs.remove(0))
}

/// Every file under `dir`, by relative path.
fn files(dir: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d)? {
            let p = e?.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.insert(p.strip_prefix(dir)?.to_path_buf(), std::fs::read(&p)?);
            }
        }
    }
    Ok(out)
}

/// Two clean builds of `content`, compared byte for byte.
fn repro() -> Result<()> {
    let a = files(&build_content_out()?)?;
    let b = files(&build_content_out()?)?;
    let mut diff = Vec::new();
    for k in a.keys().chain(b.keys()) {
        if a.get(k) != b.get(k) && !diff.contains(k) {
            diff.push(k.clone());
        }
    }
    let bytes: usize = a.values().map(Vec::len).sum();
    if !diff.is_empty() {
        bail!("content $OUT_DIR differs between two clean builds: {diff:?}");
    }
    println!(
        "sweep --repro: two clean content builds give identical $OUT_DIRs ({} files, {bytes} bytes)",
        a.len()
    );
    Ok(())
}
