//! Prodex's transport-only repair for upstream tiny_http 0.12.0.
//! No accepted HTTP request is repeated, and the listening address is retained.
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub(crate) fn until_shutdown<T>(
    phase: &str,
    shutdown: &AtomicBool,
    mut operation: impl FnMut() -> io::Result<T>,
) -> Option<T> {
    let mut failures = 0_u32;
    while !shutdown.load(Ordering::Relaxed) {
        match operation() {
            Ok(value) => {
                if failures != 0 {
                    log::info!("HTTP listener recovered phase={} failures={}", phase, failures);
                }
                return Some(value);
            }
            Err(error) => {
                failures = failures.saturating_add(1);
                // Bounded diagnostics and bounded sleep: an exhausted process
                // must neither spin nor permanently lose its listening socket.
                if failures.is_power_of_two() {
                    log::warn!("HTTP listener retry phase={} kind={:?} failures={}", phase, error.kind(), failures);
                }
                let delay = (10_u64 << failures.saturating_sub(1).min(5)).min(250);
                std::thread::sleep(Duration::from_millis(delay));
            }
        }
    }
    None
}
