//! The checkpoint invariant along the parity route (`scenario::record_route`, stub or extracted
//! data): at about [`POINTS`] capturable frames, and at the first capturable frame at or after
//! each named moment, the live World is captured, encoded (postcard, within the flash budget),
//! decoded and restored into a fresh [`Session`]. The restored session must have the live hash
//! and picture, then stay equal to the live session for [`FOLLOW`] frames of the route's inputs
//! (hash every frame, pictures at [`PICTURES`]). Three points (the first, the middle and
//! the last at least [`FOLLOW`] frames before the end) run to the end of the route. Each
//! checkpoint also round-trips through RON.

use content::Emerald;
use engine::checkpoint::{self, Checkpoint, MAX_ENCODED};
use game_native::Session;
use game_native::scenario::{Recording, record_route};

/// About this many points, evenly spread over the capturable frames.
const POINTS: u32 = 100;
/// Frames each restored session follows the live one.
const FOLLOW: u32 = 120;
/// Frames after the restore at which the pictures are compared (besides the restore itself).
const PICTURES: [u32; 4] = [1, 2, 60, FOLLOW];

fn game() -> Emerald {
    Emerald::for_shells()
}

/// `capturable[n]`: the World after `n` route steps can be captured.
fn capturable_frames(rec: &Recording) -> Vec<bool> {
    let mut s = Session::new(game());
    let mut out = Vec::with_capacity(rec.frames() as usize + 1);
    for n in 0..=rec.frames() {
        out.push(s.world().capturable().is_ok());
        s.step(rec.keys_at(n));
    }
    out
}

/// About [`POINTS`] capturable frames, evenly spread over the capturable ones (most of the
/// route is battles, scripts and fades, so windows of `frames / POINTS` would leave most
/// empty), plus the first capturable frame at or after each moment; ascending.
fn points(rec: &Recording, capturable: &[bool]) -> Vec<u32> {
    let all: Vec<u32> = (0..capturable.len() as u32)
        .filter(|&n| capturable[n as usize])
        .collect();
    let mut p: Vec<u32> = (0..POINTS as usize)
        .filter_map(|i| all.get(i * all.len() / POINTS as usize).copied())
        .collect();
    p.extend(
        rec.moments
            .iter()
            .filter_map(|m| all.iter().copied().find(|&n| n >= m.frame)),
    );
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
fn checkpoints_restore_bit_identical_along_the_route() {
    let rec = record_route().unwrap_or_else(|e| panic!("the route does not play: {e}"));
    let capturable = capturable_frames(&rec);
    let points = points(&rec, &capturable);
    assert!(points.len() >= 2, "too few capturable frames: {points:?}");
    // The last long run starts at least `FOLLOW` frames before the end of the route.
    let last = points
        .iter()
        .copied()
        .rfind(|&p| p + FOLLOW <= rec.frames())
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
            // Past the end of the route `keys_at` is nothing.
            let until = if long.contains(&n) {
                rec.frames().max(n + FOLLOW)
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
        let keys = rec.keys_at(n);
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
        "{} route: {} frames ({} capturable), {} checkpoints restored (long runs from {long:?}); largest \
         checkpoint {} bytes at frame {} (budget {MAX_ENCODED})",
        if content::STUB { "stub" } else { "extracted" },
        rec.frames(),
        capturable.iter().filter(|&&c| c).count(),
        points.len(),
        max_len.0,
        max_len.1,
    );
}
