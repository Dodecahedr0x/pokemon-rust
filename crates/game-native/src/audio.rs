//! The window's sound output (Milestone 12, feature `window`): the software MP2K synth
//! ([`sound::synth::Synth`]) on the default output device through cpal.
//!
//! The game thread hands each frame's commands over ([`Output::apply`]); the audio callback
//! renders, running the sound engine's frames at the GBA's rate on the device's clock. The
//! two clocks are not locked: the engine is at most a callback buffer behind the game, which
//! is inaudible.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use sound::synth::Synth;
use sound::{Blob, Cmd};
use std::sync::{Arc, Mutex};

pub struct Output {
    synth: Arc<Mutex<Synth<'static>>>,
    _stream: cpal::Stream,
}

impl Output {
    /// Opens the default output device for `blob` (the content's sound blob). `Err` without a
    /// blob (the stub), a device or a stream.
    pub fn open(blob: &'static [u8]) -> Result<Output, String> {
        let blob = Blob::new(blob).ok_or("no sound data in this build")?;
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("no output device")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let channels = config.channels as usize;
        let synth = Arc::new(Mutex::new(Synth::new(blob, config.sample_rate.0)));
        let s = synth.clone();
        let mut buf = Vec::new();
        // Renders `frames` stereo samples and writes each to the device's channels.
        let mut fill = move |frames: usize, put: &mut dyn FnMut(usize, i16, i16)| {
            buf.resize(frames, (0i16, 0i16));
            if let Ok(mut synth) = s.lock() {
                synth.render(&mut buf);
            } else {
                buf.fill((0, 0));
            }
            for (i, &(l, r)) in buf.iter().enumerate() {
                put(i, l, r);
            }
        };
        let err = |e| eprintln!("sound: {e}");
        let stream = match format {
            cpal::SampleFormat::F32 => device.build_output_stream(
                &config,
                move |data: &mut [f32], _| {
                    fill(data.len() / channels, &mut |i, l, r| {
                        write(&mut data[i * channels..][..channels], l, r, |v| {
                            v as f32 / 32768.0
                        })
                    })
                },
                err,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_output_stream(
                &config,
                move |data: &mut [i16], _| {
                    fill(data.len() / channels, &mut |i, l, r| {
                        write(&mut data[i * channels..][..channels], l, r, |v| v)
                    })
                },
                err,
                None,
            ),
            f => return Err(format!("unsupported sample format {f:?}")),
        }
        .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Output {
            synth,
            _stream: stream,
        })
    }

    /// Sends the game's sound commands to the engine, in order.
    pub fn apply(&self, cmds: &[Cmd]) {
        if cmds.is_empty() {
            return;
        }
        if let Ok(mut synth) = self.synth.lock() {
            for &c in cmds {
                synth.apply(c);
            }
        }
    }
}

/// One stereo sample to a device frame of `out.len()` channels (mono: their mean).
fn write<T: Copy>(out: &mut [T], l: i16, r: i16, conv: impl Fn(i16) -> T) {
    match out {
        [m] => *m = conv(((l as i32 + r as i32) / 2) as i16),
        [a, b, rest @ ..] => {
            *a = conv(l);
            *b = conv(r);
            for x in rest {
                *x = conv(0);
            }
        }
        [] => {}
    }
}
