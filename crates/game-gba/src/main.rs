#![no_std]
#![no_main]

extern crate alloc;

mod save;
mod save_logic;
mod video;

use content::Emerald;
use engine::checkpoint::{BuildHash, build_hash};
use engine::debug::{self, DebugRegion, input_slot};
use engine::{Buttons, World};
use save_logic::{PendingSave, combo_pressed, scanlines};
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
// backdrop (BLDCNT bits 0-5, mode 3; `lighten` frames, mode 2, are never black), no window may
// exclude anything from the effect (DISPCNT bits 13-15 clear; the shell never writes
// WININ/WINOUT), and no OBJ may be semi-transparent (OBJ mode 1 would alpha-blend instead;
// `video::oam_entry` always writes mode 0).
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

/// Publishes step `world.frame()` (frame last = commit). Returns the scanlines `World::hash`
/// took.
fn publish_debug(world: &World, keys: Buttons, late: u32, late_black: u32) -> u32 {
    let d = core::ptr::addr_of_mut!(PKR_DEBUG);
    let start = vcount();
    let hash = world.hash();
    let hash_lines = scanlines(start, vcount());
    unsafe {
        (&raw mut (*d).inputs[input_slot(world.frame())]).write_volatile(keys.0);
        (&raw mut (*d).hash).write_volatile(hash);
        (&raw mut (*d).late).write_volatile(late);
        (&raw mut (*d).late_black).write_volatile(late_black);
        (&raw mut (*d).frame).write_volatile(world.frame());
    }
    hash_lines
}

/// Written once, before the first frame is published.
fn publish_boot(build: BuildHash, boot: u32) {
    let d = core::ptr::addr_of_mut!(PKR_DEBUG);
    unsafe {
        (&raw mut (*d).build_engine).write_volatile(build.engine);
        (&raw mut (*d).build_content).write_volatile(build.content);
        (&raw mut (*d).boot).write_volatile(boot);
    }
}

fn publish_save(saves: u32, status: u32) {
    let d = core::ptr::addr_of_mut!(PKR_DEBUG);
    unsafe {
        (&raw mut (*d).saves).write_volatile(saves);
        (&raw mut (*d).save_status).write_volatile(status);
    }
}

fn publish_costs(hash_lines_max: u32, step_lines_max: u32) {
    let d = core::ptr::addr_of_mut!(PKR_DEBUG);
    unsafe {
        (&raw mut (*d).hash_lines_max).write_volatile(hash_lines_max);
        (&raw mut (*d).step_lines_max).write_volatile(step_lines_max);
    }
}

/// After a flash write (many frames with the picture frozen): drain agb's vblank catch-up
/// until a wait really lands at the start of vblank, so the next apply is on time.
fn resync(vblank: &agb::interrupt::VBlank) {
    loop {
        vblank.wait_for_vblank();
        if vcount() == VBLANK_LINE {
            return;
        }
    }
}

#[agb::entry]
fn main(mut gba: agb::Gba) -> ! {
    let game = Emerald::for_shells();
    let mut video = Video::new();
    let mut slots = save::init(&mut gba);
    let boot_keys = read_keys();
    let (mut world, boot) = save::boot_world(&game, slots.as_mut(), boot_keys);
    let world = &mut world;
    let mut frame = world.initial_frame();
    video.apply_blanked(&frame);
    // Its commands are done; the loop re-applies only its registers.
    frame.cmds.clear();
    publish_boot(build_hash(&game), boot);
    let (mut late, mut late_black) = (0u32, 0u32);
    let (mut hash_lines_max, mut step_lines_max) = (0u32, 0u32);
    let mut saves = 0u32;
    let mut pending: Option<PendingSave> = None;
    // Keys held at boot never complete the combo.
    let mut prev_keys = boot_keys;
    // Whether the last applied frame left the screen fully black.
    let mut black = frame.is_black();
    // Frame 0 (or the restored frame) publishes the real hash rather than the static's 0.
    publish_debug(world, Buttons::NONE, late, late_black);
    let vblank = agb::interrupt::VBlank::get();
    // Set by a flash write's resync: we already stand at the start of vblank.
    let mut synced = false;
    loop {
        if !synced {
            vblank.wait_for_vblank();
        }
        synced = false;
        // On time, we wake at the start of vblank. Anywhere else means the last step overran
        // and agb's catch-up returned at once, so this apply lands mid-frame.
        let woke_late = vcount() != VBLANK_LINE;
        // Shows the previous step's Frame (one-frame display lag, see engine::debug).
        video.apply(&frame);
        let applied = vcount();
        // Vblank lasts lines 160..228; back in 0..160 means the apply ran into the picture.
        if woke_late || applied < VBLANK_LINE {
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
        let hash_lines = publish_debug(world, keys, late, late_black);
        // Costs count on visible frames only: bulk work (map loads) runs while black.
        if !black {
            let step_lines = scanlines(applied, vcount());
            if hash_lines > hash_lines_max || step_lines > step_lines_max {
                hash_lines_max = hash_lines_max.max(hash_lines);
                step_lines_max = step_lines_max.max(step_lines);
                publish_costs(hash_lines_max, step_lines_max);
            }
        }

        // Debug save (L+R+SELECT), outside the simulation: the World never sees it.
        if combo_pressed(prev_keys, keys) {
            pending = Some(PendingSave::new());
        }
        prev_keys = keys;
        if let Some(wait) = pending.as_mut() {
            match world.checkpoint(&game) {
                Ok(cp) => {
                    pending = None;
                    let status = match slots.as_mut() {
                        Some(slots) => save::write(slots, &cp),
                        None => debug::save::STORAGE,
                    };
                    if status == debug::save::OK {
                        saves = saves.wrapping_add(1);
                    }
                    publish_save(saves, status);
                    // The picture stayed frozen on the last applied frame during the write.
                    resync(&vblank);
                    synced = true;
                }
                Err(_) => {
                    if wait.tick() {
                        pending = None;
                        publish_save(saves, debug::save::TIMEOUT);
                    }
                }
            }
        }
    }
}
