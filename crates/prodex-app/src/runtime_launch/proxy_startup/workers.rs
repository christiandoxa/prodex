//! Bounded, isolated worker lanes for the runtime rotation proxy listener.

use super::*;
use crate::runtime_proxy::{
    RuntimeProxyQueuedRequest, enqueue_runtime_proxy_request, reject_runtime_proxy_recovery_timeout,
};

type RuntimeWorkerQueues = BTreeMap<RuntimeRouteKind, mpsc::SyncSender<RuntimeProxyQueuedRequest>>;

pub(super) fn spawn_runtime_rotation_proxy_workers(
    server: &Arc<TinyServer>,
    shutdown: &Arc<AtomicBool>,
    shared: &RuntimeRotationProxyShared,
    worker_count: usize,
    long_lived_worker_count: usize,
    long_lived_queue_capacity: usize,
) -> Result<Vec<thread::JoinHandle<()>>> {
    let mut worker_threads = Vec::new();
    let mut queues = RuntimeWorkerQueues::new();
    let mut startup_guard = RuntimeRotationWorkerStartupGuard::new(server, shutdown, worker_count);
    // An idle cached WebSocket owns a connection worker, not an HTTP worker.
    // Reserve compaction separately from responses and ordinary metadata I/O.
    // These are bounded host executors; existing Mojo admission, quota, and
    // affinity policy still decides whether each dequeued request may execute.
    let lanes = [
        (
            RuntimeRouteKind::Responses,
            "responses",
            long_lived_worker_count,
        ),
        (
            RuntimeRouteKind::Websocket,
            "websocket",
            long_lived_worker_count,
        ),
        (
            RuntimeRouteKind::Compact,
            "compact",
            shared
                .lane_admission
                .limit(RuntimeRouteKind::Compact)
                .min(worker_count)
                .max(1),
        ),
        (RuntimeRouteKind::Standard, "standard", worker_count),
    ];
    for (route, lane, count) in lanes {
        let (sender, receiver) = mpsc::sync_channel(long_lived_queue_capacity);
        queues.insert(route, sender);
        let receiver = Arc::new(Mutex::new(receiver));
        for worker_index in 0..count {
            let worker_shutdown = Arc::clone(shutdown);
            let shared = shared.clone();
            let receiver = Arc::clone(&receiver);
            worker_threads.push(try_spawn_runtime_supervised_worker(
                format!("prodex-runtime-{lane}-{worker_index}"),
                shared.log_path.clone(),
                Arc::clone(shutdown),
                move || {
                    runtime_rotation_request_worker_loop(
                        Arc::clone(&receiver),
                        shared.clone(),
                        Arc::clone(&worker_shutdown),
                        lane,
                    );
                },
            )?);
        }
        runtime_proxy_log(
            shared,
            format!(
                "runtime_proxy_worker_lane lane={lane} workers={count} queue_capacity={long_lived_queue_capacity}"
            ),
        );
    }

    for worker_index in 0..worker_count {
        let server: Arc<TinyServer> = Arc::clone(server);
        let worker_shutdown = Arc::clone(shutdown);
        let shared = shared.clone();
        let queues = queues.clone();
        worker_threads.push(try_spawn_runtime_supervised_worker(
            format!("prodex-runtime-accept-{worker_index}"),
            shared.log_path.clone(),
            Arc::clone(shutdown),
            move || {
                runtime_rotation_accept_worker_loop(
                    Arc::clone(&server),
                    &queues,
                    &shared,
                    &worker_shutdown,
                );
            },
        )?);
    }

    startup_guard.disarm();
    Ok(worker_threads)
}

fn runtime_rotation_request_worker_loop(
    receiver: Arc<Mutex<mpsc::Receiver<RuntimeProxyQueuedRequest>>>,
    shared: RuntimeRotationProxyShared,
    shutdown: Arc<AtomicBool>,
    lane: &str,
) {
    loop {
        let request = receiver
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .recv();
        let Ok(request) = request else {
            break;
        };
        shared
            .lane_admission
            .long_lived_queue_wait_metric_counters()
            .record_wait(request.queued_at.elapsed());
        let mut scoped = shared.clone();
        scoped.precommit_deadline = request.deadline;
        if scoped.check_precommit_deadline().is_err() {
            reject_runtime_proxy_recovery_timeout(request.request, &scoped, "worker_queue");
        } else {
            runtime_rotation_handle_request_with_panic_log(request.request, &scoped, lane);
        }
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
    }
}

fn runtime_rotation_accept_worker_loop(
    server: Arc<TinyServer>,
    queues: &RuntimeWorkerQueues,
    shared: &RuntimeRotationProxyShared,
    shutdown: &AtomicBool,
) {
    loop {
        let Ok(request) = server.recv() else {
            break;
        };
        runtime_rotation_dispatch_received_request(request, queues, shared);
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
    }
}

fn runtime_rotation_dispatch_received_request(
    mut request: tiny_http::Request,
    queues: &RuntimeWorkerQueues,
    shared: &RuntimeRotationProxyShared,
) {
    // Preserve the authenticated local control plane even when every upstream
    // executor is occupied. Never wait for upstream work or queue space here.
    if let Some(response) =
        crate::runtime_broker::handle_runtime_proxy_admin_request(&mut request, shared)
    {
        let _ = request.respond(response);
        return;
    }
    let route = runtime_proxy_request_lane(request.url(), is_tiny_http_websocket_upgrade(&request));
    let Some(sender) = queues.get(&route) else {
        runtime_rotation_reject_queued_request(request, shared, "worker_lane_unavailable");
        return;
    };
    let scoped = shared.for_precommit_request();
    match enqueue_runtime_proxy_request(sender, request, &scoped) {
        Ok(()) => {}
        Err((reason, request)) => {
            if scoped.check_precommit_deadline().is_err() {
                reject_runtime_proxy_recovery_timeout(request, &scoped, "ingress_queue");
                return;
            }
            let reason = match reason {
                RuntimeProxyQueueRejection::Full => "worker_queue_full",
                RuntimeProxyQueueRejection::Disconnected => "worker_queue_disconnected",
            };
            runtime_rotation_reject_queued_request(request, shared, reason);
        }
    }
}

fn runtime_rotation_reject_queued_request(
    request: tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
    reason: &str,
) {
    // This lane is full, not the whole proxy or any upstream account. A
    // global overload marker would make healthy compact/control work fail too.
    reject_runtime_proxy_overloaded_request(request, shared, reason);
}

fn runtime_rotation_handle_request_with_panic_log(
    request: tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
    lane: &str,
) {
    let result = crate::runtime_panic::catch_runtime_unwind_silently(|| {
        handle_runtime_rotation_proxy_request(request, shared);
    });
    if let Err(panic) = result {
        runtime_proxy_log(
            shared,
            format!(
                "runtime_proxy_worker_panic lane={lane} panic={}",
                crate::runtime_panic::runtime_panic_payload_label(panic.as_ref()),
            ),
        );
    }
}

struct RuntimeRotationWorkerStartupGuard<'a> {
    server: &'a TinyServer,
    shutdown: &'a AtomicBool,
    worker_count: usize,
    armed: bool,
}

impl<'a> RuntimeRotationWorkerStartupGuard<'a> {
    fn new(server: &'a TinyServer, shutdown: &'a AtomicBool, worker_count: usize) -> Self {
        Self {
            server,
            shutdown,
            worker_count,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for RuntimeRotationWorkerStartupGuard<'_> {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        self.shutdown.store(true, Ordering::SeqCst);
        for _ in 0..self.worker_count.max(1) {
            self.server.unblock();
        }
    }
}
