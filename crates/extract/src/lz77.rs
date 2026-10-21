//! GBA BIOS LZ77 decompression (type 0x10), as produced by `gbagfx` and read by `LZ77UnCompVram`.

use crate::Rom;
use anyhow::{Result, bail};

/// Decompresses one stream. `src` may extend past the end of the stream; trailing bytes are ignored.
/// Like `gbagfx`, a back-reference that runs past the declared size is clipped to it.
pub fn decompress(src: &[u8]) -> Result<Vec<u8>> {
    if src.len() < 4 {
        bail!("lz77 header truncated ({} bytes)", src.len());
    }
    if src[0] != 0x10 {
        bail!("not an lz77 stream (type byte {:#04x})", src[0]);
    }
    let size = u32::from_le_bytes([src[1], src[2], src[3], 0]) as usize;
    let mut out = Vec::with_capacity(size);
    let mut pos = 4;
    let byte = |pos: usize| {
        src.get(pos).copied().ok_or_else(|| {
            anyhow::anyhow!("lz77 stream truncated at byte {pos} (expected {size} bytes out)")
        })
    };
    while out.len() < size {
        let flags = byte(pos)?;
        pos += 1;
        for bit in 0..8 {
            if out.len() >= size {
                break;
            }
            if flags & (0x80 >> bit) != 0 {
                let (b0, b1) = (byte(pos)?, byte(pos + 1)?);
                pos += 2;
                let len = (b0 >> 4) as usize + 3;
                let dist = (((b0 & 0xF) as usize) << 8 | b1 as usize) + 1;
                if dist > out.len() {
                    bail!(
                        "lz77 back-reference distance {dist} exceeds output {} at byte {}",
                        out.len(),
                        pos - 2
                    );
                }
                let start = out.len() - dist;
                for j in 0..len.min(size - out.len()) {
                    out.push(out[start + j]);
                }
            } else {
                out.push(byte(pos)?);
                pos += 1;
            }
        }
    }
    Ok(out)
}

impl Rom {
    /// Decompresses the LZ77 stream starting at file offset `off`.
    pub fn lz77_at(&self, off: u32) -> Result<Vec<u8>> {
        let len = (self.bytes().len() as u64).saturating_sub(off as u64) as usize;
        let src = self.slice(off, len)?;
        decompress(src).map_err(|e| anyhow::anyhow!("lz77 at {off:#x}: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_only() {
        // header: type 0x10, size 3; one flag byte (all literals), three literals.
        let src = [0x10, 3, 0, 0, 0x00, b'a', b'b', b'c'];
        assert_eq!(decompress(&src).unwrap(), b"abc");
    }

    #[test]
    fn overlapping_back_reference() {
        // "ab" then copy 6 bytes from distance 2 => "abababab".
        // flags 0b0010_0000: literal, literal, ref. ref = len-3=3 in high nibble, disp-1=1.
        let src = [0x10, 8, 0, 0, 0x20, b'a', b'b', 0x30, 0x01];
        assert_eq!(decompress(&src).unwrap(), b"abababab");
    }

    #[test]
    fn truncated_is_err() {
        let src = [0x10, 8, 0, 0, 0x20, b'a', b'b', 0x30];
        assert!(decompress(&src).is_err());
        assert!(decompress(&[0x10, 4, 0]).is_err());
        assert!(decompress(&[0x10, 4, 0, 0, 0x00, b'a']).is_err());
    }

    #[test]
    fn bad_header_and_bad_distance_are_err() {
        assert!(decompress(&[0x11, 1, 0, 0, 0, b'a']).is_err());
        // back-reference before the start of output
        assert!(decompress(&[0x10, 3, 0, 0, 0x80, 0x00, 0x00]).is_err());
    }
}
