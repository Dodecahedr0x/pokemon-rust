#![no_std]
#![no_main]
// `watch.rs` probes agb's IWRAM heap through `agb::InternalAllocator`.
#![feature(allocator_api)]

extern crate alloc;

mod panic;
mod save;
mod save_logic;
#[cfg(feature = "tour")]
mod tour_logic;
mod video;
mod watch;

use content::Emerald;
use engine::checkpoint::{BuildHash, build_hash};
use engine::debug::{self, DebugRegion, NO_SCRIPT, input_slot};
use engine::world::Event;
use engine::{Buttons, World};
use save_logic::{PendingSave, combo_pressed, scanlines};
use video::Video;
use watch::Watch;

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

/// Ticks of timer 3 (divider 256, 65,536 Hz) in a scanline: 1,232 cycles / 256.
const TICKS_PER_LINE: u32 = 1232;
/// [`Watch::scan`] stops this many scanlines after the vblank wake (on time, the next one is at
/// 228): 8 lines of margin, and a scan chunk takes well under one.
const SCAN_UNTIL_LINES: u32 = 220;

/// Ticks of timer 3 from `start` to `now` (it wraps every second).
fn ticks(start: u16, now: u16) -> u32 {
    now.wrapping_sub(start) as u32
}

/// Publishes step `world.frame()` (frame last = commit). Returns the scanlines `World::hash`
/// took.
fn publish_debug(world: &World, keys: Buttons, late: u32, late_black: u32) -> u32 {
    let d = core::ptr::addr_of_mut!(PKR_DEBUG);
    let start = vcount();
    let hash = world.hash();
    let hash_lines = scanlines(start, vcount());
    let script = world.last_script().map_or(NO_SCRIPT, |s| s.0);
    let dropped = world.events_dropped().min(u16::MAX as u32) as u16;
    unsafe {
        (&raw mut (*d).last_script).write_volatile(script);
        (&raw mut (*d).map).write_volatile(world.map().0);
        (&raw mut (*d).events_dropped).write_volatile(dropped);
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

/// `Event::Report`s of the step just run, added to the saturating count.
fn publish_reports(world: &World, reports: &mut u16) {
    let n = (world.events().iter())
        .filter(|e| matches!(e, Event::Report(_)))
        .count();
    *reports = reports.saturating_add(n.min(u16::MAX as usize) as u16);
    let d = core::ptr::addr_of_mut!(PKR_DEBUG);
    unsafe { (&raw mut (*d).reports).write_volatile(*reports) };
}

fn publish_watch(watch: &Watch) {
    let d = core::ptr::addr_of_mut!(PKR_DEBUG);
    unsafe {
        (&raw mut (*d).heap_max).write_volatile(watch.heap_max());
        (&raw mut (*d).stack_min).write_volatile(watch.stack_min());
        (&raw mut (*d).scan_passes).write_volatile(watch.passes());
    }
}

fn publish_save(saves: u32, status: u32) {
    let d = core::ptr::addr_of_mut!(PKR_DEBUG);
    unsafe {
        (&raw mut (*d).saves).write_volatile(saves);
        (&raw mut (*d).save_status).write_volatile(status);
    }
}

/// Worst scanline costs on visible frames (see `DebugRegion::hash_lines_max`): meaningful
/// only when `late == 0`, since each is measured mod one frame.
#[derive(Default)]
struct Costs {
    hash: u32,
    step: u32,
    whole: u32,
}

impl Costs {
    /// Starts over (the tour, on each warp: every map gets its own maxima).
    #[cfg(feature = "tour")]
    fn reset(&mut self) {
        *self = Costs::default();
        let d = core::ptr::addr_of_mut!(PKR_DEBUG);
        unsafe {
            (&raw mut (*d).hash_lines_max).write_volatile(0);
            (&raw mut (*d).step_lines_max).write_volatile(0);
            (&raw mut (*d).loop_lines_max).write_volatile(0);
        }
    }

    fn record(&mut self, hash: u32, step: u32, whole: u32) {
        if hash <= self.hash && step <= self.step && whole <= self.whole {
            return;
        }
        self.hash = self.hash.max(hash);
        self.step = self.step.max(step);
        self.whole = self.whole.max(whole);
        let d = core::ptr::addr_of_mut!(PKR_DEBUG);
        unsafe {
            (&raw mut (*d).hash_lines_max).write_volatile(self.hash);
            (&raw mut (*d).step_lines_max).write_volatile(self.step);
            (&raw mut (*d).loop_lines_max).write_volatile(self.whole);
        }
    }
}

/// The game the shell plays; the map tour starts after the starter (a party, so the trainers it
/// meets can battle), as `cargo xtask sweep` does.
#[cfg(not(feature = "tour"))]
const GAME: Emerald = Emerald::for_shells();
#[cfg(feature = "tour")]
const GAME: Emerald = Emerald::after_starter();

#[agb::entry]
fn main(mut gba: agb::Gba) -> ! {
    // First, before the World and the content allocate: the heap and stack marks.
    let mut watch = Watch::paint();
    let game = GAME;
    let mut video = Video::new();
    let timers = gba.timers.timers();
    let mut clock = timers.timer3;
    clock
        .set_divider(agb::timer::Divider::Divider256)
        .set_enabled(true);
    let mut slots = save::init(&mut gba.save, timers.timer2);
    let boot_keys = read_keys();
    let (mut world, boot) = save::boot_world(&game, slots.as_mut(), boot_keys);
    let world = &mut world;
    let mut frame = world.initial_frame();
    video.apply_blanked(&frame);
    // Its commands are done; the loop re-applies only its registers.
    frame.cmds.clear();
    // Boot order on the debug region: build and boot, then the boot World's hash, `frame`
    // last. The hash stays 0 until then (the emu takes the first non-zero hash as the boot).
    publish_boot(build_hash(&game), boot);
    let (mut late, mut late_black) = (0u32, 0u32);
    let mut costs = Costs::default();
    let mut saves = 0u32;
    let mut pending: Option<PendingSave> = None;
    // Keys held at boot never complete the combo.
    let mut prev_keys = boot_keys;
    // Whether the last applied frame left the screen fully black.
    let mut black = frame.is_black();
    // Frame 0 (or the restored frame) publishes the real hash rather than the static's 0.
    publish_debug(world, Buttons::NONE, late, late_black);
    publish_watch(&watch);
    let mut reports = 0u16;
    #[cfg(feature = "tour")]
    let mut tour = tour_logic::Tour::new(&game, &content::data::MAP_IDS);
    let vblank = agb::interrupt::VBlank::get();
    loop {
        vblank.wait_for_vblank();
        let woke = vcount();
        let woke_ticks = clock.value();
        // On time, we wake at the start of vblank. Anywhere else means the last step overran
        // and agb's catch-up returned at once, so this apply lands mid-frame.
        let woke_late = woke != VBLANK_LINE;
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
        publish_reports(world, &mut reports);
        // The tour's warp lands between two steps (its mirror in `emu/tests/tour.rs` does the
        // same), and each map gets its own maxima.
        #[cfg(feature = "tour")]
        if tour.on_keys(&game, world, prev_keys, keys) {
            costs.reset();
        }
        let hash_lines = publish_debug(world, keys, late, late_black);
        let step_lines = scanlines(applied, vcount());
        let mut wrote = false;

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
                    wrote = true;
                    // The picture stayed frozen on the last applied frame during the write,
                    // over many vblanks. One wait drains agb's catch-up (it returns at once),
                    // so the loop's own wait is a real one and lands on `VBLANK_LINE`.
                    vblank.wait_for_vblank();
                }
                Err(_) => {
                    if wait.tick() {
                        pending = None;
                        publish_save(saves, debug::save::TIMEOUT);
                    }
                }
            }
        }
        // Costs count on visible frames only (bulk work such as map loads runs while black),
        // and never across a flash write.
        if !black && !wrote {
            costs.record(hash_lines, step_lines, scanlines(woke, vcount()));
        }
        // The marks, in the slack left before the next vblank: not on a frame that woke late
        // or wrote the flash, and never past `SCAN_UNTIL_LINES` (timer 3, which does not wrap
        // within a frame as `VCOUNT` does).
        if !woke_late && !wrote {
            let until = SCAN_UNTIL_LINES * TICKS_PER_LINE / 256;
            watch.scan(|| ticks(woke_ticks, clock.value()) < until);
            publish_watch(&watch);
        }
    }
}
