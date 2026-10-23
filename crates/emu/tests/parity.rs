//! Drives the real ROM in mGBA through the scripted route (`game_native::scenario`), records
//! the inputs the ROM actually consumed, replays them natively, and requires the same World
//! hash at every sample and the same picture at every picture frame.
//! Needs target/game.gba: run via `cargo xtask parity`.
//!
//! The route depends on the content build: with extracted data it plays a new game from
//! Littleroot → Birch's rescue on Route 101 → the starter screen and the first battle → the
//! lab → wild battles in Route 101's grass (a win, a run, a level-up) → Oldale's Pokémon
//! Center → May's battle on Route 103 (about 19000 frames);
//! in stub mode it walks around the built-in map and fights in its tall-grass cell; on Silice
//! (SIL1 Task D) it is the boot lane, a new game left idle 600 frames, and the other lanes are
//! skipped ([`boot_lane_only`]). The ROM
//! must be built from the same content as this test (xtask builds both from the same tree).
//!
//! The ROM may overrun a frame only on a black screen and only for bulk work (`Frame::bulk`):
//! every `late_black` must follow a bulk step within `scenario::DISPLAY_LAG` steps, and there
//! are at most as many as bulk events (`Recording::bulk_events`: runs of bulk steps: map
//! loads, whole-view redraws, starter loads, battle loads and restores).
//!
//! Display lag (see `engine::debug`): when the emulator samples engine frame `f`, the hash is
//! the state after step `f` but the picture is the `Frame` of step `f - 1`. So picture frame `f`
//! compares the ROM hash with native after `f` steps and the ROM picture with native after
//! `f - 1` steps.

extern crate alloc;

#[path = "../../game-gba/src/probe_scene.rs"]
mod probe_scene;
#[allow(dead_code)] // only `SAVE_WAIT_FRAMES` is used here
#[path = "../../game-gba/src/save_logic.rs"]
mod save_logic;

use emu::{DebugSnapshot, Emu, RomRun, run_rom, run_rom_from};
use engine::Buttons;
use engine::checkpoint::{Checkpoint, build_hash};
use engine::data::MapId;
use engine::debug::{boot, save};
use game_native::scenario::{DISPLAY_LAG, Recording, cached_route};
use game_native::{Ppu, Session, flash};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// A picture is compared at least this often (and at every named moment).
const PICTURE_EVERY: u32 = 100;

/// Replays `run.consumed` natively, comparing the hash at every ROM sample and the picture at
/// every capture. Native renders only the frames a capture needs (`frame - 1`).
fn compare_native(run: &RomRun, mut native: Session<content::Emerald>) {
    let mut hashes = run.hashes.iter().peekable();
    let mut caps = run.captures.iter().peekable();
    for (i, &k) in run.consumed.iter().enumerate() {
        let frame = i as u32 + 1;
        if let Some(cap) = caps.next_if(|c| c.frame == frame) {
            let fb = native.framebuffer();
            let diff = cap.fb.iter().zip(fb).filter(|(a, b)| a != b).count();
            if diff != 0 {
                dump_mismatch(frame, &cap.fb, fb);
            }
            assert_eq!(
                diff,
                0,
                "{diff} pixels differ between ROM (frame {frame}) and native (frame {})",
                frame - 1
            );
        }
        native.step(k);
        assert_eq!(native.world().frame(), frame);
        if let Some(&(_, hash)) = hashes.next_if(|h| h.0 == frame) {
            assert_eq!(
                native.world().hash(),
                hash,
                "World hash diverged at frame {frame}"
            );
        }
    }
    assert!(
        caps.next().is_none(),
        "picture frame past the consumed inputs"
    );
    assert!(
        hashes.next().is_none(),
        "hash sample past the consumed inputs"
    );
}

/// Writes target/parity-<frame>-{rom,native}.ppm for a failed picture frame.
fn dump_mismatch(frame: u32, rom: &[u16], native: &[u16]) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    for (name, fb) in [("rom", rom), ("native", native)] {
        let mut data = format!(
            "P6\n{} {}\n255\n",
            game_native::SCREEN_W,
            game_native::SCREEN_H
        )
        .into_bytes();
        for &c in fb {
            for ch in [c & 31, (c >> 5) & 31, (c >> 10) & 31] {
                data.push(((ch << 3) | (ch >> 2)) as u8);
            }
        }
        let path = dir.join(format!("parity-{frame}-{name}.ppm"));
        let _ = std::fs::write(&path, data);
        eprintln!("wrote {}", path.display());
    }
}

/// Gates on the ROM's scanline costs (`engine::debug::DebugRegion`): `World::hash` and the
/// whole loop body on visible frames. `late == 0` is the hard gate (`run_rom_from` fails on a
/// late apply). Both gates are the route's measured worst + 2 after Milestone 6 Task A's buffered
/// hash (hash 63, loop 215 lines), so a cost regression fails here before it eats the margin.
const HASH_LINES_MAX: u32 = 65;
const LOOP_LINES_MAX: u32 = 217;
/// Scanlines in a frame.
const FRAME_LINES: u32 = 228;

fn check_costs(what: &str, end: &DebugSnapshot) {
    println!(
        "{what}: hash {} lines (gate {HASH_LINES_MAX}), step {} lines, loop {} lines (gate \
         {LOOP_LINES_MAX}, margin {}; {} lines left in the frame)",
        end.hash_lines_max,
        end.step_lines_max,
        end.loop_lines_max,
        LOOP_LINES_MAX as i64 - end.loop_lines_max as i64,
        FRAME_LINES as i64 - end.loop_lines_max as i64,
    );
    assert!(
        end.hash_lines_max <= HASH_LINES_MAX,
        "{what}: World::hash took {} lines",
        end.hash_lines_max
    );
    assert!(
        end.loop_lines_max <= LOOP_LINES_MAX,
        "{what}: the shell loop took {} lines",
        end.loop_lines_max
    );
    // Milestone 8 Task M: the heap and stack high-water marks and dropped events.
    emu::check_memory(what, end).unwrap_or_else(|e| panic!("{e:#}"));
}

fn rom_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/game.gba")
}

fn recording() -> &'static Recording {
    cached_route()
}

#[test]
#[ignore = "needs target/game.gba; run `cargo xtask parity`"]
fn rom_and_native_agree() {
    let rom = rom_path();
    let rec = recording();
    let picture_frames = rec.picture_frames(PICTURE_EVERY);
    println!(
        "{} route: {} frames, {} picture frames, moments {:?}",
        content::FLAVOR.name(),
        rec.frames(),
        picture_frames.len(),
        rec.moments
    );
    let run = run_rom(&rom, |f| rec.keys_at(f), rec.frames(), &picture_frames)
        .unwrap_or_else(|e| panic!("ROM run failed: {e:#}"));
    println!(
        "ROM reached engine frame {} after {} emulator frames; {} inputs consumed",
        run.hashes.last().map_or(0, |h| h.0),
        run.emu_frames,
        run.consumed.len(),
    );
    println!(
        "{} late applies while black at {:?}; {} bulk events ({} warps, {} battles, {} starters)",
        run.late_black,
        run.late_black_frames,
        rec.bulk_events(),
        rec.warps,
        rec.battles,
        rec.starters,
    );

    // Pin the button mapping: native replays what the ROM recorded, so also require that the
    // ROM recorded exactly what the script sent (catches KEYINPUT/setKeys bit-order regressions).
    for (i, &k) in run.consumed.iter().enumerate() {
        assert_eq!(
            k,
            rec.keys_at(i as u32),
            "ROM consumed a different input at step {}",
            i + 1
        );
    }
    assert_eq!(run.captures.len(), picture_frames.len());
    // Black-screen late applies are only excused for bulk work (`Frame::bulk`: map loads,
    // whole-view redraws, battle loads and restores, starter loads): each must follow a bulk
    // step within the display lag, and each bulk event (a run of bulk steps) may overrun once.
    for &late in &run.late_black_frames {
        assert!(
            rec.bulk_before(late).is_some(),
            "late apply on a black screen at engine frame {late} follows no bulk step"
        );
    }
    assert!(
        run.late_black <= rec.bulk_events(),
        "{} late applies on a black screen for {} bulk events",
        run.late_black,
        rec.bulk_events()
    );
    compare_native(&run, Session::new(content::Emerald::for_shells()));
    println!(
        "{} hashes and {} pictures identical, no late frames",
        run.hashes.len(),
        run.captures.len()
    );
    assert_eq!(run.boot.boot, boot::NEW_GAME);
    assert_eq!(
        run.boot.build,
        build_hash(&content::Emerald::for_shells()),
        "the ROM was built from other engine sources or content than this test"
    );
    check_costs("route", &run.end);
}

/// A native run on `keys` from a new game, kept for comparisons with ROM runs.
struct Native {
    /// `hashes[n]`: `World::hash` after `n` steps.
    hashes: Vec<u64>,
    /// `capturable[n]`: the World after `n` steps can be captured.
    capturable: Vec<bool>,
    /// `respawn[n]`: `World::respawn` after `n` steps.
    respawn: Vec<u8>,
    /// `maps[n]`: `World::map` after `n` steps.
    maps: Vec<MapId>,
    /// `battle[n]`: a battle runs after `n` steps.
    battle: Vec<bool>,
    /// Steps whose frame is `Frame::bulk`.
    bulk: Vec<u32>,
    /// What the ROM shows at engine frame `c` (display lag: native after `c - 1` steps), for
    /// each requested `c`.
    pictures: BTreeMap<u32, Vec<u16>>,
    /// The checkpoint after `n` steps, for each requested `n`.
    checkpoints: BTreeMap<u32, Checkpoint>,
}

fn native_run(keys: impl Fn(u32) -> Buttons, end: u32, pictures: &[u32], cps: &[u32]) -> Native {
    let mut s = Session::new(content::Emerald::for_shells());
    let mut out = Native {
        hashes: Vec::new(),
        capturable: Vec::new(),
        respawn: Vec::new(),
        maps: Vec::new(),
        battle: Vec::new(),
        bulk: Vec::new(),
        pictures: BTreeMap::new(),
        checkpoints: BTreeMap::new(),
    };
    for n in 0..=end {
        out.hashes.push(s.world().hash());
        out.capturable.push(s.world().capturable().is_ok());
        out.respawn.push(s.world().respawn());
        out.maps.push(s.world().map());
        out.battle.push(s.world().in_battle());
        if pictures.contains(&(n + 1)) {
            out.pictures.insert(n + 1, s.framebuffer().to_vec());
        }
        if cps.contains(&n) {
            let cp = s
                .checkpoint()
                .unwrap_or_else(|r| panic!("frame {n} is not capturable: {r:?}"));
            out.checkpoints.insert(n, cp);
        }
        if n < end {
            s.step(keys(n));
            if s.frame().bulk {
                out.bulk.push(n + 1);
            }
        }
    }
    out
}

/// A ROM run against the uninterrupted native run: the consumed inputs, every hash, every
/// picture, and each black-screen late apply after a bulk step (a restored boot frame, applied
/// blanked, counts as one at the start frame).
fn compare_run(what: &str, run: &RomRun, native: &Native, keys: impl Fn(u32) -> Buttons) {
    for (i, &k) in run.consumed.iter().enumerate() {
        let f = run.start + i as u32;
        assert_eq!(
            k,
            keys(f),
            "{what}: ROM consumed another input at step {}",
            f + 1
        );
    }
    for &(frame, hash) in &run.hashes {
        assert_eq!(
            hash, native.hashes[frame as usize],
            "{what}: World hash diverged at frame {frame}"
        );
    }
    for cap in &run.captures {
        let fb = &native.pictures[&cap.frame];
        let diff = cap.fb.iter().zip(fb).filter(|(a, b)| a != b).count();
        if diff != 0 {
            dump_mismatch(cap.frame, &cap.fb, fb);
        }
        assert_eq!(
            diff, 0,
            "{what}: {diff} pixels differ at frame {}",
            cap.frame
        );
    }
    let restored = (run.boot.boot == boot::RESTORED).then_some(run.start);
    for &late in &run.late_black_frames {
        assert!(
            native
                .bulk
                .iter()
                .copied()
                .chain(restored)
                .any(|b| b < late && late - b <= DISPLAY_LAG),
            "{what}: late apply on a black screen at engine frame {late} follows no bulk step"
        );
    }
    println!(
        "{what}: frames {}..={}, {} hashes and {} pictures identical, {} late applies while \
         black",
        run.start,
        run.start + run.consumed.len() as u32,
        run.hashes.len(),
        run.captures.len(),
        run.late_black,
    );
}

/// Gen3 `MAP_LITTLEROOT_TOWN_PROFESSOR_BIRCHS_LAB` and `MAP_OLDALE_TOWN_POKEMON_CENTER_1F`
/// (`content::consts` has them only with extracted data).
const MAP_LITTLEROOT_TOWN_PROFESSOR_BIRCHS_LAB: MapId = MapId(0x104);
const MAP_OLDALE_TOWN_POKEMON_CENTER_1F: MapId = MapId(0x202);

/// The checkpoints the ROM resumes from, as (name, frame) on the route: with extracted data,
/// the lab after Birch gives Mudkip, Oldale's Pokémon Center once the nurse has healed (the
/// respawn point moves), and the last capturable frame before May's battle; on the stub, its
/// start and after its battle. Picked from the recording at run time (the first capturable
/// frame at or after a moment), so a re-pinned route keeps working.
fn resume_nodes(rec: &Recording, native: &Native) -> Vec<(&'static str, u32)> {
    let moment = |name: &str| {
        rec.moments
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("no moment {name}"))
            .frame
    };
    let capturable = |n: u32| native.capturable[n as usize];
    let first_from = |from: u32| {
        (from..=rec.frames())
            .find(|&n| capturable(n))
            .unwrap_or_else(|| panic!("nothing capturable after frame {from}"))
    };
    if content::STUB {
        return vec![
            ("stub_start", first_from(moment("stub_start"))),
            ("stub_after_battle", first_from(moment("stub_after_battle"))),
        ];
    }
    let level_up = moment("level_up");
    let respawn = native.respawn[level_up as usize];
    let healed = (level_up..=rec.frames())
        .find(|&n| capturable(n) && native.respawn[n as usize] != respawn)
        .expect("the respawn never moves after the level-up (Oldale's nurse)");
    assert_eq!(
        native.maps[healed as usize], MAP_OLDALE_TOWN_POKEMON_CENTER_1F,
        "the respawn moved at frame {healed} outside Oldale's Pokémon Center"
    );
    let may = moment("may_intro");
    let before_may = (0..may)
        .rev()
        .find(|&n| capturable(n))
        .expect("nothing capturable before May's battle");
    let lab = first_from(moment("lab"));
    assert_eq!(
        native.maps[lab as usize],
        MAP_LITTLEROOT_TOWN_PROFESSOR_BIRCHS_LAB
    );
    assert_eq!(
        rec.moments.iter().find(|m| m.name == "lab").unwrap().map,
        MAP_LITTLEROOT_TOWN_PROFESSOR_BIRCHS_LAB
    );
    vec![
        ("lab", lab),
        ("oldale_center", healed),
        ("before_may", before_may),
    ]
}

/// Whether this build runs the boot lane only (Silice, SIL1 Task D: [`rom_and_native_agree`]
/// on `scenario::SILICE_ROUTE`, a new game played through its intro, then idle). Its resume
/// and save lanes need the `sil:*` nodes and a battle on the route (SIL1 Wave 3); `lane` is
/// skipped with a note.
fn boot_lane_only(lane: &str) -> bool {
    let only = content::FLAVOR == content::Flavor::Silice;
    if only {
        println!("{lane} lane: skipped on Silice (boot lane only until SIL1 Wave 3)");
    }
    only
}

/// Frames a ROM run resumed from a checkpoint follows the route (at most).
const RESUME_FRAMES: u32 = 2000;

/// Picture frames of a run from `start` to `end`: the first two, then every
/// [`PICTURE_EVERY`] frames.
fn resume_pictures(start: u32, end: u32) -> Vec<u32> {
    let every = (start / PICTURE_EVERY + 1..).map(|i| i * PICTURE_EVERY);
    let p: BTreeSet<u32> = [start + 1, start + 2]
        .into_iter()
        .chain(every.take_while(|&c| c <= end))
        .filter(|&c| c <= end)
        .collect();
    p.into_iter().collect()
}

/// Native checkpoints written as `.sav` images boot the ROM at the checkpoint, which then plays
/// the route exactly as the uninterrupted native session does.
#[test]
#[ignore = "needs target/game.gba; run `cargo xtask parity`"]
fn rom_resumes_from_native_checkpoints() {
    if boot_lane_only("resume") {
        return;
    }
    let rom = rom_path();
    let rec = recording();
    let keys = |f| rec.keys_at(f);
    let scan = native_run(keys, rec.frames(), &[], &[]);
    let nodes = resume_nodes(rec, &scan);
    let runs: Vec<(u32, u32)> = nodes
        .iter()
        .map(|&(_, f)| (f, (f + RESUME_FRAMES).min(rec.frames())))
        .collect();
    let pictures: Vec<u32> = runs
        .iter()
        .flat_map(|&(f, end)| resume_pictures(f, end))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let at: Vec<u32> = runs.iter().map(|r| r.0).collect();
    let native = native_run(keys, rec.frames(), &pictures, &at);
    let build = build_hash(&content::Emerald::for_shells());
    for (&(name, f), &(_, end)) in nodes.iter().zip(&runs) {
        let what = format!("{name} (frame {f})");
        let cp = &native.checkpoints[&f];
        let sav = flash::encode_sav(cp).unwrap_or_else(|e| panic!("{what}: {e}"));
        let run = run_rom_from(
            &rom,
            Some(&sav),
            Some(f),
            keys,
            end,
            &resume_pictures(f, end),
        )
        .unwrap_or_else(|e| panic!("{what}: ROM run failed: {e:#}"));
        assert_eq!(run.boot.boot, boot::RESTORED, "{what}: boot code");
        assert_eq!(run.boot.build, build, "{what}: ROM build");
        assert_eq!(run.start, f);
        assert_eq!(
            run.boot.hash, native.hashes[f as usize],
            "{what}: boot hash"
        );
        compare_run(&what, &run, &native, keys);
        check_costs(&what, &run.end);
    }
}

/// An in-game save (L+R+SELECT) on the ROM restores natively to the ROM's hash, and a second
/// ROM boots from the flash it wrote and matches native.
#[test]
#[ignore = "needs target/game.gba; run `cargo xtask parity`"]
fn rom_save_restores_natively() {
    /// Engine frames run after the save, so mGBA settles the write.
    const SETTLE: u32 = 30;
    /// Engine frames the second ROM runs from the save.
    const AFTER: u32 = 300;
    if boot_lane_only("save") {
        return;
    }
    let rom = rom_path();
    let rec = recording();
    let scan = native_run(|f| rec.keys_at(f), rec.frames(), &[], &[]);
    let f = resume_nodes(rec, &scan)[0].1;
    let lr = Buttons(Buttons::L.0 | Buttons::R.0);
    // The route up to `f`, L+R, then SELECT too (the combo, consumed by step `f + 2`), then
    // nothing.
    let keys = |x: u32| match x {
        _ if x < f => rec.keys_at(x),
        _ if x == f => lr,
        _ if x == f + 1 => Buttons(lr.0 | Buttons::SELECT.0),
        _ => Buttons::NONE,
    };
    let saved = f + 2;
    let end = saved + AFTER;
    let pictures = resume_pictures(saved, end);
    let native = native_run(keys, end, &pictures, &[saved]);

    let run = run_rom(&rom, keys, saved + SETTLE, &[])
        .unwrap_or_else(|e| panic!("ROM run to the save failed: {e:#}"));
    compare_run("save run", &run, &native, keys);
    check_costs("save run", &run.end);
    assert_eq!(run.end.saves, 1, "flash writes");
    assert_eq!(run.end.save_status, save::OK, "save status");
    let (meta, cp) = flash::decode_sav(&run.save).unwrap_or_else(|e| panic!("ROM flash: {e}"));
    assert_eq!(cp.header.frame, saved);
    assert_eq!(meta.frame, saved);
    // `compare_run` matched every ROM sample to native; a catch-up may drop the one at `saved`.
    assert_eq!(cp.header.world_hash, native.hashes[saved as usize]);
    assert_eq!(cp.header.build, build_hash(&content::Emerald::for_shells()));
    assert!(
        cp == native.checkpoints[&saved],
        "the ROM saved another checkpoint than native captures at frame {saved}"
    );
    assert!(
        run.save == flash::encode_sav(&cp).unwrap(),
        "the ROM's flash image differs from game_native::flash's"
    );
    let restored = Session::restore(content::Emerald::for_shells(), &cp)
        .unwrap_or_else(|e| panic!("native restore of the ROM save: {e:?}"));
    assert_eq!(restored.world().hash(), native.hashes[saved as usize]);

    let resumed = run_rom_from(&rom, Some(&run.save), Some(saved), keys, end, &pictures)
        .unwrap_or_else(|e| panic!("ROM boot from its own save failed: {e:#}"));
    assert_eq!(resumed.boot.boot, boot::RESTORED);
    assert_eq!(resumed.end.saves, 0);
    compare_run("boot from the ROM save", &resumed, &native, keys);
    check_costs("save and resume", &resumed.end);
    println!(
        "ROM saved at frame {saved} ({} checkpoint bytes); native restore and a second boot \
         agree",
        engine::checkpoint::encoded_len(&cp)
    );
}

/// The combo pressed mid-battle (the route's wild battle: `wild_intro`, or `stub_wild_intro`)
/// is pending: the ROM keeps playing on time (`late == 0`, which `run_rom_from` enforces) and
/// saves once, on the first capturable frame after the battle. The press frame is picked so
/// that frame comes within the pending wait (`save_logic::SAVE_WAIT_FRAMES`): about 300 frames
/// before the battle ends, or the battle's first frame if it is shorter.
#[test]
#[ignore = "needs target/game.gba; run `cargo xtask parity`"]
fn rom_save_mid_battle_lands_on_the_first_capturable_frame() {
    const SETTLE: u32 = 30;
    /// Frames the pending save waits, at most, in this test (well within the shell's limit).
    const WAIT: u32 = 300;
    const _: () = assert!(WAIT < save_logic::SAVE_WAIT_FRAMES as u32);
    if boot_lane_only("mid-battle save") {
        return;
    }
    let rom = rom_path();
    let rec = recording();
    let name = if content::STUB {
        "stub_wild_intro"
    } else {
        "wild_intro"
    };
    let start = rec.moments.iter().find(|m| m.name == name).unwrap().frame;
    let scan = native_run(|f| rec.keys_at(f), rec.frames(), &[], &[]);
    assert!(
        scan.battle[start as usize],
        "{name}: no battle at frame {start}"
    );
    let end = (start..=rec.frames())
        .find(|&n| scan.capturable[n as usize])
        .expect("nothing capturable after the battle");
    let f = start.max(end.saturating_sub(WAIT));
    let lr = Buttons(Buttons::L.0 | Buttons::R.0);
    // The route's keys, with L+R at `f` and L+R+SELECT at `f + 1` (the combo, consumed by step
    // `f + 2`) held on top.
    let keys = |x: u32| {
        let k = rec.keys_at(x);
        match x {
            _ if x == f => Buttons(k.0 | lr.0),
            _ if x == f + 1 => Buttons(k.0 | lr.0 | Buttons::SELECT.0),
            _ => k,
        }
    };
    let pressed = f + 2;
    let probe = native_run(keys, rec.frames(), &[], &[]);
    assert!(
        probe.battle[pressed as usize] && !probe.capturable[pressed as usize],
        "the combo lands outside the battle"
    );
    let saved = (pressed..=rec.frames())
        .find(|&n| probe.capturable[n as usize])
        .expect("nothing capturable after the combo");
    assert!(
        saved - pressed < save_logic::SAVE_WAIT_FRAMES as u32,
        "the save would time out"
    );
    let native = native_run(keys, saved + SETTLE, &[], &[saved]);

    let run = run_rom(&rom, keys, saved + SETTLE, &[])
        .unwrap_or_else(|e| panic!("ROM run through the battle failed: {e:#}"));
    compare_run("mid-battle save", &run, &native, keys);
    check_costs("mid-battle save", &run.end);
    assert_eq!(run.end.late, 0);
    assert_eq!(run.end.saves, 1, "flash writes");
    assert_eq!(run.end.save_status, save::OK, "save status");
    let (meta, cp) = flash::decode_sav(&run.save).unwrap_or_else(|e| panic!("ROM flash: {e}"));
    assert_eq!(
        (meta.frame, cp.header.frame),
        (saved, saved),
        "the save lands on the first capturable frame"
    );
    assert!(
        cp == native.checkpoints[&saved],
        "the ROM saved another checkpoint than native captures at frame {saved}"
    );
    println!(
        "combo at frame {pressed} in battle ({name}, frame {start}); saved at frame {saved}, \
         {} frames later",
        saved - pressed
    );
}

/// The real-data route needs the extracted maps; say so instead of silently testing the stub.
#[test]
fn the_route_matches_the_content_build() {
    if content::STUB {
        println!(
            "skipping the extracted-data route: content was built without content/extracted \
             (run `cargo xtask extract`); the stub route is used instead"
        );
    }
    let rec = recording();
    assert!(!rec.moments.is_empty());
}

/// The probe sequence plays natively: every frame is valid, the starter screen confirms the
/// middle ball through its scripted input (one decline first), and the transition over it ends
/// black.
#[test]
fn video_probe_sequence_plays() {
    let (frames, chosen) = probe_scene::all_frames();
    assert_eq!(chosen, Some(1));
    for (i, f) in frames.iter().enumerate() {
        assert_eq!(f.validate(), Ok(()), "probe frame {i}");
    }
    assert!(frames.last().unwrap().is_black());
    let lighten: std::collections::BTreeSet<u8> = frames
        .iter()
        .filter(|f| f.lighten)
        .map(|f| f.brightness)
        .collect();
    assert_eq!(lighten.len(), 17);
    let high_obj_tiles = frames[0]
        .cmds
        .iter()
        .filter(|c| matches!(c, engine::video::Cmd::ObjTile { index, .. } if *index >= 512))
        .count();
    assert_eq!(high_obj_tiles, 5);
}

/// The video probe ROM (`crates/game-gba/src/bin/video_probe.rs`) against the native PPU: every
/// `Frame::lighten` level, `Cmd::ObjTile` and both battle transitions, frame by frame. The probe
/// publishes the index of the frame it applied at the last vblank, which is the picture the
/// emulator has just drawn.
#[test]
#[ignore = "needs target/video-probe.gba; run `cargo xtask parity`"]
fn video_probe_matches_native() {
    let rom = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/video-probe.gba");
    let (frames, _) = probe_scene::all_frames();
    let last = frames.len() as u32 - 1;
    let mut emu = Emu::open(&rom).unwrap_or_else(|e| panic!("open probe: {e:#}"));
    let mut snap = emu
        .boot(120)
        .unwrap_or_else(|e| panic!("boot probe: {e:#}"));
    let mut ppu = Ppu::new();
    let mut applied = 0usize;
    ppu.apply(&frames[0]);
    let mut fb = vec![0u16; game_native::SCREEN_W * game_native::SCREEN_H];
    let (mut compared, mut lighten_levels) = (0, std::collections::BTreeSet::new());
    for _ in 0..3 * frames.len() {
        let i = snap.frame as usize;
        while applied < i.min(last as usize) {
            applied += 1;
            ppu.apply(&frames[applied]);
        }
        // Frame 0 is applied during forced blank, part-way through an emulator frame.
        if i > 0 && i <= last as usize {
            ppu.render(&frames[i], &mut fb);
            let rom_fb = emu.framebuffer_rgb555();
            let diff = rom_fb.iter().zip(&fb).filter(|(a, b)| a != b).count();
            if diff != 0 {
                dump_mismatch(i as u32, &rom_fb, &fb);
            }
            assert_eq!(diff, 0, "{diff} pixels differ at probe frame {i}");
            compared += 1;
            if frames[i].lighten {
                lighten_levels.insert(frames[i].brightness);
            }
        }
        assert_eq!(snap.late, 0, "probe applied a frame late by frame {i}");
        if snap.frame >= last {
            break;
        }
        emu.run_frame(Buttons::NONE).unwrap();
        snap = emu.debug().expect("debug region");
    }
    assert!(snap.frame >= last, "probe stopped at frame {}", snap.frame);
    assert!(
        compared + 2 >= last as usize,
        "only {compared} of {last} probe frames sampled"
    );
    assert_eq!(
        lighten_levels.len(),
        17,
        "lighten levels 0..=16 all compared"
    );
    println!("video probe: {compared} frames identical");
}

/// The battle probe ROM (`crates/game-gba/src/bin/battle_probe.rs`): the core turn that carries
/// a Mega Evolution (Silice), timed on the GBA against the same turn without it. The whole
/// turn runs in the frame that answers the move menu, so it must leave room in the 217-line
/// loop budget; the probe has no scene nor field (the real frame adds the battle scene's
/// update, the send-out pic reload is spread over the next frames).
#[test]
#[ignore = "needs target/battle-probe.gba; run `cargo xtask parity`"]
fn battle_probe_mega_turn() {
    let rom = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/battle-probe.gba");
    let mut emu = Emu::open(&rom).unwrap_or_else(|e| panic!("open battle probe: {e:#}"));
    let mut snap = emu
        .boot(120)
        .unwrap_or_else(|e| panic!("boot battle probe: {e:#}"));
    for _ in 0..120 {
        if snap.frame == 1 {
            break;
        }
        emu.run_frame(Buttons::NONE).unwrap();
        snap = emu.debug().expect("debug region");
    }
    assert_eq!(snap.frame, 1, "the battle probe never finished");
    let (plain, mega) = (snap.hash >> 32, snap.hash & 0xFFFF_FFFF);
    println!(
        "battle probe: turn {plain} cycles ({} lines), with Mega Evolution {mega} cycles ({} \
         lines, +{}); forms at battle start +{} lines",
        snap.hash_lines_max,
        snap.step_lines_max,
        snap.step_lines_max - snap.hash_lines_max,
        snap.loop_lines_max
    );
    assert!(mega > plain);
    assert!(snap.step_lines_max < LOOP_LINES_MAX);
}

/// A new game ([`content::Emerald::after_starter`]) that starts on `(map, x, y)`.
#[derive(Clone, Copy)]
struct StartAt {
    inner: content::Emerald,
    map: MapId,
    x: i16,
    y: i16,
}

impl engine::data::GameData for StartAt {
    explore::forward_game_data!(inner; map has_map special_ids sprite player_sprite font window
        text ui_text behavior script build_id battle wild_encounters trainer battle_text
        battle_gfx heal_location m8);

    fn new_game(&self) -> engine::data::NewGame {
        engine::data::NewGame {
            map: self.map,
            x: self.x,
            y: self.y,
            ..self.inner.new_game()
        }
    }
}

/// Milestone 8 review I-3: trainer sight on the real build. A new game starts on Route 102
/// at (35, 17), two cells east of Calvin's line (local id 2 on (33, 14), facing down, sight 3);
/// holding left walks the player into it. The ROM, booted from a native checkpoint of that
/// start, plays the steps, the sighting, the `!` and the approach with native's hashes, within
/// the frame gates. The sighting frame (relative to the first step's input) and the approach's
/// length are pinned (`SIGHTED_AFTER`, `APPROACH_FRAMES`): they are native's, which the ROM
/// matches hash for hash; comparing them with Gen3's own counts is carried to Task P.
#[test]
#[ignore = "needs target/game.gba; run `cargo xtask parity`"]
fn rom_trainer_sighting_fits_the_frame() {
    /// Frames from the first held input to the sighting (the trainer script's start).
    const SIGHTED_AFTER: u32 = 26;
    /// Frames from the sighting until Calvin stands settled beside the player.
    const APPROACH_FRAMES: u32 = 108;
    use engine::data::GameData;
    use engine::world::Event;
    if content::FLAVOR != content::Flavor::Emerald {
        println!(
            "trainer sighting: {} build, no Route 102",
            content::FLAVOR.name()
        );
        return;
    }
    let base = game_native::scenario::after_starter().expect("Emerald's after-starter start");
    let route102 = *content::data::MAP_IDS
        .iter()
        .find(|&&m| base.map(m).name == "Route102")
        .expect("Route 102");
    let calvin = base.map(route102).objects.iter().find(|o| o.local_id == 2);
    assert!(
        calvin.is_some_and(|o| (o.x, o.y, o.trainer_sight) == (33, 14, 3)),
        "Calvin's template"
    );
    let game = StartAt {
        inner: base,
        map: route102,
        x: 35,
        y: 17,
    };
    let mut s = Session::new(game);
    while s.world().capturable().is_err() {
        assert!(s.world().frame() < 600, "the start never settles");
        s.step(Buttons::NONE);
    }
    let start = s.world().frame();
    let cp = s.checkpoint().expect("capturable");
    // Hold left 40 frames (a turn, then steps until sight locks the player on (33, 17)), then
    // nothing.
    let keys = move |f: u32| {
        if (start..start + 40).contains(&f) {
            Buttons::LEFT
        } else {
            Buttons::NONE
        }
    };
    const FRAMES: u32 = 400;
    let end = start + FRAMES;
    let mut hashes = BTreeMap::new();
    let mut sighted = None;
    let mut settled = None;
    for f in start..end {
        s.step(keys(f));
        let w = s.world();
        hashes.insert(w.frame(), w.hash());
        if sighted.is_none()
            && w.events()
                .iter()
                .any(|e| matches!(e, Event::ScriptStarted(_)))
        {
            sighted = Some(w.frame());
        }
        let arrived = w
            .objects()
            .get(2)
            .is_some_and(|o| o.pos() == (33, 16) && o.is_settled());
        if sighted.is_some() && settled.is_none() && arrived {
            settled = Some(w.frame());
        }
    }
    let sighted = sighted.expect("Calvin saw the player");
    let arrived = settled.expect("Calvin walked up to the player");
    assert_eq!(sighted - start, SIGHTED_AFTER, "sighting frame");
    assert_eq!(arrived - sighted, APPROACH_FRAMES, "approach length");
    let sav = flash::encode_sav(&cp).expect("sav");
    let run = run_rom_from(&rom_path(), Some(&sav), Some(start), keys, end, &[]).expect("ROM run");
    assert_eq!(run.boot.boot, boot::RESTORED);
    for &(frame, hash) in &run.hashes {
        assert_eq!(Some(&hash), hashes.get(&frame), "hash at frame {frame}");
    }
    println!(
        "trainer sighting (Route 102, Calvin): sighted at frame +{} after the first input, \
         Calvin beside the player from +{}; {} ROM samples identical",
        sighted - start,
        arrived - start,
        run.hashes.len()
    );
    check_costs("trainer sighting", &run.end);
}

/// The committed Act 1 chain (Milestone 8 Task P), flattened from a new game.
#[path = "../../game-native/tests/act1/trace.rs"]
mod act1;

/// Prints the memory marks [`check_costs`] gates.
fn print_memory(what: &str, end: &DebugSnapshot) {
    println!(
        "{what}: heap_max {} B, stack untouched {} B, events dropped {}, reports {}",
        end.heap_max, end.stack_min, end.events_dropped, end.reports
    );
}

/// Every `spine:act1:*` node resumes on the ROM from its `.sav` and follows the act for
/// [`RESUME_FRAMES`] frames (the last node idles past the end of the act): the boot hash, every
/// sampled hash and a picture every [`PICTURE_EVERY`] frames equal native's, no late frame, and
/// the cost and memory gates hold (`check_costs`). Each node's native checkpoint reaches its
/// committed digest.
#[test]
#[ignore = "needs target/game.gba; run `cargo xtask parity`"]
fn rom_resumes_from_act1_nodes() {
    let Some(act) = act1::load() else {
        println!("act 1: {} build, no act1 nodes", content::FLAVOR.name());
        return;
    };
    let keys = |f| act.keys_at(f);
    let runs: Vec<(u32, u32)> = (act.nodes.iter())
        .map(|&(_, f)| (f, f + RESUME_FRAMES))
        .collect();
    let pictures: Vec<u32> = (runs.iter())
        .flat_map(|&(f, end)| resume_pictures(f, end))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let at: Vec<u32> = runs.iter().map(|r| r.0).collect();
    let end = runs.iter().map(|r| r.1).max().unwrap();
    let native = native_run(keys, end, &pictures, &at);
    let store =
        game_native::store::Store::open(game_native::store::Store::default_root(content::FLAVOR))
            .unwrap();
    let index = store.index().unwrap();
    let build = build_hash(&content::Emerald::for_shells());
    for (&(ref label, f), &(_, end)) in act.nodes.iter().zip(&runs) {
        let what = format!("{label} (frame {f})");
        let cp = &native.checkpoints[&f];
        assert_eq!(
            engine::checkpoint::state_digest(&cp.world),
            index.labelled(label).unwrap().meta.digest,
            "{what}: native state against the committed node"
        );
        let sav = flash::encode_sav(cp).unwrap_or_else(|e| panic!("{what}: {e}"));
        let run = run_rom_from(
            &rom_path(),
            Some(&sav),
            Some(f),
            keys,
            end,
            &resume_pictures(f, end),
        )
        .unwrap_or_else(|e| panic!("{what}: ROM run failed: {e:#}"));
        assert_eq!(run.boot.boot, boot::RESTORED, "{what}: boot code");
        assert_eq!(run.boot.build, build, "{what}: ROM build");
        assert_eq!(
            run.boot.hash, native.hashes[f as usize],
            "{what}: boot hash"
        );
        compare_run(&what, &run, &native, keys);
        print_memory(&what, &run.end);
        check_costs(&what, &run.end);
    }
}

/// A visible frame known on the Act 1 trace to apply late, or to stay on time but over
/// [`LOOP_LINES_MAX`], with its cause. A late apply shifts the ROM's input sampling (it consumes
/// another frame's keys from then on), so a replay cannot go on past it; an overrun would fail
/// the cost gate. Either way the long lane stops [`LATE_MARGIN`] frames before `frame` and
/// resumes on the ROM at the first capturable frame after it (from native's checkpoint). Each
/// entry is checked to still be late or over the gate; remove it once it fits.
struct KnownLate {
    /// The engine frame the ROM reports the late apply at.
    frame: u32,
    cause: &'static str,
}

const KNOWN_LATE: &[KnownLate] = &[KnownLate {
    frame: 99_942,
    cause: "Roxanne's battle start on the spine trace (`BattleStarted` at frame 99,940; the \
            step to 99,941, `Battle::start` with the first `NewTurn`'s gym-leader AI choice, is \
            the act's most expensive battle step: 2.96 us native against a 0.21 us median). On \
            the old `act1:*` trace the same start sat at 80,366: late before the engine fast \
            paths (`7134c3e`, 223 lines), then 212 lines with a later step of the battle at \
            81,769 over the loop gate (225 lines). On this build (after Wave C's move effects, \
            abilities and AI cases; not bisected) the start overruns the frame again. No \
            capturable frame follows \
            it before the end of the act, so the long lane compares frames 0-99,938 and the \
            `spine:act1:badge1` resume lane covers the end (Findings for M9)",
}];

/// Frames a ROM piece stops before a [`KnownLate`] frame: the late step runs two engine frames
/// in one emulator frame, so the ROM reaches `frame` straight from `frame - 2`.
const LATE_MARGIN: u32 = 4;

/// The ROM runs `(start, end)` covering `from..=to`, cut around the [`KNOWN_LATE`] frames in it:
/// each piece ends [`LATE_MARGIN`] frames before a known late apply, and the next starts at the
/// first capturable frame after it (none if the act is not capturable again by `to`).
fn act1_pieces(native_capturable: &[bool], from: u32, to: u32) -> Vec<(u32, u32)> {
    let mut pieces = Vec::new();
    let mut start = Some(from);
    for k in KNOWN_LATE
        .iter()
        .filter(|k| k.frame > from && k.frame <= to)
    {
        let Some(s) = start else { break };
        if s < k.frame - LATE_MARGIN {
            pieces.push((s, k.frame - LATE_MARGIN));
        }
        start = (k.frame..to).find(|&n| native_capturable[n as usize]);
    }
    if let Some(s) = start
        && s < to
    {
        pieces.push((s, to));
    }
    pieces
}

/// `cargo xtask parity --long` (Milestone 8 Task P): the whole Act 1 trace on the ROM from a
/// new game, every hash and a picture every [`PICTURE_EVERY`] frames compared with native, no
/// late frame, black-screen late applies only after bulk steps (at most one per bulk event),
/// and the cost and memory gates, except around [`KNOWN_LATE`] (the run stops before each and
/// resumes from native's checkpoint at the next capturable frame; each is checked to still
/// happen). Then each segment again on its own (the first from a new game, the others from their
/// parent node's `.sav`), for the worst lines and memory marks of each. The plain
/// `cargo xtask parity` skips it.
#[test]
#[ignore = "needs target/game.gba; run `cargo xtask parity --long`"]
fn act1_long_whole_trace_from_a_new_game() {
    let Some(act) = act1::load() else {
        println!("act 1: {} build, no act1 nodes", content::FLAVOR.name());
        return;
    };
    let keys = |f| act.keys_at(f);
    let end = act.frames();
    let pictures: Vec<u32> = (1..=end / PICTURE_EVERY)
        .map(|i| i * PICTURE_EVERY)
        .collect();
    let capturable = native_run(keys, end, &[], &[]).capturable;
    let whole = act1_pieces(&capturable, 0, end);
    let segments: Vec<(String, Vec<(u32, u32)>)> = (act.nodes.iter().enumerate())
        .map(|(i, (label, to))| {
            let from = if i == 0 { 0 } else { act.nodes[i - 1].1 };
            (
                format!("act 1 segment to {label}"),
                act1_pieces(&capturable, from, *to),
            )
        })
        .collect();
    let starts: BTreeSet<u32> = (whole.iter().chain(segments.iter().flat_map(|s| &s.1)))
        .map(|p| p.0)
        .filter(|&s| s > 0)
        .collect();
    let starts: Vec<u32> = starts.into_iter().collect();
    let native = native_run(keys, end, &pictures, &starts);
    let build = build_hash(&content::Emerald::for_shells());
    let piece = |what: &str, (from, to): (u32, u32)| {
        let what = format!("{what} (frames {from}..={to})");
        let sav = (from > 0).then(|| {
            flash::encode_sav(&native.checkpoints[&from]).unwrap_or_else(|e| panic!("{what}: {e}"))
        });
        let pics: Vec<u32> = (pictures.iter().copied())
            .filter(|&c| c > from && c <= to)
            .collect();
        let t = std::time::Instant::now();
        let run = run_rom_from(&rom_path(), sav.as_deref(), Some(from), keys, to, &pics)
            .unwrap_or_else(|e| panic!("{what}: ROM run failed: {e:#}"));
        assert_eq!(
            run.boot.boot,
            if from == 0 {
                boot::NEW_GAME
            } else {
                boot::RESTORED
            },
            "{what}: boot code"
        );
        assert_eq!(run.boot.build, build, "{what}: ROM build");
        assert_eq!(run.consumed.len() as u32, to - from, "{what}: steps run");
        compare_run(&what, &run, &native, keys);
        println!(
            "{what}: {} frames in {:.1} s",
            to - from,
            t.elapsed().as_secs_f64()
        );
        print_memory(&what, &run.end);
        check_costs(&what, &run.end);
        run
    };
    // Bulk events: runs of consecutive bulk steps.
    let bulk_events = native.bulk.windows(2).filter(|w| w[1] != w[0] + 1).count()
        + usize::from(!native.bulk.is_empty());
    let mut late_black = 0;
    for &p in &whole {
        late_black += piece("act 1", p).late_black;
    }
    assert!(
        late_black as usize <= bulk_events,
        "act 1: {late_black} late applies on a black screen for {bulk_events} bulk events"
    );
    let replayed: u32 = whole.iter().map(|p| p.1 - p.0).sum();
    println!("act 1: {replayed} of {end} frames replayed on the ROM, {bulk_events} bulk events");
    for (what, pieces) in &segments {
        for &p in pieces {
            piece(what, p);
        }
    }
    // Each known late apply is still late or over the loop gate: from the piece before it,
    // one frame further.
    for k in KNOWN_LATE {
        let &(from, _) = whole
            .iter()
            .chain(segments.iter().flat_map(|s| &s.1))
            .filter(|p| p.1 + LATE_MARGIN == k.frame)
            .min_by_key(|p| p.1 - p.0)
            .expect("a piece ends before each known late apply");
        let sav = (from > 0).then(|| flash::encode_sav(&native.checkpoints[&from]).unwrap());
        let how = match run_rom_from(&rom_path(), sav.as_deref(), Some(from), keys, k.frame, &[]) {
            // The exact frame: a late apply that moves (earlier or later) fails here too.
            Err(e) => {
                let err = format!("{e:#}");
                let want = format!("ROM applied a frame late by engine frame {} ", k.frame);
                assert!(
                    err.starts_with(&want),
                    "KNOWN_LATE at frame {}: another failure ({err}): fix or update it",
                    k.frame
                );
                format!("late: {err}")
            }
            Ok(run) => {
                let lines = run.end.loop_lines_max;
                assert!(
                    lines > LOOP_LINES_MAX,
                    "KNOWN_LATE at frame {} is on time and within the loop gate ({lines} lines): \
                     remove it",
                    k.frame
                );
                format!("on time, loop {lines} lines (gate {LOOP_LINES_MAX})")
            }
        };
        println!(
            "known late apply at frame {} (still {how}): {}",
            k.frame, k.cause
        );
    }
}
