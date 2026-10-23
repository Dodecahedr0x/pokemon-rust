//! Task E: the per-frame detectors fire on injected faults (and stay quiet on the route).

use content::Emerald;
use engine::Buttons;
use engine::data::GameData;
use explore::config::DetectConfig;
use explore::detect::Detectors;
use explore::findings::{kind_name, repro_trace, reproduce, signature, signature_of};
use explore::observe::{Flag, StepObserver};
use explore::testing::{Drifting, FadeTrap};
use explore::{FindingDraft, FindingKind};
use game_native::Session;
use game_native::scenario::cached_route;
use game_native::trace::rle;

fn quiet() -> DetectConfig {
    DetectConfig {
        roundtrip_every: 0,
        follow: 0,
        capturable_stuck: 0,
        slow_step_us: 0,
        ..DetectConfig::default()
    }
}

fn draft(kind: FindingKind, message: String, s: &Session<impl GameData>) -> FindingDraft {
    FindingDraft {
        kind,
        message,
        location: None,
        macro_: None,
        tail: Vec::new(),
        map: s.world().map(),
        pos: s.world().objects().player().pos(),
        script: None,
        log: Vec::new(),
    }
}

/// Steps `s` with `drive`'s keys for at most `frames` frames, feeding `det`; the first flag,
/// with every key stepped up to it (inclusive).
fn run<G: GameData + Clone>(
    s: &mut Session<G>,
    det: &mut Detectors<G>,
    frames: u32,
    mut drive: impl FnMut(&Session<G>) -> Buttons,
    keys: &mut Vec<Buttons>,
) -> Option<Flag> {
    for _ in 0..frames {
        let k = drive(s);
        s.step(k);
        keys.push(k);
        if let Some(f) = det.after_step(s, k, 0) {
            return Some(f);
        }
    }
    None
}

fn dir_keys(from: (i16, i16), to: (i16, i16)) -> Buttons {
    if from.0 < to.0 {
        Buttons::RIGHT
    } else if from.0 > to.0 {
        Buttons::LEFT
    } else if from.1 < to.1 {
        Buttons::DOWN
    } else if from.1 > to.1 {
        Buttons::UP
    } else {
        Buttons::NONE
    }
}

/// Walks to the stub's tall-grass column, then paces between the grass cell (12, 3) and
/// (12, 4). In a battle it presses nothing.
fn pace_grass<G: GameData>() -> impl FnMut(&Session<G>) -> Buttons {
    let mut target = (12, 4);
    move |s| {
        let w = s.world();
        if !w.field_active() || w.script_running() {
            return Buttons::NONE;
        }
        let p = w.objects().player();
        if p.pos() == target && p.is_settled() {
            target = if target.1 == 4 { (12, 3) } else { (12, 4) };
        }
        dir_keys(p.pos(), target)
    }
}

#[test]
fn drifting_game_breaks_the_round_trip_follow() {
    if !content::STUB {
        return; // stub geometry (tall grass at (12, 3))
    }
    let cfg = DetectConfig {
        roundtrip_every: 64,
        follow: 120,
        ..quiet()
    };
    let game = Drifting::new(Emerald::for_shells());
    let mut s = Session::new(game.clone());
    let mut det = Detectors::new(&cfg);
    let mut keys = Vec::new();
    let flag = run(&mut s, &mut det, 4000, pace_grass(), &mut keys).expect("a round-trip flag");
    assert_eq!(flag.kind, FindingKind::RoundTrip, "{}", flag.message);
    assert!(flag.message.contains("diverged"), "{}", flag.message);
    assert!(det.round_trips() > 1);

    // The repro (a fresh Drifting from a new game, as above) raises it on its last frame.
    let want = draft(flag.kind, flag.message, &s);
    let trace = repro_trace(&game, &[], &rle(keys.iter().copied())).unwrap();
    assert_eq!(trace.frames() as usize, keys.len());
    reproduce(&game, &trace, &want, &cfg).unwrap();
    let mut shorter = trace.clone();
    shorter.inputs = rle(keys[..keys.len() - 1].iter().copied());
    shorter.hashes.retain(|&(at, _)| at < trace.frames());
    assert!(reproduce(&game, &shorter, &want, &cfg).is_err());

    // The same drive on the plain game: round trips, no flag.
    let game = Emerald::for_shells();
    let mut s = Session::new(game);
    let mut det = Detectors::new(&cfg);
    let n = keys.len() as u32;
    let flag = run(&mut s, &mut det, n, pace_grass(), &mut Vec::new());
    assert_eq!(flag, None);
    assert!(det.round_trips() > 1);
}

#[test]
fn route_raises_nothing_with_the_default_config() {
    let rec = cached_route();
    let n = rec.keys.len();
    let cfg = DetectConfig {
        slow_step_us: 0,
        ..DetectConfig::default()
    };
    let mut s = Session::new(Emerald::for_shells());
    let mut det = Detectors::<Emerald>::new(&cfg);
    for (i, &k) in rec.keys[..n].iter().enumerate() {
        s.step(k);
        assert_eq!(s.world().hash(), rec.hashes[i]);
        if let Some(f) = det.after_step(&s, k, 0) {
            panic!("frame {}: {f:?}", i + 1);
        }
    }
    assert!(det.round_trips() >= 1, "{} frames", n);
}

/// The first walkable neighbour of the new-game start (collision only) and the keys toward it.
fn trap_cell(game: &impl GameData) -> ((i16, i16), Buttons) {
    let ng = game.new_game();
    let m = game.map(ng.map);
    [
        (0, 1, Buttons::DOWN),
        (-1, 0, Buttons::LEFT),
        (1, 0, Buttons::RIGHT),
        (0, -1, Buttons::UP),
    ]
    .into_iter()
    .map(|(dx, dy, k)| ((ng.x + dx, ng.y + dy), k))
    .find(|&((x, y), _)| {
        x >= 0
            && y >= 0
            && x < m.width as i16
            && y < m.height as i16
            && engine::data::block_collision(m.blocks[y as usize * m.width as usize + x as usize])
                == 0
    })
    .expect("a walkable neighbour")
}

#[test]
fn fade_trap_is_capturable_stuck() {
    let inner = Emerald::for_shells();
    let (at, toward) = trap_cell(&inner);
    let game = FadeTrap {
        inner,
        map: inner.new_game().map,
        at,
    };
    let cfg = DetectConfig {
        capturable_stuck: 120,
        ..quiet()
    };
    let mut s = Session::new(game);
    let mut det = Detectors::new(&cfg);
    let mut keys = Vec::new();
    // Settle the new game, then step onto the trap cell and let go.
    let mut settled = false;
    let flag = run(
        &mut s,
        &mut det,
        1500,
        |s| {
            let w = s.world();
            settled |= w.capturable().is_ok();
            if !settled {
                // Through Silice's opening scene; nothing on Emerald.
                game_native::scenario::opening_key(w)
            } else if w.objects().player().pos() != at && !w.script_running() {
                toward
            } else {
                Buttons::NONE
            }
        },
        &mut keys,
    )
    .expect("a capturable-stuck flag");
    assert_eq!(flag.kind, FindingKind::CapturableStuck, "{}", flag.message);
    assert!(flag.message.contains("Fade"), "{}", flag.message);
    assert_eq!(s.world().objects().player().pos(), at);

    let want = draft(flag.kind, flag.message, &s);
    let (a, b) = keys.split_at(keys.len() / 2);
    let (la, lb) = (rle(a.iter().copied()), rle(b.iter().copied()));
    let (lb, tail) = lb.split_at(lb.len() - 1);
    let trace = repro_trace(&game, &[&la, lb], tail).unwrap();
    assert_eq!(trace.frames() as usize, keys.len());
    reproduce(&game, &trace, &want, &cfg).unwrap();
    // A longer threshold does not raise it at the last frame.
    let later = DetectConfig {
        capturable_stuck: 121,
        ..cfg.clone()
    };
    assert!(reproduce(&game, &trace, &want, &later).is_err());
    // Overlap: the same finding raised again later (the count restarts after a flag). The
    // earlier raise is not terminal; only the last frame decides.
    let mut again = keys.clone();
    again.extend([Buttons::NONE; 121]);
    let trace2 = repro_trace(&game, &[], &rle(again.iter().copied())).unwrap();
    reproduce(&game, &trace2, &want, &cfg).unwrap();
    let mut between = keys.clone();
    between.extend([Buttons::NONE; 60]);
    let trace3 = repro_trace(&game, &[], &rle(between.iter().copied())).unwrap();
    assert!(reproduce(&game, &trace3, &want, &cfg).is_err());
    // Every flag of a frame: stuck and slow together.
    let mut det = Detectors::new(&DetectConfig {
        capturable_stuck: 1,
        slow_step_us: 1,
        ..quiet()
    });
    s.step(Buttons::NONE);
    assert_eq!(det.after_step_all(&s, Buttons::NONE, 0), []);
    s.step(Buttons::NONE);
    let kinds: Vec<_> = det
        .after_step_all(&s, Buttons::NONE, 10_000)
        .into_iter()
        .map(|f| f.kind)
        .collect();
    assert_eq!(
        kinds,
        [FindingKind::CapturableStuck, FindingKind::SlowFrame]
    );

    // The plain game is not stuck there.
    let plain = repro_trace(&inner, &[], &rle(keys.iter().copied())).unwrap();
    assert!(reproduce(&inner, &plain, &want, &cfg).is_err());
}

#[test]
fn slow_steps_are_flagged_and_never_reproduced() {
    let cfg = DetectConfig {
        slow_step_us: 5,
        ..quiet()
    };
    let mut s = Session::new(Emerald::for_shells());
    let mut det = Detectors::<Emerald>::new(&cfg);
    s.step(Buttons::NONE);
    assert_eq!(det.after_step(&s, Buttons::NONE, 5_000), None);
    s.step(Buttons::NONE);
    let f = det.after_step(&s, Buttons::NONE, 5_001).expect("slow");
    assert_eq!(f.kind, FindingKind::SlowFrame);
    let want = draft(f.kind, f.message, &s);
    let trace = repro_trace(&Emerald::for_shells(), &[], &rle([Buttons::NONE; 2])).unwrap();
    assert!(reproduce(&Emerald::for_shells(), &trace, &want, &cfg).is_err());
    // Off by default in the quiet config.
    let mut det = Detectors::<Emerald>::new(&quiet());
    assert_eq!(det.after_step(&s, Buttons::NONE, u64::MAX), None);
}

#[test]
fn signatures_ignore_digits_and_follow_location() {
    let s = Session::new(Emerald::for_shells());
    let a = draft(
        FindingKind::CapturableStuck,
        "stuck for 601 frames: Moving(3)".into(),
        &s,
    );
    let mut b = a.clone();
    b.message = "stuck for 750 frames: Moving(4)".into();
    b.pos = (1, 2);
    assert_eq!(signature(&a), signature(&b));
    b.kind = FindingKind::RoundTrip;
    assert_ne!(signature(&a), signature(&b));
    let mut c = a.clone();
    c.map.0 ^= 1;
    assert_ne!(signature(&a), signature(&c));
    // Panics: the location's file and the digit-stripped message, not line:col.
    let mut p = a.clone();
    p.kind = FindingKind::Panic;
    p.message = "index 3 out of range".into();
    p.location = Some("crates/engine/src/x.rs:10:2".into());
    let mut q = p.clone();
    q.message = "index 7 out of range".into();
    q.location = Some("crates/engine/src/x.rs:42:9".into());
    assert_eq!(signature(&p), signature(&q));
    q.location = Some("crates/engine/src/y.rs:10:2".into());
    assert_ne!(signature(&p), signature(&q));
    q.location = p.location.clone();
    q.message = "another message".into();
    assert_ne!(signature(&p), signature(&q));
    assert_eq!(
        signature(&p),
        signature_of(p.kind, &p.message, p.location.as_deref(), p.map, p.script)
    );
    assert_eq!(kind_name(FindingKind::RoundTrip), "round_trip");
}

#[test]
fn a_load_mid_run_is_a_discontinuity_not_a_round_trip_break() {
    let cfg = DetectConfig {
        roundtrip_every: 16,
        follow: 60,
        capturable_stuck: 600,
        ..quiet()
    };
    let mut s = Session::new(Emerald::for_shells());
    game_native::scenario::to_field(&mut s).unwrap_or_else(|e| panic!("{e}"));
    let early = s.checkpoint().expect("capturable");
    let mut det = Detectors::<Emerald>::new(&cfg);
    for _ in 0..100 {
        s.step(Buttons::NONE);
        assert_eq!(det.after_step(&s, Buttons::NONE, 0), None);
    }
    let before = det.round_trips();
    assert!(before > 0);
    // Back to an earlier frame with a shadow following: no RoundTrip, the schedule restarts.
    s.load(&early).unwrap();
    for _ in 0..200 {
        s.step(Buttons::NONE);
        assert_eq!(det.after_step(&s, Buttons::NONE, 0), None);
    }
    assert!(det.round_trips() > before);
}
