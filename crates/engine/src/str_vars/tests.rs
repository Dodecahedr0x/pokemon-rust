use super::*;
use alloc::vec;

#[test]
fn set_pads_cuts_and_keeps_the_digest() {
    let mut v = StrVars::new();
    let empty = v.digest;
    v.set(1, &[0xBB, 0xBC, 0xFF, 0x01]);
    assert_eq!(&v.get(1)[..3], &[0xBB, 0xBC, 0xFF]);
    v.set(2, &[0xA1; 300]);
    assert_eq!(v.get(2)[..STR_VAR_LEN - 1], [0xA1; STR_VAR_LEN - 1]);
    assert_eq!(v.get(2)[STR_VAR_LEN - 1], 0xFF);
    let back = StrVars::from_snapshot(&v.snapshot()).unwrap();
    assert_eq!(back, v);
    v.set(1, &[]);
    v.set(2, &[0xFF]);
    assert_eq!(v.digest, empty);
    assert!(StrVars::from_snapshot(&[vec![0xFF], vec![], vec![]]).is_err());
    assert!(StrVars::from_snapshot(&[vec![], vec![1; STR_VAR_LEN], vec![]]).is_err());
    assert!(StrVars::from_snapshot(&[vec![], vec![1; STR_VAR_LEN - 1], vec![]]).is_ok());
}

/// The digest covers the text and its terminator only, so it does not depend on the buffer
/// length: pinned against the terms of `[0xBB, 0xBC, 0xFF]` in slot 1 and the bare terminators.
#[test]
fn the_digest_stops_at_the_terminator() {
    let term = |slot: u64, i: u64, b: u64| splitmix64(4 << 48 | slot << 40 | i << 32 | b);
    let empty = (0..3).fold(0u64, |d, s| d.wrapping_add(term(s, 0, 0xFF)));
    assert_eq!(StrVars::new().digest, empty);
    let mut v = StrVars::new();
    v.set(1, &[0xBB, 0xBC]);
    let want = empty
        .wrapping_sub(term(1, 0, 0xFF))
        .wrapping_add(term(1, 0, 0xBB))
        .wrapping_add(term(1, 1, 0xBC))
        .wrapping_add(term(1, 2, 0xFF));
    assert_eq!(v.digest, want);
}
