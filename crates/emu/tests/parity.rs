//! Drives the real ROM in mGBA, records the inputs the ROM actually consumed, replays them
//! natively, and requires identical World hash and framebuffer at chosen checkpoints.
//! Needs target/game.gba: run via `cargo xtask parity`.
//!
//! Display lag (see `engine::debug`): when the emulator samples engine frame `f`, the hash is
//! the state after step `f` but the picture is the `Frame` of step `f - 1`. So a checkpoint at
//! `f` compares the ROM hash with native after `f` steps and the ROM picture with native after
//! `f - 1` steps.

use emu::Emu;
use engine::Buttons;
use engine::debug::{INPUT_RING, input_slot};
use game_native::Session;
use std::path::Path;

/// (keys, engine frames) segments.
type Script = [(Buttons, u32)];

const SCRIPT: &Script = &[
    (Buttons::RIGHT, 40),
    (Buttons::DOWN, 30),
    (Buttons(Buttons::RIGHT.0 | Buttons::DOWN.0), 20),
    (Buttons::LEFT, 15),
    (Buttons::UP, 10),
    (Buttons::A, 5),
    (Buttons::NONE, 40),
];

/// Engine frames to compare at (>= 1): mid-scroll in each direction, diagonal, idle, end.
const CHECKPOINTS: &[u32] = &[1, 2, 25, 41, 60, 85, 100, 112, 120, 160];

fn keys_for(script: &Script, frame: u32) -> Buttons {
    let mut f = frame;
    for &(k, n) in script {
        if f < n {
            return k;
        }
        f -= n;
    }
    Buttons::NONE
}

/// What the ROM showed when the emulator sampled engine frame `frame`.
struct Capture {
    frame: u32,
    fb: Vec<u16>,
}

struct RomRun {
    /// Input consumed by step `i + 1`.
    consumed: Vec<Buttons>,
    /// (engine frame, World hash) at every sample, including checkpoints.
    hashes: Vec<(u32, u64)>,
    captures: Vec<Capture>,
}

/// Runs the ROM through `script`, recording the hash at every sample and the picture at each
/// checkpoint.
///
/// Panics if the ROM steps past a checkpoint between two samples (it ran two engine steps in
/// one emulator frame), since that picture cannot be captured, or if the ROM reports a late
/// frame apply (`DebugRegion::late`), since its pictures may then differ from native.
fn run_rom(rom: &Path, script: &Script, checkpoints: &[u32]) -> RomRun {
    let mut emu = Emu::open(rom).expect("open rom");
    let mut snap = emu.boot(120).expect("boot");
    let total: u32 = script.iter().map(|s| s.1).sum();
    assert!(
        checkpoints.windows(2).all(|w| w[0] < w[1]),
        "checkpoints must ascend"
    );
    assert!(checkpoints.iter().all(|&c| (1..=total).contains(&c)));

    let mut consumed: Vec<Buttons> = Vec::new();
    let mut hashes = Vec::new();
    let mut captures = Vec::new();
    let mut next = checkpoints.iter().peekable();
    let mut guard = 0;
    while snap.frame < total {
        emu.run_frame(keys_for(script, snap.frame)).unwrap();
        snap = emu.debug().unwrap();
        let have = consumed.len() as u32;
        assert!(
            snap.frame - have <= INPUT_RING as u32,
            "ROM ran >64 steps between reads"
        );
        for f in have + 1..=snap.frame {
            consumed.push(Buttons(snap.inputs[input_slot(f)]));
        }
        assert_eq!(
            snap.late, 0,
            "ROM applied a frame late by engine frame {}",
            snap.frame
        );
        if snap.frame > have {
            hashes.push((snap.frame, snap.hash));
        }
        if let Some(&&c) = next.peek() {
            assert!(
                snap.frame <= c,
                "ROM jumped from frame {have} to {} past checkpoint {c}",
                snap.frame
            );
            if snap.frame == c {
                captures.push(Capture {
                    frame: c,
                    fb: emu.framebuffer_rgb555(),
                });
                next.next();
            }
        }
        guard += 1;
        assert!(
            guard < total * 3,
            "ROM stopped advancing at frame {}",
            snap.frame
        );
    }
    println!(
        "ROM reached engine frame {} after {guard} emulator frames; {} inputs consumed; hash {:#018x}",
        snap.frame,
        consumed.len(),
        snap.hash
    );
    RomRun {
        consumed,
        hashes,
        captures,
    }
}

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

#[test]
#[ignore = "needs target/game.gba; run `cargo xtask parity`"]
fn rom_and_native_agree() {
    let rom = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/game.gba");
    let run = run_rom(&rom, SCRIPT, CHECKPOINTS);

    // Pin the button mapping: native replays what the ROM recorded, so also require that the
    // ROM recorded exactly what the script sent (catches KEYINPUT/setKeys bit-order regressions).
    for (i, &k) in run.consumed.iter().enumerate() {
        assert_eq!(
            k,
            keys_for(SCRIPT, i as u32),
            "ROM consumed a different input at step {}",
            i + 1
        );
    }
    assert_eq!(run.captures.len(), CHECKPOINTS.len());
    compare_native(&run, Session::new(content::Emerald));
    println!(
        "{} hashes and {} pictures identical, no late frames",
        run.hashes.len(),
        run.captures.len()
    );
}
