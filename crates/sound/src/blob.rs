//! The runtime sound data: a "mini-ROM" in the layout of the MP2K structures, with offsets
//! from the blob's start where Gen3 has ROM pointers. Little-endian; every structure starts
//! 4-aligned and the blob must be loaded 4-aligned (the GBA mixer reads samples as words).
//!
//! ```text
//! header (HEADER_LEN bytes)
//!   0  b"MP2K"
//!   4  u16 VERSION, u16 song count
//!   8  u32 song table: per song (SONG_ENTRY bytes) u32 header, u8 player, u8 me, u16 0
//!  12  u32 gCryTable (CRY_TONE bytes per entry), 16 u32 gCryTable_Reverse, 20 u32 cry count
//!  24  u32 a ToneData of zeros (the cry song template's `voicegroup_dummy`)
//! SongHeader      u8 trackCount, blockCount, priority, reverb; u32 voicegroup; u32 part[n]
//! track bytes     MP2K commands; GOTO, PATT and MEMACC jumps hold blob offsets
//! ToneData        u8 type, key, length, pan_sweep; u32 wav; u8 attack, decay, sustain, release
//!                 (a keysplit's last four bytes are its table's offset)
//! WaveData        u16 type (always 0: DPCM is decoded at build time), u16 status, u32 freq,
//!                 u32 loopStart, u32 size; i8 data[size + 1] (one sample past the end, read by
//!                 the interpolation)
//! wave (PSG ch3)  16 bytes; keysplit table: 128 bytes indexed by key
//! ```
//! Offset 0 means "none" (a song without a header).

/// `b"MP2K"`.
pub const MAGIC: [u8; 4] = *b"MP2K";
/// Bumped when the layout changes.
pub const VERSION: u16 = 1;
pub const HEADER_LEN: usize = 32;
pub const SONG_ENTRY: usize = 8;
/// `sizeof(struct ToneData)`.
pub const TONE: u32 = 12;
/// `struct WaveData` before its samples.
pub const WAVE_HEADER: u32 = 16;

/// A view of a sound blob. Reads past the end give 0, as the engine's `chk_adr` guards do.
#[derive(Clone, Copy)]
pub struct Blob<'a> {
    bytes: &'a [u8],
}

/// One `gSongTable` entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Song {
    /// The SongHeader's offset (0: none).
    pub header: u32,
    /// `ms`: the music player (0 BGM, 1 SE1, 2 SE2, 3 SE3).
    pub player: u8,
}

impl<'a> Blob<'a> {
    /// `None` unless `bytes` starts with the header of this [`VERSION`].
    pub fn new(bytes: &'a [u8]) -> Option<Self> {
        let b = Blob { bytes };
        (bytes.len() >= HEADER_LEN && bytes[..4] == MAGIC && b.u16(4) == VERSION).then_some(b)
    }

    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    #[inline]
    pub fn u8(&self, at: u32) -> u8 {
        self.bytes.get(at as usize).copied().unwrap_or(0)
    }

    #[inline]
    pub fn i8(&self, at: u32) -> i8 {
        self.u8(at) as i8
    }

    pub fn u16(&self, at: u32) -> u16 {
        u16::from_le_bytes([self.u8(at), self.u8(at + 1)])
    }

    pub fn u32(&self, at: u32) -> u32 {
        u32::from_le_bytes([
            self.u8(at),
            self.u8(at + 1),
            self.u8(at + 2),
            self.u8(at + 3),
        ])
    }

    pub fn song_count(&self) -> u16 {
        self.u16(6)
    }

    pub fn song(&self, n: u16) -> Option<Song> {
        (n < self.song_count()).then(|| {
            let at = self.u32(8) + n as u32 * SONG_ENTRY as u32;
            Song {
                header: self.u32(at),
                player: self.u8(at + 4),
            }
        })
    }

    /// The `gCryTable` (or `gCryTable_Reverse`) entry `i`.
    pub fn cry_tone(&self, i: u16, reverse: bool) -> u32 {
        self.u32(if reverse { 16 } else { 12 }) + i as u32 * TONE
    }

    pub fn cry_count(&self) -> u32 {
        self.u32(20)
    }

    pub fn dummy_tone(&self) -> u32 {
        self.u32(24)
    }

    /// The samples of the WaveData at `wav` (`size` of them, plus the one past the end).
    pub fn samples(&self, wav: u32) -> &'a [u8] {
        let size = self.u32(wav + 12) as usize;
        let start = (wav + WAVE_HEADER) as usize;
        self.bytes.get(start..start + size + 1).unwrap_or(&[])
    }
}
