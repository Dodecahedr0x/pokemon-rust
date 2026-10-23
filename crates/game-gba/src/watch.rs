//! Heap and stack high-water marks for the debug region (v3, `DebugRegion::heap_max` and
//! `stack_min`).
//!
//! agb owns the `#[global_allocator]` (a crate may not declare a second one), so the heap
//! cannot be counted through a wrapper. Both are measured by painting instead: at boot, before
//! the World exists, the free heap (above agb's bump tip, up to the end of EWRAM) and the free
//! IWRAM stack (from the end of the IWRAM statics up to just below the stack pointer) are
//! filled with [`PATTERN`]. A word that no longer holds it was written since boot. Each frame,
//! in the slack after the loop body, [`Watch::scan`] walks the painted words until a deadline
//! and moves the marks:
//! - the heap mark is the highest EWRAM word ever written (agb's block allocator bumps its tip
//!   upward and reuses freed blocks below it; capacity reserved but never written is not seen);
//! - the stack mark is the lowest IWRAM word ever written (the stack grows down from
//!   `0x0300_7F00`, the BIOS's system stack).
//!
//! A pass over the painted words takes one or more frames' slack, so the published marks lag
//! the memory by at most one pass; a written word never reads as painted again, so nothing is
//! missed, only reported later.

use core::alloc::Layout;

/// What painted memory holds. Unlikely as data (not 0, not all ones, not a small integer or a
/// ROM/RAM pointer).
pub const PATTERN: u32 = 0xC5A7_3E91;
/// End of EWRAM: agb's heap ends here.
const EWRAM_END: usize = 0x0204_0000;
/// The stack paint stops this far below the stack pointer read in [`paint_stack`] (that
/// function's own frame lies above it; it calls nothing while painting).
const STACK_MARGIN: usize = 64;
/// The heap probe [`Watch::paint`] allocates to find agb's bump tip: a block this large is
/// never served from the (empty or tiny) free list at boot, so it lands on the tip.
const PROBE: usize = 4096;
/// Words scanned between two deadline checks.
const CHUNK: usize = 32;
/// Interrupt master enable.
const IME: *mut u16 = 0x0400_0208 as *mut u16;

unsafe extern "C" {
    /// End of the EWRAM statics: agb's heap starts here (`gba.ld`).
    static __ewram_data_end: u8;
    /// End of the IWRAM statics (`gba.ld`).
    static __iwram_end: u8;
}

/// The marks and the scan cursors.
pub struct Watch {
    heap_base: usize,
    /// One past the highest heap word known written (at least the painted start).
    heap_top: usize,
    /// Next heap word to check, in `heap_top..EWRAM_END`.
    heap_cursor: usize,
    stack_base: usize,
    /// The lowest stack word known written (at first, the top of the painted stack).
    stack_low: usize,
    /// Next stack word to check, in `stack_base..stack_low`.
    stack_cursor: usize,
    /// Whether the heap pass runs (else the stack pass).
    heap_turn: bool,
}

fn read(addr: usize) -> u32 {
    // SAFETY: only called on word-aligned addresses inside the painted EWRAM or IWRAM ranges.
    unsafe { (addr as *const u32).read_volatile() }
}

/// Fills `start..end` (word-aligned) with [`PATTERN`].
///
/// # Safety
/// Nothing live may lie in the range.
unsafe fn fill(start: usize, end: usize) {
    let mut a = start;
    while a < end {
        unsafe { (a as *mut u32).write_volatile(PATTERN) };
        a += 4;
    }
}

/// Paints the free stack below the current stack pointer; returns the painted range.
#[inline(never)]
fn paint_stack() -> (usize, usize) {
    let base = (core::ptr::addr_of!(__iwram_end) as usize + 3) & !3;
    let sp: usize;
    // SAFETY: reads the stack pointer.
    unsafe { core::arch::asm!("mov {}, sp", out(reg) sp, options(nomem, nostack)) };
    let top = (sp - STACK_MARGIN) & !3;
    // No interrupt handler may push a frame below `sp` while it is painted over.
    // SAFETY: IME is the interrupt master enable register; restored below.
    let ime = unsafe { IME.read_volatile() };
    unsafe { IME.write_volatile(0) };
    // SAFETY: between the IWRAM statics and the stack pointer nothing is live (the program
    // uses no IWRAM heap).
    unsafe { fill(base, top) };
    unsafe { IME.write_volatile(ime) };
    (base, top)
}

impl Watch {
    /// Paints the free heap and stack. Call first thing at boot, before anything allocates
    /// much (the World and the content tables come after).
    pub fn paint() -> Watch {
        let heap_base = core::ptr::addr_of!(__ewram_data_end) as usize;
        let layout = Layout::from_size_align(PROBE, 8).expect("probe layout");
        // SAFETY: a non-zero-sized layout; freed below.
        let probe = unsafe { alloc::alloc::alloc(layout) };
        assert!(!probe.is_null(), "heap probe failed");
        let heap_start = (probe as usize + PROBE + 3) & !3;
        // SAFETY: above the probe, which sits on the bump tip, nothing is allocated yet.
        unsafe { fill(heap_start, EWRAM_END) };
        // SAFETY: allocated above with `layout`.
        unsafe { alloc::alloc::dealloc(probe, layout) };
        let (stack_base, stack_top) = paint_stack();
        Watch {
            heap_base,
            heap_top: heap_start,
            heap_cursor: heap_start,
            stack_base,
            stack_low: stack_top,
            stack_cursor: stack_base,
            heap_turn: false,
        }
    }

    /// Heap bytes from the heap start to the highest word written (the probe counts).
    pub fn heap_max(&self) -> u32 {
        (self.heap_top - self.heap_base) as u32
    }

    /// Stack bytes never written, from the end of the IWRAM statics.
    pub fn stack_min(&self) -> u16 {
        (self.stack_low - self.stack_base).min(u16::MAX as usize) as u16
    }

    /// Moves the marks over painted words until `more()` says the slack is over (it is asked
    /// every [`CHUNK`] words). Passes alternate: a stack pass walks up from the bottom and ends
    /// at the first written word (the new mark) or at the mark; a heap pass walks from the mark
    /// to the end of EWRAM.
    pub fn scan(&mut self, mut more: impl FnMut() -> bool) {
        while more() {
            if self.heap_turn {
                let end = (self.heap_cursor + 4 * CHUNK).min(EWRAM_END);
                while self.heap_cursor < end {
                    if read(self.heap_cursor) != PATTERN {
                        self.heap_top = self.heap_cursor + 4;
                    }
                    self.heap_cursor += 4;
                }
                if self.heap_cursor >= EWRAM_END {
                    self.heap_cursor = self.heap_top;
                    self.heap_turn = false;
                }
            } else {
                let end = (self.stack_cursor + 4 * CHUNK).min(self.stack_low);
                while self.stack_cursor < end {
                    if read(self.stack_cursor) != PATTERN {
                        self.stack_low = self.stack_cursor;
                        break;
                    }
                    self.stack_cursor += 4;
                }
                if self.stack_cursor >= self.stack_low {
                    self.stack_cursor = self.stack_base;
                    self.heap_turn = true;
                }
            }
        }
    }
}
