//! `crate::audio`: the waits' frame counts and the SE priority rule.
use super::*;
use alloc::vec::Vec;

static SONGS: [SongInfo; 4] = [
    SongInfo {
        player: 0,
        priority: 0,
        frames: 0,
    },
    SongInfo {
        player: 0,
        priority: 0,
        frames: FOREVER,
    },
    SongInfo {
        player: 1,
        priority: 4,
        frames: 10,
    },
    SongInfo {
        player: 2,
        priority: 5,
        frames: 30,
    },
];
static DATA: AudioData = AudioData {
    songs: &SONGS,
    fanfares: &[(3, 20)],
    ..EMPTY_AUDIO
};

/// Frames from a call on frame 0 until `done` holds when checked before frame `n`'s tick
/// (a script resumes on frame `n`).
fn resume_frame(a: &mut Audio, done: impl Fn(&Audio) -> bool) -> u32 {
    let mut out = Vec::new();
    a.tick(&DATA, &mut out);
    let mut n = 1;
    while !done(a) {
        a.tick(&DATA, &mut out);
        n += 1;
    }
    n
}

#[test]
fn waitse_resumes_after_the_song() {
    let mut a = Audio::default();
    a.play_se(&DATA, &mut Vec::new(), 2);
    assert_eq!(resume_frame(&mut a, |a| !a.is_se_playing()), 10);
}

#[test]
fn waitfanfare_resumes_a_frame_after_the_counter() {
    let mut a = Audio::default();
    let mut out = Vec::new();
    a.play_bgm(&DATA, &mut out, 1);
    a.play_fanfare(&DATA, &mut out, 3);
    assert!(a.players[0].paused);
    assert_eq!(resume_frame(&mut a, Audio::fanfare_done), 21);
    assert!(!a.players[0].paused);
}

#[test]
fn fadeoutbgm_resumes_after_sixteen_steps() {
    let mut a = Audio::default();
    let mut out = Vec::new();
    a.play_bgm(&DATA, &mut out, 1);
    a.fade_out_bgm(&mut out, 4, true);
    assert_eq!(resume_frame(&mut a, Audio::bgm_paused_or_stopped), 64);
}

#[test]
fn a_higher_priority_se_keeps_its_player() {
    static LOW: [SongInfo; 2] = [
        SongInfo {
            player: 1,
            priority: 4,
            frames: 10,
        },
        SongInfo {
            player: 1,
            priority: 1,
            frames: 10,
        },
    ];
    let data = AudioData {
        songs: &LOW,
        ..EMPTY_AUDIO
    };
    let mut a = Audio::default();
    let mut out = Vec::new();
    a.play_se(&data, &mut out, 0);
    a.play_se(&data, &mut out, 1);
    assert_eq!(a.players[1].song, 0);
}
