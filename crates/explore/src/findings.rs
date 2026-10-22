//! Finding signatures, repro traces and reproduction (Task E).
//!
//! A finding's map (in its signature) is `World::map()` after the frame that raised it, or the
//! new game's map for a panic inside `World::new`.

use crate::config::DetectConfig;
use crate::detect::Detectors;
use crate::guard::{self, Panicked};
use crate::model::{FindingDraft, FindingKind};
use crate::probe::Probe;
use engine::Buttons;
use engine::checkpoint::{Checkpoint, build_hash};
use engine::data::{GameData, MapId};
use game_native::Session;
use game_native::trace::{HASH_EVERY, Parent, Run, Trace, TraceRecorder, rle};

/// Dedupe signature: FNV-1a 64 of the kind's stable name ([`kind_name`]), then the location's
/// file (without line and column) if any, then the message with its ASCII digits stripped,
/// then the map. A panic keeps its signature across unrelated edits that move its line; the
/// full location stays in the finding row for display.
///
/// A `Softlock` is signed on its map and region instead ([`softlock_signature`], the region
/// read back from [`softlock_message`]).
pub fn signature(f: &FindingDraft) -> u64 {
    if f.kind == FindingKind::Softlock
        && let Some(region) = softlock_region(&f.message)
    {
        return softlock_signature(f.map, region);
    }
    signature_of(f.kind, &f.message, f.location.as_deref(), f.map)
}

/// A `Softlock` finding's message (the region in decimal, so no hex letters).
pub fn softlock_message(region: u64, k: u32, reached: u32) -> String {
    format!("trapped: the {reached} states within {k} macros stay in region {region}")
}

/// The region of a [`softlock_message`].
pub fn softlock_region(message: &str) -> Option<u64> {
    message.rsplit_once(" region ")?.1.parse().ok()
}

/// A softlock's dedupe signature: FNV-1a 64 of its kind name, the map and the region.
pub fn softlock_signature(map: MapId, region: u64) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let bytes = kind_name(FindingKind::Softlock)
        .bytes()
        .chain([0xFF])
        .chain(map.0.to_le_bytes())
        .chain(region.to_le_bytes());
    for b in bytes {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// The stable name of a kind in signatures (never renamed: dedupe depends on it).
pub fn kind_name(kind: FindingKind) -> &'static str {
    match kind {
        FindingKind::Panic => "panic",
        FindingKind::Softlock => "softlock",
        FindingKind::RoundTrip => "round_trip",
        FindingKind::CapturableStuck => "capturable_stuck",
        FindingKind::PlannerDivergence => "planner_divergence",
        FindingKind::SlowFrame => "slow_frame",
    }
}

/// [`signature`] from its parts.
pub fn signature_of(kind: FindingKind, message: &str, location: Option<&str>, map: MapId) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut write = |bytes: &[u8]| {
        for &b in bytes {
            h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    write(kind_name(kind).as_bytes());
    write(&[0xFF]);
    if let Some(loc) = location {
        write(location_file(loc).as_bytes());
    }
    write(&[0xFF]);
    let text: Vec<u8> = message.bytes().filter(|b| !b.is_ascii_digit()).collect();
    write(&text);
    write(&[0xFF]);
    write(&map.0.to_le_bytes());
    h
}

/// `file` of `file:line:col` (the whole string if it has no line and column).
fn location_file(loc: &str) -> &str {
    let mut parts = loc.rsplitn(3, ':');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(col), Some(line), Some(file))
            if [col, line]
                .iter()
                .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())) =>
        {
            file
        }
        _ => loc,
    }
}

/// The finding as one NewGame trace: the lineage's inputs then the tail, hashes by replay.
///
/// The replay runs under the guard. A panic on the last frame (a `Panic` finding) ends the
/// trace with that frame's keys and the hashes of the frames before it (none for a panic in
/// `World::new` with no inputs). A panic before the last frame is an `Err` with the trace up to
/// and including the panicking frame, and that frame (0 = `World::new`).
pub fn repro_trace<G: GameData + Clone>(
    game: &G,
    lineage: &[&[Run]],
    tail: &[Run],
) -> Result<Trace, (Trace, u32)> {
    let runs: Vec<Run> = lineage
        .iter()
        .flat_map(|r| r.iter())
        .chain(tail)
        .copied()
        .collect();
    let keys: Vec<Buttons> = crate::runs::keys(&runs).collect();
    let n = keys.len();
    let build = build_hash(game);
    let mut rec = TraceRecorder::new(0);
    let Ok(mut s) = guard::catch(|| Session::new(game.clone())) else {
        let t = rec.trace(Parent::NewGame, build);
        return if n == 0 { Ok(t) } else { Err((t, 0)) };
    };
    for (i, &k) in keys.iter().enumerate() {
        if guard::step_guarded(&mut s, k).is_err() {
            let mut t = rec.trace(Parent::NewGame, build);
            t.inputs = rle(keys[..=i].iter().copied());
            return if i + 1 == n {
                Ok(t)
            } else {
                Err((t, i as u32 + 1))
            };
        }
        rec.push(k, s.world().hash());
    }
    Ok(rec.trace(Parent::NewGame, build))
}

/// Replays `trace` with the guard and the detectors; Ok iff a finding with `want`'s signature
/// is raised at its last frame.
///
/// - Slow frames are report-only and never reproduce.
/// - A softlock is not raised by stepping: reproduce it with [`end_state`] and
///   `softlock::probe`.
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
    let matches = |kind, message: &str, location: Option<&str>, map| {
        signature_of(kind, message, location, map) == want_sig
    };
    // A panic ends the replay: it reproduces only on the last frame.
    let panicked = |at: u32, p: Panicked, map: MapId| {
        if matches(FindingKind::Panic, &p.message, p.location.as_deref(), map) && at == n {
            Ok(())
        } else {
            Err(format!(
                "a panic at frame {at} of {n}: {} ({})",
                p.message,
                p.location.as_deref().unwrap_or("?")
            ))
        }
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
    let mut last = Vec::new();
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
        // Same-signature raises before the last frame are not terminal: only the last frame's
        // flags (all of them) decide.
        last = det.after_step_all(&s, keys, 0);
    }
    let map = s.world().map();
    if last.iter().any(|f| matches(f.kind, &f.message, None, map)) {
        Ok(())
    } else {
        Err(format!("not raised on the last frame ({n}): {last:?}"))
    }
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
