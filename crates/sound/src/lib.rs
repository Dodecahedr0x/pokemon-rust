//! The MP2K ("m4a", Sappy) sound engine of Gen3, ported from the decomp (`src/m4a.c`,
//! `src/m4a_1.s`, `src/m4a_tables.c`), game-agnostic and `no_std` (Milestone 12).
//!
//! - [`blob`]: the runtime sound data, a "mini-ROM" the content build writes from the extract
//!   (`sound/`): the song table, song headers, MP2K track bytes, voicegroups, keysplit tables,
//!   samples (decoded PCM) and programmable waves, with blob offsets in place of ROM pointers.
//! - [`Engine`]: the music players (BGM, SE1, SE2, SE3 and the two cry players), their tracks
//!   and the sequencer (`MPlayMain`, `ply_*`), the DirectSound channels' envelopes and sample
//!   positions (`SoundMain`), and the PSG channels' envelopes (`CgbSound`), which write the GB
//!   sound registers through [`Psg`].
//! - [`Cmd`]: what the game asks of the engine each frame (`m4aSongNumStart`,
//!   `m4aMPlayFadeOut`, `SetPokemonCry*`, ...), the World's audio output.
//! - [`synth`]: the software back end: the DirectSound mixer at Gen3's 13,379 Hz (linear
//!   interpolation, reverb) and an emulation of the four PSG channels, to 16-bit stereo.
//!
//! Determinism: the engine is pure integer code fed by commands, so the dry runs the content
//! build makes (`dry`) give the frame counts the World waits on (`waitse`, `waitmoncry`).
#![no_std]

pub mod blob;
mod cgb;
mod cmd;
pub mod dry;
mod engine;
pub mod psg;
pub mod synth;
mod tables;

pub use blob::Blob;
pub use cmd::{Cmd, Cry, Player};
pub use engine::{CHANS, ChanView, Engine, NONE, PLAYERS};
pub use psg::{NullPsg, Psg};

/// Gen3's DirectSound rate (`SOUND_MODE_FREQ_13379`).
pub const SAMPLE_RATE: u32 = 13379;
/// Samples mixed per frame at [`SAMPLE_RATE`] (`gPcmSamplesPerVBlankTable[3]`).
pub const SAMPLES_PER_FRAME: usize = 224;
/// The GBA's frame rate, in millihertz (16,777,216 / 280,896 Hz).
pub const FRAME_RATE_MHZ: u32 = 59_727;
