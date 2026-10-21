#![no_std]
#![no_main]

extern crate alloc;

mod video;

use content::Emerald;
use engine::debug::{DebugRegion, input_slot};
use engine::{Buttons, World};
use video::Video;

const KEYINPUT: *const u16 = 0x0400_0130 as *const u16;

// Initialised static => .data => EWRAM (agb gba.ld). The emulator finds it by MAGIC.
#[used]
#[unsafe(no_mangle)]
static mut PKR_DEBUG: DebugRegion = DebugRegion::new();

fn read_keys() -> Buttons {
    Buttons::from_raw(!unsafe { KEYINPUT.read_volatile() })
}

fn publish_debug(world: &World, keys: Buttons) {
    let d = core::ptr::addr_of_mut!(PKR_DEBUG);
    unsafe {
        (&raw mut (*d).inputs[input_slot(world.frame())]).write_volatile(keys.0);
        (&raw mut (*d).hash).write_volatile(world.hash());
        (&raw mut (*d).frame).write_volatile(world.frame());
    }
}

#[agb::entry]
fn main(_gba: agb::Gba) -> ! {
    let content = Emerald;
    let mut video = Video::new();
    let world = &mut World::new();
    let mut frame = world.initial_frame(&content);
    video.apply_blanked(&frame);
    // Its commands are done; the loop re-applies only its registers.
    frame.cmds.clear();
    // Frame 0 publishes the real initial hash rather than the static's placeholder 0.
    publish_debug(world, Buttons::NONE);
    let vblank = agb::interrupt::VBlank::get();
    loop {
        vblank.wait_for_vblank();
        // Shows the previous step's Frame (one-frame display lag, see engine::debug); the step
        // must finish within one frame or agb's vblank catch-up lands these writes mid-frame.
        video.apply(&frame);
        let keys = read_keys();
        frame = world.step(keys);
        publish_debug(world, keys);
    }
}
