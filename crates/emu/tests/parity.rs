//! Drives the real ROM in mGBA through the scripted route (`game_native::scenario`), records
//! the inputs the ROM actually consumed, replays them natively, and requires the same World
//! hash at every sample and the same picture at every checkpoint.
//! Needs target/game.gba: run via `cargo xtask parity`.
//!
//! The route depends on the content build: with extracted data it plays a new game from
//! Littleroot → Birch's rescue on Route 101 → the starter screen and the first battle → the
//! lab → wild battles in Route 101's grass (a win, a run, a level-up) → Oldale's Pokémon
//! Center → May's battle on Route 103 (about 19000 frames);
//! in stub mode it walks around the built-in map and fights in its tall-grass cell. The ROM
//! must be built from the same content as this test (xtask builds both from the same tree).
//!
//! The ROM may overrun a frame only on a black screen and only for bulk work (`Frame::bulk`):
//! every `late_black` must follow a bulk step within `scenario::DISPLAY_LAG` steps, and there
//! are at most as many as bulk events (`Recording::bulk_events`: runs of bulk steps: map
//! loads, whole-view redraws, starter loads, battle loads and restores).
//!
//! Display lag (see `engine::debug`): when the emulator samples engine frame `f`, the hash is
//! the state after step `f` but the picture is the `Frame` of step `f - 1`. So a checkpoint at
//! `f` compares the ROM hash with native after `f` steps and the ROM picture with native after
//! `f - 1` steps.

extern crate alloc;

#[path = "../../game-gba/src/probe_scene.rs"]
mod probe_scene;

use emu::{DebugSnapshot, Emu, RomRun, run_rom, run_rom_from};
use engine::Buttons;
use engine::checkpoint::{Checkpoint, build_hash};
use engine::debug::{boot, save};
use game_native::scenario::{DISPLAY_LAG, Recording, record_route};
use game_native::{Ppu, Session, flash};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// A picture is compared at least this often (and at every named moment).
const CHECKPOINT_EVERY: u32 = 100;

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
    assert!(caps.next().is_none(), "checkpoint past the consumed inputs");
    assert!(
        hashes.next().is_none(),
        "hash sample past the consumed inputs"
    );
}

/// Writes target/parity-<frame>-{rom,native}.ppm for a failed checkpoint.
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
/// late apply); `LOOP_LINES_MAX` keeps two lines of the 228-line frame in reserve.
const HASH_LINES_MAX: u32 = 80;
const LOOP_LINES_MAX: u32 = 226;
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
}

fn rom_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/game.gba")
}

fn recording() -> Recording {
    record_route().unwrap_or_else(|e| panic!("the route does not play natively: {e}"))
}

#[test]
#[ignore = "needs target/game.gba; run `cargo xtask parity`"]
fn rom_and_native_agree() {
    let rom = rom_path();
    let rec = recording();
    let checkpoints = rec.checkpoints(CHECKPOINT_EVERY);
    println!(
        "{} route: {} frames, {} checkpoints, moments {:?}",
        if content::STUB { "stub" } else { "extracted" },
        rec.frames(),
        checkpoints.len(),
        rec.moments
    );
    let run = run_rom(&rom, |f| rec.keys_at(f), rec.frames(), &checkpoints)
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
    assert_eq!(run.captures.len(), checkpoints.len());
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
        bulk: Vec::new(),
        pictures: BTreeMap::new(),
        checkpoints: BTreeMap::new(),
    };
    for n in 0..=end {
        out.hashes.push(s.world().hash());
        out.capturable.push(s.world().capturable().is_ok());
        out.respawn.push(s.world().respawn());
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
/// picture, and each black-screen late apply after a bulk step.
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
    for &late in &run.late_black_frames {
        assert!(
            native
                .bulk
                .iter()
                .any(|&b| b < late && late - b <= DISPLAY_LAG),
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
    let may = moment("may_intro");
    let before_may = (0..may)
        .rev()
        .find(|&n| capturable(n))
        .expect("nothing capturable before May's battle");
    vec![
        ("lab", first_from(moment("lab"))),
        ("oldale_center", healed),
        ("before_may", before_may),
    ]
}

/// Frames a ROM run resumed from a checkpoint follows the route (at most).
const RESUME_FRAMES: u32 = 2000;

/// Picture frames of a run from `start` to `end`: the first two, then every
/// [`CHECKPOINT_EVERY`] frames.
fn resume_pictures(start: u32, end: u32) -> Vec<u32> {
    let every = (start / CHECKPOINT_EVERY + 1..).map(|i| i * CHECKPOINT_EVERY);
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
    let rom = rom_path();
    let rec = recording();
    let keys = |f| rec.keys_at(f);
    let scan = native_run(keys, rec.frames(), &[], &[]);
    let nodes = resume_nodes(&rec, &scan);
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
    let rom = rom_path();
    let rec = recording();
    let scan = native_run(|f| rec.keys_at(f), rec.frames(), &[], &[]);
    let f = resume_nodes(&rec, &scan)[0].1;
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
    assert_eq!(run.end.saves, 1, "flash writes");
    assert_eq!(run.end.save_status, save::OK, "save status");
    let rom_hash = run
        .hashes
        .iter()
        .find(|h| h.0 == saved)
        .unwrap_or_else(|| panic!("no ROM sample at frame {saved}"))
        .1;
    let (meta, cp) = flash::decode_sav(&run.save).unwrap_or_else(|e| panic!("ROM flash: {e}"));
    assert_eq!(cp.header.frame, saved);
    assert_eq!(meta.frame, saved);
    assert_eq!(cp.header.world_hash, rom_hash);
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
