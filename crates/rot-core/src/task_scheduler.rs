//! Bounded async task scheduling helpers.

use futures::{StreamExt, stream};
use std::future::Future;

/// Execute jobs with bounded concurrency and collect all outputs.
pub async fn run_bounded<I, Fut, T>(jobs: I, max_concurrency: usize) -> Vec<T>
where
    I: IntoIterator<Item = Fut>,
    Fut: Future<Output = T>,
    T: Send + 'static,
{
    stream::iter(jobs)
        .buffer_unordered(max_concurrency.max(1))
        .collect::<Vec<_>>()
        .await
}

#[cfg(test)]
mod tests {
    use super::run_bounded;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::time::{Duration, sleep};

    #[tokio::test]
    async fn test_run_bounded_executes_all_jobs() {
        let counter = Arc::new(AtomicUsize::new(0));
        let jobs = (0..5)
            .map(|_| {
                let counter = Arc::clone(&counter);
                async move {
                    sleep(Duration::from_millis(5)).await;
                    counter.fetch_add(1, Ordering::SeqCst);
                    1usize
                }
            })
            .collect::<Vec<_>>();

        let outputs = run_bounded(jobs, 2).await;
        assert_eq!(outputs.len(), 5);
        assert_eq!(counter.load(Ordering::SeqCst), 5);
    }
}
