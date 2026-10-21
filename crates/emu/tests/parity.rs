//! Drives the real ROM in mGBA through the scripted route (`game_native::scenario`), records
//! the inputs the ROM actually consumed, replays them natively, and requires the same World
//! hash at every sample and the same picture at every checkpoint.
//! Needs target/game.gba: run via `cargo xtask parity`.
//!
//! The route depends on the content build: with extracted data it walks Littleroot → Brendan's
//! house and back → Route 101 → Oldale → the mart employee's guided tour → an NPC → the Pokémon
//! Center (about 2900 frames);
//! in stub mode it walks around the built-in map. The ROM must be built from the same content
//! as this test (xtask builds both from the same tree).
//!
//! Display lag (see `engine::debug`): when the emulator samples engine frame `f`, the hash is
//! the state after step `f` but the picture is the `Frame` of step `f - 1`. So a checkpoint at
//! `f` compares the ROM hash with native after `f` steps and the ROM picture with native after
//! `f - 1` steps.

use emu::{RomRun, run_rom};
use game_native::Session;
use game_native::scenario::{Recording, record_route};
use std::path::Path;

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

fn recording() -> Recording {
    record_route().unwrap_or_else(|e| panic!("the route does not play natively: {e}"))
}

#[test]
#[ignore = "needs target/game.gba; run `cargo xtask parity`"]
fn rom_and_native_agree() {
    let rom = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/game.gba");
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
        "ROM reached engine frame {} after {} emulator frames; {} inputs consumed; {} late applies while black",
        run.hashes.last().map_or(0, |h| h.0),
        run.emu_frames,
        run.consumed.len(),
        run.late_black,
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
    // Black-screen late applies are only excused for map loads: at most one per warp.
    assert!(
        run.late_black <= rec.warps,
        "{} late applies on a black screen for {} warps",
        run.late_black,
        rec.warps
    );
    compare_native(&run, Session::new(content::Emerald));
    println!(
        "{} hashes and {} pictures identical, no late frames",
        run.hashes.len(),
        run.captures.len()
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
