//! A pane: a game (native session or mGBA) owned by its own thread, driven by commands and
//! publishing a framebuffer and a status (plan: "pane.rs"). The trait and `RunMode` are the
//! Wave 0 contract; the Wave 0 spike adds `spawn`, `PaneHandle`, `StatusSlot` and `Pacer`.

use crate::frames::FrameSlot;
use engine::video::{SCREEN_H, SCREEN_W};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How a pane advances on each 60 Hz tick.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunMode {
    Play,
    Pause,
    /// `n` frames per tick, `2..=16`.
    Fast(u8),
}

/// Lives on its pane thread, so it need not be `Send`.
pub trait Pane {
    type Cmd: Send + 'static;
    type Status: Clone + Send + 'static;
    fn handle(&mut self, cmd: Self::Cmd);
    /// One 60 Hz tick: steps 0..n frames depending on the run mode.
    fn tick(&mut self);
    /// The last stepped frame, RGB555, `SCREEN_W * SCREEN_H`.
    fn framebuffer(&mut self) -> &[u16];
    fn status(&self) -> Self::Status;
}

/// The latest status of a pane, written by its thread and polled by the UI.
pub struct StatusSlot<S>(Arc<Mutex<S>>);

impl<S> Clone for StatusSlot<S> {
    fn clone(&self) -> Self {
        StatusSlot(self.0.clone())
    }
}

impl<S: Clone> StatusSlot<S> {
    pub fn new(s: S) -> Self {
        StatusSlot(Arc::new(Mutex::new(s)))
    }
    pub fn set(&self, s: S) {
        *self.0.lock().unwrap() = s;
    }
    pub fn get(&self) -> S {
        self.0.lock().unwrap().clone()
    }
}

/// The UI side of a spawned pane. Dropping it without [`stop`](Self::stop) also ends the
/// thread (its command channel closes), without waiting for it.
pub struct PaneHandle<C, S> {
    pub frames: FrameSlot,
    pub status: StatusSlot<S>,
    tx: Sender<C>,
    join: JoinHandle<()>,
}

impl<C, S> PaneHandle<C, S> {
    /// Queues a command; it is handled before the pane's next tick. A no-op once the pane
    /// thread has ended (e.g. after a panic in the pane).
    pub fn send(&self, c: C) {
        let _ = self.tx.send(c);
    }

    /// Closes the command channel and waits for the thread to finish its current tick.
    pub fn stop(self) {
        drop(self.tx);
        let _ = self.join.join();
    }

    /// Whether the pane thread has ended (stopped or panicked).
    pub fn is_finished(&self) -> bool {
        self.join.is_finished()
    }
}

/// Spawns a thread that builds the pane with `make` (so !Send state is born there), then loops:
/// drain commands, tick, publish frame + status, sleep to the next 1/60 s ([`Pacer`]).
///
/// The frame is published only when its pixels changed (a paused pane sends nothing, so its
/// seq stays put); the status is published every tick. The first frame and status are
/// published before the first tick. Commands arriving while the thread sleeps are handled
/// at once; the tick still waits for its deadline.
pub fn spawn<P: Pane, F: FnOnce() -> P + Send + 'static>(
    name: &str,
    make: F,
) -> PaneHandle<P::Cmd, P::Status>
where
    P::Status: Default,
{
    let frames = FrameSlot::default();
    let status = StatusSlot::new(P::Status::default());
    let (tx, rx) = mpsc::channel::<P::Cmd>();
    let (f, s) = (frames.clone(), status.clone());
    let join = std::thread::Builder::new()
        .name(format!("pane:{name}"))
        .spawn(move || {
            let mut pane = make();
            let mut last: Vec<u16> = Vec::new();
            let mut publish = |pane: &mut P| {
                let fb = pane.framebuffer();
                if fb != last.as_slice() {
                    f.publish(fb);
                    last.clear();
                    last.extend_from_slice(fb);
                }
                s.set(pane.status());
            };
            publish(&mut pane);
            let mut pacer = Pacer::new(60);
            loop {
                let deadline = Instant::now() + pacer.next(Instant::now());
                loop {
                    let wait = deadline.saturating_duration_since(Instant::now());
                    match rx.recv_timeout(wait) {
                        Ok(cmd) => pane.handle(cmd),
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
                pane.tick();
                publish(&mut pane);
            }
        })
        .expect("spawning a pane thread");
    PaneHandle {
        frames,
        status,
        tx,
        join,
    }
}

/// Fixed-rate deadlines for a pane loop. After a stall the loop catches up at most one
/// period (one immediate tick), then keeps the rate from there instead of bursting.
#[derive(Clone, Debug)]
pub struct Pacer {
    period: Duration,
    next: Instant,
}

impl Pacer {
    pub fn new(hz: u32) -> Self {
        Self::starting_at(hz, Instant::now())
    }

    /// A pacer whose first deadline is `start` + one period.
    pub fn starting_at(hz: u32, start: Instant) -> Self {
        Pacer {
            period: Duration::from_secs(1) / hz.max(1),
            next: start,
        }
    }

    /// Advances to the next deadline and returns how long to sleep until it (zero when late).
    /// Never accumulates more than one period of debt.
    pub fn next(&mut self, now: Instant) -> Duration {
        self.next += self.period;
        if let Some(floor) = now.checked_sub(self.period)
            && self.next < floor
        {
            self.next = floor;
        }
        self.next.saturating_duration_since(now)
    }
}

/// The spike pane: a gradient that scrolls one pixel per stepped frame.
pub struct TestPattern {
    run: RunMode,
    frame: u32,
    fb: Vec<u16>,
}

/// [`TestPattern`]'s status.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TestPatternStatus {
    pub frame: u32,
}

impl Default for TestPattern {
    fn default() -> Self {
        Self::new()
    }
}

impl TestPattern {
    pub fn new() -> Self {
        TestPattern {
            run: RunMode::Play,
            frame: 0,
            fb: vec![0; SCREEN_W * SCREEN_H],
        }
    }
}

impl Pane for TestPattern {
    type Cmd = RunMode;
    type Status = TestPatternStatus;
    fn handle(&mut self, cmd: RunMode) {
        self.run = cmd;
    }
    fn tick(&mut self) {
        self.frame += match self.run {
            RunMode::Play => 1,
            RunMode::Pause => 0,
            RunMode::Fast(n) => n as u32,
        };
    }
    fn framebuffer(&mut self) -> &[u16] {
        let f = self.frame as usize;
        for (y, row) in self.fb.chunks_exact_mut(SCREEN_W).enumerate() {
            for (x, px) in row.iter_mut().enumerate() {
                let r = ((x + f) % SCREEN_W * 32 / SCREEN_W) as u16;
                let g = (y * 32 / SCREEN_H) as u16;
                let b = ((x + y + 2 * f) / 8 % 32) as u16;
                *px = r | (g << 5) | (b << 10);
            }
        }
        &self.fb
    }
    fn status(&self) -> TestPatternStatus {
        TestPatternStatus { frame: self.frame }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: Duration = Duration::from_nanos(16_666_666);

    #[test]
    fn pacer_sleeps_to_each_deadline() {
        let t0 = Instant::now();
        let mut p = Pacer::starting_at(60, t0);
        assert_eq!(p.next(t0), P);
        // Woke at the deadline, ticked for 4 ms.
        assert_eq!(
            p.next(t0 + P + Duration::from_millis(4)),
            P - Duration::from_millis(4)
        );
        // Deadlines do not drift with the wake-up time.
        assert_eq!(p.next(t0 + 2 * P), P);
    }

    #[test]
    fn pacer_debt_is_at_most_one_period() {
        let t0 = Instant::now();
        let mut p = Pacer::starting_at(60, t0);
        assert_eq!(p.next(t0), P);
        // The tick due at t0 + P overran to t0 + 2.5P: the tick due at t0 + 2P runs at once...
        assert_eq!(p.next(t0 + 2 * P + P / 2), Duration::ZERO);
        // ...and the old rhythm resumes (due at t0 + 3P).
        assert_eq!(p.next(t0 + 2 * P + P / 2), P / 2);

        // A 1 s stall: the late tick plus one catch-up tick run at once, not 60.
        let mut p = Pacer::starting_at(60, t0);
        assert_eq!(p.next(t0), P);
        let stall = t0 + Duration::from_secs(1);
        assert_eq!(p.next(stall), Duration::ZERO);
        assert_eq!(p.next(stall), Duration::ZERO, "one period of debt");
        assert_eq!(
            p.next(stall),
            P,
            "the rest of the debt was dropped, not replayed"
        );
        assert_eq!(p.next(stall + P), P);
    }

    /// A pane recording what the thread did to it.
    struct Fake {
        log: Arc<Mutex<Vec<String>>>,
        fb: Vec<u16>,
        ticks: u32,
    }

    impl Pane for Fake {
        type Cmd = u16;
        type Status = u32;
        fn handle(&mut self, cmd: u16) {
            self.log.lock().unwrap().push(format!("cmd {cmd}"));
            self.fb.fill(cmd);
        }
        fn tick(&mut self) {
            self.ticks += 1;
        }
        fn framebuffer(&mut self) -> &[u16] {
            &self.fb
        }
        fn status(&self) -> u32 {
            self.ticks
        }
    }

    fn wait_until(what: &str, mut f: impl FnMut() -> bool) {
        let end = Instant::now() + Duration::from_secs(10);
        while !f() {
            assert!(Instant::now() < end, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn a_spawned_pane_handles_commands_and_publishes() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let l = log.clone();
        // `make` runs on the pane thread: an Rc (!Send) can live in it.
        let h = spawn("fake", move || {
            let not_send = std::rc::Rc::new(());
            l.lock()
                .unwrap()
                .push(format!("made {}", std::rc::Rc::strong_count(&not_send)));
            Fake {
                log: l,
                fb: vec![0; SCREEN_W * SCREEN_H],
                ticks: 0,
            }
        });
        wait_until("the first frame", || h.frames.seq() == 1);
        wait_until("ticks", || h.status.get() >= 3);
        // An unchanged framebuffer is not republished.
        assert_eq!(h.frames.seq(), 1);

        h.send(31);
        wait_until("the red frame", || {
            h.frames
                .latest(1)
                .is_some_and(|(_, rgba)| rgba[..4] == [255, 0, 0, 255])
        });
        let seq = h.frames.seq();
        h.send(31 << 5);
        wait_until("the green frame", || {
            h.frames
                .latest(seq)
                .is_some_and(|(_, rgba)| rgba[..4] == [0, 255, 0, 255])
        });

        let ticks = h.status.get();
        assert!(!h.is_finished());
        h.stop();
        assert_eq!(
            *log.lock().unwrap(),
            ["made 1", "cmd 31", &format!("cmd {}", 31 << 5)]
        );
        assert!(ticks >= 3);
    }

    #[test]
    fn a_spawned_pane_ticks_at_about_60_hz() {
        let h = spawn("pattern", TestPattern::new);
        let t0 = Instant::now();
        let f0 = h.status.get().frame;
        std::thread::sleep(Duration::from_millis(500));
        let frames = h.status.get().frame - f0;
        let hz = frames as f64 / t0.elapsed().as_secs_f64();
        h.send(RunMode::Pause);
        h.stop();
        // Loose bounds: CI machines are noisy.
        assert!((20.0..=70.0).contains(&hz), "{hz} Hz");
    }

    #[test]
    fn test_pattern_follows_the_run_mode() {
        let mut p = TestPattern::new();
        p.tick();
        assert_eq!(p.status().frame, 1);
        let a = p.framebuffer().to_vec();
        p.handle(RunMode::Pause);
        p.tick();
        assert_eq!(p.framebuffer(), a.as_slice());
        p.handle(RunMode::Fast(4));
        p.tick();
        assert_eq!(p.status().frame, 5);
        assert_ne!(p.framebuffer(), a.as_slice());
    }
}
