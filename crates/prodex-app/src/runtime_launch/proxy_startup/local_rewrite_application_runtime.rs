use std::{
    io,
    sync::atomic::Ordering,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use http_body_util::{BodyExt as _, LengthLimitError};
use prodex_gateway_server::{GatewayHandlerError, GatewayHandlerRequest, GatewayHandlerResult};
use tokio::sync::{Semaphore, TryAcquireError, oneshot};

use super::local_rewrite::{
    RuntimeLocalRewritePrepared, prepare_runtime_local_rewrite_application,
    spawn_runtime_local_rewrite_workers,
};
use super::local_rewrite_gateway_credentials::{
    RuntimeGatewayCredentialRefreshPlan, runtime_gateway_pin_request_credentials,
};
use super::local_rewrite_options::RuntimeLocalRewriteProxyStartOptions;
use super::local_rewrite_pipeline::run_runtime_local_rewrite_pipeline;
use super::local_rewrite_request::{
    RuntimeLocalRewriteRequest, runtime_direct_request_body_channel,
};
use crate::{
    RuntimeConfig, RuntimeProxyBodyTooLarge, runtime_proxy_flush_logs_for_path, runtime_proxy_log,
};

const HEALTH_REQUEST_LIMIT: usize = 4;

pub(crate) struct RuntimeGatewayApplication {
    shared: super::local_rewrite::RuntimeLocalRewriteProxyShared,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
    worker_threads: Mutex<Vec<thread::JoinHandle<()>>>,
    request_slots: Arc<Semaphore>,
    request_limit: usize,
    health_slots: Arc<Semaphore>,
    _marker_guard: crate::RuntimeProxyMarkerGuard,
}

pub(crate) fn start_runtime_gateway_application_with_runtime_config(
    options: RuntimeLocalRewriteProxyStartOptions<'_>,
    runtime_config: Arc<RuntimeConfig>,
    secret_refresh: Option<RuntimeGatewayCredentialRefreshPlan>,
    request_constraints: prodex_provider_core::ProviderRequestConstraintPolicy,
    resolved_harness: prodex_provider_core::ResolvedHarnessMode,
) -> anyhow::Result<RuntimeGatewayApplication> {
    let prepared = prepare_runtime_local_rewrite_application(
        options,
        runtime_config,
        false,
        secret_refresh,
        request_constraints,
        resolved_harness,
        ("in_process", None),
    )?;
    RuntimeGatewayApplication::new(prepared)
}

impl RuntimeGatewayApplication {
    fn new(prepared: RuntimeLocalRewritePrepared) -> anyhow::Result<Self> {
        let RuntimeLocalRewritePrepared {
            runtime_config: _,
            shared,
            shutdown,
            worker_count: _,
            secret_refresh,
            log_path: _,
            marker_guard,
        } = prepared;
        let workers = spawn_runtime_local_rewrite_workers(
            &shared,
            None,
            &shutdown,
            0,
            secret_refresh,
            false,
        )?;
        debug_assert!(workers.gemini_live_sidecar_addr.is_none());
        let request_limit = shared.runtime_shared.active_request_limit.max(1);
        Ok(Self {
            shared,
            shutdown,
            worker_threads: Mutex::new(workers.worker_threads),
            request_slots: Arc::new(Semaphore::new(request_limit)),
            request_limit,
            health_slots: Arc::new(Semaphore::new(HEALTH_REQUEST_LIMIT)),
            _marker_guard: marker_guard,
        })
    }

    pub(crate) async fn handle(
        &self,
        handler_request: GatewayHandlerRequest,
    ) -> GatewayHandlerResult {
        if self.shutdown.load(Ordering::Acquire) {
            return Err(GatewayHandlerError::Unavailable);
        }
        let permit = try_acquire_gateway_request_permit(
            handler_request.route,
            &self.request_slots,
            &self.health_slots,
        )?;
        if self.shutdown.load(Ordering::Acquire) {
            return Err(GatewayHandlerError::Unavailable);
        }

        let GatewayHandlerRequest {
            peer_addr,
            client_ip,
            peer_is_trusted_proxy,
            target,
            route: _,
            request,
        } = handler_request;
        let method = request.method().as_str().to_string();
        let network_zone =
            runtime_gateway_network_zone(peer_addr, client_ip, peer_is_trusted_proxy);
        let headers = request
            .headers()
            .iter()
            .map(|(name, value)| {
                value
                    .to_str()
                    .map(|value| (name.as_str().to_string(), value.to_string()))
                    .map_err(|_| GatewayHandlerError::InvalidRequest)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let (_, mut incoming) = request.into_parts();
        let max_request_body_bytes = self
            .shared
            .runtime_shared
            .runtime_config
            .max_request_body_bytes;
        let (body_sender, body) = runtime_direct_request_body_channel();
        let (head_sender, head_receiver) = oneshot::channel();
        let (cancel_sender, mut cancel_receiver) = oneshot::channel();
        let _cancel_on_drop = CancelBodyPumpOnDrop(Some(cancel_sender));

        let pump_permit = Arc::clone(&permit);
        tokio::spawn(async move {
            let _permit = pump_permit;
            loop {
                let frame = tokio::select! {
                    _ = &mut cancel_receiver => return,
                    frame = incoming.frame() => frame,
                };
                match frame {
                    Some(Ok(frame)) => {
                        let Ok(bytes) = frame.into_data() else {
                            continue;
                        };
                        if bytes.is_empty() {
                            continue;
                        }
                        let sent = tokio::select! {
                            _ = &mut cancel_receiver => return,
                            sent = body_sender.send(bytes) => sent,
                        };
                        if sent.is_err() {
                            return;
                        }
                    }
                    Some(Err(error)) => {
                        body_sender
                            .send_error(runtime_request_body_error(error, max_request_body_bytes))
                            .await;
                        return;
                    }
                    None => {
                        let _ = body_sender.finish().await;
                        return;
                    }
                }
            }
        });

        let shared = runtime_gateway_pin_request_credentials(&self.shared);
        let worker_permit = Arc::clone(&permit);
        tokio::task::spawn_blocking(move || {
            let _permit = worker_permit;
            let request = RuntimeLocalRewriteRequest::direct(
                method,
                target.path_and_query().to_string(),
                headers,
                network_zone,
                body,
                head_sender,
                Arc::clone(&_permit),
            );
            let result = crate::runtime_panic::catch_runtime_unwind_silently(|| {
                run_runtime_local_rewrite_pipeline(request, target, &shared);
            });
            if let Err(panic) = result {
                runtime_proxy_log(
                    &shared.runtime_shared,
                    format!(
                        "runtime_proxy_worker_panic lane=in_process_gateway panic={}",
                        crate::runtime_panic::runtime_panic_payload_label(panic.as_ref())
                    ),
                );
            }
        });

        head_receiver
            .await
            .map_err(|_| GatewayHandlerError::Unavailable)?
    }

    pub(crate) fn shutdown_and_drain(&self, timeout: Duration) -> bool {
        self.shutdown.store(true, Ordering::Release);
        self.request_slots.close();
        self.health_slots.close();
        let deadline = Instant::now() + timeout;
        while self.request_slots.available_permits() != self.request_limit
            || self.health_slots.available_permits() != HEALTH_REQUEST_LIMIT
            || self
                .shared
                .gateway_background_task_count
                .load(Ordering::Acquire)
                != 0
        {
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let workers_finished = loop {
            let finished = self
                .worker_threads
                .lock()
                .map(|workers| workers.iter().all(thread::JoinHandle::is_finished))
                .unwrap_or(false);
            if finished || Instant::now() >= deadline {
                break finished;
            }
            thread::sleep(Duration::from_millis(10));
        };
        if workers_finished && let Ok(mut workers) = self.worker_threads.lock() {
            while let Some(worker) = workers.pop() {
                let _ = worker.join();
            }
        }
        runtime_proxy_flush_logs_for_path(&self.shared.runtime_shared.log_path);
        workers_finished
    }
}

fn runtime_gateway_network_zone(
    peer_addr: std::net::SocketAddr,
    client_ip: std::net::IpAddr,
    peer_is_trusted_proxy: bool,
) -> prodex_domain::NetworkZone {
    if client_ip.is_loopback() {
        prodex_domain::NetworkZone::Local
    } else if peer_is_trusted_proxy && client_ip_is_private(client_ip) {
        prodex_domain::NetworkZone::TrustedInternal
    } else if peer_is_trusted_proxy {
        prodex_domain::NetworkZone::Public
    } else if peer_addr.ip().is_loopback() {
        prodex_domain::NetworkZone::Local
    } else {
        prodex_domain::NetworkZone::Unknown
    }
}

fn client_ip_is_private(client_ip: std::net::IpAddr) -> bool {
    match client_ip {
        std::net::IpAddr::V4(address) => address.is_private() || address.is_link_local(),
        std::net::IpAddr::V6(address) => {
            address.is_unique_local() || address.is_unicast_link_local()
        }
    }
}

impl Drop for RuntimeGatewayApplication {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        self.request_slots.close();
        self.health_slots.close();
        if let Ok(workers) = self.worker_threads.get_mut() {
            let detached_workers = detach_unfinished_workers(workers);
            if detached_workers != 0 {
                runtime_proxy_log(
                    &self.shared.runtime_shared,
                    format!(
                        "runtime gateway application shutdown detach_workers={detached_workers}"
                    ),
                );
            }
        }
        runtime_proxy_flush_logs_for_path(&self.shared.runtime_shared.log_path);
    }
}

fn detach_unfinished_workers(workers: &mut Vec<thread::JoinHandle<()>>) -> usize {
    let mut detached = 0;
    while let Some(worker) = workers.pop() {
        if worker.is_finished() {
            let _ = worker.join();
        } else {
            detached += 1;
        }
    }
    detached
}

fn try_acquire_gateway_request_permit(
    route: prodex_gateway_http::GatewayHttpRouteKind,
    request_slots: &Arc<Semaphore>,
    health_slots: &Arc<Semaphore>,
) -> Result<Arc<tokio::sync::OwnedSemaphorePermit>, GatewayHandlerError> {
    let health_request = matches!(
        route,
        prodex_gateway_http::GatewayHttpRouteKind::HealthLive
            | prodex_gateway_http::GatewayHttpRouteKind::HealthReady
            | prodex_gateway_http::GatewayHttpRouteKind::HealthStartup
    );
    let slots = if health_request {
        health_slots
    } else {
        request_slots
    };
    Arc::clone(slots)
        .try_acquire_owned()
        .map(Arc::new)
        .map_err(|error| match error {
            TryAcquireError::Closed => GatewayHandlerError::Unavailable,
            TryAcquireError::NoPermits => GatewayHandlerError::Overloaded,
        })
}

struct CancelBodyPumpOnDrop(Option<oneshot::Sender<()>>);

impl Drop for CancelBodyPumpOnDrop {
    fn drop(&mut self) {
        if let Some(cancel) = self.0.take() {
            let _ = cancel.send(());
        }
    }
}

fn runtime_request_body_error(
    error: prodex_gateway_server::GatewayBoxError,
    max_request_body_bytes: u64,
) -> io::Error {
    if error.downcast_ref::<LengthLimitError>().is_some() {
        return io::Error::new(
            io::ErrorKind::InvalidData,
            RuntimeProxyBodyTooLarge::new(max_request_body_bytes, None),
        );
    }
    io::Error::other(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saturated_model_admission_is_fail_fast_while_health_remains_available() {
        let request_slots = Arc::new(Semaphore::new(1));
        let health_slots = Arc::new(Semaphore::new(1));
        let _busy = Arc::clone(&request_slots).try_acquire_owned().unwrap();
        let started = Instant::now();

        assert!(matches!(
            try_acquire_gateway_request_permit(
                prodex_gateway_http::GatewayHttpRouteKind::DataPlaneResponses,
                &request_slots,
                &health_slots,
            ),
            Err(GatewayHandlerError::Overloaded)
        ));
        assert!(started.elapsed() < Duration::from_millis(20));
        assert!(
            try_acquire_gateway_request_permit(
                prodex_gateway_http::GatewayHttpRouteKind::HealthReady,
                &request_slots,
                &health_slots,
            )
            .is_ok()
        );
    }

    #[test]
    fn unfinished_worker_detach_is_bounded() {
        let mut workers = vec![thread::spawn(|| thread::sleep(Duration::from_millis(200)))];
        let started = Instant::now();

        assert_eq!(detach_unfinished_workers(&mut workers), 1);
        assert!(started.elapsed() < Duration::from_millis(50));
        assert!(workers.is_empty());
    }

    #[test]
    fn validated_peer_metadata_maps_to_low_cardinality_governance_zone() {
        assert_eq!(
            runtime_gateway_network_zone(
                "127.0.0.1:4317".parse().unwrap(),
                "127.0.0.1".parse().unwrap(),
                false,
            ),
            prodex_domain::NetworkZone::Local
        );
        assert_eq!(
            runtime_gateway_network_zone(
                "10.0.0.2:4317".parse().unwrap(),
                "10.0.1.7".parse().unwrap(),
                true,
            ),
            prodex_domain::NetworkZone::TrustedInternal
        );
        assert_eq!(
            runtime_gateway_network_zone(
                "10.0.0.2:4317".parse().unwrap(),
                "203.0.113.7".parse().unwrap(),
                true,
            ),
            prodex_domain::NetworkZone::Public
        );
        assert_eq!(
            runtime_gateway_network_zone(
                "192.168.1.2:4317".parse().unwrap(),
                "192.168.1.2".parse().unwrap(),
                false,
            ),
            prodex_domain::NetworkZone::Unknown
        );
    }
}
