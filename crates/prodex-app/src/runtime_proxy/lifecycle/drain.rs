use crate::{RuntimeRotationProxy, runtime_proxy_flush_logs_for_path, runtime_proxy_log_to_path};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};

impl RuntimeRotationProxy {
    pub(crate) fn shutdown_and_drain(&self, timeout: Duration) -> bool {
        self.draining.store(true, Ordering::SeqCst);
        runtime_proxy_log_to_path(
            &self.log_path,
            &format!(
                "runtime_proxy_shutdown event=draining_start active_requests={}",
                self.active_request_count.load(Ordering::SeqCst)
            ),
        );
        let drained = wait_for_active_requests(&self.active_request_count, timeout);
        self.shutdown.store(true, Ordering::SeqCst);
        for _ in 0..self.accept_worker_count {
            self.server.unblock();
        }
        runtime_proxy_log_to_path(
            &self.log_path,
            &format!(
                "runtime_proxy_shutdown event={} active_requests={}",
                if drained {
                    "drain_completed"
                } else {
                    "drain_timeout"
                },
                self.active_request_count.load(Ordering::SeqCst)
            ),
        );
        let logs_flushed = runtime_proxy_flush_logs_for_path(&self.log_path).is_ok();
        drained && logs_flushed
    }
}

fn wait_for_active_requests(active: &AtomicUsize, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while active.load(Ordering::SeqCst) != 0 {
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(10));
    }
    true
}

#[cfg(test)]
mod tests {
    use super::wait_for_active_requests;
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        thread,
        time::Duration,
    };

    #[test]
    fn drain_waits_for_inflight_release_and_times_out_when_stuck() {
        let active = Arc::new(AtomicUsize::new(1));
        let release = Arc::clone(&active);
        let worker = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            release.store(0, Ordering::SeqCst);
        });
        assert!(wait_for_active_requests(&active, Duration::from_secs(1)));
        worker.join().unwrap();

        active.store(1, Ordering::SeqCst);
        assert!(!wait_for_active_requests(&active, Duration::from_millis(1)));
    }
}
