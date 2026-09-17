use super::{
    RuntimeConfig, RuntimeLocalRewriteProxyShared, spawn_runtime_local_rewrite_listener_worker,
};
use crate::runtime_core_shared::{
    initialize_runtime_proxy_log_path_from_config, runtime_proxy_log_to_path,
};
use anyhow::{Context as _, Result};
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use tiny_http::Server as TinyServer;

pub(in crate::runtime_launch::proxy_startup) struct RuntimeLocalRewriteWorkers {
    pub(in crate::runtime_launch::proxy_startup) worker_threads: Vec<thread::JoinHandle<()>>,
    pub(in crate::runtime_launch::proxy_startup) gemini_live_sidecar_addr:
        Option<std::net::SocketAddr>,
}

pub(super) fn runtime_local_rewrite_log_path(runtime_config: &RuntimeConfig) -> Result<PathBuf> {
    let log_path = initialize_runtime_proxy_log_path_from_config(runtime_config)?;
    for key in runtime_config.compatibility_defaults() {
        runtime_proxy_log_to_path(
            &log_path,
            &runtime_proxy_structured_log_message(
                "runtime_config_compatibility_default",
                [runtime_proxy_log_field("key", *key)],
            ),
        );
    }
    Ok(log_path)
}

pub(super) fn runtime_local_rewrite_server(
    preferred_listen_addr: Option<&str>,
) -> Result<(Arc<TinyServer>, std::net::SocketAddr)> {
    let bind_addr = preferred_listen_addr.unwrap_or("127.0.0.1:0");
    let server = Arc::new(TinyServer::http(bind_addr).map_err(|err| {
        anyhow::anyhow!("failed to bind runtime local rewrite proxy on {bind_addr}: {err}")
    })?);
    let listen_addr = server
        .server_addr()
        .to_ip()
        .context("runtime local rewrite proxy did not expose a TCP listen address")?;
    Ok((server, listen_addr))
}

pub(in crate::runtime_launch::proxy_startup) fn spawn_runtime_local_rewrite_workers(
    shared: &RuntimeLocalRewriteProxyShared,
    server: Option<&Arc<TinyServer>>,
    shutdown: &Arc<AtomicBool>,
    worker_count: usize,
    #[cfg(test)] listener_ready: Option<std::sync::mpsc::Sender<()>>,
    spawn_gemini_sidecar_listener: bool,
) -> Result<RuntimeLocalRewriteWorkers> {
    let mut worker_threads = Vec::new();
    if let Some(pool) = shared.gemini_oauth_pool.as_ref()
        && let Some(worker) = pool.spawn_quota_refresh(shared.runtime_shared.log_path.clone())
    {
        worker_threads.push(worker);
    }
    let _ = spawn_gemini_sidecar_listener;
    let gemini_live_sidecar_addr = None;
    for worker_index in 0..worker_count {
        let Some(server) = server else {
            break;
        };
        let worker = spawn_runtime_local_rewrite_listener_worker(
            worker_index,
            Arc::clone(server),
            Arc::clone(shutdown),
            shared.clone(),
            #[cfg(test)]
            listener_ready.clone(),
        );
        match worker {
            Ok(worker) => worker_threads.push(worker),
            Err(err) => {
                shutdown.store(true, Ordering::SeqCst);
                for _ in 0..worker_index {
                    server.unblock();
                }
                return Err(err.into());
            }
        }
    }
    Ok(RuntimeLocalRewriteWorkers {
        worker_threads,
        gemini_live_sidecar_addr,
    })
}
