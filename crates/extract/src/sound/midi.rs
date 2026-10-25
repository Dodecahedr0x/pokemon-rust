//! Standard MIDI files to MP2K songs (Milestone 12): the author's tracks
//! (`games/silice/music/MUS_SIL_*.mid`) become [`SongFile`]s the content build adds to the
//! extract's song table, as the decomp's `mid2agb` turns `sound/songs/midi/*.mid` into `.s`.
//!
//! The conversion follows `mid2agb` the way Emerald builds songs (`-E`, exact gate time; master
//! volume, reverb and priority stay at mid2agb's defaults, as in `midi.cfg` those are per song):
//! - time: 24 MP2K clocks per quarter note, MIDI ticks truncated (`tick * 24 / division`).
//!   A note's length is that truncation of its own tick length (at least 1), which is not the
//!   gap between the truncated start and end when the division is not 24;
//! - one MP2K track per MIDI channel that plays a note, in channel order. Tempo and the loop
//!   markers are read from the first track only, and the tempo is written on the first track
//!   (`TEMPO` is beats per minute / 2);
//! - program change -> `VOICE`, CC 7 -> `VOL`, CC 10 -> `PAN`, CC 1 -> `MOD`, CC 20 -> `BENDR`,
//!   CC 21 -> `LFOS`, CC 22 -> `MODT`, CC 24 -> `TUNE`, CC 26 -> `LFODL`, CC 30 then CC 29 ->
//!   `xIECV` (8) or `xIECL` (9), the pitch wheel -> `BEND` (its high 7 bits). Every track starts
//!   with `KEYSH 0`. Controllers that share a clock keep file order, so the last one wins.
//!   A track with no `VOL` at or before its first note gets `VOL 127` once, ahead of the loop;
//! - note velocity through mid2agb's `g_noteVelocityLUT`;
//! - a note of up to 96 clocks is one `Nxx` (the largest table length that fits, plus 1-3 gate
//!   ticks); a longer one is `TIE` ... `EOT`;
//! - waits are `Wxx` from the clock table, 96 at most each;
//! - the markers `[` and `]` on the first track loop the song: every track gets a label at `[`
//!   and jumps back to it at `]` (`GOTO`); without them the song ends (`FINE`).

use crate::schema::{SongCmd, SongFile};
use crate::sound::track::{CLOCKS, clock_index};
use anyhow::{Context, Result, bail, ensure};
use std::collections::BTreeMap;

/// MP2K clocks per quarter note.
pub const CLOCKS_PER_BEAT: u32 = 24;
/// The longest `Nxx` / `Wxx`.
const MAX_CLOCK: u32 = 96;

/// One event of a channel, at an MP2K clock. Order at a clock is [`rank`], then file order:
/// deriving `Ord` would let a larger controller value beat the one that came later.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ev {
    // Sorted within a clock: note ends, then controls, then note starts.
    Off {
        key: u8,
    },
    Tempo(u8),
    Voice(u8),
    Vol(u8),
    Pan(u8),
    Mod(u8),
    Bendr(u8),
    Lfos(u8),
    Modt(u8),
    Tune(u8),
    Lfodl(u8),
    /// `xIECV` / `xIECL` (`XCMD` 8 or 9), from CC 30 (the sub-command) then CC 29 (the value).
    X(u8, u8),
    Bend(u8),
    On {
        key: u8,
        vel: u8,
        len: u32,
    },
}

/// Where `ev` sits among the events of one clock. mid2agb's `EventCompare`: note ends, then
/// tempo and the instrument, then controllers in file order, then the pitch bend, then notes
/// from low to high.
fn rank(ev: &Ev) -> (u8, u8) {
    match ev {
        Ev::Off { key } => (0, *key),
        Ev::Tempo(_) => (1, 0),
        Ev::Voice(_) => (2, 0),
        Ev::Vol(_)
        | Ev::Pan(_)
        | Ev::Mod(_)
        | Ev::Bendr(_)
        | Ev::Lfos(_)
        | Ev::Modt(_)
        | Ev::Tune(_)
        | Ev::Lfodl(_)
        | Ev::X(_, _) => (3, 0),
        Ev::Bend(_) => (4, 0),
        Ev::On { key, .. } => (5, *key),
    }
}

/// An event at an MP2K clock. `seq` is its place in the file.
#[derive(Clone, Copy, Debug)]
struct At {
    t: u32,
    seq: u32,
    ev: Ev,
}

/// mid2agb's `g_noteVelocityLUT`: groups of four map up to the next multiple of four, and
/// 125..=127 stay 127.
fn velocity(v: u8) -> u8 {
    if v == 0 {
        0
    } else if v >= 125 {
        127
    } else {
        v.div_ceil(4) * 4
    }
}

/// A parsed MIDI file, as MP2K-clock events.
#[derive(Debug, Default)]
struct Parsed {
    /// Per channel, in file order.
    chans: BTreeMap<u8, Vec<At>>,
    tempos: Vec<At>,
    loop_start: Option<u32>,
    loop_end: Option<u32>,
    /// The last event's clock, including a tie that ends past it.
    end: u32,
    /// The last MIDI tick, where a note that never got a note-off ends.
    end_tick: u64,
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
    // mid2agb truncates; it does not round. A note's length truncates its own tick span.
    let clock = |tick: u64| ((tick * CLOCKS_PER_BEAT as u64) / division as u64) as u32;
    let note_len = |span: u64| clock(span).max(1);
    let mut p = Parsed::default();
    // Open notes: (channel, key) -> (start tick, start clock, index of the On).
    let mut open: BTreeMap<(u8, u8), (u64, u32, usize)> = BTreeMap::new();
    // CC 30's extended sub-command, per channel (mid2agb's `s_extendedCommand`, which a track
    // reads from its own channel only).
    let mut ext = [None; 16];
    let mut nseq = 0u32;
    // Tempo and loop markers come from the first track, as `ReadSeqEvents` does.
    let mut first_track = true;
    for _ in 0..ntracks {
        let id = r.take(4)?.to_vec();
        let len = r.be(4)? as usize;
        let data = r.take(len)?;
        if id != b"MTrk" {
            continue;
        }
        let this_seq = first_track;
        first_track = false;
        let mut t = Reader { b: data, at: 0 };
        let (mut tick, mut status) = (0u64, 0u8);
        while t.at < data.len() {
            tick += t.vlq()? as u64;
            let now = clock(tick);
            p.end = p.end.max(now);
            p.end_tick = p.end_tick.max(tick);
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
                    // Meta events cancel running status.
                    status = 0;
                    let ty = t.u8()?;
                    let n = t.vlq()? as usize;
                    let d = t.take(n)?;
                    match ty {
                        0x2F => break,
                        0x51 if this_seq && n == 3 => {
                            let us = (d[0] as u32) << 16 | (d[1] as u32) << 8 | d[2] as u32;
                            let bpm = (60_000_000 + us / 2) / us.max(1);
                            p.tempos.push(At {
                                t: now,
                                seq: nseq,
                                ev: Ev::Tempo((bpm / 2).clamp(1, 255) as u8),
                            });
                            nseq += 1;
                        }
                        0x01..=0x07 if this_seq => match d {
                            b"[" => p.loop_start = Some(now),
                            b"]" => p.loop_end = Some(now),
                            _ => {}
                        },
                        _ => {}
                    }
                }
                0xF0 | 0xF7 => {
                    status = 0;
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
                    let mut tie_at = None;
                    {
                        let evs = p.chans.entry(ch).or_default();
                        match first & 0xF0 {
                            0x90 if b > 0 => {
                                // A retrigger ends the open note first.
                                if let Some((start_tick, start_clock, i)) = open.remove(&(ch, a)) {
                                    tie_at = finish(
                                        evs,
                                        i,
                                        start_clock,
                                        note_len(tick.saturating_sub(start_tick)),
                                        &mut nseq,
                                    );
                                }
                                open.insert((ch, a), (tick, now, evs.len()));
                                push_at(
                                    evs,
                                    &mut nseq,
                                    now,
                                    Ev::On {
                                        key: a,
                                        vel: velocity(b),
                                        len: 0,
                                    },
                                );
                            }
                            0x80 | 0x90 => {
                                if let Some((start_tick, start_clock, i)) = open.remove(&(ch, a)) {
                                    tie_at = finish(
                                        evs,
                                        i,
                                        start_clock,
                                        note_len(tick.saturating_sub(start_tick)),
                                        &mut nseq,
                                    );
                                }
                            }
                            0xB0 => match a {
                                1 => push_at(evs, &mut nseq, now, Ev::Mod(b)),
                                7 => push_at(evs, &mut nseq, now, Ev::Vol(b)),
                                10 => push_at(evs, &mut nseq, now, Ev::Pan(b)),
                                20 => push_at(evs, &mut nseq, now, Ev::Bendr(b)),
                                21 => push_at(evs, &mut nseq, now, Ev::Lfos(b)),
                                22 => push_at(evs, &mut nseq, now, Ev::Modt(b)),
                                24 => push_at(evs, &mut nseq, now, Ev::Tune(b)),
                                26 => push_at(evs, &mut nseq, now, Ev::Lfodl(b)),
                                30 => ext[ch as usize] = Some(b),
                                29 => {
                                    if let Some(sub @ (8 | 9)) = ext[ch as usize] {
                                        push_at(evs, &mut nseq, now, Ev::X(sub, b));
                                    }
                                }
                                _ => {}
                            },
                            0xC0 => push_at(evs, &mut nseq, now, Ev::Voice(a)),
                            0xE0 => push_at(evs, &mut nseq, now, Ev::Bend(b)),
                            _ => {}
                        }
                    }
                    if let Some(t) = tie_at {
                        p.end = p.end.max(t);
                    }
                }
                s => bail!("unsupported MIDI status byte {s:#04X}"),
            }
        }
    }
    // Notes still open end with the song.
    let end_tick = p.end_tick;
    let mut ties = Vec::new();
    for ((ch, _), (start_tick, start_clock, i)) in open {
        if let Some(t) = finish(
            p.chans.get_mut(&ch).unwrap(),
            i,
            start_clock,
            note_len(end_tick.saturating_sub(start_tick)),
            &mut nseq,
        ) {
            ties.push(t);
        }
    }
    for t in ties {
        p.end = p.end.max(t);
    }
    Ok(p)
}

fn push_at(evs: &mut Vec<At>, nseq: &mut u32, t: u32, ev: Ev) {
    evs.push(At { t, seq: *nseq, ev });
    *nseq += 1;
}

/// Sets the length of the note at `i` (already in clocks) and, when it is a tie, pushes the
/// `EOT` at `start + len`. Returns that clock.
fn finish(evs: &mut Vec<At>, i: usize, start: u32, len: u32, nseq: &mut u32) -> Option<u32> {
    let Ev::On { key, len: l, .. } = &mut evs[i].ev else {
        return None;
    };
    *l = len;
    if len <= MAX_CLOCK {
        return None;
    }
    let key = *key;
    let t = start.saturating_add(len);
    evs.push(At {
        t,
        seq: *nseq,
        ev: Ev::Off { key },
    });
    *nseq += 1;
    Some(t)
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
        if !evs.iter().any(|e| matches!(e.ev, Ev::On { .. })) {
            continue;
        }
        let mut evs: Vec<At> = evs
            .iter()
            .copied()
            .filter(|e| e.t < end || matches!(e.ev, Ev::Off { .. }) && e.t <= end)
            .collect();
        if tracks.is_empty() {
            evs.extend(p.tempos.iter().copied());
        }
        // File order within a rank, so two controllers at one clock keep the later value.
        evs.sort_by(|a, b| (a.t, rank(&a.ev), a.seq).cmp(&(b.t, rank(&b.ev), b.seq)));
        // mid2agb inserts `VOL 127` when no volume controller lands at or before the first
        // note. It is printed before the loop label, so a later iteration does not snap back.
        let note_at = evs
            .iter()
            .find_map(|e| matches!(e.ev, Ev::On { .. }).then_some(e.t));
        let vol_before =
            note_at.is_some_and(|n| evs.iter().any(|e| matches!(e.ev, Ev::Vol(_)) && e.t <= n));
        let mut out = vec![SongCmd::Keysh(0)];
        if !vol_before {
            out.push(SongCmd::Vol(127));
        }
        let mut now = 0u32;
        let mut looped = false;
        for At { t, ev: e, .. } in evs {
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
                Ev::Bendr(v) => SongCmd::Bendr(v),
                Ev::Lfos(v) => SongCmd::Lfos(v),
                Ev::Modt(v) => SongCmd::Modt(v),
                Ev::Tune(v) => SongCmd::Tune(v),
                Ev::Lfodl(v) => SongCmd::Lfodl(v),
                Ev::X(sub, arg) => SongCmd::X(sub, vec![arg]),
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
                Vol(127),
                L(0),
                N(12, Some(36), Some(92), None),
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

    /// No CC 7, so the track starts at `VOL 127` outside the loop. CC 20, 21 and 30/29 are
    /// bend range, LFO speed and the pseudo-echo, as mid2agb maps them. Velocity 90 quantizes
    /// to 92.
    #[test]
    fn controllers_default_volume_and_velocity() {
        let mut trk = Vec::new();
        let mut ev = |delta: u8, bytes: &[u8]| {
            trk.push(delta);
            trk.extend_from_slice(bytes);
        };
        ev(0, &[0xFF, 0x51, 3, 0x07, 0xA1, 0x20]);
        ev(0, &[0xFF, 0x06, 1, b'[']);
        ev(0, &[0xB0, 20, 12]); // BENDR
        ev(0, &[0xB0, 21, 22]); // LFOS
        ev(0, &[0xB0, 30, 8]); // xIECV
        ev(0, &[0xB0, 29, 40]);
        ev(0, &[0x90, 60, 90]);
        ev(96, &[0x80, 60, 0]);
        ev(0, &[0xFF, 0x06, 1, b']']);
        ev(0, &[0xFF, 0x2F, 0]);
        let mut f = b"MThd\0\0\0\x06\0\0\0\x01\0\x60MTrk".to_vec();
        f.extend_from_slice(&(trk.len() as u32).to_be_bytes());
        f.extend_from_slice(&trk);
        let s = convert(&f, "v").unwrap();
        use SongCmd::*;
        assert_eq!(
            s.tracks[0],
            vec![
                Keysh(0),
                Vol(127),
                L(0),
                Tempo(60),
                Bendr(12),
                Lfos(22),
                X(8, vec![40]),
                N(24, Some(60), Some(92), None),
                W(24),
                Goto(0),
                Fine,
            ]
        );
    }

    #[test]
    fn velocity_table_matches_mid2agb() {
        assert_eq!(velocity(0), 0);
        assert_eq!(velocity(1), 4);
        assert_eq!(velocity(90), 92);
        assert_eq!(velocity(124), 124);
        assert_eq!(velocity(125), 127);
        assert_eq!(velocity(127), 127);
    }

    /// Two echo levels at one clock: the later, smaller one wins. Sorting by the value would
    /// leave 18 sounding (`mus_abnormal_weather`).
    #[test]
    fn same_clock_controllers_keep_file_order() {
        let mut trk = vec![0]; // delta 0, then the events
        trk.extend_from_slice(&[
            0xB0, 30, 8, 0, 0xB0, 29, 18, 0, 0xB0, 30, 8, 0, 0xB0, 29, 16,
        ]);
        trk.extend_from_slice(&[0, 0xB0, 7, 100, 0, 0xB0, 7, 40]);
        trk.extend_from_slice(&[0, 0x90, 60, 64, 24, 0x80, 60, 0, 0, 0xFF, 0x2F, 0]);
        let mut f = b"MThd\0\0\0\x06\0\0\0\x01\0\x18MTrk".to_vec();
        f.extend_from_slice(&(trk.len() as u32).to_be_bytes());
        f.extend_from_slice(&trk);
        let s = convert(&f, "v").unwrap();
        use SongCmd::*;
        assert_eq!(
            s.tracks[0],
            vec![
                Keysh(0),
                X(8, vec![18]),
                X(8, vec![16]),
                Vol(100),
                Vol(40),
                N(24, Some(60), Some(64), None),
                W(24),
                Fine,
            ]
        );
    }

    /// 480 ticks per quarter. A 59-tick note is 2 clocks (`24 * 59 / 480`), not the 3 clocks
    /// between the truncated start (tick 1 -> 0) and end (tick 60 -> 3).
    #[test]
    fn note_length_truncates_its_own_ticks() {
        let mut trk = vec![1, 0x90, 60, 64]; // tick 1, note on
        trk.extend_from_slice(&[59, 0x80, 60, 0, 0, 0xFF, 0x2F, 0]);
        let mut f = b"MThd\0\0\0\x06\0\0\0\x01\x01\xE0MTrk".to_vec();
        f.extend_from_slice(&(trk.len() as u32).to_be_bytes());
        f.extend_from_slice(&trk);
        let s = convert(&f, "v").unwrap();
        assert!(
            s.tracks[0].contains(&SongCmd::N(2, Some(60), Some(64), None)),
            "{:?}",
            s.tracks[0]
        );
    }
}
