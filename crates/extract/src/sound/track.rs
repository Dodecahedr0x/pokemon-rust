//! MP2K track bytecode (`sound/MPlayDef.s`, m4a's `MPlayMain`) to and from [`SongCmd`]s.
//!
//! **Decoding** follows the bytes linearly from the track's start, with the running status the
//! assembler meant (the last command byte at or above `VOICE`), until a `FINE` (or a byte m4a runs
//! as `ply_fine`) or a `GOTO` with no jump target after it. A target before the start extends the
//! range back (the track then starts with a `Goto` to its entry). Every jump target gets an `L`.
//!
//! **Encoding** writes every command byte except a repeat of the running status (as mid2agb and
//! the decomp's songs do), never across a label or after a `PATT`, where the running status
//! depends on the path.
//! Pointers are `base + offset` of their label, little endian.

use crate::schema::SongCmd;
use anyhow::{Result, bail};
use std::collections::{BTreeMap, BTreeSet};

pub const FINE: u8 = 0xB1;
pub const GOTO: u8 = 0xB2;
pub const PATT: u8 = 0xB3;
pub const PEND: u8 = 0xB4;
pub const REPT: u8 = 0xB5;
pub const MEMACC: u8 = 0xB9;
pub const PRIO: u8 = 0xBA;
pub const TEMPO: u8 = 0xBB;
pub const KEYSH: u8 = 0xBC;
pub const VOICE: u8 = 0xBD;
pub const VOL: u8 = 0xBE;
pub const PAN: u8 = 0xBF;
pub const BEND: u8 = 0xC0;
pub const BENDR: u8 = 0xC1;
pub const LFOS: u8 = 0xC2;
pub const LFODL: u8 = 0xC3;
pub const MOD: u8 = 0xC4;
pub const MODT: u8 = 0xC5;
pub const TUNE: u8 = 0xC8;
pub const PORT: u8 = 0xCC;
pub const XCMD: u8 = 0xCD;
pub const EOT: u8 = 0xCE;
pub const TIE: u8 = 0xCF;

/// The wait and note lengths, `gClockTable` (49 entries: `W00`..`W96`).
pub const CLOCKS: [u8; 49] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x1C, 0x1E, 0x20, 0x24, 0x28, 0x2A, 0x2C,
    0x30, 0x34, 0x36, 0x38, 0x3C, 0x40, 0x42, 0x44, 0x48, 0x4C, 0x4E, 0x50, 0x54, 0x58, 0x5A, 0x5C,
    0x60,
];

/// The `gClockTable` index of `ticks`, if it is a table value.
pub fn clock_index(ticks: u8) -> Option<u8> {
    CLOCKS.iter().position(|&c| c == ticks).map(|i| i as u8)
}

/// Argument bytes of `XCMD` sub-command `sub` (`gXcmdTable`); `None`: it runs as `ply_fine`
/// (0, 3) or is out of the table.
pub fn xcmd_args(sub: u8) -> Option<usize> {
    match sub {
        1 | 13 => Some(4),
        2 | 4..=11 => Some(1),
        12 => Some(2),
        _ => None,
    }
}

/// Whether m4a runs command byte `cmd` (0xB1..=0xCE) as `ply_fine`.
pub fn is_fine(cmd: u8) -> bool {
    matches!(cmd, FINE | 0xB6..=0xB8 | 0xC6 | 0xC7 | 0xC9..=0xCB)
}

/// Reads bytes at absolute addresses (a ROM or an encoded blob).
pub trait Bytes {
    fn byte(&self, at: u32) -> Option<u8>;
    /// A pointer at `at`, as an address of these bytes.
    fn ptr(&self, at: u32) -> Option<u32>;
}

/// One decoded command at its address, and the addresses it may jump to.
struct Decoded {
    cmd: Raw,
    len: u32,
}

enum Raw {
    Cmd(SongCmd),
    /// `GOTO` (true) or `PATT` to an address.
    Jump(bool, u32),
    Rept(u8, u32),
    MemAcc(u8, u8, u8, u32),
}

fn opt(b: &impl Bytes, at: u32) -> Option<u8> {
    b.byte(at).filter(|&v| v < 0x80)
}

/// Decodes the command at `at` with running status `rs` (0: none). Returns it and the new
/// running status.
fn decode_one(b: &impl Bytes, at: u32, rs: u8) -> Result<(Decoded, u8)> {
    let first = b
        .byte(at)
        .ok_or_else(|| anyhow::anyhow!("track byte at {at:#x} out of range"))?;
    let (cmd, mut p) = if first >= 0x80 {
        (first, at + 1)
    } else if rs >= VOICE {
        (rs, at)
    } else {
        bail!("argument byte {first:#x} at {at:#x} without a running status");
    };
    let rs = if cmd >= VOICE { cmd } else { rs };
    let arg = |p: &mut u32| -> Result<u8> {
        let v = b
            .byte(*p)
            .ok_or_else(|| anyhow::anyhow!("track argument at {p:#x} out of range"))?;
        *p += 1;
        Ok(v)
    };
    let raw = match cmd {
        0x80..=0xB0 => Raw::Cmd(SongCmd::W(CLOCKS[(cmd - 0x80) as usize])),
        GOTO | PATT => {
            let t = b
                .ptr(p)
                .ok_or_else(|| anyhow::anyhow!("bad jump pointer at {p:#x}"))?;
            p += 4;
            Raw::Jump(cmd == GOTO, t)
        }
        PEND => Raw::Cmd(SongCmd::Pend),
        REPT => {
            let n = arg(&mut p)?;
            let t = b
                .ptr(p)
                .ok_or_else(|| anyhow::anyhow!("bad REPT pointer at {p:#x}"))?;
            p += 4;
            Raw::Rept(n, t)
        }
        MEMACC => {
            let (op, addr, data) = (arg(&mut p)?, arg(&mut p)?, arg(&mut p)?);
            if (6..=17).contains(&op) {
                let t = b
                    .ptr(p)
                    .ok_or_else(|| anyhow::anyhow!("bad MEMACC pointer at {p:#x}"))?;
                p += 4;
                Raw::MemAcc(op, addr, data, t)
            } else {
                Raw::Cmd(SongCmd::MemAcc(op, addr, data, None))
            }
        }
        c if is_fine(c) => Raw::Cmd(if c == FINE {
            SongCmd::Fine
        } else {
            SongCmd::Op(c)
        }),
        PRIO => Raw::Cmd(SongCmd::Prio(arg(&mut p)?)),
        TEMPO => Raw::Cmd(SongCmd::Tempo(arg(&mut p)?)),
        KEYSH => Raw::Cmd(SongCmd::Keysh(arg(&mut p)?)),
        VOICE => Raw::Cmd(SongCmd::Voice(arg(&mut p)?)),
        VOL => Raw::Cmd(SongCmd::Vol(arg(&mut p)?)),
        PAN => Raw::Cmd(SongCmd::Pan(arg(&mut p)?)),
        BEND => Raw::Cmd(SongCmd::Bend(arg(&mut p)?)),
        BENDR => Raw::Cmd(SongCmd::Bendr(arg(&mut p)?)),
        LFOS => Raw::Cmd(SongCmd::Lfos(arg(&mut p)?)),
        LFODL => Raw::Cmd(SongCmd::Lfodl(arg(&mut p)?)),
        MOD => Raw::Cmd(SongCmd::Mod(arg(&mut p)?)),
        MODT => Raw::Cmd(SongCmd::Modt(arg(&mut p)?)),
        TUNE => Raw::Cmd(SongCmd::Tune(arg(&mut p)?)),
        PORT => Raw::Cmd(SongCmd::Port(arg(&mut p)?, arg(&mut p)?)),
        XCMD => {
            let sub = arg(&mut p)?;
            let Some(n) = xcmd_args(sub) else {
                bail!("XCMD sub-command {sub} at {at:#x} (runs as ply_fine) is not supported");
            };
            if sub == 1 {
                bail!("XCMD xWAVE (a sample pointer) at {at:#x} is not supported");
            }
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                v.push(arg(&mut p)?);
            }
            Raw::Cmd(SongCmd::X(sub, v))
        }
        EOT => {
            let k = opt(b, p);
            p += k.is_some() as u32;
            Raw::Cmd(SongCmd::Eot(k))
        }
        TIE..=0xFF => {
            let len = CLOCKS[(cmd - TIE) as usize];
            let key = opt(b, p);
            p += key.is_some() as u32;
            let vel = key.and_then(|_| opt(b, p));
            p += vel.is_some() as u32;
            let gate = vel.and_then(|_| opt(b, p));
            p += gate.is_some() as u32;
            Raw::Cmd(SongCmd::N(len, key, vel, gate))
        }
        _ => bail!("track command {cmd:#x} at {at:#x}"),
    };
    Ok((
        Decoded {
            cmd: raw,
            len: p - at,
        },
        rs,
    ))
}

/// Whether execution never falls through `raw`.
fn ends(raw: &Raw) -> bool {
    matches!(raw, Raw::Jump(true, _) | Raw::Rept(0, _))
        || matches!(raw, Raw::Cmd(SongCmd::Fine | SongCmd::Op(_)))
}

fn targets(raw: &Raw) -> Option<u32> {
    match raw {
        Raw::Jump(_, t) | Raw::Rept(_, t) | Raw::MemAcc(_, _, _, t) => Some(*t),
        Raw::Cmd(_) => None,
    }
}

/// Decodes linearly from `lo`, and every jump target seen.
/// Commands by address, and the jump targets.
type Linear = (Vec<(u32, Raw)>, BTreeSet<u32>);

fn decode_range(b: &impl Bytes, lo: u32, start: u32) -> Result<Linear> {
    let mut out = Vec::new();
    let mut want = BTreeSet::from([start]);
    let (mut at, mut rs) = (lo, 0u8);
    loop {
        if out.len() > 200_000 {
            bail!("track at {start:#x} does not end");
        }
        let (d, nrs) = decode_one(b, at, rs)?;
        rs = nrs;
        if let Some(t) = targets(&d.cmd) {
            want.insert(t);
        }
        let end = ends(&d.cmd);
        out.push((at, d.cmd));
        at += d.len;
        if end && want.range(at..).next().is_none() {
            // mid2agb ends a looping track with `GOTO label` and a dead `FINE`.
            if b.byte(at) == Some(FINE) && !matches!(out.last(), Some((_, Raw::Cmd(_)))) {
                out.push((at, Raw::Cmd(SongCmd::Fine)));
            }
            break;
        }
    }
    Ok((out, want))
}

/// Decodes the track starting at `start` (see the module docs).
pub fn decode(b: &impl Bytes, start: u32) -> Result<Vec<SongCmd>> {
    let mut lo = start;
    let (cmds, want) = loop {
        let (cmds, want) = decode_range(b, lo, start)?;
        let first = *want.iter().next().unwrap();
        if first < lo {
            lo = first;
            continue;
        }
        break (cmds, want);
    };
    let addrs: BTreeSet<u32> = cmds.iter().map(|(a, _)| *a).collect();
    if let Some(t) = want.iter().find(|t| !addrs.contains(t)) {
        bail!("track at {start:#x}: jump target {t:#x} is not a command boundary");
    }
    // Labels in address order (the entry only when something jumps to it or it is not first).
    let jumped: BTreeSet<u32> = cmds.iter().filter_map(|(_, r)| targets(r)).collect();
    let mut labels = BTreeMap::new();
    for &a in &addrs {
        if jumped.contains(&a) || (a == start && start != lo) {
            let n = labels.len() as u16;
            labels.insert(a, n);
        }
    }
    let mut out = Vec::with_capacity(cmds.len() + labels.len() + 1);
    if start != lo {
        out.push(SongCmd::Goto(labels[&start]));
    }
    for (a, raw) in cmds {
        if let Some(&l) = labels.get(&a) {
            out.push(SongCmd::L(l));
        }
        out.push(match raw {
            Raw::Cmd(c) => c,
            Raw::Jump(true, t) => SongCmd::Goto(labels[&t]),
            Raw::Jump(false, t) => SongCmd::Patt(labels[&t]),
            Raw::Rept(n, t) => SongCmd::Rept(n, labels[&t]),
            Raw::MemAcc(op, addr, data, t) => SongCmd::MemAcc(op, addr, data, Some(labels[&t])),
        });
    }
    Ok(out)
}

/// The command byte of `c` (`None` for a label).
fn cmd_byte(c: &SongCmd) -> Result<Option<u8>> {
    use SongCmd::*;
    Ok(Some(match c {
        L(_) => return Ok(None),
        W(t) => 0x80 + clock_index(*t).ok_or_else(|| anyhow::anyhow!("wait of {t} ticks"))?,
        N(t, ..) => TIE + clock_index(*t).ok_or_else(|| anyhow::anyhow!("note of {t} ticks"))?,
        Eot(_) => EOT,
        Fine => FINE,
        Goto(_) => GOTO,
        Patt(_) => PATT,
        Pend => PEND,
        Rept(..) => REPT,
        MemAcc(..) => MEMACC,
        Prio(_) => PRIO,
        Tempo(_) => TEMPO,
        Keysh(_) => KEYSH,
        Voice(_) => VOICE,
        Vol(_) => VOL,
        Pan(_) => PAN,
        Bend(_) => BEND,
        Bendr(_) => BENDR,
        Lfos(_) => LFOS,
        Lfodl(_) => LFODL,
        Mod(_) => MOD,
        Modt(_) => MODT,
        Tune(_) => TUNE,
        Port(..) => PORT,
        X(..) => XCMD,
        Op(o) => *o,
    }))
}

/// Encodes a track whose first byte will sit at `base`. Pointers are absolute (`base` + offset).
pub fn encode(cmds: &[SongCmd], base: u32) -> Result<Vec<u8>> {
    // Two passes: label offsets, then bytes (command lengths do not depend on the offsets).
    let mut labels = BTreeMap::new();
    let mut out = Vec::new();
    for pass in 0..2 {
        out.clear();
        let mut rs = 0u8;
        // The last command can take one more argument byte (a note's or `EOT`'s optional
        // ones): a running-status byte after it would be read as that argument.
        let mut open = false;
        for c in cmds {
            let Some(byte) = cmd_byte(c)? else {
                if let SongCmd::L(l) = c {
                    labels.insert(*l, out.len() as u32);
                }
                rs = 0;
                open = false;
                continue;
            };
            // A note's running status needs its key byte (the byte that triggers it).
            let keyless_note = matches!(c, SongCmd::N(_, None, ..));
            if byte < VOICE || byte != rs || keyless_note || open {
                out.push(byte);
            }
            open = match c {
                SongCmd::N(_, _, _, gate) => gate.is_none(),
                SongCmd::Eot(key) => key.is_none(),
                _ => false,
            };
            if byte >= VOICE {
                rs = byte;
            } else if byte == PATT {
                // Back from the pattern, the running status is the pattern's.
                rs = 0;
            }
            let label = |l: &u16| -> Result<[u8; 4]> {
                if pass == 0 {
                    return Ok([0; 4]);
                }
                let off = labels
                    .get(l)
                    .ok_or_else(|| anyhow::anyhow!("no label {l}"))?;
                Ok((base + off).to_le_bytes())
            };
            use SongCmd::*;
            match c {
                L(_) | W(_) | Fine | Pend | Op(_) => {}
                N(_, key, vel, gate) => {
                    for v in [key, vel, gate].into_iter().map_while(|v| *v) {
                        if v >= 0x80 {
                            bail!("note argument {v:#x}");
                        }
                        out.push(v);
                    }
                    if key.is_none() && (vel.is_some() || gate.is_some())
                        || vel.is_none() && gate.is_some()
                    {
                        bail!("note arguments must be given in order: {c:?}");
                    }
                }
                Eot(k) => out.extend(k.iter().copied()),
                Goto(l) | Patt(l) => out.extend(label(l)?),
                Rept(n, l) => {
                    out.push(*n);
                    out.extend(label(l)?);
                }
                MemAcc(op, addr, data, l) => {
                    out.extend([*op, *addr, *data]);
                    if let Some(l) = l {
                        out.extend(label(l)?);
                    }
                }
                Prio(v) | Tempo(v) | Keysh(v) | Voice(v) | Vol(v) | Pan(v) | Bend(v) | Bendr(v)
                | Lfos(v) | Lfodl(v) | Mod(v) | Modt(v) | Tune(v) => out.push(*v),
                Port(r, v) => out.extend([*r, *v]),
                X(sub, args) => {
                    if xcmd_args(*sub) != Some(args.len()) || *sub == 1 {
                        bail!("XCMD {sub} with {} argument bytes", args.len());
                    }
                    out.push(*sub);
                    out.extend(args);
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Mem(u32, Vec<u8>);
    impl Bytes for Mem {
        fn byte(&self, at: u32) -> Option<u8> {
            self.1.get(at.checked_sub(self.0)? as usize).copied()
        }
        fn ptr(&self, at: u32) -> Option<u32> {
            let o = at.checked_sub(self.0)? as usize;
            let v = u32::from_le_bytes(self.1.get(o..o + 4)?.try_into().ok()?);
            Some(v)
        }
    }

    #[test]
    fn decode_encode_round_trip_with_running_status_and_labels() {
        let base: u32 = 0x1000;
        let mut b = vec![
            KEYSH,
            0,
            TEMPO,
            50,
            VOICE,
            4, // header
            XCMD,
            8,
            10,
            9,
            8, // XCMD xIECV 10, (running status) xIECL 8
            VOL,
            90, //
            0xD0 + 23,
            60,
            100,       // N24 Cn3 v100
            0x80 + 24, // W24
            62,        // running status note, key 62
            0x80 + 24,
            EOT,
            62, // EOT Dn3
        ];
        let loop_at = base + 6;
        b.push(GOTO);
        b.extend(loop_at.to_le_bytes());
        b.push(FINE);
        let m = Mem(base, b.clone());
        let cmds = decode(&m, base).unwrap();
        assert_eq!(cmds[3], SongCmd::L(0));
        assert_eq!(cmds[4], SongCmd::X(8, vec![10]));
        assert_eq!(cmds[5], SongCmd::X(9, vec![8]));
        assert_eq!(cmds[7], SongCmd::N(24, Some(60), Some(100), None));
        assert_eq!(cmds[9], SongCmd::N(24, Some(62), None, None));
        assert_eq!(encode(&cmds, base).unwrap(), b);
    }

    #[test]
    fn a_target_before_the_start_starts_with_a_goto() {
        let base: u32 = 0x2000;
        let mut b = vec![0x80 + 12, FINE, GOTO];
        b.extend(base.to_le_bytes());
        let m = Mem(base, b);
        let cmds = decode(&m, base + 2).unwrap();
        assert_eq!(
            cmds,
            [
                SongCmd::Goto(1),
                SongCmd::L(0),
                SongCmd::W(12),
                SongCmd::Fine,
                SongCmd::L(1),
                SongCmd::Goto(0)
            ]
        );
    }
}
