use super::{AtomicBool, Duration, Instant, RuntimeProxyQueuedRequest, RuntimeProxyRequestQueue};
use std::sync::Arc;

fn request(path: &str, deadline: Instant) -> RuntimeProxyQueuedRequest {
    RuntimeProxyQueuedRequest {
        request: tiny_http::TestRequest::new().with_path(path).into(),
        deadline: Some(deadline),
        queued_at: Instant::now(),
    }
}

#[test]
fn expired_queue_capacity_is_reclaimed_without_any_worker_receiving() {
    let queue = RuntimeProxyRequestQueue::new(1);
    let now = Instant::now();
    assert!(queue.try_send(request("/expired", now)).is_ok());
    assert!(queue.try_send(request("/full", now)).is_err());
    let expired = queue.take_expired(now);
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0].request.url(), "/expired");
    assert!(
        queue
            .try_send(request("/recovered", now + Duration::from_secs(60)))
            .is_ok()
    );
    assert_eq!(queue.try_recv().unwrap().request.url(), "/recovered");
}

#[test]
fn queue_expiry_preserves_live_fifo_and_original_deadlines() {
    let queue = RuntimeProxyRequestQueue::new(3);
    let now = Instant::now();
    let deadline = now + Duration::from_secs(60);
    for (path, time) in [
        ("/first", deadline),
        ("/expired", now),
        ("/second", deadline),
    ] {
        assert!(queue.try_send(request(path, time)).is_ok());
    }
    assert_eq!(queue.take_expired(now).len(), 1);
    for path in ["/first", "/second"] {
        let item = queue.try_recv().unwrap();
        assert_eq!(item.request.url(), path);
        assert_eq!(item.deadline, Some(deadline));
    }
    assert!(queue.try_recv().is_none());
}

#[test]
fn queue_wait_is_shutdown_aware_and_concurrent_enqueues_remain_bounded() {
    let queue = Arc::new(RuntimeProxyRequestQueue::new(2));
    let deadline = Instant::now() + Duration::from_secs(60);
    let producers: Vec<_> = (0..8)
        .map(|_| {
            let queue = Arc::clone(&queue);
            std::thread::spawn(move || queue.try_send(request("/bounded", deadline)).is_ok())
        })
        .collect();
    let accepted = producers
        .into_iter()
        .map(|p| usize::from(p.join().unwrap()))
        .sum::<usize>();
    assert_eq!(accepted, 2);
    let shutdown = AtomicBool::new(true);
    assert!(queue.recv(&shutdown).is_none());
}
