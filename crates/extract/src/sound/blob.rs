//! The runtime sound blob (`sound::blob`) built from an extract's `sound/` directory: the
//! content build's input to the engine. ROM pointers become blob offsets; DPCM samples are
//! already decoded by the extract.

use super::track;
use super::wav_samples;
use crate::schema::{SamplesFile, SongFile, SongTableFile, Voice, VoiceRef, VoiceTail, VoicesFile};
use anyhow::{Context, Result, anyhow, bail};
use sound::blob::{HEADER_LEN, MAGIC, SONG_ENTRY, TONE, VERSION, WAVE_HEADER};
use std::collections::BTreeMap;
use std::path::Path;

/// The four index files and the samples of an extract's `sound/` directory.
pub struct SoundDir {
    pub songs: SongTableFile,
    pub song_files: BTreeMap<String, SongFile>,
    pub voices: VoicesFile,
    pub samples: SamplesFile,
    pub pcm: BTreeMap<String, Vec<i8>>,
}

fn ron_file<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let s = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    ron::from_str(&s).with_context(|| format!("parsing {}", path.display()))
}

impl SoundDir {
    /// Reads `dir` (an extract's `sound/`).
    pub fn read(dir: &Path) -> Result<SoundDir> {
        let songs: SongTableFile = ron_file(&dir.join("songs.ron"))?;
        let mut song_files = BTreeMap::new();
        for name in songs.songs.iter().filter_map(|s| s.song.as_ref()) {
            if !song_files.contains_key(name) {
                let f = ron_file(&dir.join(format!("songs/{name}.ron")))?;
                song_files.insert(name.clone(), f);
            }
        }
        let voices: VoicesFile = ron_file(&dir.join("voices.ron"))?;
        let samples: SamplesFile = ron_file(&dir.join("samples.ron"))?;
        let mut pcm = BTreeMap::new();
        for s in &samples.samples {
            let path = dir.join(format!("samples/{}.wav", s.name));
            let bytes =
                std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            pcm.insert(s.name.clone(), wav_samples(&bytes)?);
        }
        Ok(SoundDir {
            songs,
            song_files,
            voices,
            samples,
            pcm,
        })
    }

    /// Writes the blob (layout in `sound::blob`).
    pub fn blob(&self) -> Result<Vec<u8>> {
        let align = |n: usize| (n + 3) & !3;
        let mut at = HEADER_LEN;
        let table_at = at;
        at += self.songs.songs.len() * SONG_ENTRY;
        let voices_at = align(at);
        at = voices_at + self.voices.voices.len() * TONE as usize;
        let dummy_at = at;
        at += TONE as usize;
        let mut keysplits = BTreeMap::new();
        for (name, table) in &self.voices.keysplits {
            if table.len() != 128 {
                bail!("key split {name}: {} keys", table.len());
            }
            keysplits.insert(name.as_str(), at as u32);
            at += 128;
        }
        let mut waves = BTreeMap::new();
        for (name, w) in &self.samples.waves {
            if w.len() != 16 {
                bail!("wave {name}: {} bytes", w.len());
            }
            waves.insert(name.as_str(), at as u32);
            at += 16;
        }
        let mut samples = BTreeMap::new();
        for s in &self.samples.samples {
            at = align(at);
            samples.insert(s.name.as_str(), at as u32);
            at += WAVE_HEADER as usize + s.size as usize + 1;
        }
        let groups: BTreeMap<&str, u32> = (self.voices.groups.iter())
            .map(|(n, i)| (n.as_str(), (voices_at as u32) + i * TONE))
            .collect();

        let mut out = vec![0u8; at];
        out[0..4].copy_from_slice(&MAGIC);
        put16(&mut out, 4, VERSION);
        put16(&mut out, 6, self.songs.songs.len() as u16);
        put32(&mut out, 8, table_at as u32);
        put32(
            &mut out,
            12,
            voices_at as u32 + self.voices.cry_table * TONE,
        );
        put32(
            &mut out,
            16,
            voices_at as u32 + self.voices.cry_table_reverse * TONE,
        );
        put32(&mut out, 20, self.voices.cry_count);
        put32(&mut out, 24, dummy_at as u32);

        for (i, v) in self.voices.voices.iter().enumerate() {
            let o = voices_at + i * TONE as usize;
            match v {
                Voice::Raw(b) => {
                    let n = b.len().min(TONE as usize);
                    out[o..o + n].copy_from_slice(&b[..n]);
                }
                Voice::Tone {
                    ty,
                    key,
                    length,
                    pan_sweep,
                    wav,
                    tail,
                } => {
                    out[o..o + 4].copy_from_slice(&[*ty, *key, *length, *pan_sweep]);
                    let get = |m: &BTreeMap<&str, u32>, n: &str, what: &str| {
                        m.get(n)
                            .copied()
                            .ok_or_else(|| anyhow!("voice {i}: no {what} {n}"))
                    };
                    let w = match wav {
                        VoiceRef::Sample(n) => get(&samples, n, "sample")?,
                        VoiceRef::Wave(n) => get(&waves, n, "wave")?,
                        VoiceRef::Group(n) => get(&groups, n, "voicegroup")?,
                        VoiceRef::Value(v) => *v,
                    };
                    put32(&mut out, o + 4, w);
                    match tail {
                        VoiceTail::Adsr(a) => out[o + 8..o + 12].copy_from_slice(a),
                        VoiceTail::Table(n) => {
                            put32(&mut out, o + 8, get(&keysplits, n, "key split")?)
                        }
                    }
                }
            }
        }
        for (name, table) in &self.voices.keysplits {
            let o = keysplits[name.as_str()] as usize;
            out[o..o + 128].copy_from_slice(table);
        }
        for (name, w) in &self.samples.waves {
            let o = waves[name.as_str()] as usize;
            out[o..o + 16].copy_from_slice(w);
        }
        for s in &self.samples.samples {
            let o = samples[s.name.as_str()] as usize;
            let pcm = &self.pcm[&s.name];
            put16(&mut out, o + 2, s.status);
            put32(&mut out, o + 4, s.freq);
            put32(&mut out, o + 8, s.loop_start);
            put32(&mut out, o + 12, s.size);
            let d = o + WAVE_HEADER as usize;
            for k in 0..s.size as usize {
                out[d + k] = pcm.get(k).copied().unwrap_or(0) as u8;
            }
            // The sample past the end, which the interpolation reads: a looping sample's loop
            // start, else silence.
            let last = if s.status & 0x4000 != 0 {
                pcm.get(s.loop_start as usize).copied().unwrap_or(0)
            } else {
                0
            };
            out[d + s.size as usize] = last as u8;
        }

        // Songs: one header and its tracks per song file, then the table.
        let mut headers = BTreeMap::new();
        for (name, f) in &self.song_files {
            let vg = *groups
                .get(f.voicegroup.as_str())
                .ok_or_else(|| anyhow!("song {name}: no voicegroup {}", f.voicegroup))?;
            let mut at = align(out.len());
            out.resize(at, 0);
            let header_at = at;
            let mut header = vec![f.tracks.len() as u8, f.block_count, f.priority, f.reverb];
            header.extend(vg.to_le_bytes());
            at += 8 + 4 * f.tracks.len();
            let mut bytes = Vec::new();
            for (t, cmds) in f.tracks.iter().enumerate() {
                let b = track::encode(cmds, at as u32)
                    .with_context(|| format!("song {name} track {t}"))?;
                header.extend((at as u32).to_le_bytes());
                at += b.len();
                bytes.extend(b);
            }
            out.extend(header);
            out.extend(bytes);
            headers.insert(name.as_str(), header_at as u32);
        }
        for (i, s) in self.songs.songs.iter().enumerate() {
            let o = table_at + i * SONG_ENTRY;
            let h = s.song.as_deref().map_or(0, |n| headers[n]);
            put32(&mut out, o, h);
            out[o + 4] = s.player;
            out[o + 5] = s.me;
        }
        let len = align(out.len());
        out.resize(len, 0);
        Ok(out)
    }
}

fn put16(out: &mut [u8], at: usize, v: u16) {
    out[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

fn put32(out: &mut [u8], at: usize, v: u32) {
    out[at..at + 4].copy_from_slice(&v.to_le_bytes());
}
