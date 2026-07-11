use std::thread;
use std::time::{Duration, Instant};

pub(crate) fn wait_for_poll<T, F>(
    label: &str,
    timeout: Duration,
    interval: Duration,
    mut poll: F,
) -> T
where
    F: FnMut() -> Option<T>,
{
    let started_at = Instant::now();
    loop {
        if let Some(value) = poll() {
            return value;
        }

        assert!(
            started_at.elapsed() < timeout,
            "timed out waiting for {label}"
        );
        thread::sleep(interval);
    }
}
