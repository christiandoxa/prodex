//! Worker spawning for the runtime rotation proxy listener.

use super::*;

pub(super) fn spawn_runtime_rotation_proxy_workers(
    server: &Arc<TinyServer>,
    shutdown: &Arc<AtomicBool>,
    shared: &RuntimeRotationProxyShared,
    worker_count: usize,
    long_lived_worker_count: usize,
    long_lived_queue_capacity: usize,
) -> Vec<thread::JoinHandle<()>> {
    let mut worker_threads = Vec::new();
    let (long_lived_sender, long_lived_receiver) =
        mpsc::sync_channel::<tiny_http::Request>(long_lived_queue_capacity);
    let long_lived_receiver = Arc::new(Mutex::new(long_lived_receiver));

    for _ in 0..long_lived_worker_count {
        let shutdown = Arc::clone(shutdown);
        let shared = shared.clone();
        let receiver = Arc::clone(&long_lived_receiver);
        worker_threads.push(thread::spawn(move || {
            loop {
                let request = {
                    let guard = receiver.lock();
                    let Ok(receiver) = guard else {
                        break;
                    };
                    receiver.recv()
                };
                match request {
                    Ok(request) => {
                        let (mutex, condvar) = &*shared.lane_admission.wait;
                        let guard = mutex
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        condvar.notify_all();
                        drop(guard);
                        let result = crate::runtime_panic::catch_runtime_unwind_silently(|| {
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
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }
            }
        }));
    }

    for _ in 0..worker_count {
        let server: Arc<TinyServer> = Arc::clone(server);
        let shutdown = Arc::clone(shutdown);
        let shared = shared.clone();
        let long_lived_sender = long_lived_sender.clone();
        worker_threads.push(thread::spawn(move || {
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
                    Err(_) if shutdown.load(Ordering::SeqCst) => break,
                    Err(_) => {}
                }
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }
            }
        }));
    }

    worker_threads
}
