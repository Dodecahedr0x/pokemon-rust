#![no_std]
#![no_main]

extern crate alloc;

mod video;

use content::Emerald;
use engine::debug::{DebugRegion, input_slot};
use engine::{Buttons, World};
use video::Video;

const KEYINPUT: *const u16 = 0x0400_0130 as *const u16;
const VCOUNT: *const u16 = 0x0400_0006 as *const u16;
/// First vblank line; the vblank interrupt fires as it starts.
const VBLANK_LINE: u16 = 160;

// Initialised static => .data => EWRAM (agb gba.ld). The emulator finds it by MAGIC.
#[used]
#[unsafe(no_mangle)]
static mut PKR_DEBUG: DebugRegion = DebugRegion::new();

// The black-screen test below (`late_black`, `Frame::is_black`) is sound only if BLDY at
// `BRIGHTNESS_BLACK` blackens every pixel: brightness decrease must target BG0-3, OBJ and the
// backdrop (BLDCNT bits 0-5, mode 3; `lighten` frames, mode 2, are never black), no window may exclude anything from the effect (DISPCNT bits 13-15 clear;
// the shell never writes WININ/WINOUT), and no OBJ may be semi-transparent (OBJ mode 1 would
// alpha-blend instead; `video::oam_entry` always writes mode 0).
const _: () = {
    assert!(video::BLDCNT_DARKEN_ALL == 0x3F | (3 << 6));
    assert!(video::BLDCNT_LIGHTEN_ALL == 0x3F | (2 << 6));
    assert!(video::DISPCNT_BASE & (7 << 13) == 0);
};

fn read_keys() -> Buttons {
    Buttons::from_raw(!unsafe { KEYINPUT.read_volatile() })
}

fn vcount() -> u16 {
    unsafe { VCOUNT.read_volatile() }
}

fn publish_debug(world: &World, keys: Buttons, late: u32, late_black: u32) {
    let d = core::ptr::addr_of_mut!(PKR_DEBUG);
    unsafe {
        (&raw mut (*d).inputs[input_slot(world.frame())]).write_volatile(keys.0);
        (&raw mut (*d).hash).write_volatile(world.hash());
        (&raw mut (*d).late).write_volatile(late);
        (&raw mut (*d).late_black).write_volatile(late_black);
        (&raw mut (*d).frame).write_volatile(world.frame());
    }
}

#[agb::entry]
fn main(_gba: agb::Gba) -> ! {
    let game = Emerald::default();
    let mut video = Video::new();
    let world = &mut World::new(&game);
    let mut frame = world.initial_frame();
    video.apply_blanked(&frame);
    // Its commands are done; the loop re-applies only its registers.
    frame.cmds.clear();
    let (mut late, mut late_black) = (0u32, 0u32);
    // Whether the last applied frame left the screen fully black.
    let mut black = frame.is_black();
    // Frame 0 publishes the real initial hash rather than the static's placeholder 0.
    publish_debug(world, Buttons::NONE, late, late_black);
    let vblank = agb::interrupt::VBlank::get();
    loop {
        vblank.wait_for_vblank();
        // On time, we wake at the start of vblank. Anywhere else means the last step overran
        // and agb's catch-up returned at once, so this apply lands mid-frame.
        let woke_late = vcount() != VBLANK_LINE;
        // Shows the previous step's Frame (one-frame display lag, see engine::debug).
        video.apply(&frame);
        // Vblank lasts lines 160..228; back in 0..160 means the apply ran into the picture.
        if woke_late || vcount() < VBLANK_LINE {
            // Black before and after (map loads while faded out): nothing visible differs.
            let now_black = frame.is_black();
            if black && now_black {
                late_black = late_black.wrapping_add(1);
            } else {
                late = late.wrapping_add(1);
            }
        }
        black = frame.is_black();
        let keys = read_keys();
        frame = world.step(&game, keys);
        publish_debug(world, keys, late, late_black);
    }
}
