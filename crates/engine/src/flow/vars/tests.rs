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
