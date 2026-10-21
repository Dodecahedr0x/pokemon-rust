use anyhow::{Context, Result, anyhow, bail};
use std::path::Path;

/// Address the cartridge ROM is mapped at on the GBA.
pub const ROM_BASE: u32 = 0x0800_0000;
const MAX_ROM_LEN: usize = 32 * 1024 * 1024;
const GAME_CODE_OFFSET: usize = 0xAC;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameCode {
    /// Emerald (USA/Europe), matches the pokeemerald decomp.
    Bpee,
    /// Émeraude (France).
    Bpef,
}

pub struct Rom {
    bytes: Vec<u8>,
    code: GameCode,
}

impl Rom {
    pub fn open(path: &Path) -> Result<Rom> {
        let bytes =
            std::fs::read(path).with_context(|| format!("reading ROM {}", path.display()))?;
        Self::from_bytes(bytes).with_context(|| format!("loading ROM {}", path.display()))
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Rom> {
        if bytes.len() > MAX_ROM_LEN || bytes.len() < GAME_CODE_OFFSET + 4 {
            bail!("unexpected ROM size {} bytes", bytes.len());
        }
        let code = match &bytes[GAME_CODE_OFFSET..GAME_CODE_OFFSET + 4] {
            b"BPEE" => GameCode::Bpee,
            b"BPEF" => GameCode::Bpef,
            other => bail!("unsupported game code {:?}", String::from_utf8_lossy(other)),
        };
        Ok(Rom { bytes, code })
    }

    pub fn code(&self) -> GameCode {
        self.code
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn sha1_hex(&self) -> String {
        sha1_smol::Sha1::from(&self.bytes).digest().to_string()
    }

    pub fn slice(&self, off: u32, len: usize) -> Result<&[u8]> {
        let start = off as usize;
        start
            .checked_add(len)
            .and_then(|end| self.bytes.get(start..end))
            .ok_or_else(|| anyhow!("read of {len} bytes at {off:#x} is out of bounds"))
    }

    fn array<const N: usize>(&self, off: u32) -> Result<[u8; N]> {
        Ok(self.slice(off, N)?.try_into().unwrap())
    }

    pub fn u8(&self, off: u32) -> Result<u8> {
        Ok(self.array::<1>(off)?[0])
    }

    pub fn u16(&self, off: u32) -> Result<u16> {
        self.array(off).map(u16::from_le_bytes)
    }

    pub fn u32(&self, off: u32) -> Result<u32> {
        self.array(off).map(u32::from_le_bytes)
    }

    pub fn i16(&self, off: u32) -> Result<i16> {
        self.array(off).map(i16::from_le_bytes)
    }

    pub fn i32(&self, off: u32) -> Result<i32> {
        self.array(off).map(i32::from_le_bytes)
    }

    /// The u32 at `off` read as a GBA ROM pointer: 0 is `None`, otherwise it must point inside the ROM.
    pub fn ptr(&self, off: u32) -> Result<Option<u32>> {
        match self.u32(off)? {
            0 => Ok(None),
            p if p >= ROM_BASE && ((p - ROM_BASE) as usize) < self.bytes.len() => {
                Ok(Some(p - ROM_BASE))
            }
            p => bail!("value {p:#010x} at {off:#x} is not a ROM pointer"),
        }
    }

    pub fn find_all(&self, needle: &[u8]) -> Vec<u32> {
        if needle.is_empty() {
            return Vec::new();
        }
        self.bytes
            .windows(needle.len())
            .enumerate()
            .filter(|(_, w)| *w == needle)
            .map(|(i, _)| i as u32)
            .collect()
    }

    /// 4-byte-aligned offsets whose u32 equals `ROM_BASE + target`.
    pub fn pointers_to(&self, target: u32) -> Vec<u32> {
        let want = (ROM_BASE + target).to_le_bytes();
        self.bytes
            .chunks_exact(4)
            .enumerate()
            .filter(|(_, c)| *c == want)
            .map(|(i, _)| (i * 4) as u32)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rom_with(code: &[u8; 4], tail: &[u8]) -> Rom {
        let mut b = vec![0u8; 0xC0];
        b[0xAC..0xB0].copy_from_slice(code);
        b.extend_from_slice(tail);
        Rom::from_bytes(b).unwrap()
    }

    #[test]
    fn detects_game_code() {
        assert_eq!(rom_with(b"BPEF", &[]).code(), GameCode::Bpef);
        assert_eq!(rom_with(b"BPEE", &[]).code(), GameCode::Bpee);
        let mut b = vec![0u8; 0xC0];
        b[0xAC..0xB0].copy_from_slice(b"AXVE");
        assert!(Rom::from_bytes(b).is_err());
    }

    #[test]
    fn little_endian_reads_and_bounds() {
        let r = rom_with(b"BPEF", &[0x34, 0x12, 0xFF, 0xFF]);
        assert_eq!(r.u16(0xC0).unwrap(), 0x1234);
        assert_eq!(r.i16(0xC2).unwrap(), -1);
        assert_eq!(r.u32(0xC0).unwrap(), 0xFFFF_1234);
        assert!(r.u32(0xC2).is_err());
        assert!(r.slice(u32::MAX, 2).is_err());
    }

    #[test]
    fn pointers() {
        let len = 0xC0 + 12;
        let mut tail = Vec::new();
        tail.extend_from_slice(&0u32.to_le_bytes());
        tail.extend_from_slice(&(ROM_BASE + 0xC0).to_le_bytes());
        tail.extend_from_slice(&(ROM_BASE + len as u32).to_le_bytes());
        let r = rom_with(b"BPEF", &tail);
        assert_eq!(r.ptr(0xC0).unwrap(), None);
        assert_eq!(r.ptr(0xC4).unwrap(), Some(0xC0));
        assert!(
            r.ptr(0xC8).is_err(),
            "one past the end is not a valid pointer"
        );
        assert_eq!(r.pointers_to(0xC0), vec![0xC4]);
    }

    #[test]
    fn find_all_overlapping() {
        let r = rom_with(b"BPEF", &[1, 1, 1]);
        assert_eq!(r.find_all(&[1, 1]), vec![0xC0, 0xC1]);
        assert!(r.find_all(&[]).is_empty());
    }
}
