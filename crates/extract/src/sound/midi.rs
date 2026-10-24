//! Standard MIDI files to MP2K songs (Milestone 12): the author's tracks
//! (`games/silice/music/MUS_SIL_*.mid`) become [`SongFile`]s the content build adds to the
//! extract's song table, as the decomp's `mid2agb` turns `sound/songs/midi/*.mid` into `.s`.
//!
//! The conversion, after `mid2agb`'s defaults:
//! - time: 24 MP2K clocks per quarter note (MIDI ticks scaled from the file's division, rounded);
//! - one MP2K track per MIDI channel that plays a note, in channel order; the tempo goes on the
//!   first (`TEMPO` is beats per minute / 2);
//! - program change -> `VOICE` (an index in the song's voicegroup), CC 7 -> `VOL`, CC 10 ->
//!   `PAN`, CC 1 -> `MOD`, the pitch wheel -> `BEND` (its high 7 bits), every track starting
//!   with `KEYSH 0`;
//! - a note of up to 96 clocks is one `Nxx` (the largest table length that fits, plus 1-3 gate
//!   ticks); a longer one is `TIE` ... `EOT`;
//! - waits are `Wxx` from the clock table, 96 at most each;
//! - the markers (or texts) `[` and `]` loop the song: every track gets a label at `[` and
//!   jumps back to it at `]` (`GOTO`), as `mid2agb`'s `-L` loop markers; without them the song
//!   ends (`FINE`) after its last event.

use crate::schema::{SongCmd, SongFile};
use crate::sound::track::{CLOCKS, clock_index};
use anyhow::{Context, Result, bail, ensure};
use std::collections::BTreeMap;

/// MP2K clocks per quarter note.
pub const CLOCKS_PER_BEAT: u32 = 24;
/// The longest `Nxx` / `Wxx`.
const MAX_CLOCK: u32 = 96;

/// One event of a channel, at an MP2K clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Ev {
    // Sorted within a clock: note ends, then controls, then note starts.
    Off { key: u8 },
    Tempo(u8),
    Voice(u8),
    Vol(u8),
    Pan(u8),
    Mod(u8),
    Bend(u8),
    On { key: u8, vel: u8, len: u32 },
}

/// A parsed MIDI file, as MP2K-clock events.
#[derive(Debug, Default)]
struct Parsed {
    /// Per channel: (clock, event).
    chans: BTreeMap<u8, Vec<(u32, Ev)>>,
    tempos: Vec<(u32, u8)>,
    loop_start: Option<u32>,
    loop_end: Option<u32>,
    end: u32,
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn u8(&mut self) -> Result<u8> {
        let v = *self.b.get(self.at).context("unexpected end of MIDI data")?;
        self.at += 1;
        Ok(v)
    }
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        let s = (self.b.get(self.at..self.at + n)).context("unexpected end of MIDI data")?;
        self.at += n;
        Ok(s)
    }
    fn be(&mut self, n: usize) -> Result<u32> {
        Ok(self.take(n)?.iter().fold(0u32, |a, &x| a << 8 | x as u32))
    }
    fn vlq(&mut self) -> Result<u32> {
        let mut v = 0u32;
        for _ in 0..4 {
            let x = self.u8()?;
            v = v << 7 | (x & 0x7F) as u32;
            if x & 0x80 == 0 {
                return Ok(v);
            }
        }
        bail!("variable-length quantity over 4 bytes")
    }
}

fn parse(bytes: &[u8]) -> Result<Parsed> {
    let mut r = Reader { b: bytes, at: 0 };
    ensure!(r.take(4)? == b"MThd", "not a MIDI file (no MThd)");
    let len = r.be(4)? as usize;
    ensure!(len >= 6, "MThd too short");
    let format = r.be(2)?;
    let ntracks = r.be(2)?;
    let division = r.be(2)?;
    r.take(len - 6)?;
    ensure!(format <= 1, "MIDI format {format} (only 0 and 1)");
    ensure!(
        division & 0x8000 == 0 && division > 0,
        "SMPTE time division is not supported"
    );
    let clock = |tick: u64| {
        ((tick * CLOCKS_PER_BEAT as u64 + division as u64 / 2) / division as u64) as u32
    };
    let mut p = Parsed::default();
    // Open notes: (channel, key) -> (start clock, index of the On in its channel).
    let mut open: BTreeMap<(u8, u8), (u32, usize)> = BTreeMap::new();
    for _ in 0..ntracks {
        let id = r.take(4)?.to_vec();
        let len = r.be(4)? as usize;
        let data = r.take(len)?;
        if id != b"MTrk" {
            continue;
        }
        let mut t = Reader { b: data, at: 0 };
        let (mut tick, mut status) = (0u64, 0u8);
        while t.at < data.len() {
            tick += t.vlq()? as u64;
            let now = clock(tick);
            p.end = p.end.max(now);
            let mut first = t.u8()?;
            if first < 0x80 {
                ensure!(status >= 0x80, "running status before any status byte");
                t.at -= 1;
                first = status;
            } else if first < 0xF0 {
                status = first;
            }
            let ch = first & 0x0F;
            match first {
                0xFF => {
                    let ty = t.u8()?;
                    let n = t.vlq()? as usize;
                    let d = t.take(n)?;
                    match ty {
                        0x2F => break,
                        0x51 if n == 3 => {
                            let us = (d[0] as u32) << 16 | (d[1] as u32) << 8 | d[2] as u32;
                            let bpm = (60_000_000 + us / 2) / us.max(1);
                            p.tempos.push((now, (bpm / 2).clamp(1, 255) as u8));
                        }
                        0x01 | 0x06 => match d {
                            b"[" => p.loop_start = Some(now),
                            b"]" => p.loop_end = Some(now),
                            _ => {}
                        },
                        _ => {}
                    }
                }
                0xF0 | 0xF7 => {
                    let n = t.vlq()? as usize;
                    t.take(n)?;
                }
                0x80..=0xEF => {
                    let a = t.u8()?;
                    let b = if matches!(first & 0xF0, 0xC0 | 0xD0) {
                        0
                    } else {
                        t.u8()?
                    };
                    let evs = p.chans.entry(ch).or_default();
                    match first & 0xF0 {
                        0x90 if b > 0 => {
                            // A retrigger ends the open note first.
                            if let Some((s, i)) = open.remove(&(ch, a)) {
                                close(evs, i, s, now);
                            }
                            open.insert((ch, a), (now, evs.len()));
                            evs.push((
                                now,
                                Ev::On {
                                    key: a,
                                    vel: b,
                                    len: 0,
                                },
                            ));
                        }
                        0x80 | 0x90 => {
                            if let Some((s, i)) = open.remove(&(ch, a)) {
                                close(evs, i, s, now);
                            }
                        }
                        0xB0 => match a {
                            1 => evs.push((now, Ev::Mod(b))),
                            7 => evs.push((now, Ev::Vol(b))),
                            10 => evs.push((now, Ev::Pan(b))),
                            _ => {}
                        },
                        0xC0 => evs.push((now, Ev::Voice(a))),
                        0xE0 => evs.push((now, Ev::Bend(b))),
                        _ => {}
                    }
                }
                s => bail!("unsupported MIDI status byte {s:#04X}"),
            }
        }
    }
    // Notes still open end with the song.
    for ((ch, _), (s, i)) in open {
        let end = p.end;
        close(p.chans.get_mut(&ch).unwrap(), i, s, end);
    }
    Ok(p)
}

/// Sets the length of the note at `i` that started at `start` and ends at `end` (one clock
/// at least) and pushes its end.
fn close(evs: &mut Vec<(u32, Ev)>, i: usize, start: u32, end: u32) {
    let len = end.saturating_sub(start).max(1);
    if let Ev::On { key, len: l, .. } = &mut evs[i].1 {
        *l = len;
        if len > MAX_CLOCK {
            let key = *key;
            evs.push((start + len, Ev::Off { key }));
        }
    }
}

/// `Wxx`s for `n` clocks.
fn waits(mut n: u32, out: &mut Vec<SongCmd>) {
    while n > 0 {
        let w = (CLOCKS.iter().rev())
            .find(|&&c| c as u32 <= n && c > 0)
            .copied()
            .unwrap();
        out.push(SongCmd::W(w));
        n -= w as u32;
    }
}

/// The song `bytes` (a standard MIDI file) plays with `voicegroup`.
pub fn convert(bytes: &[u8], voicegroup: &str) -> Result<SongFile> {
    let p = parse(bytes)?;
    let end = match p.loop_end {
        Some(e) => e,
        None => p.end,
    };
    let mut tracks = Vec::new();
    for evs in p.chans.values() {
        if !evs.iter().any(|e| matches!(e.1, Ev::On { .. })) {
            continue;
        }
        let mut evs: Vec<(u32, Ev)> = evs
            .iter()
            .copied()
            .filter(|e| e.0 < end || matches!(e.1, Ev::Off { .. }) && e.0 <= end)
            .collect();
        if tracks.is_empty() {
            evs.extend(p.tempos.iter().map(|&(t, v)| (t, Ev::Tempo(v))));
        }
        evs.sort();
        let mut out = vec![SongCmd::Keysh(0)];
        let mut now = 0u32;
        let mut looped = false;
        for (t, e) in evs {
            if let Some(ls) = p.loop_start
                && !looped
                && ls <= t
            {
                waits(ls - now, &mut out);
                now = ls;
                out.push(SongCmd::L(0));
                looped = true;
            }
            waits(t - now, &mut out);
            now = t;
            out.push(match e {
                Ev::Off { key } => SongCmd::Eot(Some(key)),
                Ev::Tempo(v) => SongCmd::Tempo(v),
                Ev::Voice(v) => SongCmd::Voice(v),
                Ev::Vol(v) => SongCmd::Vol(v),
                Ev::Pan(v) => SongCmd::Pan(v),
                Ev::Mod(v) => SongCmd::Mod(v),
                Ev::Bend(v) => SongCmd::Bend(v),
                Ev::On { key, vel, len } => {
                    // A note past the loop's end is cut there.
                    let len = len.min(end - t).max(1);
                    if len > MAX_CLOCK {
                        SongCmd::N(0, Some(key), Some(vel), None)
                    } else {
                        let n = (CLOCKS.iter().rev())
                            .find(|&&c| c as u32 <= len && c > 0)
                            .copied()
                            .unwrap();
                        let gate = (len - n as u32) as u8;
                        debug_assert!(clock_index(n).is_some() && gate <= 3);
                        SongCmd::N(n, Some(key), Some(vel), (gate > 0).then_some(gate))
                    }
                }
            });
        }
        if let Some(ls) = p.loop_start
            && !looped
        {
            waits(ls.saturating_sub(now), &mut out);
            now = now.max(ls);
            out.push(SongCmd::L(0));
            looped = true;
        }
        waits(end.saturating_sub(now), &mut out);
        if looped && p.loop_end.is_some() {
            out.push(SongCmd::Goto(0));
        }
        out.push(SongCmd::Fine);
        tracks.push(out);
    }
    ensure!(!tracks.is_empty(), "no notes");
    ensure!(
        tracks.len() <= 10,
        "{} channels play notes; MP2K's BGM player has 10 tracks",
        tracks.len()
    );
    Ok(SongFile {
        block_count: 0,
        priority: 0,
        reverb: 0,
        voicegroup: voicegroup.to_string(),
        tracks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A format-0 file: 96 ticks per beat, 120 bpm, a loop around two notes on channel 0
    /// (a quarter and a 5-beat tied note) and one eighth on channel 9.
    fn sample() -> Vec<u8> {
        let mut trk = Vec::new();
        let mut ev = |delta: u8, bytes: &[u8]| {
            trk.push(delta);
            trk.extend_from_slice(bytes);
        };
        ev(0, &[0xFF, 0x51, 3, 0x07, 0xA1, 0x20]); // 500,000 us: 120 bpm
        ev(0, &[0xFF, 0x06, 1, b'[']);
        ev(0, &[0xC0, 5]);
        ev(0, &[0xB0, 7, 100]);
        ev(0, &[0x90, 60, 100]);
        ev(0, &[0x99, 36, 90]);
        ev(48, &[0x89, 36, 0]);
        ev(48, &[0x80, 60, 0]);
        ev(0, &[0x90, 64, 80]);
        // 5 beats = 480 ticks (VLQ 0x83 0x60): 120 clocks, a tie.
        trk.extend_from_slice(&[0x83, 0x60, 0x80, 64, 0]);
        trk.extend_from_slice(&[0, 0xFF, 0x06, 1, b']']);
        trk.extend_from_slice(&[0, 0xFF, 0x2F, 0]);
        let mut f = b"MThd\0\0\0\x06\0\0\0\x01\0\x60MTrk".to_vec();
        f.extend_from_slice(&(trk.len() as u32).to_be_bytes());
        f.extend_from_slice(&trk);
        f
    }

    #[test]
    fn converts_notes_ties_loops_and_tempo() {
        let s = convert(&sample(), "voicegroup000").unwrap();
        assert_eq!(s.tracks.len(), 2);
        use SongCmd::*;
        assert_eq!(
            s.tracks[0],
            vec![
                Keysh(0),
                L(0),
                Tempo(60),
                Voice(5),
                Vol(100),
                N(24, Some(60), Some(100), None),
                W(24),
                N(0, Some(64), Some(80), None),
                W(96),
                W(24),
                Eot(Some(64)),
                Goto(0),
                Fine,
            ]
        );
        assert_eq!(
            s.tracks[1],
            vec![
                Keysh(0),
                L(0),
                N(12, Some(36), Some(90), None),
                W(96),
                W(48),
                Goto(0),
                Fine,
            ]
        );
        // It encodes.
        for t in &s.tracks {
            crate::sound::track::encode(t, 0x1000).unwrap();
        }
    }

    #[test]
    fn odd_lengths_take_gate_ticks_and_split_waits() {
        let mut out = Vec::new();
        waits(100, &mut out);
        assert_eq!(out, vec![SongCmd::W(96), SongCmd::W(4)]);
        out.clear();
        waits(27, &mut out);
        assert_eq!(out, vec![SongCmd::W(24), SongCmd::W(3)]);
    }

    #[test]
    fn rejects_garbage() {
        assert!(convert(b"RIFF", "v").is_err());
    }
}
