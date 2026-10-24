//! The MP2K (m4a) sound data (Milestone 12): `gSongTable` and its songs, the voicegroups with
//! the cry tables, the direct sound samples, the programmable waves and the key split tables,
//! plus `sFanfares` and `gSpeciesIdToCryId`. Written to `sound/` (see [`export`]).
//!
//! **Locating** (no per-version offsets):
//! - `gSongTable`: the one run of [`SONG_COUNT`] `struct Song { header, ms, me }` entries whose
//!   headers are ROM pointers with at most 16 tracks, `ms` and `me` at most 3;
//! - the voice memory: every voicegroup the songs reach (with the sub-groups of key split and
//!   rhythm voices), as one run of 12-byte entries from the first group to 128 entries past the
//!   last ([`VoicesFile`]);
//! - the cry tables: the two runs of `cry` (type 0x20) and `cry_reverse` (0x30) entries in it;
//! - `sFanfares`: `{ MUS_LEVEL_UP, 80 }, { MUS_OBTAIN_ITEM, 160 }` ... ([`FANFARE_COUNT`]);
//! - `gSpeciesIdToCryId`: the cry ids of Treecko, Grovyle, Sceptile, Torchic... ([`CRY_ID_COUNT`]).
//!
//! **Names** come from the decomp when it is given ([`DecompNames`]): songs' voicegroups from
//! `sound/songs/*.s` and `midi.cfg`, then the samples, waves, key split tables and sub-groups
//! from `sound/voicegroups/**.inc` by entry position. Anything else is positional
//! (`voicegroup_017`, `sample_042`, `wave_03`, `keysplit_1`), numbered in ROM order.

pub mod track;

use crate::ROM_BASE;
use crate::export::Out;
use crate::rom::Rom;
use crate::schema::{
    Hex, SampleInfo, SamplesFile, SongCmd, SongFile, SongTableEntry, SongTableFile,
    SoundTablesFile, Voice, VoiceRef, VoiceTail, VoicesFile,
};
use crate::text::charmap::SOUNDS;
use anyhow::{Context, Result, bail};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Songs in `gSongTable` (`charmap::SOUNDS`, `include/constants/songs.h`).
pub const SONG_COUNT: usize = 610;
/// `sFanfares` entries (`src/sound.c`).
pub const FANFARE_COUNT: usize = 18;
/// `gSpeciesIdToCryId` entries (`src/data/pokemon/cry_ids.h`: Treecko to Chimecho).
pub const CRY_ID_COUNT: usize = 135;
/// Voices a voicegroup may index (a `VOICE` operand is a byte below 0x80).
pub const GROUP_VOICES: u32 = 128;
const TONE_SIZE: u32 = 12;
/// `sizeof(struct WaveData)` without `data`.
pub const WAVE_HEADER: u32 = 16;
/// Samples per DPCM block, and the block's bytes (`wav2agb -c`).
pub const DPCM_BLOCK: usize = 0x40;
pub const DPCM_BLOCK_BYTES: usize = 0x21;
/// m4a's `gDeltaEncodingTable`.
pub const DPCM_DELTAS: [i8; 16] = [
    0, 1, 4, 9, 16, 25, 36, 49, -64, -49, -36, -25, -16, -9, -4, -1,
];
/// Programmable wave bytes (32 4-bit samples).
pub const WAVE_BYTES: usize = 16;

pub const SONGS_FILE: &str = "sound/songs.ron";
pub const VOICES_FILE: &str = "sound/voices.ron";
pub const SAMPLES_FILE: &str = "sound/samples.ron";
pub const TABLES_FILE: &str = "sound/tables.ron";

fn is_ptr(rom: &Rom, v: u32) -> bool {
    v >= ROM_BASE && ((v - ROM_BASE) as usize) < rom.bytes().len()
}

/// The ROM as track bytes (addresses are file offsets).
struct RomBytes<'a>(&'a Rom);

impl track::Bytes for RomBytes<'_> {
    fn byte(&self, at: u32) -> Option<u8> {
        self.0.u8(at).ok()
    }
    fn ptr(&self, at: u32) -> Option<u32> {
        let v = self.0.u32(at).ok()?;
        is_ptr(self.0, v).then(|| v - ROM_BASE)
    }
}

/// The file offset of `gSongTable` (see the module docs).
pub fn locate_song_table(rom: &Rom) -> Result<u32> {
    let len = rom.bytes().len() as u32;
    let entry_ok = |at: u32| -> bool {
        let (Ok(p), Ok(ms), Ok(me)) = (rom.u32(at), rom.u16(at + 4), rom.u16(at + 6)) else {
            return false;
        };
        if !is_ptr(rom, p) || ms > 3 || me > 3 {
            return false;
        }
        let h = p - ROM_BASE;
        let Ok(tracks) = rom.u8(h) else { return false };
        tracks <= 16 && (tracks == 0 || rom.u32(h + 4).is_ok_and(|t| is_ptr(rom, t)))
    };
    let mut hits = Vec::new();
    let mut at = 0;
    while at + 8 <= len {
        let mut n = 0u32;
        while at + 8 * (n + 1) <= len && entry_ok(at + 8 * n) {
            n += 1;
        }
        if n as usize >= SONG_COUNT {
            hits.push((at, n));
            at += 8 * n;
        } else {
            at += 4;
        }
    }
    match hits[..] {
        [(at, _)] => Ok(at),
        _ => bail!("gSongTable: expected one run of {SONG_COUNT} songs, found {hits:#x?}"),
    }
}

/// One `struct SongHeader`.
#[derive(Clone, Debug)]
pub struct Header {
    pub offset: u32,
    pub block_count: u8,
    pub priority: u8,
    pub reverb: u8,
    /// The voicegroup (file offset), `None` with no tracks.
    pub tone: Option<u32>,
    pub parts: Vec<u32>,
}

fn read_header(rom: &Rom, h: u32) -> Result<Header> {
    let tracks = rom.u8(h)?;
    let mut parts = Vec::new();
    for i in 0..tracks as u32 {
        parts.push(rom.req_ptr(h + 8 + 4 * i, "song track")?);
    }
    Ok(Header {
        offset: h,
        block_count: rom.u8(h + 1)?,
        priority: rom.u8(h + 2)?,
        reverb: rom.u8(h + 3)?,
        tone: if tracks == 0 {
            None
        } else {
            Some(rom.req_ptr(h + 4, "voicegroup")?)
        },
        parts,
    })
}

/// A decoded `struct WaveData`.
#[derive(Clone, Debug, PartialEq)]
pub struct Wave {
    pub kind: u16,
    pub status: u16,
    pub freq: u32,
    pub loop_start: u32,
    pub data: Vec<i8>,
}

fn wave_header_ok(rom: &Rom, at: u32) -> bool {
    let (Ok(kind), Ok(status), Ok(freq), Ok(size)) = (
        rom.u16(at),
        rom.u16(at + 2),
        rom.u32(at + 4),
        rom.u32(at + 12),
    ) else {
        return false;
    };
    kind <= 1 && (status == 0 || status == 0x4000) && freq > 0 && size > 0 && size < 0x40_0000
}

/// Decodes `n` samples of DPCM data (`wav2agb -c`, m4a's `SoundMainRAM_Unk1`): 33-byte blocks
/// of 64 samples, a first sample then a 4-bit delta index per sample (the first byte's low
/// nibble, then high nibble first). Bytes past `data` read as 0.
pub fn dpcm_decode(data: &[u8], n: usize) -> Vec<i8> {
    let at = |i: usize| data.get(i).copied().unwrap_or(0);
    let mut out = Vec::with_capacity(n);
    let mut s = 0i8;
    for i in 0..n {
        let (block, j) = (i / DPCM_BLOCK, i % DPCM_BLOCK);
        let b = block * DPCM_BLOCK_BYTES;
        if j == 0 {
            s = at(b) as i8;
        } else {
            let byte = at(b + 1 + j / 2);
            let idx = if j % 2 == 1 { byte & 0xF } else { byte >> 4 };
            s = s.wrapping_add(DPCM_DELTAS[idx as usize]);
        }
        out.push(s);
    }
    out
}

/// The DPCM bytes of `n` samples (blocks rounded up).
pub fn dpcm_len(n: usize) -> usize {
    n.div_ceil(DPCM_BLOCK) * DPCM_BLOCK_BYTES
}

pub fn read_wave(rom: &Rom, at: u32) -> Result<Wave> {
    if !wave_header_ok(rom, at) {
        bail!("no WaveData at {at:#x}");
    }
    let (kind, size) = (rom.u16(at)?, rom.u32(at + 12)? as usize);
    let raw = rom.bytes();
    let from = (at + WAVE_HEADER) as usize;
    let data = if kind == 1 {
        let end = (from + dpcm_len(size)).min(raw.len());
        dpcm_decode(&raw[from..end], size)
    } else {
        rom.slice(at + WAVE_HEADER, size)?
            .iter()
            .map(|&b| b as i8)
            .collect()
    };
    Ok(Wave {
        kind,
        status: rom.u16(at + 2)?,
        freq: rom.u32(at + 4)?,
        loop_start: rom.u32(at + 8)?,
        data,
    })
}

/// A raw `ToneData` and how its pointers read.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Tone {
    Sample { ty: u8, wav: u32 },
    Wave { ty: u8, wav: u32 },
    Value { ty: u8, v: u32 },
    Split { group: u32, table: u32 },
    Rhythm { group: u32 },
    Raw,
}

fn read_tone(rom: &Rom, at: u32, groups_ok: &dyn Fn(u32) -> bool) -> Tone {
    let (Ok(ty), Ok(w), Ok(tail)) = (rom.u8(at), rom.u32(at + 4), rom.u32(at + 8)) else {
        return Tone::Raw;
    };
    let ptr = is_ptr(rom, w).then(|| w - ROM_BASE);
    match ty {
        0x40 => match ptr {
            Some(g) if groups_ok(g) && is_ptr(rom, tail) => Tone::Split {
                group: g,
                table: tail - ROM_BASE,
            },
            _ => Tone::Raw,
        },
        0x80 => match ptr {
            Some(g) if groups_ok(g) => Tone::Rhythm { group: g },
            _ => Tone::Raw,
        },
        // Direct sound: FIX (0x08), REV (0x10) and CMP (0x20) flags.
        t if t & 0xC7 == 0 => match ptr {
            Some(p) if wave_header_ok(rom, p) => Tone::Sample { ty, wav: p },
            _ => Tone::Raw,
        },
        1 | 2 | 9 | 10 if w <= 3 => Tone::Value { ty, v: w },
        4 | 12 if w <= 1 => Tone::Value { ty, v: w },
        3 | 11 => match ptr {
            Some(p) if rom.slice(p, WAVE_BYTES).is_ok() => Tone::Wave { ty, wav: p },
            _ => Tone::Raw,
        },
        _ => Tone::Raw,
    }
}

/// Everything read from the ROM, by file offset.
pub struct SoundData {
    pub table: u32,
    /// `(header offset or None, ms, me)` per song.
    pub songs: Vec<(Option<u32>, u8, u8)>,
    pub headers: BTreeMap<u32, Header>,
    /// Decoded tracks by header.
    pub tracks: BTreeMap<u32, Vec<Vec<SongCmd>>>,
    /// The voice memory's first entry and its entry count.
    pub voices_at: u32,
    pub voice_count: u32,
    tones: Vec<Tone>,
    /// Voicegroup offsets (songs' and sub-groups').
    pub groups: BTreeSet<u32>,
    pub samples: BTreeMap<u32, Wave>,
    pub waves: BTreeMap<u32, Vec<u8>>,
    pub keysplits: BTreeMap<u32, Vec<u8>>,
    pub cry_table: u32,
    pub cry_table_reverse: u32,
    pub cry_count: u32,
    pub fanfares: (u32, Vec<(u16, u16)>),
    pub cry_ids: (u32, Vec<u16>),
}

/// Runs of `n` or more 12-byte `cry` entries of type `ty` (0x20 or 0x30), within `lo..hi`.
fn cry_runs(rom: &Rom, ty: u8, lo: u32, hi: u32) -> Vec<(u32, u32)> {
    let ok = |at: u32| {
        rom.slice(at, 4).is_ok_and(|b| b == [ty, 60, 0, 0])
            && rom.u32(at + 4).is_ok_and(|p| is_ptr(rom, p))
            && rom.slice(at + 8, 4).is_ok_and(|b| b == [0xFF, 0, 0xFF, 0])
    };
    let mut runs = Vec::new();
    let mut at = lo;
    while at + TONE_SIZE <= hi {
        let mut n = 0;
        while at + TONE_SIZE * (n + 1) <= hi && ok(at + TONE_SIZE * n) {
            n += 1;
        }
        if n >= 100 {
            runs.push((at, n));
            at += TONE_SIZE * n;
        } else {
            at += TONE_SIZE;
        }
    }
    runs
}

fn song_id(name: &str) -> Option<u16> {
    SOUNDS.iter().find(|(_, n)| *n == name).map(|&(v, _)| v)
}

/// `sFanfares`: located by its first three entries.
fn locate_fanfares(rom: &Rom) -> Result<(u32, Vec<(u16, u16)>)> {
    let first = [
        ("MUS_LEVEL_UP", 80u16),
        ("MUS_OBTAIN_ITEM", 160),
        ("MUS_EVOLVED", 220),
    ];
    let mut needle = Vec::new();
    for (name, frames) in first {
        let id = song_id(name).with_context(|| format!("{name} in SOUNDS"))?;
        needle.extend(id.to_le_bytes());
        needle.extend(frames.to_le_bytes());
    }
    let hits: Vec<u32> = rom
        .find_all(&needle)
        .into_iter()
        .filter(|o| o % 2 == 0)
        .collect();
    let [at] = hits[..] else {
        bail!("sFanfares: expected 1 match, found {hits:#x?}");
    };
    let mut v = Vec::new();
    for i in 0..FANFARE_COUNT as u32 {
        v.push((rom.u16(at + 4 * i)?, rom.u16(at + 4 * i + 2)?));
    }
    Ok((at, v))
}

/// `gSpeciesIdToCryId`: located by its first entries (Treecko's line, then Torchic's).
fn locate_cry_ids(rom: &Rom) -> Result<(u32, Vec<u16>)> {
    let first: [u16; 9] = [273, 274, 275, 270, 271, 272, 276, 277, 278];
    let needle: Vec<u8> = first.iter().flat_map(|v| v.to_le_bytes()).collect();
    let hits: Vec<u32> = rom
        .find_all(&needle)
        .into_iter()
        .filter(|o| o % 2 == 0)
        .collect();
    let [at] = hits[..] else {
        bail!("gSpeciesIdToCryId: expected 1 match, found {hits:#x?}");
    };
    Ok((at, rom.u16s(at, CRY_ID_COUNT)?))
}

/// Reads the whole sound data (see the module docs).
pub fn read(rom: &Rom) -> Result<SoundData> {
    let table = locate_song_table(rom)?;
    let mut songs = Vec::with_capacity(SONG_COUNT);
    let mut headers = BTreeMap::new();
    for i in 0..SONG_COUNT as u32 {
        let at = table + 8 * i;
        let h = rom.req_ptr(at, "song header")?;
        let (ms, me) = (rom.u16(at + 4)? as u8, rom.u16(at + 6)? as u8);
        let header = match headers.get(&h) {
            Some(x) => x,
            None => headers.entry(h).or_insert(read_header(rom, h)?),
        };
        songs.push((header.tone.map(|_| h), ms, me));
    }
    let mut tracks = BTreeMap::new();
    for (&h, header) in &headers {
        if header.tone.is_none() {
            continue;
        }
        let mut ts = Vec::new();
        for &p in &header.parts {
            ts.push(
                track::decode(&RomBytes(rom), p)
                    .with_context(|| format!("song header {h:#x}, track at {p:#x}"))?,
            );
        }
        tracks.insert(h, ts);
    }

    // The voice memory: song groups, then sub-groups until none is new.
    let song_groups: BTreeSet<u32> = headers.values().filter_map(|h| h.tone).collect();
    let lo = *song_groups.iter().next().context("no voicegroup")?;
    let len = rom.bytes().len() as u32;
    let mut groups = song_groups.clone();
    let (mut hi, mut tones);
    loop {
        let last = *groups.iter().next_back().unwrap();
        hi = (last + GROUP_VOICES * TONE_SIZE).min(len - len % TONE_SIZE);
        let in_region = |g: u32| g >= lo && g < hi && (g - lo) % TONE_SIZE == 0;
        tones = Vec::new();
        let mut at = lo;
        while at + TONE_SIZE <= hi {
            tones.push(read_tone(rom, at, &in_region));
            at += TONE_SIZE;
        }
        let mut grown = false;
        // Sub-groups reachable from the groups' 128 voices.
        let mut todo: Vec<u32> = groups.iter().copied().collect();
        let mut seen = BTreeSet::new();
        while let Some(g) = todo.pop() {
            if !seen.insert(g) {
                continue;
            }
            let first = ((g - lo) / TONE_SIZE) as usize;
            for t in tones.iter().skip(first).take(GROUP_VOICES as usize) {
                if let Tone::Split { group, .. } | Tone::Rhythm { group } = *t {
                    if groups.insert(group) {
                        grown = true;
                    }
                    todo.push(group);
                }
            }
        }
        if !grown {
            break;
        }
    }
    let voice_count = tones.len() as u32;
    let mut samples = BTreeMap::new();
    let mut waves = BTreeMap::new();
    let mut keysplits = BTreeMap::new();
    for t in &tones {
        match *t {
            Tone::Sample { wav, .. } if !samples.contains_key(&wav) => {
                samples.insert(wav, read_wave(rom, wav)?);
            }
            Tone::Wave { wav, .. } => {
                waves.insert(wav, rom.slice(wav, WAVE_BYTES)?.to_vec());
            }
            Tone::Split { table, .. } => {
                let n = (len - table).min(128) as usize;
                let mut b = rom.slice(table, n)?.to_vec();
                b.resize(128, 0);
                keysplits.insert(table, b);
            }
            _ => {}
        }
    }
    let (normal, reverse) = (cry_runs(rom, 0x20, lo, hi), cry_runs(rom, 0x30, lo, hi));
    let ([(cry_table, n)], [(cry_table_reverse, n_rev)]) = (&normal[..], &reverse[..]) else {
        bail!("cry tables: expected one run each, found {normal:#x?} and {reverse:#x?}");
    };
    if n != n_rev {
        bail!("cry tables of {n} and {n_rev} entries");
    }
    Ok(SoundData {
        table,
        songs,
        headers,
        tracks,
        voices_at: lo,
        voice_count,
        tones,
        groups,
        samples,
        waves,
        keysplits,
        cry_table: (cry_table - lo) / TONE_SIZE,
        cry_table_reverse: (cry_table_reverse - lo) / TONE_SIZE,
        cry_count: *n,
        fanfares: locate_fanfares(rom)?,
        cry_ids: locate_cry_ids(rom)?,
    })
}

/// Names transferred from the decomp's sound sources (see the module docs).
#[derive(Default, Debug)]
pub struct DecompNames {
    /// `gSongTable`'s labels, by song number.
    pub songs: Vec<String>,
    /// Song label (`mus_littleroot`) -> voicegroup label (`voicegroup_littleroot`).
    pub song_groups: BTreeMap<String, String>,
    /// Voicegroup label -> (its label offset in voices, its entries' macro and arguments).
    pub groups: BTreeMap<String, (u32, Vec<(String, Vec<String>)>)>,
    /// `gCryTable` entries' samples (`Cry_Bulbasaur`).
    pub cries: Vec<String>,
}

fn macro_line(line: &str) -> Option<(String, Vec<String>)> {
    let line = line.split('@').next()?.trim();
    let (m, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    if m.is_empty() || m.starts_with('.') || m.ends_with(':') {
        return None;
    }
    let args = rest
        .split(',')
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty());
    Some((m.to_string(), args.collect()))
}

impl DecompNames {
    /// Reads `sound/` of a pokeemerald checkout.
    pub fn read(decomp: &Path) -> Result<DecompNames> {
        let sound = decomp.join("sound");
        let read = |p: &Path| {
            std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))
        };
        let mut names = DecompNames::default();
        for line in read(&sound.join("song_table.inc"))?.lines() {
            if let Some(rest) = line.trim().strip_prefix("song ") {
                names
                    .songs
                    .push(rest.split(',').next().unwrap_or("").trim().to_string());
            }
        }
        for line in read(&sound.join("songs/midi/midi.cfg"))?.lines() {
            let Some((file, opts)) = line.split_once(':') else {
                continue;
            };
            let Some(label) = file.trim().strip_suffix(".mid") else {
                continue;
            };
            let g = (opts.split_whitespace())
                .find_map(|o| o.strip_prefix("-G"))
                .unwrap_or("_dummy");
            names
                .song_groups
                .insert(label.to_string(), format!("voicegroup{g}"));
        }
        let mut dir: Vec<_> = std::fs::read_dir(sound.join("songs"))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "s"))
            .collect();
        dir.sort();
        for p in dir {
            for line in read(&p)?.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix(".equ") {
                    let mut it = rest.split(',').map(str::trim);
                    if let (Some(k), Some(v)) = (it.next(), it.next())
                        && let Some(label) = k.strip_suffix("_grp")
                    {
                        names.song_groups.insert(label.to_string(), v.to_string());
                    }
                }
            }
        }
        // Voicegroups: `voice_group <name>[, <offset>]`, in every included file.
        let mut files = Vec::new();
        for line in read(&sound.join("voice_groups.inc"))?.lines() {
            let line = line.split('@').next().unwrap_or("");
            if let Some(f) = line.trim().strip_prefix(".include") {
                files.push(decomp.join(f.trim().trim_matches('"')));
            }
        }
        for f in files {
            let mut current: Option<String> = None;
            for line in read(&f)?.lines() {
                let Some((m, args)) = macro_line(line) else {
                    continue;
                };
                if m == "voice_group" {
                    let name = format!("voicegroup_{}", args[0]);
                    let off = args.get(1).map_or(Ok(0), |a| a.parse::<u32>())?;
                    names.groups.insert(name.clone(), (off, Vec::new()));
                    current = Some(name);
                } else if let Some(c) = &current {
                    names.groups.get_mut(c).unwrap().1.push((m, args));
                }
            }
        }
        for line in read(&sound.join("cry_tables.inc"))?.lines() {
            if let Some(("cry", args)) = macro_line(line).as_ref().map(|(m, a)| (m.as_str(), a)) {
                names.cries.push(args[0].clone());
                continue;
            }
            if line.trim_start().starts_with("gCryTable_Reverse") {
                break;
            }
        }
        Ok(names)
    }
}

/// The name of each voicegroup, sample, wave and key split table.
#[derive(Default, Debug)]
pub struct Names {
    pub groups: BTreeMap<u32, String>,
    pub samples: BTreeMap<u32, String>,
    pub waves: BTreeMap<u32, String>,
    pub keysplits: BTreeMap<u32, String>,
}

fn sample_label(s: &str) -> &str {
    s.strip_prefix("DirectSoundWaveData_").unwrap_or(s)
}

/// Song labels of `gSongTable` (lowercase constants: `mus_littleroot`).
fn song_label(id: usize) -> String {
    SOUNDS
        .iter()
        .find(|(v, _)| *v as usize == id)
        .map_or(format!("song_{id:03}"), |(_, n)| n.to_lowercase())
}

impl SoundData {
    fn tone_at(&self, at: u32) -> Option<&Tone> {
        let i = at.checked_sub(self.voices_at)? / TONE_SIZE;
        (at >= self.voices_at && (at - self.voices_at) % TONE_SIZE == 0)
            .then(|| self.tones.get(i as usize))
            .flatten()
    }

    /// Decomp names where `decomp` gives them, else positional (see the module docs).
    pub fn names(&self, decomp: Option<&DecompNames>) -> Names {
        let mut n = Names::default();
        if let Some(d) = decomp {
            for (i, (h, ..)) in self.songs.iter().enumerate() {
                let label = d.songs.get(i).cloned().unwrap_or_else(|| song_label(i));
                let (Some(h), Some(g)) = (h, d.song_groups.get(&label)) else {
                    continue;
                };
                if let Some(tone) = self.headers[h].tone {
                    n.groups.entry(tone).or_insert_with(|| g.clone());
                }
            }
            // Entries of named groups name their samples, waves, tables and sub-groups;
            // repeat until no group is newly named.
            loop {
                let before = n.groups.len();
                let named: Vec<(u32, String)> =
                    n.groups.iter().map(|(a, s)| (*a, s.clone())).collect();
                for (at, name) in named {
                    let Some((off, entries)) = d.groups.get(&name) else {
                        continue;
                    };
                    for (k, (m, args)) in entries.iter().enumerate() {
                        let e = at + TONE_SIZE * (off + k as u32);
                        match (self.tone_at(e), m.as_str()) {
                            (Some(Tone::Sample { wav, .. }), m)
                                if m.starts_with("voice_directsound") =>
                            {
                                n.samples
                                    .entry(*wav)
                                    .or_insert(sample_label(&args[2]).to_string());
                            }
                            (Some(Tone::Wave { wav, .. }), m)
                                if m.starts_with("voice_programmable_wave") =>
                            {
                                n.waves.entry(*wav).or_insert(args[2].clone());
                            }
                            (Some(Tone::Split { group, table }), "voice_keysplit") => {
                                n.groups.entry(*group).or_insert(args[0].clone());
                                n.keysplits.entry(*table).or_insert(args[1].clone());
                            }
                            (Some(Tone::Rhythm { group }), "voice_keysplit_all") => {
                                n.groups.entry(*group).or_insert(args[0].clone());
                            }
                            _ => {}
                        }
                    }
                }
                if n.groups.len() == before {
                    break;
                }
            }
            for (i, name) in d.cries.iter().enumerate() {
                let e = self.voices_at + TONE_SIZE * (self.cry_table + i as u32);
                if let Some(Tone::Sample { wav, .. }) = self.tone_at(e) {
                    n.samples.entry(*wav).or_insert(name.clone());
                }
            }
        }
        // Positional names for the rest, in ROM order, never reusing a decomp name.
        fn fill(
            map: &mut BTreeMap<u32, String>,
            all: impl Iterator<Item = u32>,
            f: impl Fn(usize) -> String,
        ) {
            let taken: BTreeSet<String> = map.values().cloned().collect();
            let mut i = 0;
            for a in all {
                if map.contains_key(&a) {
                    continue;
                }
                let name = loop {
                    let s = f(i);
                    i += 1;
                    if !taken.contains(&s) {
                        break s;
                    }
                };
                map.insert(a, name);
            }
        }
        fill(&mut n.groups, self.groups.iter().copied(), |i| {
            format!("voicegroup_{i:03}")
        });
        fill(&mut n.samples, self.samples.keys().copied(), |i| {
            format!("sample_{i:03}")
        });
        fill(&mut n.waves, self.waves.keys().copied(), |i| {
            format!("wave_{i:02}")
        });
        fill(&mut n.keysplits, self.keysplits.keys().copied(), |i| {
            format!("keysplit_{i}")
        });
        n
    }

    fn voice(&self, t: &Tone, at: u32, rom: &Rom, n: &Names) -> Result<Voice> {
        let b = rom.slice(at, TONE_SIZE as usize)?;
        let adsr = VoiceTail::Adsr([b[8], b[9], b[10], b[11]]);
        let tone = |ty, wav| Voice::Tone {
            ty,
            key: b[1],
            length: b[2],
            pan_sweep: b[3],
            wav,
            tail: adsr.clone(),
        };
        Ok(match *t {
            Tone::Sample { ty, wav } => tone(ty, VoiceRef::Sample(n.samples[&wav].clone())),
            Tone::Wave { ty, wav } => tone(ty, VoiceRef::Wave(n.waves[&wav].clone())),
            Tone::Value { ty, v } => tone(ty, VoiceRef::Value(v)),
            Tone::Rhythm { group } => tone(0x80, VoiceRef::Group(n.groups[&group].clone())),
            Tone::Split { group, table } => Voice::Tone {
                ty: 0x40,
                key: b[1],
                length: b[2],
                pan_sweep: b[3],
                wav: VoiceRef::Group(n.groups[&group].clone()),
                tail: VoiceTail::Table(n.keysplits[&table].clone()),
            },
            Tone::Raw => Voice::Raw(b.to_vec()),
        })
    }
}

/// 8-bit mono PCM WAV bytes (unsigned samples) at `rate` Hz.
pub fn wav_bytes(data: &[i8], rate: u32) -> Vec<u8> {
    let n = data.len() as u32;
    let mut w = Vec::with_capacity(44 + data.len());
    w.extend(b"RIFF");
    w.extend((36 + n).to_le_bytes());
    w.extend(b"WAVEfmt ");
    w.extend(16u32.to_le_bytes());
    w.extend(1u16.to_le_bytes()); // PCM
    w.extend(1u16.to_le_bytes()); // mono
    w.extend(rate.to_le_bytes());
    w.extend(rate.to_le_bytes()); // bytes per second
    w.extend(1u16.to_le_bytes()); // block align
    w.extend(8u16.to_le_bytes());
    w.extend(b"data");
    w.extend(n.to_le_bytes());
    w.extend(data.iter().map(|&s| (s as u8) ^ 0x80));
    w
}

/// The samples of an 8-bit mono PCM WAV written by [`wav_bytes`].
pub fn wav_samples(bytes: &[u8]) -> Result<Vec<i8>> {
    if bytes.len() < 44 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        bail!("not a WAV file");
    }
    let mut at = 12;
    let mut format = None;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        let body = bytes
            .get(at + 8..at + 8 + len)
            .context("truncated WAV chunk")?;
        if id == b"fmt " {
            let ch = u16::from_le_bytes([body[2], body[3]]);
            let bits = u16::from_le_bytes([body[14], body[15]]);
            format = Some((ch, bits));
        } else if id == b"data" {
            if format != Some((1, 8)) {
                bail!("WAV must be 8-bit mono, got {format:?}");
            }
            return Ok(body.iter().map(|&b| (b ^ 0x80) as i8).collect());
        }
        at += 8 + len + (len & 1);
    }
    bail!("WAV without data")
}

/// Writes `sound/` (see the schema's minor 14) and returns the provenance entries.
pub fn export(
    rom: &Rom,
    out: &mut Out,
    decomp: Option<&DecompNames>,
    provenance: &mut BTreeMap<String, Hex>,
) -> Result<()> {
    let d = read(rom)?;
    let n = d.names(decomp);
    provenance.insert("sound/song_table".into(), Hex(d.table));
    provenance.insert("sound/voices".into(), Hex(d.voices_at));
    provenance.insert("sound/fanfares".into(), Hex(d.fanfares.0));
    provenance.insert("sound/cry_ids".into(), Hex(d.cry_ids.0));

    // Songs: one file per header, named by its first song.
    let mut files: BTreeMap<u32, String> = BTreeMap::new();
    let mut table = Vec::new();
    for (i, &(h, ms, me)) in d.songs.iter().enumerate() {
        let song = h.map(|h| files.entry(h).or_insert_with(|| song_label(i)).clone());
        table.push(SongTableEntry {
            name: SOUNDS
                .iter()
                .find(|(v, _)| *v as usize == i)
                .map_or(format!("SONG_{i:03}"), |(_, s)| s.to_string()),
            player: ms,
            me,
            song,
        });
    }
    out.ron(SONGS_FILE, &SongTableFile { songs: table })?;
    for (h, name) in &files {
        let header = &d.headers[h];
        let tone = header.tone.unwrap();
        out.ron(
            &format!("sound/songs/{name}.ron"),
            &SongFile {
                block_count: header.block_count,
                priority: header.priority,
                reverb: header.reverb,
                voicegroup: n.groups[&tone].clone(),
                tracks: d.tracks[h].clone(),
            },
        )?;
        provenance.insert(format!("sound/song/{name}"), Hex(*h));
    }

    let mut voices = Vec::with_capacity(d.tones.len());
    for (i, t) in d.tones.iter().enumerate() {
        voices.push(d.voice(t, d.voices_at + TONE_SIZE * i as u32, rom, &n)?);
    }
    let groups = (d.groups.iter())
        .map(|g| (n.groups[g].clone(), (g - d.voices_at) / TONE_SIZE))
        .collect();
    let keysplits = (d.keysplits.iter())
        .map(|(a, b)| (n.keysplits[a].clone(), b.clone()))
        .collect();
    out.ron(
        VOICES_FILE,
        &VoicesFile {
            voices,
            groups,
            keysplits,
            cry_table: d.cry_table,
            cry_table_reverse: d.cry_table_reverse,
            cry_count: d.cry_count,
        },
    )?;

    let mut samples = Vec::new();
    for (at, w) in &d.samples {
        let name = &n.samples[at];
        out.bytes(
            &format!("sound/samples/{name}.wav"),
            &wav_bytes(&w.data, (w.freq + 512) / 1024),
        )?;
        samples.push(SampleInfo {
            name: name.clone(),
            kind: w.kind,
            status: w.status,
            freq: w.freq,
            loop_start: w.loop_start,
            size: w.data.len() as u32,
        });
    }
    let waves = (d.waves.iter())
        .map(|(a, b)| (n.waves[a].clone(), b.clone()))
        .collect();
    out.ron(SAMPLES_FILE, &SamplesFile { samples, waves })?;

    let fanfares = (d.fanfares.1.iter())
        .map(|&(s, f)| (song_label(s as usize).to_uppercase(), f))
        .collect();
    out.ron(
        TABLES_FILE,
        &SoundTablesFile {
            fanfares,
            cry_ids: d.cry_ids.1.clone(),
        },
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpcm_decodes_blocks() {
        // Block 0: first sample 10, deltas +1 (idx 1, low nibble of byte 1), +4 (idx 2, high
        // nibble of byte 2), -1 (idx 15, low nibble of byte 2).
        let mut b = vec![0u8; DPCM_BLOCK_BYTES * 2];
        b[0] = 10;
        b[1] = 0x01;
        b[2] = 0x2F;
        b[DPCM_BLOCK_BYTES] = (-5i8) as u8;
        let s = dpcm_decode(&b, 66);
        assert_eq!(&s[..4], &[10, 11, 15, 14]);
        assert_eq!(s[63], 14);
        assert_eq!(&s[64..], &[-5, -5]);
        assert_eq!(dpcm_len(65), 66);
    }

    #[test]
    fn wav_round_trips() {
        let data = [0i8, 127, -128, -1, 5];
        let w = wav_bytes(&data, 13379);
        assert_eq!(wav_samples(&w).unwrap(), data);
    }
}
