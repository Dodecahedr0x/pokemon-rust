//! The panic handler of the game and of the probe ROMs (each includes this file and defines
//! its own `PKR_DEBUG`). agb's handler, with its backtrace screen, is off
//! (`default-features = false`): this one prints the message to mGBA's debug log, writes the
//! panic location's line in the debug region (`DebugRegion::panicked`, saturating, never 0;
//! `u16::MAX` without a location), then halts.

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    agb::eprintln!("{info}");
    let line = info
        .location()
        .map_or(u16::MAX, |l| l.line().clamp(1, u16::MAX as u32) as u16);
    let d = core::ptr::addr_of_mut!(crate::PKR_DEBUG);
    unsafe { (&raw mut (*d).panicked).write_volatile(line) };
    loop {
        agb::halt();
    }
}
