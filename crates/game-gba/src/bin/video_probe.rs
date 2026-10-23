//! Video probe ROM: applies the frames of `probe_scene` one per vblank with the game's own
//! `video` applier and publishes the index of the frame shown in the debug region, so the
//! parity test can compare each picture with the native PPU (`Frame::lighten` at every level,
//! `Cmd::ObjTile`, the battle transitions). Built next to the game by `cargo xtask rom`.

#![no_std]
#![no_main]

extern crate alloc;

#[path = "../panic.rs"]
mod panic;

#[allow(dead_code)]
#[path = "../video.rs"]
mod video;

#[allow(dead_code)]
#[path = "../probe_scene.rs"]
mod probe_scene;

use engine::debug::DebugRegion;
use video::Video;

const VCOUNT: *const u16 = 0x0400_0006 as *const u16;
const VBLANK_LINE: u16 = 160;

#[used]
#[unsafe(no_mangle)]
static mut PKR_DEBUG: DebugRegion = DebugRegion::new();

fn vcount() -> u16 {
    unsafe { VCOUNT.read_volatile() }
}

/// `frame` = index of the probe frame applied at the last vblank; `late` = applies that ran
/// outside vblank.
fn publish(frame: u32, late: u32) {
    let d = core::ptr::addr_of_mut!(PKR_DEBUG);
    unsafe {
        (&raw mut (*d).late).write_volatile(late);
        (&raw mut (*d).frame).write_volatile(frame);
    }
}

#[agb::entry]
fn main(_gba: agb::Gba) -> ! {
    let mut video = Video::new();
    let mut probe = probe_scene::Probe::new();
    video.apply_blanked(&probe.next());
    publish(0, 0);
    let mut i = 0u32;
    let mut late = 0u32;
    let mut frame = probe.next();
    let vblank = agb::interrupt::VBlank::get();
    loop {
        vblank.wait_for_vblank();
        let woke_late = vcount() != VBLANK_LINE;
        video.apply(&frame);
        if woke_late || vcount() < VBLANK_LINE {
            late = late.wrapping_add(1);
        }
        i += 1;
        publish(i, late);
        // The next frame is built while this one is shown.
        frame = probe.next();
    }
}
