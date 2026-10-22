//! The generated blobs (`include_bytes!`d): `$OUT_DIR/battle.bin` (battle tiles, palettes,
//! tilemaps, glyphs, pics) and `$OUT_DIR/data.bin` (map blocks, tilesets, sprite frames, the
//! font, the window and text bytes), each item 4-aligned, sliced into `'static` tables at
//! compile time (`build/blob_data.rs` writes them). Large data as bytes keeps the generated Rust
//! small: it compiles much faster, in much less memory, than literals.

use engine::Tile;

// The blob holds little-endian words, as the GBA (and every host we build for) reads them.
const _: () = assert!(cfg!(target_endian = "little"));

/// A 4-aligned byte blob.
#[repr(C, align(4))]
pub struct Blob<const N: usize>(pub [u8; N]);

impl<const N: usize> Blob<N> {
    /// `n` values of `T` at byte offset `at`. `T` is plain data (integers and arrays of them)
    /// whose alignment is at most 4.
    const fn at<T>(&'static self, at: usize, n: usize) -> &'static [T] {
        assert!(core::mem::align_of::<T>() <= 4 && at.is_multiple_of(core::mem::align_of::<T>()));
        assert!(at + n * core::mem::size_of::<T>() <= N);
        // SAFETY: in bounds and aligned (checked above, and the blob is 4-aligned); `T` is
        // plain data, valid for any bytes; the blob is immutable and `'static`.
        unsafe { core::slice::from_raw_parts(self.0.as_ptr().add(at).cast::<T>(), n) }
    }

    pub const fn tiles(&'static self, at: usize, n: usize) -> &'static [Tile] {
        self.at(at, n)
    }

    pub const fn pal(&'static self, at: usize) -> &'static [u16; 16] {
        &self.at::<[u16; 16]>(at, 1)[0]
    }

    pub const fn pals(&'static self, at: usize, n: usize) -> &'static [[u16; 16]] {
        self.at(at, n)
    }

    pub const fn u16s(&'static self, at: usize, n: usize) -> &'static [u16] {
        self.at(at, n)
    }

    pub const fn glyphs(&'static self, at: usize, n: usize) -> &'static [[u8; 64]] {
        self.at(at, n)
    }

    pub const fn metatiles(&'static self, at: usize, n: usize) -> &'static [[u16; 8]] {
        self.at(at, n)
    }

    pub const fn bytes(&'static self, at: usize, n: usize) -> &'static [u8] {
        self.at(at, n)
    }
}
