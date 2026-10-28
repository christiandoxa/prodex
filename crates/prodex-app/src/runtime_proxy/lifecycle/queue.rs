//! A bounded ingress queue with removable, never-dispatched work. Expiration is
//! driven separately from inference workers; it cannot require a free worker.
use super::{RuntimeProxyQueueRejection, RuntimeProxyQueuedRequest};
use std::collections::VecDeque;
use std::sync::{
    Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

pub(crate) struct RuntimeProxyRequestQueue {
    items: Mutex<VecDeque<RuntimeProxyQueuedRequest>>,
    ready: Condvar,
    capacity: usize,
}

impl RuntimeProxyRequestQueue {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            items: Mutex::new(VecDeque::new()),
            ready: Condvar::new(),
            capacity: capacity.max(1),
        }
    }

    pub(crate) fn try_send(
        &self,
        request: RuntimeProxyQueuedRequest,
    ) -> Result<(), Box<(RuntimeProxyQueueRejection, RuntimeProxyQueuedRequest)>> {
        let mut items = self.items.lock().unwrap_or_else(|p| p.into_inner());
        if items.len() >= self.capacity {
            return Err(Box::new((RuntimeProxyQueueRejection::Full, request)));
        }
        items.push_back(request);
        self.ready.notify_one();
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn try_recv(&self) -> Option<RuntimeProxyQueuedRequest> {
        self.items
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .pop_front()
    }

    pub(crate) fn recv(&self, shutdown: &AtomicBool) -> Option<RuntimeProxyQueuedRequest> {
        let mut items = self.items.lock().unwrap_or_else(|p| p.into_inner());
        while !shutdown.load(Ordering::SeqCst) {
            if let Some(request) = items.pop_front() {
                return Some(request);
            }
            items = self
                .ready
                .wait_timeout(items, Duration::from_millis(100))
                .unwrap_or_else(|p| p.into_inner())
                .0;
        }
        None
    }

    pub(crate) fn take_expired(&self, now: Instant) -> Vec<RuntimeProxyQueuedRequest> {
        let mut items = self.items.lock().unwrap_or_else(|p| p.into_inner());
        let mut expired = Vec::new();
        for _ in 0..items.len() {
            if let Some(request) = items.pop_front() {
                if request.deadline.is_some_and(|deadline| now >= deadline) {
                    expired.push(request);
                } else {
                    items.push_back(request);
                }
            }
        }
        expired
    }
}

#[cfg(test)]
#[path = "queue_tests.rs"]
mod tests;
