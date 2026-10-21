//! Drives the real ROM in mGBA, records the inputs the ROM actually consumed,
//! replays them natively, and requires identical World hash and framebuffer.
//! Needs target/game.gba: run via `cargo xtask parity`.

use emu::Emu;
use engine::Buttons;
use engine::debug::{INPUT_RING, input_slot};
use game_native::Session;
use std::path::Path;

/// (keys, engine frames) segments; must end with an idle tail so the image is stable.
const SCRIPT: &[(Buttons, u32)] = &[
    (Buttons::RIGHT, 40),
    (Buttons::DOWN, 30),
    (Buttons(Buttons::RIGHT.0 | Buttons::DOWN.0), 20),
    (Buttons::LEFT, 15),
    (Buttons::UP, 10),
    (Buttons::A, 5),
    (Buttons::NONE, 40),
];

fn keys_for(frame: u32) -> Buttons {
    let mut f = frame;
    for &(k, n) in SCRIPT {
        if f < n {
            return k;
        }
        f -= n;
    }
    Buttons::NONE
}

#[test]
#[ignore = "needs target/game.gba; run `cargo xtask parity`"]
fn rom_and_native_agree() {
    let rom = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/game.gba");
    let mut emu = Emu::open(&rom).expect("open rom");
    let mut snap = emu.boot(120).expect("boot");
    let total: u32 = SCRIPT.iter().map(|s| s.1).sum();

    let mut consumed: Vec<Buttons> = Vec::new();
    let mut guard = 0;
    while snap.frame < total {
        emu.run_frame(keys_for(snap.frame)).unwrap();
        snap = emu.debug().unwrap();
        let have = consumed.len() as u32;
        assert!(
            snap.frame - have <= INPUT_RING as u32,
            "ROM ran >64 steps between reads"
        );
        for f in have + 1..=snap.frame {
            consumed.push(Buttons(snap.inputs[input_slot(f)]));
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

    // Pin the button mapping: native replays what the ROM recorded, so also require that the
    // ROM recorded exactly what the script sent (catches KEYINPUT/setKeys bit-order regressions).
    for (i, &k) in consumed.iter().enumerate() {
        assert_eq!(
            k,
            keys_for(i as u32),
            "ROM consumed a different input at step {}",
            i + 1
        );
    }

    let mut native = Session::new(content::Emerald);
    for &k in &consumed {
        native.step(k);
    }
    assert_eq!(native.world().frame(), snap.frame);
    assert_eq!(native.world().hash(), snap.hash, "World hash diverged");

    let rom_fb = emu.framebuffer_rgb555();
    let diff = rom_fb
        .iter()
        .zip(native.framebuffer())
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(diff, 0, "{diff} pixels differ between ROM and native");
}
