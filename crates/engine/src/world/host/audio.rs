//! Sound requests (Milestone 12): Gen3's `scrcmd.c` sound commands over [`crate::audio`].
//! Each command's `m4a*` calls go to the frame's [`crate::video::Frame::sound`]; the waits
//! (`waitse`, `waitfanfare`, `fadeoutbgm`, `waitmoncry`) are Gen3's native-script waits,
//! answered from the next frame on once their condition holds.

use super::{Host, UNIT};
use crate::data::GameData;
use crate::script::{Request, Response};

pub(super) fn begin<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let data = h.game.audio();
    let s = &mut *h.s;
    let a = &mut s.audio;
    let out = &mut h.f.sound;
    match *r {
        Request::PlaySe(song) => a.play_se(data, out, song),
        Request::PlayFanfare(song) => a.play_fanfare(data, out, song),
        Request::PlayBgm { song, save } => {
            if save {
                a.saved = song;
            }
            a.play_new_map_music(song);
        }
        Request::SaveBgm(song) => a.saved = song,
        Request::FadeDefaultBgm => {
            // `Overworld_ChangeMusicToDefault`.
            let music = s.default_music(h.game);
            if s.audio.current != music {
                s.audio.fade_out_and_play(out, music, 8);
            }
        }
        Request::FadeNewBgm(song) => a.change_music_to(out, data, song),
        Request::FadeOutBgm(speed) => {
            a.fade_out_bgm(out, 4 * speed.max(1) as u16, true);
            return h.wait(r);
        }
        Request::FadeInBgm(speed) => a.fade_in_bgm(out, 4 * speed.max(1) as u16),
        Request::PlayCry { species, mode } => a.play_cry(data, out, species, mode),
        Request::WaitSe | Request::WaitFanfare | Request::WaitCry => return h.wait(r),
        _ => unreachable!("not a sound request: {r:?}"),
    }
    UNIT
}

/// Whether a sound wait is over.
pub(super) fn done<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> bool {
    let a = &h.s.audio;
    match r {
        Request::WaitSe => !a.is_se_playing(),
        Request::WaitFanfare => a.fanfare_done(),
        Request::FadeOutBgm(_) => a.bgm_paused_or_stopped(),
        Request::WaitCry => a.cry_done(),
        _ => true,
    }
}
