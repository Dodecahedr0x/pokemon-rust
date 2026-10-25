use super::*;

fn berry(name: [u8; 6], max_yield: u8, stage: u8) -> [u8; ENIGMA_BERRY_LEN] {
    let mut raw = [0; ENIGMA_BERRY_LEN];
    raw[..6].copy_from_slice(&name);
    raw[6] = 0xFF;
    raw[MAX_YIELD] = max_yield;
    raw[STAGE_DURATION] = stage;
    let sum = checksum(&raw);
    raw[CHECKSUM..].copy_from_slice(&sum.to_le_bytes());
    raw
}

fn push_u16(out: &mut Vec<u8>, v: u16) {
    out.extend(v.to_le_bytes());
}

fn push_u32(out: &mut Vec<u8>, v: u32) {
    out.extend(v.to_le_bytes());
}

/// `checkcompat` (English, RS, Emerald, link offset 0), `setenigmaberry` of `raw`, `end`.
fn script(raw: &[u8; ENIGMA_BERRY_LEN]) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(OP_CHECKCOMPAT);
    push_u32(&mut out, 0);
    push_u16(&mut out, 1);
    push_u32(&mut out, 1);
    push_u16(&mut out, 4);
    push_u32(&mut out, VERSION_EMERALD);
    // The berry sits after `end`, so the VM does not execute its bytes.
    out.push(OP_SETENIGMABERRY);
    let at = (out.len() + 4 + 1) as u32;
    push_u32(&mut out, at);
    out.push(OP_END);
    assert_eq!(out.len() as u32, at);
    out.extend(raw);
    out
}

#[test]
fn setenigmaberry_installs_a_valid_berry() {
    let raw = berry([0x11, 0x22, 0x33, 0x44, 0x55, 0x66], 3, 12);
    let outcome = run(&script(&raw));
    assert_eq!(outcome.status, STATUS_SUCCESS);
    assert_eq!(outcome.berry, Some(raw));
    assert!(is_valid(&raw));
    assert_eq!(name(&raw)[..6], [0x11, 0x22, 0x33, 0x44, 0x55, 0x66]);
}

#[test]
fn a_bad_berry_checksum_is_stored_as_a_load_error() {
    let mut raw = berry([1, 2, 3, 4, 5, 6], 2, 4);
    raw[CHECKSUM] ^= 0xFF;
    let outcome = run(&script(&raw));
    assert_eq!(outcome.status, STATUS_LOAD_ERROR);
    assert_eq!(outcome.berry, Some(raw));
    assert!(!is_valid(&raw));
}

#[test]
fn an_incompatible_card_copies_nothing() {
    let mut bytes = script(&berry([1, 2, 3, 4, 5, 6], 2, 4));
    // `unk0` (the halfword after the link offset) loses its English bit.
    bytes[5] = 0;
    bytes[6] = 0;
    let outcome = run(&bytes);
    assert_eq!(outcome.status, STATUS_FAILURE);
    assert_eq!(outcome.berry, None);
}

#[test]
fn a_script_checksum_mismatch_keeps_the_copied_berry() {
    let raw = berry([9, 8, 7, 6, 5, 4], 1, 8);
    let mut bytes = Vec::new();
    bytes.push(OP_CHECKCOMPAT);
    push_u32(&mut bytes, 0);
    push_u16(&mut bytes, 1);
    push_u32(&mut bytes, 1);
    push_u16(&mut bytes, 4);
    push_u32(&mut bytes, VERSION_EMERALD);
    bytes.push(OP_SETENIGMABERRY);
    let ptr_at = bytes.len();
    push_u32(&mut bytes, 0);
    bytes.push(OP_CHECKSUM);
    push_u32(&mut bytes, 0xFFFF_FFFF);
    let range_at = bytes.len();
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    bytes.push(OP_END);
    let berry_at = bytes.len() as u32;
    bytes[ptr_at..ptr_at + 4].copy_from_slice(&berry_at.to_le_bytes());
    bytes[range_at..range_at + 4].copy_from_slice(&berry_at.to_le_bytes());
    let end = berry_at + ENIGMA_BERRY_LEN as u32;
    bytes[range_at + 4..range_at + 8].copy_from_slice(&end.to_le_bytes());
    bytes.extend(raw);
    let outcome = run(&bytes);
    assert_eq!(outcome.status, STATUS_LOAD_ERROR);
    assert_eq!(outcome.berry, Some(raw));
}
