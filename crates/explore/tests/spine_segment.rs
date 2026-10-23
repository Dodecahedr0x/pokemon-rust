//! A development aid for the long Emerald spine (ignored; run by hand, release):
//!
//! - `PKR_SPINE_DIR=<dir> PKR_SPINE_SAVE_AT=815,1185 cargo test --release -p explore --test
//!   spine_segment save -- --ignored --nocapture` plays the whole spine and saves the
//!   positions at those step indices (`<dir>/pos-<i>.bin` and `.opts`);
//! - `PKR_SPINE_DIR=<dir> PKR_SPINE_FROM=815 PKR_SPINE_TO=830 cargo test --release -p explore
//!   --test spine_segment replay -- --ignored --nocapture` replays steps `FROM..TO` from the
//!   saved position (`spine::run_from`; the build hash is not checked), printing each step's
//!   end and party; with `PKR_SPINE_TRACE=1` it also replays the inputs frame by frame and
//!   prints each change of map, cell, battle or script.

use content::Emerald;
use explore::config::Config;
use explore::spine::{self, Start, StepOpts};
use std::path::PathBuf;
use std::sync::Arc;

fn dir() -> PathBuf {
    PathBuf::from(std::env::var("PKR_SPINE_DIR").expect("PKR_SPINE_DIR"))
}

fn cp_for(from: usize) -> engine::checkpoint::Checkpoint {
    let mut cp = engine::checkpoint::decode(
        &std::fs::read(dir().join(format!("pos-{from}")).with_extension("bin")).unwrap(),
    )
    .expect("a checkpoint");
    cp.header.build = engine::checkpoint::BuildHash {
        engine: 0,
        content: 0,
    };
    cp
}

/// Replays `inputs` from `cp`, printing each change of map, cell or battle.
fn trace(game: &Emerald, cp: &engine::checkpoint::Checkpoint, inputs: &[game_native::trace::Run]) {
    let mut w = engine::World::restore(game, cp).expect("restore");
    let mut last = None;
    let mut f = 0u64;
    for r in inputs {
        for _ in 0..r.frames {
            w.step(game, r.keys);
            f += 1;
            let now = (
                w.map(),
                w.objects().player().pos(),
                w.in_battle(),
                w.script_running(),
            );
            if last != Some(now) {
                println!("  trace {f} {:?} keys {:?}", now, r.keys);
                last = Some(now);
            }
        }
    }
}

fn num(var: &str) -> Option<usize> {
    std::env::var(var).ok().and_then(|v| v.parse().ok())
}

#[test]
#[ignore]
fn save() {
    let at: Vec<usize> = std::env::var("PKR_SPINE_SAVE_AT")
        .expect("PKR_SPINE_SAVE_AT")
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let to = num("PKR_SPINE_TO").unwrap_or(spine::steps().len());
    let s = spine::run(
        &Emerald::for_shells(),
        &spine::steps()[..to],
        &Config::default(),
    );
    println!(
        "played to step {} of {to}, {} frames, error {:?}",
        s.positions.len().saturating_sub(1),
        s.frames,
        s.error.as_ref().map(|e| e.to_string())
    );
    std::fs::create_dir_all(dir()).unwrap();
    for p in &s.positions {
        if at.contains(&p.index) {
            let base = dir().join(format!("pos-{}", p.index));
            std::fs::write(
                base.with_extension("bin"),
                engine::checkpoint::encode(&p.end),
            )
            .unwrap();
            let o = p.opts;
            let opts = format!(
                "{} {} {} {} {}",
                o.run_wild as u8,
                o.best_move as u8,
                o.learn as u8,
                o.choice.map_or(255, u16::from),
                p.frame
            );
            std::fs::write(base.with_extension("opts"), opts).unwrap();
            println!(
                "saved position {} ({:?} at {:?})",
                p.index,
                p.map(),
                p.pos()
            );
        }
    }
}

#[test]
#[ignore]
fn replay() {
    let from = num("PKR_SPINE_FROM").expect("PKR_SPINE_FROM");
    let to = num("PKR_SPINE_TO").unwrap_or(spine::steps().len());
    let base = dir().join(format!("pos-{from}"));
    let cp = engine::checkpoint::decode(&std::fs::read(base.with_extension("bin")).unwrap())
        .expect("a checkpoint");
    let o: Vec<u64> = std::fs::read_to_string(base.with_extension("opts"))
        .unwrap()
        .split_whitespace()
        .map(|s| s.parse().unwrap())
        .collect();
    let start = Start {
        end: Arc::new(cp),
        index: from,
        opts: StepOpts {
            run_wild: o[0] != 0,
            best_move: o[1] != 0,
            learn: o[2] != 0,
            choice: (o[3] != 255).then_some(o[3] as u8),
        },
        frame: o[4],
    };
    let s = spine::run_from(
        &Emerald::for_shells(),
        &spine::steps()[..to],
        &Config::default(),
        Some(start),
    );
    let game = Emerald::for_shells();
    for p in &s.positions {
        // The party: species, level and HP per mon.
        let party: Vec<(u16, u8, u16)> = engine::World::restore(&game, &p.end)
            .map(|w| {
                w.party()
                    .iter()
                    .map(|m| (m.species.0, m.level, m.hp))
                    .collect()
            })
            .unwrap_or_default();
        println!(
            "{:5} {:?} {:?} {:?} frame {} party {:?}",
            p.index,
            p.label,
            p.map(),
            p.pos(),
            p.frame,
            party
        );
    }
    for r in &s.reports {
        println!("report {r:?}");
    }
    if std::env::var("PKR_SPINE_TRACE").is_ok() {
        trace(&game, &cp_for(from), &s.inputs);
    }
    if let Some(e) = &s.error {
        panic!("{e}");
    }
    println!("bypasses {:?}", s.bypasses());
}
