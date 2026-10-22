//! Finding signatures, repro traces and reproduction (Task E).
//!
//! A finding's map (in its signature) is `World::map()` after the frame that raised it, or the
//! new game's map for a panic inside `World::new`.

use crate::config::DetectConfig;
use crate::detect::Detectors;
use crate::guard::{self, Panicked};
use crate::model::{FindingDraft, FindingKind};
use crate::observe::StepObserver;
use crate::probe::Probe;
use engine::Buttons;
use engine::checkpoint::{Checkpoint, build_hash};
use engine::data::{GameData, MapId};
use game_native::Session;
use game_native::trace::{HASH_EVERY, Parent, Run, Trace, TraceRecorder, rle};

/// Dedupe signature: kind, location (or message with digits stripped), map.
pub fn signature(f: &FindingDraft) -> u64 {
    signature_of(f.kind, &f.message, f.location.as_deref(), f.map)
}

/// [`signature`] from its parts.
pub fn signature_of(kind: FindingKind, message: &str, location: Option<&str>, map: MapId) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut write = |bytes: &[u8]| {
        for &b in bytes {
            h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    write(&[kind as u8, 0xFF]);
    match location {
        Some(loc) => write(loc.as_bytes()),
        None => {
            let text: Vec<u8> = message.bytes().filter(|b| !b.is_ascii_digit()).collect();
            write(&text);
        }
    }
    write(&[0xFF]);
    write(&map.0.to_le_bytes());
    h
}

/// The finding as one NewGame trace: the lineage's inputs then the tail, hashes by replay.
///
/// The replay runs under the guard. If a frame panics (a `Panic` finding's last frame), the
/// trace ends with that frame's keys and carries the hashes of the frames before it; a panic in
/// `World::new` gives a trace without hashes.
pub fn repro_trace<G: GameData + Clone>(game: &G, lineage: &[&[Run]], tail: &[Run]) -> Trace {
    let runs: Vec<Run> = lineage
        .iter()
        .flat_map(|r| r.iter())
        .chain(tail)
        .copied()
        .collect();
    let keys: Vec<Buttons> = crate::runs::keys(&runs).collect();
    let build = build_hash(game);
    let mut rec = TraceRecorder::new(0);
    let Ok(mut s) = guard::catch(|| Session::new(game.clone())) else {
        let mut t = rec.trace(Parent::NewGame, build);
        t.inputs = rle(keys);
        return t;
    };
    for (i, &k) in keys.iter().enumerate() {
        if guard::step_guarded(&mut s, k).is_err() {
            let mut t = rec.trace(Parent::NewGame, build);
            t.inputs = rle(keys[..=i].iter().copied());
            return t;
        }
        rec.push(k, s.world().hash());
    }
    rec.trace(Parent::NewGame, build)
}

/// Replays `trace` with the guard and the detectors; Ok iff a finding with `want`'s signature
/// is raised at its last frame.
///
/// - Slow frames are report-only and never reproduce.
/// - A softlock is not raised by stepping: reproduce it with [`end_state`] and
///   `softlock::probe_region`.
/// - On the trace's build, a recorded hash the replay misses is an error.
pub fn reproduce<G: GameData + Clone>(
    game: &G,
    trace: &Trace,
    want: &FindingDraft,
    cfg: &DetectConfig,
) -> Result<(), String> {
    match want.kind {
        FindingKind::SlowFrame => return Err("slow frames are report-only".into()),
        FindingKind::Softlock => {
            return Err("softlocks reproduce with findings::end_state + softlock::probe".into());
        }
        _ => {}
    }
    if trace.parent != Parent::NewGame {
        return Err(format!(
            "the trace starts at {}, not a new game",
            trace.parent
        ));
    }
    let want_sig = signature(want);
    let n = trace.frames();
    let raised = |at: u32, kind, message: &str, location: Option<&str>, map| {
        if signature_of(kind, message, location, map) != want_sig {
            return None;
        }
        Some(if at == n {
            Ok(())
        } else {
            Err(format!(
                "raised at frame {at}, before the trace's last frame {n}"
            ))
        })
    };
    let panicked = |at: u32, p: Panicked, map: MapId| {
        raised(
            at,
            FindingKind::Panic,
            &p.message,
            p.location.as_deref(),
            map,
        )
        .unwrap_or_else(|| {
            Err(format!(
                "another panic at frame {at}: {} ({})",
                p.message,
                p.location.as_deref().unwrap_or("?")
            ))
        })
    };
    let mut s = match guard::catch(|| Session::new(Probe::new(game.clone()))) {
        Ok(s) => s,
        Err(p) => return panicked(0, p, game.new_game().map),
    };
    let check_hashes = trace.build == build_hash(game);
    let mut hashes = trace.hashes.iter().copied().peekable();
    let mut det = Detectors::<G>::new(&DetectConfig {
        slow_step_us: 0,
        ..cfg.clone()
    });
    for (i, keys) in (1..).zip(trace.keys()) {
        if let Err(p) = guard::step_guarded(&mut s, keys) {
            return panicked(i, p, s.world().map());
        }
        s.game().take_started();
        if check_hashes {
            while hashes.next_if(|&(at, _)| at < i).is_some() {}
            if let Some((_, want)) = hashes.next_if(|&(at, _)| at == i) {
                let got = s.world().hash();
                if got != want {
                    return Err(format!(
                        "the replay diverged at frame {i}: {got:#x} != {want:#x}"
                    ));
                }
            }
        }
        if let Some(f) = det.after_step(&s, keys, 0)
            && let Some(r) = raised(i, f.kind, &f.message, None, s.world().map())
        {
            return r;
        }
    }
    Err(format!("not raised in {n} frames"))
}

/// The capturable end state of a NewGame `trace` (checked against its hashes on its build):
/// where a `Softlock` finding's probe starts.
pub fn end_state<G: GameData + Clone>(game: &G, trace: &Trace) -> Result<Checkpoint, String> {
    if trace.parent != Parent::NewGame {
        return Err(format!(
            "the trace starts at {}, not a new game",
            trace.parent
        ));
    }
    let check_hashes = trace.build == build_hash(game);
    let mut s = guard::catch(|| Session::new(game.clone())).map_err(|p| p.message)?;
    let mut hashes = trace.hashes.iter().copied().peekable();
    for (i, keys) in (1..).zip(trace.keys()) {
        guard::step_guarded(&mut s, keys)
            .map_err(|p| format!("panic at frame {i}: {}", p.message))?;
        if check_hashes && (i % HASH_EVERY == 0 || i == trace.frames()) {
            while hashes.next_if(|&(at, _)| at < i).is_some() {}
            if let Some((_, want)) = hashes.next_if(|&(at, _)| at == i)
                && s.world().hash() != want
            {
                return Err(format!("the replay diverged at frame {i}"));
            }
        }
    }
    s.checkpoint()
        .map_err(|e| format!("the end state is not capturable: {e:?}"))
}
