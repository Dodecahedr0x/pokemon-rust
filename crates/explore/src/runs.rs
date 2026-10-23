//! Run codecs.
//!
//! - Text: `"keys*frames keys*frames …"` (keys = raw `Buttons` bits), the `Trace` node-file run
//!   syntax (whose serde module is private to `game-native`). Findings' tails use it.
//! - Binary ([`to_blob`], schema v3 `node.inputs`): LEB128 varint items. A run is the header
//!   `keys << 1` then its frames; a **repeat pair** is the header `count << 1 | 1`: the last two
//!   runs again, `count` more times (the A/release alternation of battle inputs).

use engine::Buttons;
use game_native::trace::Run;

/// `runs` as `"keys*frames"` separated by single spaces; empty for no runs.
pub fn to_text(runs: &[Run]) -> String {
    let mut s = String::new();
    for (i, r) in runs.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(&format!("{}*{}", r.keys.0, r.frames));
    }
    s
}

/// Parses [`to_text`] (any ASCII whitespace between runs).
pub fn from_text(s: &str) -> Result<Vec<Run>, String> {
    s.split_ascii_whitespace()
        .map(|run| {
            let bad = || format!("bad input run {run:?} (want keys*frames)");
            let (keys, frames) = run.split_once('*').ok_or_else(bad)?;
            Ok(Run {
                keys: Buttons(keys.parse().map_err(|_| bad())?),
                frames: frames.parse().map_err(|_| bad())?,
            })
        })
        .collect()
}

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push(v as u8 | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn get_varint(b: &[u8], at: &mut usize) -> Result<u64, String> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *b.get(*at).ok_or("truncated input blob")?;
        *at += 1;
        v |= u64::from(byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            return Ok(v);
        }
    }
    Err("varint too long in input blob".into())
}

/// `runs` as the schema v3 blob: varint runs and repeat pairs (module doc). A pair repeats
/// when the next runs equal the last two, as often as they do (greedy, so the encoding is a
/// function of the runs).
pub fn to_blob(runs: &[Run]) -> Vec<u8> {
    let mut out = Vec::with_capacity(runs.len() * 2);
    let mut i = 0;
    while i < runs.len() {
        if i >= 2 {
            let (a, b) = (runs[i - 2], runs[i - 1]);
            let mut n = 0;
            while i + 2 * n + 1 < runs.len() && runs[i + 2 * n] == a && runs[i + 2 * n + 1] == b {
                n += 1;
            }
            if n > 0 {
                put_varint(&mut out, (n as u64) << 1 | 1);
                i += 2 * n;
                continue;
            }
        }
        put_varint(&mut out, u64::from(runs[i].keys.0) << 1);
        put_varint(&mut out, u64::from(runs[i].frames));
        i += 1;
    }
    out
}

/// The most runs [`from_blob`] decodes (a node holds a few thousand): a corrupt repeat count
/// is an error, not a huge allocation.
pub const MAX_BLOB_RUNS: usize = 1 << 22;

/// Parses [`to_blob`].
pub fn from_blob(b: &[u8]) -> Result<Vec<Run>, String> {
    let mut runs: Vec<Run> = Vec::new();
    let mut at = 0;
    while at < b.len() {
        let h = get_varint(b, &mut at)?;
        if h & 1 == 1 {
            let n = h >> 1;
            let [.., x, y] = runs[..] else {
                return Err("repeat pair before two runs in input blob".into());
            };
            let room = (MAX_BLOB_RUNS - runs.len()) as u64 / 2;
            if n == 0 || n > room {
                return Err(format!("bad repeat count {n} in input blob"));
            }
            for _ in 0..n {
                runs.extend([x, y]);
            }
        } else {
            let keys = u16::try_from(h >> 1).map_err(|_| "keys out of range in input blob")?;
            let frames = u32::try_from(get_varint(b, &mut at)?)
                .map_err(|_| "frames out of range in input blob")?;
            if runs.len() == MAX_BLOB_RUNS {
                return Err(format!("more than {MAX_BLOB_RUNS} runs in input blob"));
            }
            runs.push(Run {
                keys: Buttons(keys),
                frames,
            });
        }
    }
    Ok(runs)
}

/// The keys of every frame, in order.
pub fn keys(runs: &[Run]) -> impl Iterator<Item = Buttons> + '_ {
    runs.iter()
        .flat_map(|r| core::iter::repeat_n(r.keys, r.frames as usize))
}

/// Frames the runs step (saturating).
pub fn frames(runs: &[Run]) -> u32 {
    runs.iter().fold(0u32, |n, r| n.saturating_add(r.frames))
}

/// The runs of the first `n` frames, and the rest (a run straddling `n` is split).
pub fn split_at(runs: &[Run], n: u64) -> (Vec<Run>, Vec<Run>) {
    let (mut head, mut tail) = (Vec::new(), Vec::new());
    let mut left = n;
    for r in runs {
        let f = u64::from(r.frames);
        if left >= f {
            head.push(*r);
            left -= f;
        } else if left > 0 {
            head.push(Run {
                keys: r.keys,
                frames: left as u32,
            });
            tail.push(Run {
                keys: r.keys,
                frames: r.frames - left as u32,
            });
            left = 0;
        } else {
            tail.push(*r);
        }
    }
    (head, tail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_native::trace::rle;

    #[test]
    fn round_trip() {
        let keys_in = [
            Buttons::NONE,
            Buttons::NONE,
            Buttons::A,
            Buttons(Buttons::UP.0 | Buttons::B.0),
            Buttons::NONE,
        ];
        let runs = rle(keys_in);
        let text = to_text(&runs);
        assert_eq!(text, "0*2 1*1 66*1 0*1");
        assert_eq!(from_text(&text).unwrap(), runs);
        assert_eq!(keys(&runs).collect::<Vec<_>>(), keys_in);
        assert_eq!(frames(&runs), 5);
    }

    #[test]
    fn empty_and_whitespace() {
        assert_eq!(to_text(&[]), "");
        assert_eq!(from_text("").unwrap(), vec![]);
        assert_eq!(
            from_text("  16*3\n\t0*1 ").unwrap(),
            vec![
                Run {
                    keys: Buttons::RIGHT,
                    frames: 3
                },
                Run {
                    keys: Buttons::NONE,
                    frames: 1
                }
            ]
        );
    }

    #[test]
    fn rejects_bad_runs() {
        for bad in ["1", "1*", "*2", "x*2", "1*-1", "70000*1", "1*2*3"] {
            assert!(from_text(bad).is_err(), "{bad}");
        }
    }

    fn run(keys: u16, frames: u32) -> Run {
        Run {
            keys: Buttons(keys),
            frames,
        }
    }

    #[test]
    fn blob_round_trip_and_repeat_pairs() {
        // A battle: A and release alternating, then a walk.
        let mut runs = vec![run(0, 40)];
        for _ in 0..50 {
            runs.extend([run(1, 1), run(0, 7)]);
        }
        runs.extend([run(64, 16), run(0, 300), run(0x3FF, u32::MAX)]);
        let blob = to_blob(&runs);
        assert_eq!(from_blob(&blob).unwrap(), runs);
        // 0*40, A*1, 0*7, one repeat pair (49 more), then three runs.
        assert_eq!(blob[..7], [0, 40, 2, 1, 0, 7, 49 << 1 | 1]);
        assert!(blob.len() < 24, "{} bytes", blob.len());
        assert!(blob.len() * 10 < to_text(&runs).len());
        for runs in [
            vec![],
            vec![run(1, 1)],
            vec![run(1, 1), run(2, 2), run(1, 1)],
        ] {
            assert_eq!(from_blob(&to_blob(&runs)).unwrap(), runs);
        }
    }

    #[test]
    fn blob_rejects_bad_input() {
        for bad in [
            &[0x80][..],                                 // truncated varint
            &[2],                                        // a run without frames
            &[3],                                        // a repeat before two runs
            &[2, 1, 4, 1, 1],                            // a repeat of 0
            &[0x80, 0x80, 0x08, 1],                      // keys above u16
            &[2, 0x80, 0x80, 0x80, 0x80, 0x10],          // frames above u32
            &[2, 1, 4, 1, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F], // a repeat of 2^31 - 1
        ] {
            assert!(from_blob(bad).is_err(), "{bad:?}");
        }
        // Up to the cap.
        let mut ok = vec![2, 1, 4, 1];
        put_varint(&mut ok, ((MAX_BLOB_RUNS as u64 - 2) / 2) << 1 | 1);
        assert_eq!(from_blob(&ok).unwrap().len(), MAX_BLOB_RUNS);
        ok.extend([2, 1]);
        assert!(from_blob(&ok).is_err(), "one run past the cap");
    }

    #[test]
    fn frames_saturate() {
        let runs = [
            Run {
                keys: Buttons::A,
                frames: u32::MAX,
            },
            Run {
                keys: Buttons::B,
                frames: 5,
            },
        ];
        assert_eq!(frames(&runs), u32::MAX);
    }
}
