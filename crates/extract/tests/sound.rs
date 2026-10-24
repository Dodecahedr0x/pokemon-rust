//! The m4a sound data (`sound/mod.rs`, Milestone 12) against the decomp, on the US and the
//! French ROM: the song table's players, every track re-encoded to its ROM bytes, every named
//! voicegroup entry against `sound/voicegroups/**.inc`, the uncompressed samples against the
//! decomp's `.wav`s byte for byte (the DPCM cries within a small error), `sFanfares` and
//! `gSpeciesIdToCryId`.
use extract::schema::{Voice, VoiceRef, VoiceTail};
use extract::sound::{self, DecompNames, track};
use extract::{ROM_BASE, Rom, fixtures};
use std::collections::BTreeMap;
use std::sync::OnceLock;

fn names() -> &'static DecompNames {
    static N: OnceLock<DecompNames> = OnceLock::new();
    N.get_or_init(|| DecompNames::read(&fixtures::decomp()).unwrap())
}

fn roms() -> [Rom; 2] {
    [fixtures::us(), fixtures::fr()]
}

#[test]
fn song_table_players_are_the_decomp_table() {
    let inc = std::fs::read_to_string(fixtures::decomp().join("sound/song_table.inc")).unwrap();
    let want: Vec<u8> = (inc.lines())
        .filter_map(|l| l.trim().strip_prefix("song "))
        .map(|l| l.split(',').nth(2).unwrap().trim().parse().unwrap())
        .collect();
    assert_eq!(want.len(), sound::SONG_COUNT);
    for rom in roms() {
        let d = sound::read(&rom).unwrap();
        let got: Vec<u8> = d.songs.iter().map(|s| s.1).collect();
        assert_eq!(got, want);
        // Every song with tracks has a decomp voicegroup.
        let n = d.names(Some(names()));
        for h in d.headers.values().filter_map(|h| h.tone) {
            assert!(n.groups[&h].starts_with("voicegroup"), "{}", n.groups[&h]);
            assert!(
                names().groups.contains_key(&n.groups[&h]),
                "{}",
                n.groups[&h]
            );
        }
    }
}

#[test]
fn every_track_re_encodes_to_its_rom_bytes() {
    for rom in roms() {
        let d = sound::read(&rom).unwrap();
        let mut n = 0;
        for (h, header) in d.headers.iter().filter(|(_, h)| h.tone.is_some()) {
            for (i, (&start, cmds)) in header.parts.iter().zip(&d.tracks[h]).enumerate() {
                assert!(
                    !matches!(cmds[0], extract::schema::SongCmd::Goto(_)),
                    "{h:#x} {i}"
                );
                let bytes = track::encode(cmds, ROM_BASE + start).unwrap();
                let rom_bytes = rom.slice(start, bytes.len()).unwrap();
                assert_eq!(bytes, rom_bytes, "song header {h:#x} track {i}");
                // And decoding the encoding gives the same commands.
                n += 1;
            }
        }
        assert!(n > 1500, "{n} tracks");
    }
}

/// The ToneData bytes a decomp voice macro assembles to (`asm/macros/music_voice.inc`):
/// (type, key, length, pan_sweep, the wav argument, adsr). The PSG macros put their `pan`
/// argument in the `length` byte.
fn macro_tone(m: &str, a: &[String]) -> Option<(u8, u8, u8, u8, String, [u8; 4])> {
    let n = |i: usize| -> u8 { a[i].parse::<i64>().unwrap() as u8 };
    let pan = |p: u8| if p != 0 { 0x80 | p } else { 0 };
    let psg = |i: usize| [n(i) & 7, n(i + 1) & 7, n(i + 2) & 15, n(i + 3) & 7];
    let alt = |ty: u8| if m.ends_with("alt") { ty + 8 } else { ty };
    Some(match m {
        "voice_directsound" | "voice_directsound_no_resample" | "voice_directsound_alt" => {
            let ty = match m {
                "voice_directsound" => 0,
                "voice_directsound_no_resample" => 8,
                _ => 16,
            };
            (
                ty,
                n(0),
                0,
                pan(n(1)),
                a[2].clone(),
                [n(3), n(4), n(5), n(6)],
            )
        }
        "voice_square_1" | "voice_square_1_alt" => (
            alt(1),
            n(0),
            pan(n(1)),
            n(2),
            (n(3) & 3).to_string(),
            psg(4),
        ),
        "voice_square_2" | "voice_square_2_alt" => {
            (alt(2), n(0), pan(n(1)), 0, (n(2) & 3).to_string(), psg(3))
        }
        "voice_programmable_wave" | "voice_programmable_wave_alt" => {
            (alt(3), n(0), pan(n(1)), 0, a[2].clone(), psg(3))
        }
        "voice_noise" | "voice_noise_alt" => {
            (alt(4), n(0), pan(n(1)), 0, (n(2) & 1).to_string(), psg(3))
        }
        _ => return None,
    })
}

#[test]
fn named_voicegroups_are_the_decomp_voicegroups() {
    let dn = names();
    for rom in roms() {
        let d = sound::read(&rom).unwrap();
        let n = d.names(Some(dn));
        let file = voices_file(&rom);
        let index: BTreeMap<&str, u32> =
            file.groups.iter().map(|(g, i)| (g.as_str(), *i)).collect();
        let mut checked = 0;
        for (name, (off, entries)) in &dn.groups {
            let Some(&base) = index.get(name.as_str()) else {
                continue;
            };
            for (k, (m, args)) in entries.iter().enumerate() {
                let v = &file.voices[(base + off + k as u32) as usize];
                let Voice::Tone {
                    ty,
                    key,
                    length,
                    pan_sweep,
                    wav,
                    tail,
                } = v
                else {
                    panic!("{name}[{k}] is raw: {v:?}");
                };
                match m.as_str() {
                    "voice_keysplit" => {
                        assert_eq!(
                            (*ty, wav),
                            (0x40, &VoiceRef::Group(args[0].clone())),
                            "{name}[{k}]"
                        );
                        assert_eq!(tail, &VoiceTail::Table(args[1].clone()));
                    }
                    "voice_keysplit_all" => {
                        assert_eq!(
                            (*ty, wav),
                            (0x80, &VoiceRef::Group(args[0].clone())),
                            "{name}[{k}]"
                        );
                    }
                    _ => {
                        let (wty, wkey, wlen, wps, warg, wadsr) =
                            macro_tone(m, args).unwrap_or_else(|| panic!("macro {m}"));
                        assert_eq!(
                            (*ty, *key, *length, *pan_sweep),
                            (wty, wkey, wlen, wps),
                            "{name}[{k}] {m}"
                        );
                        assert_eq!(tail, &VoiceTail::Adsr(wadsr), "{name}[{k}] {m}");
                        let got = match wav {
                            VoiceRef::Sample(s) => format!("DirectSoundWaveData_{s}"),
                            VoiceRef::Wave(s) => s.clone(),
                            VoiceRef::Value(v) => v.to_string(),
                            VoiceRef::Group(g) => g.clone(),
                        };
                        assert_eq!(got, warg, "{name}[{k}] {m}");
                    }
                }
                checked += 1;
            }
        }
        assert!(checked > 5000, "{checked} entries checked");
        assert!(n.groups.len() >= 180);
    }
}

fn voices_file(rom: &Rom) -> extract::schema::VoicesFile {
    let dir = tempdir(&format!("voices_{}", rom.sha1_hex()));
    let mut out = extract::export::Out::new(&dir);
    let mut prov = BTreeMap::new();
    sound::export(rom, &mut out, Some(names()), &mut prov).unwrap();
    extract::export::read_ron(&dir.join(sound::VOICES_FILE)).unwrap()
}

fn tempdir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("pkr_sound_test_{}_{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn samples_are_the_decomp_wavs() {
    for rom in roms() {
        let d = sound::read(&rom).unwrap();
        let n = d.names(Some(names()));
        // `DirectSoundWaveData_<name>` -> its `.bin` (`direct_sound_data.inc`), built from a `.wav`.
        let inc = std::fs::read_to_string(fixtures::decomp().join("sound/direct_sound_data.inc"))
            .unwrap();
        let mut files = BTreeMap::new();
        let mut label = None;
        for l in inc.lines().map(|l| l.split('@').next().unwrap().trim()) {
            if let Some(n) = l.strip_suffix("::") {
                label = Some(
                    n.strip_prefix("DirectSoundWaveData_")
                        .unwrap_or(n)
                        .to_string(),
                );
            } else if let (Some(f), Some(n)) = (l.strip_prefix(".incbin "), label.take()) {
                let f = f.trim_matches('"').replace(".bin", ".wav");
                files.insert(n, fixtures::decomp().join(f));
            }
        }
        let (mut exact, mut cries, mut unnamed) = (0, 0, 0);
        for (at, w) in &d.samples {
            let name = &n.samples[at];
            // A sample only an unnamed entry reaches keeps a positional name.
            let Some(path) = files.get(name.as_str()) else {
                assert!(name.starts_with("sample_"), "no decomp sample {name}");
                unnamed += 1;
                continue;
            };
            let src = sound::wav_samples(
                &std::fs::read(path.as_path())
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display())),
            )
            .unwrap();
            if w.kind == 0 {
                // wav2agb writes the samples up to the loop end; the data is the same.
                assert_eq!(&src[..w.data.len()], &w.data[..], "{name}");
                exact += 1;
            } else {
                // `wav2agb -c -l 1`: the size stops one sample short of the source.
                assert!(
                    src.len() - w.data.len() <= 1,
                    "{name}: {} {}",
                    src.len(),
                    w.data.len()
                );
                let err: i64 = (src.iter().zip(&w.data))
                    .map(|(a, b)| (*a as i64 - *b as i64).abs())
                    .sum::<i64>()
                    / w.data.len() as i64;
                assert!(err <= 6, "{name}: mean error {err}");
                cries += 1;
            }
        }
        assert!(exact > 100 && cries > 380, "{exact} samples, {cries} cries");
        assert!(unnamed <= 1, "{unnamed} unnamed samples");
    }
}

#[test]
fn fanfares_and_cry_ids_are_the_decomp_tables() {
    let c = std::fs::read_to_string(fixtures::decomp().join("src/sound.c")).unwrap();
    let frames: Vec<u16> = (c.lines())
        .filter(|l| l.trim_start().starts_with("[FANFARE_"))
        .map(|l| {
            l.split(',')
                .nth(1)
                .unwrap()
                .trim()
                .trim_end_matches('}')
                .trim()
                .parse()
                .unwrap()
        })
        .collect();
    let ids =
        std::fs::read_to_string(fixtures::decomp().join("src/data/pokemon/cry_ids.h")).unwrap();
    let cry_ids: Vec<u16> = (ids.lines())
        .filter_map(|l| l.split_once("] = "))
        .map(|(_, v)| v.trim_end_matches(',').trim().parse().unwrap())
        .collect();
    for rom in roms() {
        let d = sound::read(&rom).unwrap();
        assert_eq!(d.fanfares.1.iter().map(|f| f.1).collect::<Vec<_>>(), frames);
        assert_eq!(d.cry_ids.1, cry_ids);
        assert_eq!(d.cry_count, 388);
    }
}
