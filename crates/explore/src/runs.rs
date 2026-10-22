//! Run text codec: `"keys*frames keys*frames …"` (keys = raw `Buttons` bits), the `Trace`
//! node-file run syntax (whose serde module is private to `game-native`).

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

/// The keys of every frame, in order.
pub fn keys(runs: &[Run]) -> impl Iterator<Item = Buttons> + '_ {
    runs.iter()
        .flat_map(|r| core::iter::repeat_n(r.keys, r.frames as usize))
}

/// Frames the runs step (saturating).
pub fn frames(runs: &[Run]) -> u32 {
    runs.iter().fold(0u32, |n, r| n.saturating_add(r.frames))
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
