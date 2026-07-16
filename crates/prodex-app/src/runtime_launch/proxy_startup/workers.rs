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
                loop {
                    let request = receiver
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .recv();
                    match request {
                        Ok(request) => {
                            let (mutex, condvar) = shared.lane_admission.wait();
                            let guard = mutex
                                .lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                            condvar.notify_all();
                            drop(guard);
                            let result =
                                crate::runtime_panic::catch_runtime_unwind_silently(|| {
                                    handle_runtime_rotation_proxy_request(request, &shared);
                                });
                            if let Err(panic) = result {
                                runtime_proxy_log(
                                    &shared,
                                    format!(
                                        "runtime_proxy_worker_panic lane=long_lived panic={}",
                                        crate::runtime_panic::runtime_panic_payload_label(
                                            panic.as_ref()
                                        )
                                    ),
                                );
                            }
                        }
                        Err(_) => break,
                    }
                    if worker_shutdown.load(Ordering::SeqCst) {
                        break;
                    }
                }
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
                loop {
                    match server.recv() {
                        Ok(request) => {
                            let websocket = is_tiny_http_websocket_upgrade(&request);
                            let long_lived =
                                runtime_proxy_request_is_long_lived(request.url(), websocket);
                            if long_lived {
                                match enqueue_runtime_proxy_long_lived_request_with_wait(
                                    &long_lived_sender,
                                    request,
                                    &shared,
                                ) {
                                    Ok(()) => {}
                                    Err((RuntimeProxyQueueRejection::Full, request)) => {
                                        mark_runtime_proxy_local_overload(
                                            &shared,
                                            "long_lived_queue_full",
                                        );
                                        reject_runtime_proxy_overloaded_request(
                                            request,
                                            &shared,
                                            "long_lived_queue_full",
                                        );
                                    }
                                    Err((RuntimeProxyQueueRejection::Disconnected, request)) => {
                                        mark_runtime_proxy_local_overload(
                                            &shared,
                                            "long_lived_queue_disconnected",
                                        );
                                        reject_runtime_proxy_overloaded_request(
                                            request,
                                            &shared,
                                            "long_lived_queue_disconnected",
                                        );
                                    }
                                }
                            } else {
                                let result =
                                    crate::runtime_panic::catch_runtime_unwind_silently(|| {
                                        handle_runtime_rotation_proxy_request(request, &shared);
                                    });
                                if let Err(panic) = result {
                                    runtime_proxy_log(
                                        &shared,
                                        format!(
                                            "runtime_proxy_worker_panic lane=standard panic={}",
                                            crate::runtime_panic::runtime_panic_payload_label(
                                                panic.as_ref()
                                            )
                                        ),
                                    );
                                }
                            }
                        }
                        Err(_) if worker_shutdown.load(Ordering::SeqCst) => break,
                        Err(_) => break,
                    }
                    if worker_shutdown.load(Ordering::SeqCst) {
                        break;
                    }
                }
            },
        )?);
    }

    startup_guard.disarm();
    Ok(worker_threads)
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
