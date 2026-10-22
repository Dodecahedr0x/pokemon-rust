//! The frame transport core (plan: "frames.rs"): a pane thread publishes its latest RGB555
//! framebuffer as RGBA into a [`FrameSlot`]; the webview pulls it with
//! `fetch("/frames/<pane>?after=<seq>")` (served by `ui`'s asset handler) and draws it with
//! `putImageData`. Raw bytes, no JSON or base64.

use engine::video::{SCREEN_H, SCREEN_W};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// One RGBA8 frame: `SCREEN_W * SCREEN_H * 4` bytes (153,600).
pub const FRAME_BYTES: usize = SCREEN_W * SCREEN_H * 4;

/// RGB555 -> RGBA8 with the play bin's expansion ((v << 3) | (v >> 2)), alpha 255.
/// `out` holds at least `fb.len() * 4` bytes.
pub fn rgba_from_rgb555(fb: &[u16], out: &mut [u8]) {
    fn expand(v: u16) -> u8 {
        let v = (v & 0x1F) as u8;
        (v << 3) | (v >> 2)
    }
    for (px, o) in fb.iter().zip(out.chunks_exact_mut(4)) {
        o[0] = expand(*px);
        o[1] = expand(*px >> 5);
        o[2] = expand(*px >> 10);
        o[3] = 255;
    }
}

#[derive(Default)]
struct Published {
    /// 0 until the first publish, then +1 per publish.
    seq: u64,
    rgba: Arc<Vec<u8>>,
    at: Option<Instant>,
    /// Pending [`FrameSlot::wait_then`] requests, answered by the slot's waiter thread.
    waiters: Vec<Waiter>,
    /// Whether the waiter thread is running.
    serving: bool,
}

/// What a [`FrameSlot::wait_then`] callback receives: the frame, `None` on timeout, or the
/// error that kept the waiter thread from starting.
pub type Answer = std::io::Result<Option<Latest>>;

struct Waiter {
    after: u64,
    deadline: Instant,
    answer: Box<dyn FnOnce(Answer) + Send>,
}

#[derive(Default)]
struct Shared {
    published: Mutex<Published>,
    changed: Condvar,
}

/// The latest frame of one pane, shared between its thread (writer) and the UI (reader).
#[derive(Clone, Default)]
pub struct FrameSlot(Arc<Shared>);

/// A frame read from a [`FrameSlot`]: its seq, its RGBA bytes and how long ago it was published.
pub type Latest = (u64, Arc<Vec<u8>>, Duration);

/// How long an idle waiter thread lingers for the next request before it ends.
const WAITER_IDLE: Duration = Duration::from_secs(1);

impl FrameSlot {
    /// Converts `fb` (RGB555, `SCREEN_W * SCREEN_H`) and makes it the latest frame; seq += 1.
    /// Wakes [`wait`](Self::wait)ers and the waiter thread.
    pub fn publish(&self, fb: &[u16]) {
        let mut rgba = vec![0; FRAME_BYTES];
        rgba_from_rgb555(fb, &mut rgba);
        let at = Instant::now();
        let mut p = self.0.published.lock().unwrap();
        p.seq += 1;
        p.rgba = Arc::new(rgba);
        p.at = Some(at);
        drop(p);
        self.0.changed.notify_all();
    }

    /// The latest frame if its seq differs from `after` (a reader that is ahead of the slot,
    /// e.g. one that outlived a fresh slot, gets the current frame).
    pub fn latest(&self, after: u64) -> Option<(u64, Arc<Vec<u8>>)> {
        self.latest_with_age(after)
            .map(|(seq, rgba, _)| (seq, rgba))
    }

    /// [`latest`](Self::latest) plus how long ago it was published (the `x-age-us` header).
    pub fn latest_with_age(&self, after: u64) -> Option<Latest> {
        Self::pick(&self.0.published.lock().unwrap(), after)
    }

    /// Like [`latest_with_age`](Self::latest_with_age), but when no frame other than `after`
    /// exists yet, blocks up to `timeout` for the next publish.
    pub fn wait(&self, after: u64, timeout: Duration) -> Option<Latest> {
        let p = self.0.published.lock().unwrap();
        let (p, _) = self
            .0
            .changed
            .wait_timeout_while(p, timeout, |p| Self::pick(p, after).is_none())
            .unwrap();
        Self::pick(&p, after)
    }

    /// The non-blocking [`wait`](Self::wait) behind the `/frames` long poll: `answer` gets the
    /// first frame whose seq differs from `after`, or `Ok(None)` after `timeout`. It runs on the
    /// slot's waiter thread (one per slot, started on demand, ending after [`WAITER_IDLE`]
    /// without requests), or at once on the caller's thread with `Err` if that thread cannot
    /// be started.
    pub fn wait_then(
        &self,
        after: u64,
        timeout: Duration,
        answer: impl FnOnce(Answer) + Send + 'static,
    ) {
        let mut p = self.0.published.lock().unwrap();
        if !p.serving {
            let shared = self.0.clone();
            let started = std::thread::Builder::new()
                .name("frames:waiter".into())
                .spawn(move || serve(&shared));
            if let Err(e) = started {
                drop(p);
                return answer(Err(e));
            }
            p.serving = true;
        }
        p.waiters.push(Waiter {
            after,
            deadline: Instant::now() + timeout,
            answer: Box::new(answer),
        });
        drop(p);
        self.0.changed.notify_all();
    }

    fn pick(p: &Published, after: u64) -> Option<Latest> {
        let at = p.at?;
        (p.seq != after).then(|| (p.seq, p.rgba.clone(), at.elapsed()))
    }

    /// The seq of the latest frame (0 before the first publish).
    pub fn seq(&self) -> u64 {
        self.0.published.lock().unwrap().seq
    }

    #[cfg(test)]
    fn serving(&self) -> bool {
        self.0.published.lock().unwrap().serving
    }
}

/// The waiter thread: answers each request once its frame exists or its deadline passed
/// (outside the lock), then sleeps until the next publish, request or deadline.
fn serve(shared: &Shared) {
    let mut p = shared.published.lock().unwrap();
    loop {
        let now = Instant::now();
        let mut ready = Vec::new();
        let mut i = 0;
        while i < p.waiters.len() {
            let w = &p.waiters[i];
            let frame = FrameSlot::pick(&p, w.after);
            if frame.is_some() || now >= w.deadline {
                ready.push((p.waiters.swap_remove(i).answer, frame));
            } else {
                i += 1;
            }
        }
        if !ready.is_empty() {
            drop(p);
            for (answer, frame) in ready {
                answer(Ok(frame));
            }
            p = shared.published.lock().unwrap();
            continue;
        }
        let Some(deadline) = p.waiters.iter().map(|w| w.deadline).min() else {
            let (q, timeout) = shared.changed.wait_timeout(p, WAITER_IDLE).unwrap();
            p = q;
            if timeout.timed_out() && p.waiters.is_empty() {
                p.serving = false;
                return;
            }
            continue;
        };
        p = shared
            .changed
            .wait_timeout(p, deadline.saturating_duration_since(now))
            .unwrap()
            .0;
    }
}

/// `GET /frames/<pane>?after=<seq>`: 200 + body (RGBA) + `x-seq`, or 204. `Cache-Control: no-store`.
///
/// Parses `path_and_query` (leading `/` optional) into the pane name and `after` (0 when
/// absent). Other query parameters are ignored. `None` if it is not a frames request, the pane
/// name is empty or nested, or `after` is not a number.
pub fn parse_frame_request(path_and_query: &str) -> Option<(&str, u64)> {
    let s = path_and_query.strip_prefix('/').unwrap_or(path_and_query);
    let s = s.strip_prefix("frames/")?;
    let (pane, query) = s.split_once('?').unwrap_or((s, ""));
    if pane.is_empty() || pane.contains('/') {
        return None;
    }
    let mut after = 0;
    for kv in query.split('&') {
        if let Some(v) = kv.strip_prefix("after=") {
            after = v.parse().ok()?;
        }
    }
    Some((pane, after))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_expansion_matches_the_play_bin() {
        // Red 31, green 16, blue 1; then black; then white.
        let fb = [31 | (16 << 5) | (1 << 10), 0, 0x7FFF];
        let mut out = [0u8; 12];
        rgba_from_rgb555(&fb, &mut out);
        assert_eq!(out, [255, 132, 8, 255, 0, 0, 0, 255, 255, 255, 255, 255]);
        // Every channel value expands as (v << 3) | (v >> 2) and ignores bit 15.
        for v in 0..32u16 {
            let mut o = [0u8; 4];
            rgba_from_rgb555(&[0x8000 | v | (v << 5) | (v << 10)], &mut o);
            let e = ((v << 3) | (v >> 2)) as u8;
            assert_eq!(o, [e, e, e, 255]);
        }
    }

    #[test]
    fn a_full_frame_fills_frame_bytes() {
        let fb = vec![0x7FFF; SCREEN_W * SCREEN_H];
        let slot = FrameSlot::default();
        slot.publish(&fb);
        let (_, rgba) = slot.latest(0).unwrap();
        assert_eq!(rgba.len(), FRAME_BYTES);
        assert!(rgba.iter().all(|&b| b == 255));
    }

    #[test]
    fn seq_starts_at_zero_and_counts_publishes() {
        let slot = FrameSlot::default();
        assert_eq!(slot.seq(), 0);
        assert!(slot.latest(0).is_none(), "nothing published yet");
        let fb = vec![0; SCREEN_W * SCREEN_H];
        slot.publish(&fb);
        assert_eq!(slot.latest(0).unwrap().0, 1);
        assert!(slot.latest(1).is_none(), "nothing newer than 1");
        slot.publish(&fb);
        slot.publish(&fb);
        // A reader that missed frames gets only the newest one.
        assert_eq!(slot.latest(1).unwrap().0, 3);
        assert!(slot.latest(3).is_none());
        // A reader ahead of the slot (it outlived a fresh slot) gets the current frame.
        assert_eq!(slot.latest(7).unwrap().0, 3);
        // Clones share the slot.
        let other = slot.clone();
        other.publish(&fb);
        assert_eq!(slot.seq(), 4);
    }

    #[test]
    fn latest_is_the_last_published_frame() {
        let slot = FrameSlot::default();
        slot.publish(&vec![0; SCREEN_W * SCREEN_H]);
        let (_, old) = slot.latest(0).unwrap();
        slot.publish(&vec![31; SCREEN_W * SCREEN_H]);
        let (_, new) = slot.latest(1).unwrap();
        assert_eq!(&old[..4], &[0, 0, 0, 255], "a held frame is not mutated");
        assert_eq!(&new[..4], &[255, 0, 0, 255]);
        let (_, _, age) = slot.latest_with_age(0).unwrap();
        assert!(age < Duration::from_secs(5));
    }

    #[test]
    fn wait_returns_at_once_or_on_the_next_publish() {
        let slot = FrameSlot::default();
        let fb = vec![0; SCREEN_W * SCREEN_H];
        assert!(slot.wait(0, Duration::from_millis(10)).is_none(), "timeout");
        slot.publish(&fb);
        assert_eq!(slot.wait(0, Duration::ZERO).unwrap().0, 1, "already newer");
        let s = slot.clone();
        let t = std::thread::spawn(move || s.wait(1, Duration::from_secs(10)).map(|l| l.0));
        std::thread::sleep(Duration::from_millis(20));
        slot.publish(&fb);
        assert_eq!(t.join().unwrap(), Some(2));
        // Ahead of the slot: answered at once, not after the timeout.
        let t0 = Instant::now();
        assert_eq!(slot.wait(9, Duration::from_secs(10)).unwrap().0, 2);
        assert!(t0.elapsed() < Duration::from_secs(5));
    }

    /// Calls `wait_then` and returns a receiver for (answer seq, answering thread).
    fn ask(
        slot: &FrameSlot,
        after: u64,
        timeout: Duration,
    ) -> std::sync::mpsc::Receiver<(Option<u64>, std::thread::ThreadId)> {
        let (tx, rx) = std::sync::mpsc::channel();
        slot.wait_then(after, timeout, move |a| {
            let seq = a.unwrap().map(|l| l.0);
            tx.send((seq, std::thread::current().id())).unwrap();
        });
        rx
    }

    const LONG: Duration = Duration::from_secs(10);

    #[test]
    fn wait_then_answers_from_one_waiter_thread() {
        let slot = FrameSlot::default();
        let fb = vec![0; SCREEN_W * SCREEN_H];
        assert!(!slot.serving());
        // Timeout: None.
        let t0 = Instant::now();
        let (seq, waiter) = ask(&slot, 0, Duration::from_millis(30)).recv().unwrap();
        assert_eq!(seq, None);
        assert!(t0.elapsed() >= Duration::from_millis(30));
        assert_ne!(waiter, std::thread::current().id());
        assert!(slot.serving());
        // Pending requests are answered on the next publish, in any order of `after`.
        let a = ask(&slot, 0, LONG);
        let b = ask(&slot, 0, LONG);
        std::thread::sleep(Duration::from_millis(20));
        assert!(a.try_recv().is_err(), "nothing published yet");
        slot.publish(&fb);
        assert_eq!(a.recv().unwrap(), (Some(1), waiter));
        assert_eq!(b.recv().unwrap(), (Some(1), waiter));
        // Already available: answered at once; a stale or ahead `after` too.
        assert_eq!(ask(&slot, 0, LONG).recv().unwrap(), (Some(1), waiter));
        assert_eq!(ask(&slot, 5, LONG).recv().unwrap(), (Some(1), waiter));
        // A later request does not delay an earlier, shorter deadline.
        let long = ask(&slot, 1, LONG);
        let short = ask(&slot, 1, Duration::from_millis(30));
        assert_eq!(short.recv().unwrap(), (None, waiter));
        slot.publish(&fb);
        assert_eq!(long.recv().unwrap(), (Some(2), waiter));
        // Idle, the waiter thread ends; the next request starts a new one.
        let end = Instant::now() + LONG;
        while slot.serving() {
            assert!(Instant::now() < end, "the waiter thread never ended");
            std::thread::sleep(Duration::from_millis(20));
        }
        let (seq, again) = ask(&slot, 0, LONG).recv().unwrap();
        assert_eq!(seq, Some(2));
        assert_ne!(again, std::thread::current().id());
    }

    #[test]
    fn wait_then_survives_many_requests() {
        // Long-poll traffic at 60 fps for two panes would spawn ~120 threads a second with a
        // thread per request; here hundreds of requests share one thread.
        let slot = FrameSlot::default();
        let fb = vec![0; SCREEN_W * SCREEN_H];
        let mut threads = std::collections::HashSet::new();
        for i in 0..300u64 {
            let rx = ask(&slot, i, LONG);
            slot.publish(&fb);
            let (seq, t) = rx.recv().unwrap();
            assert_eq!(seq, Some(i + 1));
            threads.insert(t);
        }
        assert_eq!(threads.len(), 1);
    }

    #[test]
    fn frame_requests_parse() {
        assert_eq!(
            parse_frame_request("/frames/native?after=12"),
            Some(("native", 12))
        );
        assert_eq!(parse_frame_request("frames/rom?after=0"), Some(("rom", 0)));
        assert_eq!(parse_frame_request("/frames/rom"), Some(("rom", 0)));
        assert_eq!(parse_frame_request("/frames/a?t=5&after=7"), Some(("a", 7)));
        assert_eq!(
            parse_frame_request(&format!("/frames/a?after={}", u64::MAX)),
            Some(("a", u64::MAX))
        );
        assert_eq!(parse_frame_request("/frames/?after=1"), None);
        assert_eq!(parse_frame_request("/frames/a/b?after=1"), None);
        assert_eq!(parse_frame_request("/frames/a?after=x"), None);
        assert_eq!(parse_frame_request("/frames/a?after=-1"), None);
        assert_eq!(parse_frame_request("/maps/a?after=1"), None);
        assert_eq!(parse_frame_request("/framesa?after=1"), None);
    }
}
