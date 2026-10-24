//! Milestone 12: the sound blob and tables the build writes from the extract, played by the
//! engine and the software synth. Skipped (passes) on a build without sound (the stub).

use content::sound::{AUDIO, MAP_MUSIC, TRAINER_MUSIC, song_named};
use engine::audio::FOREVER;
use sound::synth::Synth;
use sound::{Blob, Cmd};

fn blob() -> Option<Blob<'static>> {
    (!AUDIO.blob.is_empty()).then(|| Blob::new(AUDIO.blob).expect("the sound blob's header"))
}

/// Every song plays two seconds through the synth without a panic, and a map's music is not
/// silence.
#[test]
fn every_song_plays() {
    let Some(blob) = blob() else { return };
    let mut loud = 0;
    for n in 0..blob.song_count() {
        let mut s = Synth::new(blob, 32768);
        s.apply(Cmd::Start(n));
        let mut out = vec![(0i16, 0i16); 32768 * 2];
        s.render(&mut out);
        if out
            .iter()
            .any(|&(l, r)| l.unsigned_abs() > 256 || r.unsigned_abs() > 256)
        {
            loud += 1;
        }
    }
    // A few songs are silent stubs (`MUS_DUMMY` and unused numbers).
    assert!(loud > 500, "{loud} songs make a sound");
    for &(_, m) in MAP_MUSIC
        .iter()
        .filter(|m| m.1 < content::sound::MUS_ROUTE118)
    {
        let mut s = Synth::new(blob, 32768);
        s.apply(Cmd::Start(m));
        let mut out = vec![(0i16, 0i16); 32768 * 3];
        s.render(&mut out);
        assert!(
            out.iter().any(|&(l, _)| l.unsigned_abs() > 256),
            "map music {m} is silent"
        );
    }
}

/// The dry runs' lengths: map music loops, sound effects end, the fanfares last about their
/// `sFanfares` durations (Gen3's own estimates from the MIDI files), cries end within seconds.
#[test]
fn dry_run_lengths_are_gen3_like() {
    if blob().is_none() {
        return;
    }
    let song = |name| AUDIO.song(song_named(name).unwrap());
    for name in [
        "MUS_LITTLEROOT",
        "MUS_ROUTE101",
        "MUS_VS_WILD",
        "MUS_POKE_CENTER",
    ] {
        assert_eq!(song(name).frames, FOREVER, "{name}");
        assert_eq!(song(name).player, 0, "{name}");
    }
    for name in ["SE_SELECT", "SE_DOOR", "SE_PC_ON", "SE_EXIT"] {
        let s = song(name);
        assert!(
            s.player > 0 && (1..120).contains(&s.frames),
            "{name}: {s:?}"
        );
    }
    // `MUS_LEVEL_UP` 80, `MUS_OBTAIN_ITEM` 160, `MUS_OBTAIN_BADGE` 340 frames.
    for &(n, frames) in &AUDIO.fanfares[..6] {
        let got = AUDIO.song(n).frames as i32;
        assert!(
            (got - frames as i32).abs() * 3 < frames as i32 * 2,
            "fanfare {n}: {got} frames, sFanfares {frames}"
        );
    }
    assert_eq!(AUDIO.cry_frames.len(), 388);
    for c in AUDIO.cry_frames {
        assert!(c.iter().all(|&f| (2..400).contains(&f)), "{c:?}");
    }
    // Torchic's cry (`SpeciesToCryId`): its cry id past Celebi's.
    assert_eq!(AUDIO.species_cry[1], 0);
    assert_eq!(AUDIO.species_cry[280], 270);
}

/// Silice: each author track (`games/silice/music/<NAME>.mid`) is a song of its own past
/// Emerald's table (not its stand-in) and makes a sound.
#[test]
fn silice_author_tracks_play() {
    let Some(blob) = blob() else { return };
    if !matches!(content::FLAVOR, content::Flavor::Silice) {
        return;
    }
    let music = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../games/silice/music");
    let Ok(dir) = std::fs::read_dir(&music) else {
        return;
    };
    let emerald_songs = (extract::text::charmap::SOUNDS.iter())
        .map(|s| s.0 + 1)
        .max()
        .unwrap();
    for path in dir.filter_map(|e| e.ok().map(|e| e.path())) {
        if path.extension().is_none_or(|x| x != "mid") {
            continue;
        }
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let n = song_named(&name).unwrap_or_else(|| panic!("{name} has no song"));
        assert!(n >= emerald_songs, "{name} plays song {n}, an Emerald one");
        let mut s = Synth::new(blob, 32768);
        s.apply(Cmd::Start(n));
        let mut out = vec![(0i16, 0i16); 32768 * 2];
        s.render(&mut out);
        assert!(
            out.iter().any(|&(l, _)| l.unsigned_abs() > 256),
            "{name} is silent"
        );
    }
}

/// Trainers get Gen3's class music; the low-HP SE loops.
#[test]
fn trainer_and_battle_music() {
    if blob().is_none() {
        return;
    }
    let n = |s| song_named(s).unwrap();
    assert!(TRAINER_MUSIC.contains(&(n("MUS_VS_GYM_LEADER"), n("MUS_VICTORY_GYM_LEADER"))));
    assert!(TRAINER_MUSIC.contains(&(n("MUS_VS_TRAINER"), n("MUS_VICTORY_TRAINER"))));
    assert_eq!(AUDIO.song(AUDIO.ids.low_health).frames, FOREVER);
}
