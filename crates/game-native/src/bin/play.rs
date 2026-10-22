//! Plays the game natively in a window (feature `window`).
//!
//! Keys: arrows = D-pad, X = A, Z = B, A = L, S = R, Enter = Start, Backspace = Select,
//! F5 saves a checkpoint, F9 reloads the current one, F8 rewinds, Escape quits.
//! The window is 4x the GBA screen; the game runs at the window's 60 fps target.
//!
//! `play --load <id|label>` starts at a checkpoint node, `play --list` prints the tree. The
//! store is `checkpoints/{stub|emerald}` (or `$PKR_CHECKPOINTS`), as for `cargo xtask
//! checkpoints`.

use engine::Buttons;
use game_native::checkpoints::materialize;
use game_native::controls::{Controls, SaveRequest};
use game_native::store::Store;
use game_native::trace::{CheckpointId, Parent};
use game_native::{SCREEN_H, SCREEN_W};
use minifb::{Key, KeyRepeat, Scale, Window, WindowOptions};
use std::io::ErrorKind;
use std::process::ExitCode;

const KEYS: [(Key, Buttons); 10] = [
    (Key::Up, Buttons::UP),
    (Key::Down, Buttons::DOWN),
    (Key::Left, Buttons::LEFT),
    (Key::Right, Buttons::RIGHT),
    (Key::X, Buttons::A),
    (Key::Z, Buttons::B),
    (Key::A, Buttons::L),
    (Key::S, Buttons::R),
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

/// Adds the save, retrying with a suffix when its label is taken.
fn add(store: &Store, req: &SaveRequest) -> std::io::Result<CheckpointId> {
    let mut label = req.label.clone();
    for n in 2.. {
        match store.add(&label, req.trace.clone(), &req.checkpoint) {
            Err(e) if e.kind() == ErrorKind::AlreadyExists => label = format!("{} #{n}", req.label),
            r => return r,
        }
    }
    unreachable!()
}

fn save(store: &Store, c: &mut Controls<content::Emerald>, req: Option<SaveRequest>) {
    if let Some(req) = req {
        match add(store, &req) {
            Ok(id) => c.saved(id),
            Err(e) => c.note(format!("save failed: {e}")),
        }
    }
}

fn load(store: &Store, c: &mut Controls<content::Emerald>, id: CheckpointId) -> Result<(), String> {
    let cp = materialize(store, &content::Emerald::for_shells(), id)?;
    c.load(id, &cp).map_err(|e| format!("restore: {e:?}"))
}

fn list(store: &Store) -> std::io::Result<()> {
    for n in store.topo_order()? {
        let parent = match n.trace.parent {
            Parent::NewGame => "new game".to_string(),
            Parent::Checkpoint(p) => p.to_string(),
        };
        println!(
            "{}  {:>6}  map {:#06x}  from {:<8}  {}",
            n.meta.id, n.meta.frame, n.meta.map.0, parent, n.meta.label
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let store = match Store::open(Store::default_root(content::STUB)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("checkpoint store: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut controls = Controls::new(content::Emerald::for_shells());
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        [] => {}
        ["--list"] => {
            return match list(&store) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            };
        }
        ["--load", node] => {
            let r = store
                .find(node)
                .map_err(|e| e.to_string())
                .and_then(|n| load(&store, &mut controls, n.meta.id));
            if let Err(e) = r {
                eprintln!("--load {node}: {e}");
                return ExitCode::FAILURE;
            }
        }
        _ => {
            eprintln!("usage: play [--load <id|label> | --list]");
            return ExitCode::FAILURE;
        }
    }
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
    let mut buf = vec![0u32; SCREEN_W * SCREEN_H];
    let mut title = String::new();
    while window.is_open() && !window.is_key_down(Key::Escape) {
        if window.is_key_pressed(Key::F5, KeyRepeat::No) {
            let req = controls.save();
            save(&store, &mut controls, req);
        }
        if window.is_key_pressed(Key::F9, KeyRepeat::No) {
            match controls.parent() {
                Parent::Checkpoint(id) => {
                    if let Err(e) = load(&store, &mut controls, id) {
                        controls.note(format!("load failed: {e}"));
                    }
                }
                Parent::NewGame => controls.note("no checkpoint to load"),
            }
        }
        if window.is_key_pressed(Key::F8, KeyRepeat::No) {
            controls.rewind();
        }
        let keys = KEYS
            .iter()
            .filter(|(k, _)| window.is_key_down(*k))
            .fold(Buttons::NONE, |acc, &(_, b)| Buttons(acc.0 | b.0));
        let req = controls.step(keys);
        save(&store, &mut controls, req);
        for (dst, &c) in buf.iter_mut().zip(controls.session_mut().framebuffer()) {
            *dst = rgb555_to_0rgb(c);
        }
        let t = controls.title();
        if t != title {
            window.set_title(&t);
            title = t;
        }
        window
            .update_with_buffer(&buf, SCREEN_W, SCREEN_H)
            .expect("draw the frame");
    }
    ExitCode::SUCCESS
}
