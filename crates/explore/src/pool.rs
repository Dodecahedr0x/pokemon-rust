//! A deterministic worker pool: independent items on `jobs` threads, results in input order.

use rayon::prelude::*;

/// `f` over `items` on a rayon pool of `jobs` threads (0: rayon's default), results in the
/// items' order whatever the thread count. Each call builds its own pool, so the explorer never
/// competes with the global one.
pub fn run_ordered<T: Sync, R: Send>(
    jobs: usize,
    items: &[T],
    f: impl Fn(&T) -> R + Sync + Send,
) -> Vec<R> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(jobs)
        .build()
        .expect("a rayon pool");
    pool.install(|| items.par_iter().map(&f).collect())
}

/// The threads [`run_ordered`] uses for `jobs`.
pub fn threads(jobs: usize) -> usize {
    if jobs == 0 {
        rayon::current_num_threads()
    } else {
        jobs
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn results_keep_the_input_order() {
        let items: Vec<u32> = (0..100).collect();
        for jobs in [1, 3, 8] {
            let out = super::run_ordered(jobs, &items, |&i| i * 2);
            assert_eq!(out, items.iter().map(|i| i * 2).collect::<Vec<_>>());
        }
    }
}
