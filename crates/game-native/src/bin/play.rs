//! Plays the game natively in a window (feature `window`).
//!
//! Keys: arrows = D-pad, X = A, Z = B, Enter = Start, Backspace = Select, Escape quits.
//! The window is 4x the GBA screen; the game runs at the window's 60 fps target.

use engine::Buttons;
use game_native::{SCREEN_H, SCREEN_W, Session};
use minifb::{Key, Scale, Window, WindowOptions};

const KEYS: [(Key, Buttons); 8] = [
    (Key::Up, Buttons::UP),
    (Key::Down, Buttons::DOWN),
    (Key::Left, Buttons::LEFT),
    (Key::Right, Buttons::RIGHT),
    (Key::X, Buttons::A),
    (Key::Z, Buttons::B),
    (Key::Enter, Buttons::START),
    (Key::Backspace, Buttons::SELECT),
];

fn rgb555_to_0rgb(c: u16) -> u32 {
    let ch = |v: u16| {
        let v = (v & 31) as u32;
        (v << 3) | (v >> 2)
    };
    ch(c) << 16 | ch(c >> 5) << 8 | ch(c >> 10)
}

fn main() {
    let mut window = Window::new(
        "pokemon-rust",
        SCREEN_W,
        SCREEN_H,
        WindowOptions {
            scale: Scale::X4,
            ..WindowOptions::default()
        },
    )
    .expect("open a window");
    window.set_target_fps(60);
    let mut session = Session::new(content::Emerald);
    let mut buf = vec![0u32; SCREEN_W * SCREEN_H];
    while window.is_open() && !window.is_key_down(Key::Escape) {
        let keys = KEYS
            .iter()
            .filter(|(k, _)| window.is_key_down(*k))
            .fold(Buttons::NONE, |acc, &(_, b)| Buttons(acc.0 | b.0));
        session.step(keys);
        for (dst, &c) in buf.iter_mut().zip(session.framebuffer()) {
            *dst = rgb555_to_0rgb(c);
        }
        window
            .update_with_buffer(&buf, SCREEN_W, SCREEN_H)
            .expect("draw the frame");
    }
}
