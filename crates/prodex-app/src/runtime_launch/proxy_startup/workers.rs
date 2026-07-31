//! Worker spawning for the runtime rotation proxy listener.

use super::*;

pub(super) fn spawn_runtime_rotation_proxy_workers(
    server: &Arc<TinyServer>,
    shutdown: &Arc<AtomicBool>,
    shared: &RuntimeRotationProxyShared,
    worker_count: usize,
    long_lived_worker_count: usize,
    long_lived_queue_capacity: usize,
) -> Result<Vec<thread::JoinHandle<()>>> {
    let mut worker_threads = Vec::new();
    let (long_lived_sender, long_lived_receiver) =
        mpsc::sync_channel::<tiny_http::Request>(long_lived_queue_capacity);
    let long_lived_receiver = Arc::new(Mutex::new(long_lived_receiver));
    let mut startup_guard = RuntimeRotationWorkerStartupGuard::new(server, shutdown, worker_count);

    for worker_index in 0..long_lived_worker_count {
        let worker_shutdown = Arc::clone(shutdown);
        let shared = shared.clone();
        let receiver = Arc::clone(&long_lived_receiver);
        worker_threads.push(try_spawn_runtime_supervised_worker(
            format!("prodex-runtime-long-lived-{worker_index}"),
            shared.log_path.clone(),
            Arc::clone(shutdown),
            move || {
                runtime_rotation_long_lived_worker_loop(
                    Arc::clone(&receiver),
                    shared.clone(),
                    Arc::clone(&worker_shutdown),
                );
            },
        )?);
    }

    for worker_index in 0..worker_count {
        let server: Arc<TinyServer> = Arc::clone(server);
        let worker_shutdown = Arc::clone(shutdown);
        let shared = shared.clone();
        let long_lived_sender = long_lived_sender.clone();
        worker_threads.push(try_spawn_runtime_supervised_worker(
            format!("prodex-runtime-accept-{worker_index}"),
            shared.log_path.clone(),
            Arc::clone(shutdown),
            move || {
                runtime_rotation_accept_worker_loop(
                    Arc::clone(&server),
                    long_lived_sender.clone(),
                    shared.clone(),
                    Arc::clone(&worker_shutdown),
                );
            },
        )?);
    }

    startup_guard.disarm();
    Ok(worker_threads)
}

fn runtime_rotation_long_lived_worker_loop(
    receiver: Arc<Mutex<mpsc::Receiver<tiny_http::Request>>>,
    shared: RuntimeRotationProxyShared,
    shutdown: Arc<AtomicBool>,
) {
    loop {
        let request = receiver
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .recv();
        let Ok(request) = request else {
            break;
        };
        runtime_rotation_handle_long_lived_request(request, &shared);
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
    }
}

fn runtime_rotation_handle_long_lived_request(
    request: tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
) {
    let (mutex, condvar) = shared.lane_admission.wait();
    let guard = mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    condvar.notify_all();
    drop(guard);
    runtime_rotation_handle_request_with_panic_log(request, shared, "long_lived");
}

fn runtime_rotation_accept_worker_loop(
    server: Arc<TinyServer>,
    long_lived_sender: mpsc::SyncSender<tiny_http::Request>,
    shared: RuntimeRotationProxyShared,
    shutdown: Arc<AtomicBool>,
) {
    loop {
        let Ok(request) = server.recv() else {
            break;
        };
        runtime_rotation_dispatch_received_request(request, &long_lived_sender, &shared);
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
    }
}

fn runtime_rotation_dispatch_received_request(
    request: tiny_http::Request,
    long_lived_sender: &mpsc::SyncSender<tiny_http::Request>,
    shared: &RuntimeRotationProxyShared,
) {
    let websocket = is_tiny_http_websocket_upgrade(&request);
    if !runtime_proxy_request_is_long_lived(request.url(), websocket) {
        runtime_rotation_handle_request_with_panic_log(request, shared, "standard");
        return;
    }
    match enqueue_runtime_proxy_long_lived_request_with_wait(long_lived_sender, request, shared) {
        Ok(()) => {}
        Err((RuntimeProxyQueueRejection::Full, request)) => {
            runtime_rotation_reject_queued_request(request, shared, "long_lived_queue_full");
        }
        Err((RuntimeProxyQueueRejection::Disconnected, request)) => {
            runtime_rotation_reject_queued_request(
                request,
                shared,
                "long_lived_queue_disconnected",
            );
        }
    }
}

fn runtime_rotation_reject_queued_request(
    request: tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
    reason: &str,
) {
    mark_runtime_proxy_local_overload(shared, reason);
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
                crate::runtime_panic::runtime_panic_payload_label(panic.as_ref())
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
