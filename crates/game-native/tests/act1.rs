//! The checkpoint invariant along Act 1 (Milestone 8 Task P): the flattened trace of the
//! committed `act1:*` chain (`act1/trace.rs`), from a new game to `FLAG_BADGE01_GET`. At about
//! [`POINTS`] capturable frames, and at every node's frame, the live World is captured, encoded
//! (postcard, within the flash budget), decoded and restored into a fresh [`Session`], which must
//! have the live hash and picture and then stay equal to the live session for [`FOLLOW`] frames
//! (hash every frame, pictures at [`PICTURES`]). Three points (the first, the middle and the last
//! at least [`FOLLOW`] frames before the end) run to the end of the act. Every node's frame must
//! reach its recorded digest.
//!
//! The run also prints the act's numbers: frames per segment, maps entered, battles by outcome,
//! and every report (`Unsupported`, `Degraded`) with the script that raised it.
//!
//! Release only, `#[ignore]`d: run by `cargo xtask parity`.

#[path = "act1/trace.rs"]
mod trace;

use content::Emerald;
use engine::battle::BattleOutcome;
use engine::checkpoint::{self, Checkpoint, MAX_ENCODED, state_digest};
use engine::script::Report;
use engine::world::Event;
use game_native::Session;
use game_native::store::Store;
use std::collections::{BTreeMap, BTreeSet};

/// About this many points, evenly spread over the capturable frames.
const POINTS: u32 = 100;
/// Frames each restored session follows the live one.
const FOLLOW: u32 = 120;
/// Frames after the restore at which the pictures are compared (besides the restore itself).
const PICTURES: [u32; 4] = [1, 2, 60, FOLLOW];

fn game() -> Emerald {
    Emerald::for_shells()
}

/// What happened in one segment of the act (from a node's parent to the node).
#[derive(Default)]
struct Segment {
    frames: u32,
    maps: BTreeSet<u16>,
    entered: u32,
    battles: BTreeMap<&'static str, u32>,
    reports: BTreeMap<String, u32>,
}

fn outcome(o: BattleOutcome) -> &'static str {
    match o {
        BattleOutcome::Won => "won",
        BattleOutcome::Lost | BattleOutcome::Drew => "lost",
        BattleOutcome::Ran => "ran",
        _ => "other",
    }
}

fn script_name(s: Option<engine::data::ScriptId>) -> &'static str {
    s.and_then(|s| content::data::SCRIPT_NAMES.get(s.0 as usize).copied())
        .unwrap_or("-")
}

/// A pass over the act: `capturable[n]` (the World after `n` steps can be captured), and the
/// segments' numbers, printed.
fn scan(act: &trace::Act1) -> Vec<bool> {
    let mut s = Session::new(game());
    let mut out = Vec::with_capacity(act.keys.len() + 1);
    let mut segments: Vec<Segment> = Vec::new();
    let mut seg = Segment::default();
    let mut ends = act.nodes.iter().peekable();
    for n in 0..=act.frames() {
        out.push(s.world().capturable().is_ok());
        if ends.next_if(|(_, f)| *f == n).is_some() {
            seg.frames = n - segments.iter().map(|g| g.frames).sum::<u32>();
            segments.push(std::mem::take(&mut seg));
        }
        if n == act.frames() {
            break;
        }
        s.step(act.keys_at(n));
        let w = s.world();
        assert_eq!(w.events_dropped(), 0, "events dropped at frame {}", n + 1);
        for e in w.events() {
            match e {
                Event::MapEntered(m) => {
                    seg.entered += 1;
                    seg.maps.insert(m.0);
                }
                Event::BattleEnded(o) => *seg.battles.entry(outcome(*o)).or_default() += 1,
                Event::Report(r) => {
                    let what = match r {
                        Report::Unsupported(n) => format!("Unsupported({n})"),
                        Report::Degraded(n) => format!("Degraded({n})"),
                    };
                    let key = format!("{what} in {}", script_name(w.last_script()));
                    *seg.reports.entry(key).or_default() += 1;
                }
                _ => {}
            }
        }
    }
    assert_eq!(segments.len(), act.nodes.len(), "a node frame was missed");
    let mut all_maps: BTreeSet<u16> = BTreeSet::new();
    for ((label, frame), g) in act.nodes.iter().zip(&segments) {
        all_maps.extend(&g.maps);
        println!(
            "{label:<15} to frame {frame:>6}: {:>6} frames, {} maps entered ({} distinct), \
             battles {:?}",
            g.frames,
            g.entered,
            g.maps.len(),
            g.battles
        );
        for (r, hits) in &g.reports {
            println!("    report {r} x{hits}");
        }
    }
    println!(
        "act 1: {} frames, {} distinct maps entered, {} capturable frames",
        act.frames(),
        all_maps.len(),
        out.iter().filter(|&&c| c).count()
    );
    out
}

/// About [`POINTS`] capturable frames, evenly spread over the capturable ones, plus every node's
/// frame; ascending.
fn points(act: &trace::Act1, capturable: &[bool]) -> Vec<u32> {
    let all: Vec<u32> = (0..capturable.len() as u32)
        .filter(|&n| capturable[n as usize])
        .collect();
    let mut p: Vec<u32> = (0..POINTS as usize)
        .filter_map(|i| all.get(i * all.len() / POINTS as usize).copied())
        .collect();
    for &(ref label, f) in &act.nodes {
        assert!(
            capturable[f as usize],
            "{label} (frame {f}) is not capturable"
        );
        p.push(f);
    }
    p.sort_unstable();
    p.dedup();
    p
}

/// A restored session following the live one.
struct Resumed {
    from: u32,
    until: u32,
    session: Session<Emerald>,
}

fn same_picture(live: &mut Session<Emerald>, other: &mut Session<Emerald>, what: &str) {
    let (a, b) = (live.framebuffer(), other.framebuffer());
    let diff = a.iter().zip(b).filter(|(x, y)| x != y).count();
    assert_eq!(diff, 0, "{diff} pixels differ {what}");
}

/// Postcard within the flash budget, back to the same checkpoint; RON too. Returns the
/// postcard size.
fn round_trip(cp: &Checkpoint, n: u32) -> (Checkpoint, usize) {
    let bytes = checkpoint::encode_checked(cp)
        .unwrap_or_else(|e| panic!("checkpoint at frame {n} does not encode: {e:?}"));
    assert!(bytes.len() <= MAX_ENCODED);
    let decoded = checkpoint::decode(&bytes)
        .unwrap_or_else(|e| panic!("checkpoint at frame {n} does not decode: {e:?}"));
    assert!(decoded == *cp, "postcard round trip changed frame {n}");
    let ron = ron::to_string(cp).expect("RON encode");
    let back: Checkpoint = ron::from_str(&ron).expect("RON decode");
    assert!(back == *cp, "RON round trip changed frame {n}");
    (decoded, bytes.len())
}

#[test]
#[ignore = "release; run by `cargo xtask parity`"]
fn checkpoints_restore_bit_identical_along_act1() {
    let Some(act) = trace::load() else {
        println!("act 1: stub build, no act1 nodes");
        return;
    };
    let digests: BTreeMap<u32, u64> = {
        let store = Store::open(Store::default_root(content::FLAVOR)).unwrap();
        let index = store.index().unwrap();
        (act.nodes.iter())
            .map(|(l, f)| (*f, index.labelled(l).unwrap().meta.digest))
            .collect()
    };
    let capturable = scan(&act);
    let points = points(&act, &capturable);
    let end = act.frames();
    let last = points
        .iter()
        .copied()
        .rfind(|&p| p + FOLLOW <= end)
        .expect("no capturable frame FOLLOW frames before the end");
    let long = [points[0], points[points.len() / 2], last];

    let mut live = Session::new(game());
    let mut resumed: Vec<Resumed> = Vec::new();
    let mut max_len = (0, 0);
    let mut next = points.iter().copied().peekable();
    let mut n = 0;
    loop {
        if next.next_if_eq(&n).is_some() {
            let cp = live
                .checkpoint()
                .unwrap_or_else(|r| panic!("frame {n} was capturable, now {r:?}"));
            if let Some(&d) = digests.get(&n) {
                assert_eq!(state_digest(&cp.world), d, "node digest at frame {n}");
            }
            let (decoded, len) = round_trip(&cp, n);
            max_len = max_len.max((len, n));
            let mut session = Session::restore(game(), &decoded)
                .unwrap_or_else(|e| panic!("restore at frame {n}: {e:?}"));
            assert_eq!(session.world().frame(), n);
            assert_eq!(
                session.world().hash(),
                live.world().hash(),
                "restored hash at frame {n}"
            );
            same_picture(&mut live, &mut session, &format!("on restore at frame {n}"));
            // Past the end of the act `keys_at` is nothing.
            let until = if long.contains(&n) {
                end.max(n + FOLLOW)
            } else {
                n + FOLLOW
            };
            resumed.push(Resumed {
                from: n,
                until,
                session,
            });
        }
        resumed.retain(|r| {
            let stepped = r.session.world().frame() - r.from;
            assert!(
                r.until > n || stepped >= FOLLOW,
                "restored at {} only ran {stepped}",
                r.from
            );
            r.until > n
        });
        if resumed.is_empty() && next.peek().is_none() {
            break;
        }
        let keys = act.keys_at(n);
        live.step(keys);
        n += 1;
        for r in &mut resumed {
            r.session.step(keys);
            assert_eq!(
                r.session.world().hash(),
                live.world().hash(),
                "restored at frame {} diverged at frame {n}",
                r.from
            );
            if PICTURES.contains(&(n - r.from)) {
                same_picture(
                    &mut live,
                    &mut r.session,
                    &format!("at frame {n} (restored at {})", r.from),
                );
            }
        }
    }
    println!(
        "act 1: {} checkpoints restored (long runs from {long:?}); largest checkpoint {} bytes at \
         frame {} (budget {MAX_ENCODED})",
        points.len(),
        max_len.0,
        max_len.1,
    );
}
