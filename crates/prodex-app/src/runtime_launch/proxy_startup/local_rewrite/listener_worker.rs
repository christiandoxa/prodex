use super::{
    RuntimeLocalRewriteProxyShared, handle_runtime_local_rewrite_proxy_request,
    runtime_gateway_pin_request_credentials, runtime_proxy_log,
};
use crate::runtime_background::try_spawn_runtime_supervised_worker;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use tiny_http::Server as TinyServer;

pub(super) fn spawn_runtime_local_rewrite_listener_worker(
    worker_index: usize,
    server: Arc<TinyServer>,
    shutdown: Arc<AtomicBool>,
    shared: RuntimeLocalRewriteProxyShared,
) -> std::io::Result<thread::JoinHandle<()>> {
    try_spawn_runtime_supervised_worker(
        format!("prodex-runtime-local-rewrite-{worker_index}"),
        shared.runtime_shared.log_path.clone(),
        Arc::clone(&shutdown),
        move || runtime_local_rewrite_worker_loop(&server, &shutdown, &shared),
    )
}

fn runtime_local_rewrite_worker_loop(
    server: &TinyServer,
    shutdown: &AtomicBool,
    shared: &RuntimeLocalRewriteProxyShared,
) {
    loop {
        match server.recv() {
            Ok(request) => {
                let request_shared = runtime_gateway_pin_request_credentials(shared);
                let result = crate::runtime_panic::catch_runtime_unwind_silently(|| {
                    handle_runtime_local_rewrite_proxy_request(request, &request_shared);
                });
                if let Err(panic) = result {
                    runtime_proxy_log(
                        &request_shared.runtime_shared,
                        format!(
                            "runtime_proxy_worker_panic lane=local_rewrite panic={}",
                            crate::runtime_panic::runtime_panic_payload_label(panic.as_ref())
                        ),
                    );
                }
            }
            Err(_) if shutdown.load(Ordering::SeqCst) => break,
            Err(_) => break,
        }
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
    }
}
