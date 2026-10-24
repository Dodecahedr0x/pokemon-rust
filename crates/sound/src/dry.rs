//! Dry runs: the engine alone, no mixing, to time what Gen3's scripts wait on. The content
//! build runs them over every song and cry and gives the World the frame counts.
//!
//! Each count is the number of `SoundMain` calls (one per frame, in VBlank, after the frame's
//! script and tasks) after which the condition no longer holds, so a script that started the
//! sound on frame `N` sees it end on frame `N + count`.

use crate::{Blob, Cmd, Cry, Engine, NullPsg, SAMPLES_PER_FRAME};

/// Longer than any Gen3 wait; a song that never ends (a looping SE) reports this.
pub const LIMIT: u32 = 60 * 60;

/// Frames until `IsSEPlaying` turns false after `m4aSongNumStart(song)` on an idle engine.
pub fn se_frames(blob: Blob<'_>, song: u16) -> u32 {
    let mut psg = NullPsg;
    let mut e = Engine::new(blob, &mut psg);
    e.apply(Cmd::Start(song), &mut psg);
    run(&mut e, |e| e.is_se_playing())
}

/// Frames until the cry ends after `PlayCryInternal` with `cry` on an idle engine: its track's
/// `FINE` (`IsPokemonCryPlaying`), or one frame after its sample ran out, whichever is first.
///
/// The second case stands for the BGM's next note taking the freed channel (`ply_note` takes a
/// free channel first and unlinks it from the cry's track), which Gen3's field and battles do
/// within a frame or two while music plays; a dry run has no music to do it.
pub fn cry_frames(blob: Blob<'_>, cry: Cry) -> u32 {
    let mut psg = NullPsg;
    let mut e = Engine::new(blob, &mut psg);
    e.apply(Cmd::Cry(cry), &mut psg);
    let mut sounding_until = None;
    let mut n = 0;
    while n < LIMIT {
        e.frame(&mut psg);
        e.advance(SAMPLES_PER_FRAME as u32);
        n += 1;
        if !e.is_cry_playing() {
            return n;
        }
        if sounding_until.is_none() && !e.is_cry_sounding() {
            sounding_until = Some(n + 1);
        }
        if sounding_until == Some(n) {
            return n;
        }
    }
    n
}

/// Frames until player `p`'s song ends (no track left) after `m4aSongNumStart(song)`.
pub fn song_frames(blob: Blob<'_>, song: u16, p: crate::Player) -> u32 {
    let mut psg = NullPsg;
    let mut e = Engine::new(blob, &mut psg);
    e.apply(Cmd::Start(song), &mut psg);
    run(&mut e, |e| !e.is_stopped(p))
}

fn run(e: &mut Engine<'_>, mut on: impl FnMut(&Engine<'_>) -> bool) -> u32 {
    let mut psg = NullPsg;
    let mut n = 0;
    while n < LIMIT {
        e.frame(&mut psg);
        e.advance(SAMPLES_PER_FRAME as u32);
        n += 1;
        if !on(e) {
            break;
        }
    }
    n
}
