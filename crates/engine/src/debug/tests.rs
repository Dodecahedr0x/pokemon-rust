use super::*;
use core::mem::{offset_of, size_of};

#[test]
fn layout_is_stable() {
    assert_eq!(offset_of!(DebugRegion, magic), 0);
    assert_eq!(offset_of!(DebugRegion, frame), OFF_FRAME as usize);
    assert_eq!(offset_of!(DebugRegion, hash), OFF_HASH as usize);
    assert_eq!(offset_of!(DebugRegion, inputs), OFF_INPUTS as usize);
    assert_eq!(offset_of!(DebugRegion, late), OFF_LATE as usize);
    assert_eq!(offset_of!(DebugRegion, late_black), OFF_LATE_BLACK as usize);
    assert_eq!(offset_of!(DebugRegion, build_engine), 160);
    assert_eq!(offset_of!(DebugRegion, build_content), 168);
    assert_eq!(offset_of!(DebugRegion, boot), 176);
    assert_eq!(offset_of!(DebugRegion, saves), 180);
    assert_eq!(offset_of!(DebugRegion, save_status), 184);
    assert_eq!(offset_of!(DebugRegion, hash_lines_max), 188);
    assert_eq!(offset_of!(DebugRegion, step_lines_max), 192);
    assert_eq!(offset_of!(DebugRegion, loop_lines_max), 196);
    assert_eq!(size_of::<DebugRegion>(), 200);
    assert_eq!(core::mem::align_of::<DebugRegion>(), 8);
    assert_eq!(&DebugRegion::new().magic, b"PKRDBG02");
}

#[test]
fn input_slot_for_frame() {
    assert_eq!(input_slot(1), 0);
    assert_eq!(input_slot(64), 63);
    assert_eq!(input_slot(65), 0);
}
