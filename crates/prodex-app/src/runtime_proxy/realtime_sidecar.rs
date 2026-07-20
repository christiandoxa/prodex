use super::{
    RuntimeProxyRequest, RuntimeRotationProxyShared,
    acquire_runtime_proxy_active_request_slot_with_wait, is_runtime_realtime_websocket_path,
    mark_runtime_proxy_local_overload, run_runtime_proxy_websocket_session, runtime_proxy_log,
    runtime_proxy_log_field, runtime_proxy_next_request_id, runtime_proxy_structured_log_message,
    runtime_websocket_error_log_value, try_spawn_runtime_supervised_worker,
};
use crate::TinyReadWrite;
use anyhow::{Context, Result};
use std::io::{self, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const RUNTIME_REALTIME_PUMP_TIMEOUT: Duration = Duration::from_millis(20);
const RUNTIME_REALTIME_IDLE_SLEEP: Duration = Duration::from_millis(5);
const RUNTIME_REALTIME_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);

struct RuntimeRealtimeHandshakeCapture {
    captured: Arc<Mutex<Option<RuntimeProxyRequest>>>,
}

impl tungstenite::handshake::server::Callback for RuntimeRealtimeHandshakeCapture {
    fn on_request(
        self,
        request: &tungstenite::handshake::server::Request,
        response: tungstenite::handshake::server::Response,
    ) -> std::result::Result<
        tungstenite::handshake::server::Response,
        tungstenite::handshake::server::ErrorResponse,
    > {
        let path_and_query = request
            .uri()
            .path_and_query()
            .map(|value| value.as_str())
            .unwrap_or("/");
        if !is_runtime_realtime_websocket_path(path_and_query) {
            return Err(tungstenite::http::Response::builder()
                .status(404)
                .body(Some("unsupported realtime websocket path".to_string()))
                .expect("static realtime rejection response should build"));
        }
        let headers = request
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_string(), value.to_string()))
            })
            .collect();
        *self
            .captured
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(RuntimeProxyRequest {
            method: request.method().as_str().to_string(),
            path_and_query: path_and_query.to_string(),
            headers,
            body: Vec::new(),
        });
        Ok(response)
    }
}

pub(crate) fn spawn_runtime_realtime_websocket_sidecar(
    shared: &RuntimeRotationProxyShared,
    shutdown: &Arc<AtomicBool>,
    worker_threads: &mut Vec<thread::JoinHandle<()>>,
    worker_count: usize,
) -> Result<SocketAddr> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .context("failed to bind runtime realtime websocket sidecar")?;
    listener
        .set_nonblocking(true)
        .context("failed to configure runtime realtime websocket sidecar")?;
    let listen_addr = listener
        .local_addr()
        .context("failed to read runtime realtime websocket sidecar address")?;

    for worker_index in 0..worker_count.max(1) {
        let listener = listener
            .try_clone()
            .context("failed to clone runtime realtime websocket listener")?;
        let shared = shared.clone();
        let worker_shutdown = Arc::clone(shutdown);
        worker_threads.push(try_spawn_runtime_supervised_worker(
            format!("prodex-runtime-realtime-{worker_index}"),
            shared.log_path.clone(),
            Arc::clone(shutdown),
            move || {
                while !worker_shutdown.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            let request_id = runtime_proxy_next_request_id(&shared);
                            let guard = match acquire_runtime_proxy_active_request_slot_with_wait(
                                &shared,
                                "websocket",
                                "/realtime",
                            ) {
                                Ok(guard) => guard,
                                Err(_) => {
                                    mark_runtime_proxy_local_overload(
                                        &shared,
                                        "realtime_sidecar_admission",
                                    );
                                    let _ = stream.write_all(
                                        b"HTTP/1.1 503 Service Unavailable\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
                                    );
                                    continue;
                                }
                            };
                            if let Err(error) = handle_runtime_realtime_websocket_stream(
                                request_id,
                                stream,
                                &shared,
                            ) {
                                runtime_proxy_log(
                                    &shared,
                                    runtime_proxy_structured_log_message(
                                        "realtime_sidecar_error",
                                        [
                                            runtime_proxy_log_field(
                                                "request",
                                                request_id.to_string(),
                                            ),
                                            runtime_proxy_log_field(
                                                "error",
                                                runtime_websocket_error_log_value(
                                                    &error.to_string(),
                                                ),
                                            ),
                                        ],
                                    ),
                                );
                            }
                            drop(guard);
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(RUNTIME_REALTIME_IDLE_SLEEP);
                        }
                        Err(error) => {
                            runtime_proxy_log(
                                &shared,
                                runtime_proxy_structured_log_message(
                                    "realtime_sidecar_accept_error",
                                    [runtime_proxy_log_field(
                                        "error",
                                        runtime_websocket_error_log_value(&error.to_string()),
                                    )],
                                ),
                            );
                            thread::sleep(Duration::from_millis(50));
                        }
                    }
                }
            },
        )?);
    }

    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "realtime_sidecar_started",
            [runtime_proxy_log_field(
                "listen_addr",
                listen_addr.to_string(),
            )],
        ),
    );
    Ok(listen_addr)
}

fn handle_runtime_realtime_websocket_stream(
    request_id: u64,
    stream: TcpStream,
    shared: &RuntimeRotationProxyShared,
) -> Result<()> {
    stream
        .set_read_timeout(Some(RUNTIME_REALTIME_HANDSHAKE_TIMEOUT))
        .context("failed to configure realtime websocket handshake read timeout")?;
    stream
        .set_write_timeout(Some(RUNTIME_REALTIME_HANDSHAKE_TIMEOUT))
        .context("failed to configure realtime websocket handshake write timeout")?;
    let timeout_stream = stream
        .try_clone()
        .context("failed to clone realtime websocket stream")?;
    let captured = Arc::new(Mutex::new(None));
    let stream = Box::new(stream) as Box<dyn TinyReadWrite + Send>;
    let mut local_socket = tungstenite::accept_hdr(
        stream,
        RuntimeRealtimeHandshakeCapture {
            captured: Arc::clone(&captured),
        },
    )
    .map_err(|error| anyhow::anyhow!("failed to accept realtime websocket: {error}"))?;
    timeout_stream
        .set_read_timeout(Some(RUNTIME_REALTIME_PUMP_TIMEOUT))
        .context("failed to configure realtime websocket pump timeout")?;
    let handshake_request = captured
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
        .context("realtime websocket handshake metadata was not captured")?;

    run_runtime_proxy_websocket_session(
        request_id,
        &mut local_socket,
        &handshake_request,
        shared,
        true,
    )
}
