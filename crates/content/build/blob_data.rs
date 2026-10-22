//! Bulk data as byte blobs (Milestone 8 Task E): the generator writes tiles, palettes,
//! metatiles, blocks, glyphs and text bytes into a [`BlobOut`] and emits only the Rust
//! expression that slices them back (`DATA.tiles(at, n)`, `content::blob::Blob`). The Rust
//! source stays small, which keeps rustc's time and memory down on the full import: literals
//! of the same data made an 11.7 MB `data.rs` and a 4.9 GB rustc.
//!
//! `data.rs` slices `$OUT_DIR/data.bin` and `battle.rs` slices `$OUT_DIR/battle.bin`. Items are
//! 4-aligned. Identical items are stored once (tilesets shared by maps, repeated sprite frames,
//! repeated texts); the first copy keeps its offset, so the layout is deterministic.

use crate::input::{Glyph, Tile};
use std::collections::HashMap;

/// A blob being built: its bytes and, by content, where each item already is.
pub struct BlobOut {
    /// The Rust path of the `Blob` static the expressions slice (`DATA`, `BLOB`).
    name: &'static str,
    pub bytes: Vec<u8>,
    seen: HashMap<Vec<u8>, usize>,
    /// Bytes saved by sharing identical items.
    pub deduped: usize,
}

impl BlobOut {
    pub fn new(name: &'static str) -> BlobOut {
        BlobOut {
            name,
            bytes: Vec::new(),
            seen: HashMap::new(),
            deduped: 0,
        }
    }

    pub fn align(&mut self) -> usize {
        while !self.bytes.len().is_multiple_of(4) {
            self.bytes.push(0);
        }
        self.bytes.len()
    }

    /// The offset of `item`: an identical earlier item's, else appended (4-aligned).
    fn put(&mut self, item: Vec<u8>) -> usize {
        if item.is_empty() {
            return 0;
        }
        if let Some(&at) = self.seen.get(&item) {
            self.deduped += item.len();
            return at;
        }
        let at = self.align();
        self.bytes.extend_from_slice(&item);
        self.seen.insert(item, at);
        at
    }

    fn u16_bytes(v: &[u16]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    pub fn tiles(&mut self, t: &[Tile]) -> String {
        let at = self.put(t.iter().flatten().flat_map(|r| r.to_le_bytes()).collect());
        format!("{}.tiles({at}, {})", self.name, t.len())
    }

    pub fn u16s(&mut self, v: &[u16]) -> String {
        let at = self.put(Self::u16_bytes(v));
        format!("{}.u16s({at}, {})", self.name, v.len())
    }

    pub fn pal(&mut self, p: &[u16; 16]) -> String {
        let at = self.put(Self::u16_bytes(p));
        format!("{}.pal({at})", self.name)
    }

    pub fn pals(&mut self, p: &[[u16; 16]]) -> String {
        let at = self.put(Self::u16_bytes(p.as_flattened()));
        format!("{}.pals({at}, {})", self.name, p.len())
    }

    pub fn metatiles(&mut self, m: &[[u16; 8]]) -> String {
        let at = self.put(Self::u16_bytes(m.as_flattened()));
        format!("{}.metatiles({at}, {})", self.name, m.len())
    }

    pub fn glyphs(&mut self, g: &[Glyph]) -> String {
        let at = self.put(g.iter().flatten().copied().collect());
        format!("{}.glyphs({at}, {})", self.name, g.len())
    }

    pub fn bytes(&mut self, b: &[u8]) -> String {
        let at = self.put(b.to_vec());
        format!("{}.bytes({at}, {})", self.name, b.len())
    }

    /// The `Blob` static, `include_bytes!` of `$OUT_DIR/<file>` (written by the caller with
    /// [`BlobOut::finish`]'s bytes).
    pub fn static_decl(&mut self, file: &str) -> String {
        let n = self.align();
        format!(
            "pub static {}: crate::blob::Blob<{n}> = crate::blob::Blob(*include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{file}\")));\n",
            self.name
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_items_are_stored_once() {
        let mut b = BlobOut::new("DATA");
        let t: Vec<Tile> = vec![[1; 8], [2; 8]];
        let a = b.tiles(&t);
        let c = b.tiles(&t);
        assert_eq!(a, c);
        assert_eq!(b.bytes.len(), 64);
        assert_eq!(b.deduped, 64);
        assert_eq!(b.bytes(&[1, 2, 3]), "DATA.bytes(64, 3)");
        assert_eq!(b.u16s(&[7]), "DATA.u16s(68, 1)");
        assert_eq!(b.bytes(&[]), "DATA.bytes(0, 0)");
    }
}
