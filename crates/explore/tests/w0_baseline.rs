//! Wave 0 baseline: native frames/s on one core, replaying the parity route.
//!
//! `cargo test -p explore --release --test w0_baseline -- --ignored --nocapture`

use content::Emerald;
use game_native::Session;
use game_native::scenario::cached_route;
use std::time::Instant;

fn frames_per_sec(mut step: impl FnMut(usize), frames: usize, reps: u32) -> f64 {
    let t = Instant::now();
    for _ in 0..reps {
        for i in 0..frames {
            step(i);
        }
    }
    (frames as f64 * f64::from(reps)) / t.elapsed().as_secs_f64()
}

#[test]
#[ignore = "timing; run in release"]
fn baseline_frames_per_sec() {
    let rec = cached_route();
    let n = rec.keys.len();
    let reps = 5;
    let mut best_plain: f64 = 0.0;
    let mut best_events: f64 = 0.0;
    for _ in 0..3 {
        let mut plain = None;
        best_plain = best_plain.max(frames_per_sec(
            |i| {
                if i == 0 {
                    plain = Some(Session::new(Emerald::for_shells()));
                }
                plain.as_mut().unwrap().step(rec.keys[i]);
            },
            n,
            reps,
        ));
        let mut events = None;
        best_events = best_events.max(frames_per_sec(
            |i| {
                if i == 0 {
                    events = Some(Session::new(Emerald::for_shells()));
                }
                let s = events.as_mut().unwrap();
                s.step(rec.keys[i]);
                std::hint::black_box(s.world().events().len());
            },
            n,
            reps,
        ));
    }
    println!(
        "route {n} frames (stub: {}), {reps} reps, best of 3: Session {best_plain:.0} frames/s, \
         Session reading events {best_events:.0} frames/s",
        content::STUB
    );
}
