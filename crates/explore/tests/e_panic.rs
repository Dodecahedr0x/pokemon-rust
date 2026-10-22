//! Task E: an injected game panic is caught with its Rust location, and its repro trace panics
//! at the same frame.

use content::{Emerald, NEW_GAME_SCRIPT};
use engine::Buttons;
use engine::data::{GameData, ScriptId};
use explore::config::DetectConfig;
use explore::findings::{repro_trace, reproduce};
use explore::guard::{self, Panicked, step_guarded};
use explore::testing::{INJECTED_PANIC, PanicOnScript};
use explore::{FindingDraft, FindingKind};
use game_native::Session;
use game_native::scenario::cached_route;
use game_native::trace::{Run, Trace, rle};

/// The first script started by a step of the route (the real flavor), and its frame.
fn first_stepped_script() -> (ScriptId, usize) {
    let rec = cached_route();
    let mut s = Session::new(Emerald::for_shells());
    for (i, &k) in rec.keys.iter().enumerate() {
        s.step(k);
        // The first step also shows the boot's events (`World::new`): skip them.
        let first = s.world().events().iter().find_map(|e| match e {
            &engine::world::Event::ScriptStarted(id) if i > 0 => Some(id),
            _ => None,
        });
        if let Some(id) = first {
            return (id, i + 1);
        }
    }
    panic!("the route starts no script");
}

fn draft(p: &Panicked, map: engine::data::MapId) -> FindingDraft {
    FindingDraft {
        kind: FindingKind::Panic,
        message: p.message.clone(),
        location: p.location.clone(),
        macro_: None,
        tail: Vec::new(),
        map,
        pos: (0, 0),
        script: None,
        log: Vec::new(),
    }
}

fn assert_injected(p: &Panicked, script: ScriptId) {
    assert_eq!(p.message, format!("{INJECTED_PANIC} {}", script.0));
    let loc = p.location.as_deref().expect("a location");
    assert!(loc.contains("testing.rs"), "{loc}");
}

/// Replays `trace` under the guard: the frame that panicked (0 = `World::new`).
fn panic_frame(game: &PanicOnScript<Emerald>, trace: &Trace) -> Option<u32> {
    let mut s = match guard::catch(|| Session::new(*game)) {
        Ok(s) => s,
        Err(_) => return Some(0),
    };
    (1..)
        .zip(trace.keys())
        .find_map(|(i, k)| step_guarded(&mut s, k).is_err().then_some(i))
}

#[test]
fn panic_on_script_is_caught_and_reproduced() {
    let cfg = DetectConfig::default();
    let inner = Emerald::for_shells();
    if content::STUB {
        // No event scripts on the stub: the new-game script runs inside `World::new`.
        let game = PanicOnScript {
            inner,
            script: NEW_GAME_SCRIPT,
        };
        let p = guard::catch(|| Session::new(game)).err().expect("a panic");
        assert_injected(&p, NEW_GAME_SCRIPT);
        let trace = repro_trace(&game, &[], &[]).unwrap();
        assert_eq!((trace.frames(), trace.hashes.len()), (0, 0));
        // Inputs after the panic: the panic frame (0) is reported with the trace.
        let more = rle([Buttons::NONE; 3]);
        let (t, at) = repro_trace(&game, &[&more], &[]).unwrap_err();
        assert_eq!((t.frames(), at), (0, 0));
        assert_eq!(panic_frame(&game, &trace), Some(0));
        reproduce(&game, &trace, &draft(&p, inner.new_game().map), &cfg).unwrap();
        // The plain game does not reproduce it.
        assert!(reproduce(&inner, &trace, &draft(&p, inner.new_game().map), &cfg).is_err());
        return;
    }
    let (script, started) = first_stepped_script();
    assert_ne!(script, NEW_GAME_SCRIPT);
    let game = PanicOnScript { inner, script };
    let rec = cached_route();
    let mut s = Session::new(game);
    let mut keys: Vec<Buttons> = Vec::new();
    let p = rec
        .keys
        .iter()
        .find_map(|&k| {
            keys.push(k);
            step_guarded(&mut s, k).err()
        })
        .expect("the route panics");
    assert_injected(&p, script);
    let at = keys.len();
    assert!(
        at == started || at == started + 1,
        "panic at {at}, script started at {started}"
    );
    let want = draft(&p, s.world().map());
    drop(s);

    // Lineage of two nodes, then a one-run tail ending on the panicking frame.
    let runs = rle(keys.iter().copied());
    let mid = runs.len() / 2;
    let (a, rest) = runs.split_at(mid);
    let (b, tail): (&[Run], &[Run]) = rest.split_at(rest.len() - 1);
    let trace = repro_trace(&game, &[a, b], tail).unwrap();
    assert_eq!(trace.frames() as usize, at);
    // The lineage itself panics (inputs follow the panic): the frame comes back with the trace.
    let after = rle([Buttons::NONE; 5]);
    let (t, f) = repro_trace(&game, &[&runs, &after], &[]).unwrap_err();
    assert_eq!((t.frames() as usize, f as usize), (at, at));
    assert_eq!(t.inputs, trace.inputs);
    // The signature ignores the panic's line and column (the row keeps them).
    let mut moved = want.clone();
    moved.location = want.location.as_ref().map(|l| {
        let file = l.rsplitn(3, ':').nth(2).unwrap();
        format!("{file}:1:1")
    });
    assert_ne!(moved.location, want.location);
    reproduce(&game, &trace, &moved, &cfg).unwrap();
    assert!(trace.hashes.iter().all(|&(f, _)| (f as usize) < at));
    assert_eq!(panic_frame(&game, &trace), Some(at as u32));
    reproduce(&game, &trace, &want, &cfg).unwrap();
    // One frame short: not raised.
    let mut short = trace.clone();
    short.inputs = rle(keys[..at - 1].iter().copied());
    assert!(reproduce(&game, &short, &want, &cfg).is_err());
}
