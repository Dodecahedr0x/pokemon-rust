use super::*;
use alloc::vec;

#[test]
fn set_pads_cuts_and_keeps_the_digest() {
    let mut v = StrVars::new();
    let empty = v.digest;
    v.set(1, &[0xBB, 0xBC, 0xFF, 0x01]);
    assert_eq!(&v.get(1)[..3], &[0xBB, 0xBC, 0xFF]);
    v.set(2, &[0xA1; 40]);
    assert_eq!(v.get(2), &[0xA1; STR_VAR_LEN]);
    let back = StrVars::from_snapshot(&v.snapshot()).unwrap();
    assert_eq!(back, v);
    v.set(1, &[]);
    v.set(2, &[0xFF]);
    assert_eq!(v.digest, empty);
    assert!(StrVars::from_snapshot(&[vec![0xFF], vec![], vec![]]).is_err());
}
