//! The script string vars (Gen3 `gStringVar1..3`, Milestone 8): what `buffer*` commands write
//! and `{STR_VAR_1}`… placeholders print. Gen3-encoded, 0xFF-terminated and 0xFF-padded.
//!
//! As [`crate::flow::Vars`], a digest of the contents is kept up to date on every write, so
//! [`StrVars::hash`] is O(1).

use crate::hash::{Fnv64, splitmix64};
use alloc::vec::Vec;

/// Bytes per string var (terminator included when shorter).
pub const STR_VAR_LEN: usize = 32;
/// Number of string vars.
pub const STR_VARS: usize = 3;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StrVars {
    bufs: [[u8; STR_VAR_LEN]; STR_VARS],
    digest: u64,
}

impl Default for StrVars {
    fn default() -> Self {
        Self::new()
    }
}

/// Digest contribution of string var `slot` holding `buf`.
fn buf_term(slot: usize, buf: &[u8; STR_VAR_LEN]) -> u64 {
    (buf.iter().enumerate()).fold(0u64, |d, (i, &b)| {
        d.wrapping_add(splitmix64(
            4 << 48 | (slot as u64) << 40 | (i as u64) << 32 | b as u64,
        ))
    })
}

impl StrVars {
    /// All three empty (0xFF throughout).
    pub fn new() -> Self {
        let bufs = [[0xFF; STR_VAR_LEN]; STR_VARS];
        let digest = (0..STR_VARS).fold(0u64, |d, i| d.wrapping_add(buf_term(i, &bufs[i])));
        StrVars { bufs, digest }
    }

    /// String var `slot` (0..3), 0xFF-padded.
    pub fn get(&self, slot: usize) -> &[u8; STR_VAR_LEN] {
        &self.bufs[slot]
    }

    /// Sets string var `slot` to `text` up to its first 0xFF, cut at [`STR_VAR_LEN`] bytes and
    /// 0xFF-padded.
    pub fn set(&mut self, slot: usize, text: &[u8]) {
        let mut buf = [0xFF; STR_VAR_LEN];
        for (o, &c) in buf.iter_mut().zip(text.iter().take_while(|&&c| c != 0xFF)) {
            *o = c;
        }
        self.digest = (self.digest.wrapping_sub(buf_term(slot, &self.bufs[slot])))
            .wrapping_add(buf_term(slot, &buf));
        self.bufs[slot] = buf;
    }

    /// Feeds the contents digest (O(1)).
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&self.digest.to_le_bytes());
    }

    /// Each var up to its first 0xFF.
    pub fn snapshot(&self) -> [Vec<u8>; STR_VARS] {
        self.bufs
            .map(|b| b.iter().copied().take_while(|&c| c != 0xFF).collect())
    }

    /// Refuses a var longer than [`STR_VAR_LEN`] or holding 0xFF.
    pub fn from_snapshot(s: &[Vec<u8>; STR_VARS]) -> Result<StrVars, &'static str> {
        let mut v = StrVars::new();
        for (slot, text) in s.iter().enumerate() {
            if text.len() > STR_VAR_LEN || text.contains(&0xFF) {
                return Err("bad string var");
            }
            v.set(slot, text);
        }
        Ok(v)
    }
}

#[cfg(test)]
mod tests;
