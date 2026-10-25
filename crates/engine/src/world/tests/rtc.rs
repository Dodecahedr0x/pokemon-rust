//! `gettime` (Gen3 `ScrCmd_gettime`): hours, minutes and seconds of the local clock into
//! `VAR_0x8000`..`VAR_0x8002`.

use super::*;
use crate::script::Ctx;

const VAR_HOURS: VarId = VarId(0x8000);
const VAR_MINUTES: VarId = VarId(0x8001);
const VAR_SECONDS: VarId = VarId(0x8002);

/// 60 frames a second, the deterministic clock's tick ([`FRAMES_PER_DAY`]).
fn frame_at(days: u32, hours: u32, minutes: u32, seconds: u32) -> u32 {
    days * FRAMES_PER_DAY + 60 * (hours * 3600 + minutes * 60 + seconds)
}

fn stale(w: &mut World) {
    w.s.vars.set_var(VAR_HOURS, 0xFFFF);
    w.s.vars.set_var(VAR_MINUTES, 0xFFFF);
    w.s.vars.set_var(VAR_SECONDS, 0xFFFF);
    w.s.vars.set_var(VAR_RESULT, 0x7777);
}

fn run_gettime(w: &mut World) {
    let mut f = Frame::default();
    let mut runner = Runner::new();
    runner.start(Box::pin(gettime(runner.ctx())));
    let mut host = Host {
        game: &Game,
        s: &mut w.s,
        f: &mut f,
        immediate: false,
    };
    assert!(!runner.step(&mut host), "gettime finished");
}

async fn gettime(ctx: Ctx) {
    ctx.gettime().await;
}

fn assert_time(w: &World, hours: u16, minutes: u16, seconds: u16) {
    assert_eq!(w.var(VAR_HOURS), hours, "VAR_0x8000 hours");
    assert_eq!(w.var(VAR_MINUTES), minutes, "VAR_0x8001 minutes");
    assert_eq!(w.var(VAR_SECONDS), seconds, "VAR_0x8002 seconds");
    assert_eq!(
        w.var(VAR_RESULT),
        0x7777,
        "gettime does not write VAR_RESULT"
    );
}

#[test]
fn gettime_writes_a_known_local_time() {
    let mut w = World::new(&Game);
    // The shell's RTC, the way a cartridge clock is read: 15:42:07, not the frame clock.
    stale(&mut w);
    w.set_rtc(Some(Rtc {
        days: 12,
        hours: 15,
        minutes: 42,
        seconds: 7,
    }));
    run_gettime(&mut w);
    assert_time(&w, 15, 42, 7);
    assert_eq!(w.days(), 12);

    // No shell clock: the deterministic clock. Frame 3:05:09 on day 0, then 01:02:03 on day 1.
    stale(&mut w);
    w.frame = frame_at(0, 3, 5, 9);
    w.set_rtc(None);
    run_gettime(&mut w);
    assert_time(&w, 3, 5, 9);
    assert_eq!(w.days(), 0);

    stale(&mut w);
    w.frame = frame_at(1, 1, 2, 3);
    w.set_rtc(None);
    run_gettime(&mut w);
    assert_time(&w, 1, 2, 3);
    assert_eq!(w.days(), 1);
}
