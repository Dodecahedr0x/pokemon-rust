#![no_std]
#![no_main]

extern crate alloc;

use content::Emerald;
use engine::debug::{DebugRegion, input_slot};
use engine::scene::MAP_TILES;
use engine::{Buttons, Content, World};

const DISPCNT: *mut u16 = 0x0400_0000 as *mut u16;
const BG0CNT: *mut u16 = 0x0400_0008 as *mut u16;
const BG0HOFS: *mut u16 = 0x0400_0010 as *mut u16;
const BG0VOFS: *mut u16 = 0x0400_0012 as *mut u16;
const KEYINPUT: *const u16 = 0x0400_0130 as *const u16;
const BG_PALETTE: *mut u16 = 0x0500_0000 as *mut u16;
const CHARBLOCK0: *mut u32 = 0x0600_0000 as *mut u32;
const SCREENBLOCK31: *mut u16 = 0x0600_F800 as *mut u16;

const DISPCNT_FORCED_BLANK: u16 = 1 << 7;
const DISPCNT_MODE0_BG0: u16 = 1 << 8;
const BG0CNT_CB0_SB31_4BPP_256: u16 = 31 << 8;

// Initialised static => .data => EWRAM (agb gba.ld). The emulator finds it by MAGIC.
#[used]
#[unsafe(no_mangle)]
static mut PKR_DEBUG: DebugRegion = DebugRegion::new();

fn upload_scene(content: &impl Content) {
    let scene = content.scene();
    unsafe {
        DISPCNT.write_volatile(DISPCNT_FORCED_BLANK);
        for (i, &c) in scene.palette.iter().enumerate() {
            BG_PALETTE.add(i).write_volatile(c);
        }
        for (t, tile) in scene.tiles.iter().enumerate() {
            for (r, &row) in tile.iter().enumerate() {
                CHARBLOCK0.add(t * 8 + r).write_volatile(row);
            }
        }
        for i in 0..MAP_TILES * MAP_TILES {
            SCREENBLOCK31.add(i).write_volatile(scene.map[i]);
        }
        BG0CNT.write_volatile(BG0CNT_CB0_SB31_4BPP_256);
        DISPCNT.write_volatile(DISPCNT_MODE0_BG0);
    }
}

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
    upload_scene(&content);
    let vblank = agb::interrupt::VBlank::get();
    let mut world = World::new();
    let mut out = world.frame_out();
    // Frame 0 publishes the real initial hash rather than the static's placeholder 0.
    publish_debug(&world, Buttons::NONE);
    loop {
        vblank.wait_for_vblank();
        // Shows the previous step's FrameOut (one-frame display lag, see engine::debug); the step
        // must finish within one frame or agb's vblank catch-up lands these writes mid-frame.
        unsafe {
            BG0HOFS.write_volatile(out.bg0_scroll.0 & 0x1FF);
            BG0VOFS.write_volatile(out.bg0_scroll.1 & 0x1FF);
        }
        let keys = read_keys();
        out = world.step(keys);
        publish_debug(&world, keys);
    }
}
