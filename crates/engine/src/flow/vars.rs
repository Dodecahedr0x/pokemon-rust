//! Event flags and vars (Gen3 `event_data.c`).

use crate::data::{FlagId, VarId};
use crate::hash::Fnv64;
use alloc::boxed::Box;
use alloc::vec;

/// Flags `0..FLAG_COUNT` are stored (bitset).
pub const FLAG_COUNT: u16 = 0x4000;
/// Saved vars `VARS_START..VARS_END`.
pub const VARS_START: u16 = 0x4000;
pub const VARS_END: u16 = 0x4100;
/// Special vars `SPECIAL_VARS_START..SPECIAL_VARS_END` (Gen3 `gSpecialVars`, `VAR_RESULT`…).
pub const SPECIAL_VARS_START: u16 = 0x8000;
pub const SPECIAL_VARS_END: u16 = 0x8100;
/// Temp flags `0..TEMP_FLAGS_END` (Gen3 `FLAG_TEMP_1..FLAG_TEMP_1F`), cleared on map change.
pub const TEMP_FLAGS_END: u16 = 0x20;
/// Temp vars `VARS_START..TEMP_VARS_END` (Gen3 `VAR_TEMP_0..VAR_TEMP_F`), cleared on map change.
pub const TEMP_VARS_END: u16 = 0x4010;

const SAVED_VARS: usize = (VARS_END - VARS_START) as usize;
const SPECIAL_VARS: usize = (SPECIAL_VARS_END - SPECIAL_VARS_START) as usize;

/// Event flags and vars. Heap-backed (3 KiB) so `World` stays small on the GBA stack.
///
/// The GBA hashes the world every frame, so `hash` must not walk 3 KiB: a digest of the
/// contents (a wrapping sum of one mixed word per set flag and per non-zero var) is kept up to
/// date on every write instead.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Vars {
    flags: Box<[u8]>,
    /// Saved vars, then special vars.
    vars: Box<[u16]>,
    digest: u64,
}

/// splitmix64 finalizer: spreads one (kind, id, value) word over 64 bits.
const fn mix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Digest contribution of flag `f` when set.
const fn flag_term(f: u16) -> u64 {
    mix(1 << 32 | f as u64)
}

/// Digest contribution of var slot `i` holding `value` (0 contributes nothing).
const fn var_term(i: usize, value: u16) -> u64 {
    if value == 0 {
        0
    } else {
        mix(2 << 32 | (i as u64) << 16 | value as u64)
    }
}

impl Default for Vars {
    fn default() -> Self {
        Self::new()
    }
}

impl Vars {
    pub fn new() -> Self {
        Self {
            flags: vec![0u8; FLAG_COUNT as usize / 8].into_boxed_slice(),
            vars: vec![0u16; SAVED_VARS + SPECIAL_VARS].into_boxed_slice(),
            digest: 0,
        }
    }

    /// Flag 0 is not a flag in Gen3 (`GetFlagPointer(0)` is NULL): it reads false, ignores writes.
    fn flag_slot(f: FlagId) -> Option<(usize, u8)> {
        (f.0 != 0 && f.0 < FLAG_COUNT).then(|| ((f.0 / 8) as usize, 1 << (f.0 % 8)))
    }

    fn var_slot(v: VarId) -> Option<usize> {
        match v.0 {
            VARS_START..VARS_END => Some((v.0 - VARS_START) as usize),
            SPECIAL_VARS_START..SPECIAL_VARS_END => {
                Some(SAVED_VARS + (v.0 - SPECIAL_VARS_START) as usize)
            }
            _ => None,
        }
    }

    pub fn flag(&self, f: FlagId) -> bool {
        Self::flag_slot(f).is_some_and(|(i, m)| self.flags[i] & m != 0)
    }

    pub fn set_flag(&mut self, f: FlagId, on: bool) {
        if let Some((i, m)) = Self::flag_slot(f) {
            let was = self.flags[i] & m != 0;
            if on == was {
                return;
            }
            if on {
                self.flags[i] |= m;
                self.digest = self.digest.wrapping_add(flag_term(f.0));
            } else {
                self.flags[i] &= !m;
                self.digest = self.digest.wrapping_sub(flag_term(f.0));
            }
        }
    }

    /// The var's value; 0 outside the stored ranges.
    pub fn var(&self, v: VarId) -> u16 {
        Self::var_slot(v).map_or(0, |i| self.vars[i])
    }

    pub fn set_var(&mut self, v: VarId, value: u16) {
        if let Some(i) = Self::var_slot(v) {
            let old = core::mem::replace(&mut self.vars[i], value);
            self.digest =
                (self.digest.wrapping_sub(var_term(i, old))).wrapping_add(var_term(i, value));
        }
    }

    /// Gen3 `VarGet`: ids below `VARS_START` are literals and read as themselves. Used where Gen3
    /// data may hold either a var or a value (map script tables).
    pub fn var_or_literal(&self, id: u16) -> u16 {
        if id < VARS_START {
            id
        } else {
            self.var(VarId(id))
        }
    }

    /// Gen3 `ClearTempFieldEventData` (temp flags and vars part): run on every map change (warp
    /// load and connection crossing). Gen3 also clears `FLAG_SYS_ENC_UP_ITEM`,
    /// `FLAG_SYS_ENC_DOWN_ITEM`, `FLAG_SYS_USE_STRENGTH`, `FLAG_SYS_CTRL_OBJ_DELETE` and
    /// `FLAG_NURSE_UNION_ROOM_REMINDER` there; those are game constants, so content or the
    /// integration must clear them.
    pub fn clear_temp(&mut self) {
        for f in 1..TEMP_FLAGS_END {
            self.set_flag(FlagId(f), false);
        }
        for v in VARS_START..TEMP_VARS_END {
            self.set_var(VarId(v), 0);
        }
    }

    /// The digest of every flag and var, recomputed from scratch (what `digest` tracks).
    fn full_digest(&self) -> u64 {
        let flags = (1..FLAG_COUNT)
            .filter(|&f| self.flag(FlagId(f)))
            .fold(0u64, |d, f| d.wrapping_add(flag_term(f)));
        (self.vars.iter().enumerate()).fold(flags, |d, (i, &v)| d.wrapping_add(var_term(i, v)))
    }

    /// Feeds the contents digest (O(1)); equal contents give equal hashes.
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        debug_assert!(cfg!(not(test)) || self.digest == self.full_digest());
        h.write(&self.digest.to_le_bytes());
    }
}

// Milestone 5 Task B.
impl Vars {
    /// Set flags and non-zero vars, ascending.
    pub fn snapshot(&self) -> crate::checkpoint::VarsSnap {
        let flags = (1..FLAG_COUNT)
            .map(FlagId)
            .filter(|&f| self.flag(f))
            .collect();
        let ids = (VARS_START..VARS_END).chain(SPECIAL_VARS_START..SPECIAL_VARS_END);
        let vars = ids
            .map(|v| (VarId(v), self.var(VarId(v))))
            .filter(|&(_, x)| x != 0)
            .collect();
        crate::checkpoint::VarsSnap { flags, vars }
    }

    /// Rebuilt through the setters, so the digest matches the contents.
    pub fn from_snapshot(s: &crate::checkpoint::VarsSnap) -> Vars {
        let mut v = Vars::new();
        for &f in &s.flags {
            v.set_flag(f, true);
        }
        for &(id, x) in &s.vars {
            v.set_var(id, x);
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_is_sparse_ascending_and_round_trips() {
        let mut v = Vars::new();
        v.set_flag(FlagId(0x3FFF), true);
        v.set_flag(FlagId(0x20), true);
        v.set_var(VarId(0x8001), 7);
        v.set_var(VarId(0x4005), 3);
        v.set_var(VarId(0x4006), 0);
        let s = v.snapshot();
        assert_eq!(s.flags, [FlagId(0x20), FlagId(0x3FFF)]);
        assert_eq!(s.vars, [(VarId(0x4005), 3), (VarId(0x8001), 7)]);
        let r = Vars::from_snapshot(&s);
        assert_eq!(r, v);
        assert_eq!(r.digest, r.full_digest());
    }

    #[test]
    fn flags_set_clear_and_independent() {
        let mut v = Vars::new();
        assert!(!v.flag(FlagId(0x123)));
        v.set_flag(FlagId(0x123), true);
        assert!(v.flag(FlagId(0x123)));
        assert!(!v.flag(FlagId(0x122)) && !v.flag(FlagId(0x124)));
        v.set_flag(FlagId(0x3FFF), true);
        assert!(v.flag(FlagId(0x3FFF)));
        v.set_flag(FlagId(0x123), false);
        assert!(!v.flag(FlagId(0x123)));
        assert!(v.flag(FlagId(0x3FFF)));
    }

    #[test]
    fn out_of_range_flags_read_false_and_ignore_writes() {
        let mut v = Vars::new();
        let before = v.clone();
        for f in [0, 0x4000, 0x8000, 0xFFFF] {
            v.set_flag(FlagId(f), true);
            assert!(!v.flag(FlagId(f)), "flag {f:#x}");
        }
        assert_eq!(v, before);
    }

    #[test]
    fn both_var_ranges_store_u16() {
        let mut v = Vars::new();
        for (id, x) in [(0x4000, 1), (0x40FF, 0xFFFF), (0x8000, 2), (0x80FF, 3)] {
            v.set_var(VarId(id), x);
            assert_eq!(v.var(VarId(id)), x, "var {id:#x}");
        }
        assert_eq!(v.var(VarId(0x4001)), 0);
        assert_eq!(v.var(VarId(0x8001)), 0);
    }

    #[test]
    fn out_of_range_vars_read_zero_and_ignore_writes() {
        let mut v = Vars::new();
        let before = v.clone();
        for id in [0, 0x3FFF, 0x4100, 0x7FFF, 0x8100, 0xFFFF] {
            v.set_var(VarId(id), 9);
            assert_eq!(v.var(VarId(id)), 0, "var {id:#x}");
        }
        assert_eq!(v, before);
    }

    #[test]
    fn var_or_literal_is_gen3_var_get() {
        let mut v = Vars::new();
        v.set_var(VarId(0x4050), 7);
        assert_eq!(v.var_or_literal(3), 3);
        assert_eq!(v.var_or_literal(0x3FFF), 0x3FFF);
        assert_eq!(v.var_or_literal(0x4050), 7);
    }

    #[test]
    fn clear_temp_clears_only_temp_flags_and_vars() {
        let mut v = Vars::new();
        for f in 1..0x22 {
            v.set_flag(FlagId(f), true);
        }
        for id in 0x4000..0x4012 {
            v.set_var(VarId(id), 5);
        }
        v.set_var(VarId(0x8000), 5);
        v.clear_temp();
        for f in 1..0x20 {
            assert!(!v.flag(FlagId(f)), "temp flag {f:#x}");
        }
        assert!(v.flag(FlagId(0x20)) && v.flag(FlagId(0x21)));
        for id in 0x4000..0x4010 {
            assert_eq!(v.var(VarId(id)), 0, "temp var {id:#x}");
        }
        assert_eq!(v.var(VarId(0x4010)), 5);
        assert_eq!(v.var(VarId(0x4011)), 5);
        assert_eq!(v.var(VarId(0x8000)), 5, "special vars survive");
    }

    #[test]
    fn digest_tracks_every_write() {
        let mut v = Vars::new();
        let mut rng = crate::Rng::new(7);
        for _ in 0..2000 {
            let r = rng.next_u32();
            match r % 3 {
                0 => v.set_flag(FlagId((r >> 8) as u16 % 0x60), r & 0x80 != 0),
                1 => v.set_var(VarId(0x4000 + (r >> 8) as u16 % 0x20), (r >> 20) as u16 % 3),
                _ => v.set_var(VarId(0x8000 + (r >> 8) as u16 % 0x10), (r >> 20) as u16),
            }
            if r.is_multiple_of(97) {
                v.clear_temp();
            }
            assert_eq!(v.digest, v.full_digest());
        }
        // Same contents through different histories: same hash.
        let (mut a, mut b) = (Vars::new(), Vars::new());
        a.set_var(VarId(0x4050), 3);
        a.set_flag(FlagId(0x100), true);
        b.set_flag(FlagId(0x100), true);
        b.set_var(VarId(0x4050), 9);
        b.set_var(VarId(0x4050), 3);
        assert_eq!(a, b);
    }

    #[test]
    fn hash_covers_flags_and_both_var_ranges() {
        let h = |v: &Vars| {
            let mut h = Fnv64::new();
            v.hash(&mut h);
            h.finish()
        };
        let base = h(&Vars::new());
        assert_eq!(base, h(&Vars::new()));
        let mut a = Vars::new();
        a.set_flag(FlagId(0x3FFF), true);
        let mut b = Vars::new();
        b.set_var(VarId(0x40FF), 1);
        let mut c = Vars::new();
        c.set_var(VarId(0x80FF), 1);
        let all = [base, h(&a), h(&b), h(&c)];
        for i in 0..all.len() {
            for j in i + 1..all.len() {
                assert_ne!(all[i], all[j]);
            }
        }
    }
}
