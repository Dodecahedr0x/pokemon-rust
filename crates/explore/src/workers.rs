//! A deterministic worker pool for independent jobs (explorer speed work, 2026-10-23).
//!
//! [`run_ordered`] evaluates jobs (each typically starting from its own snapshot: a branch, a
//! probe, a replay) on up to `threads` scoped threads and returns the results **in job order**,
//! whatever the thread count and however long each job takes. A caller that plans its jobs
//! from a seed alone and merges the results in that order gets the same outcome on 1 thread
//! as on 16. Self-contained: it knows nothing of the search that uses it.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The default worker count: the available parallelism less a quarter (at least 1), leaving
/// room on a machine shared with builds and other runs.
pub fn default_threads() -> usize {
    let n = std::thread::available_parallelism().map_or(1, |n| n.get());
    (n - n / 4).max(1)
}

/// `threads`, or [`default_threads`] for 0.
pub fn threads_or_default(threads: usize) -> usize {
    if threads > 0 {
        threads
    } else {
        default_threads()
    }
}

/// `f` on every job, on up to `threads` threads (0: [`default_threads`]); the results in job
/// order. Jobs are handed out in order to whichever thread is free, so a slow job never holds
/// the others back. A panicking job panics the call (after the other threads stop).
pub fn run_ordered<J: Sync, R: Send>(
    jobs: &[J],
    threads: usize,
    f: impl Fn(&J) -> R + Sync,
) -> Vec<R> {
    let threads = threads_or_default(threads).min(jobs.len());
    if threads <= 1 {
        return jobs.iter().map(&f).collect();
    }
    let next = AtomicUsize::new(0);
    let slots: Vec<Mutex<Option<R>>> = jobs.iter().map(|_| Mutex::new(None)).collect();
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(i) else {
                        break;
                    };
                    let r = f(job);
                    *slots[i].lock().expect("a slot is written once") = Some(r);
                }
            });
        }
    });
    slots
        .into_iter()
        .map(|s| {
            s.into_inner()
                .expect("a slot is written once")
                .expect("every job ran")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A job whose result and duration depend on its input only.
    fn job(&(seed, spin): &(u64, u32)) -> u64 {
        let mut x = seed;
        for _ in 0..spin {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
        }
        x
    }

    #[test]
    fn the_results_are_in_job_order_on_any_thread_count() {
        // Uneven durations, so that the threads finish out of order.
        let jobs: Vec<(u64, u32)> = (0..97)
            .map(|i| (i * 0x9E37_79B9 + 1, (i as u32 * 7919) % 200_000))
            .collect();
        let one = run_ordered(&jobs, 1, job);
        assert_eq!(one, jobs.iter().map(job).collect::<Vec<_>>());
        for threads in [2, 4, 16, 0] {
            assert_eq!(run_ordered(&jobs, threads, job), one, "{threads} threads");
        }
        // A sequential merge of the ordered results is the same on 1 and 4 threads.
        let merge = |rs: Vec<u64>| rs.iter().fold(0u64, |h, &r| h.rotate_left(5) ^ r);
        assert_eq!(merge(run_ordered(&jobs, 4, job)), merge(one));
    }

    #[test]
    fn no_jobs_and_one_job() {
        assert!(run_ordered(&[] as &[(u64, u32)], 4, job).is_empty());
        assert_eq!(run_ordered(&[(5, 3)], 4, job), [job(&(5, 3))]);
    }

    #[test]
    fn the_default_leaves_a_margin() {
        let n = std::thread::available_parallelism().map_or(1, |n| n.get());
        let d = default_threads();
        assert!(d >= 1 && d <= n);
        if n >= 4 {
            assert!(d < n);
        }
        assert_eq!(threads_or_default(3), 3);
        assert_eq!(threads_or_default(0), d);
    }
}
