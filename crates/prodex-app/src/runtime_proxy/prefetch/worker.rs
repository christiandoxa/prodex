use super::{
    RUNTIME_PROXY_PREFETCH_MAX_CHUNK_BYTES, RUNTIME_PROXY_PREFETCH_QUEUE_CAPACITY,
    RuntimePrefetchChunk, RuntimePrefetchSendOutcome, RuntimePrefetchSharedState,
    runtime_proxy_log_to_path, runtime_reqwest_error_kind,
};
use redaction::redaction_redact_secret_like_text;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{SyncSender, TrySendError};
use std::time::{Duration, Instant};

fn runtime_prefetch_set_terminal_error(
    shared: &RuntimePrefetchSharedState,
    kind: io::ErrorKind,
    message: impl Into<String>,
) {
    let mut terminal_error = shared
        .terminal_error
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if terminal_error.is_none() {
        *terminal_error = Some((kind, message.into()));
    }
}

fn runtime_prefetch_error_log_value(error: &str) -> String {
    redaction_redact_secret_like_text(error).replace('\n', " ")
}

pub(crate) fn runtime_prefetch_terminal_error(
    shared: &RuntimePrefetchSharedState,
) -> Option<(io::ErrorKind, String)> {
    shared
        .terminal_error
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

pub(crate) fn runtime_prefetch_release_queued_bytes(
    shared: &RuntimePrefetchSharedState,
    bytes: usize,
) {
    if bytes > 0 {
        let _ = shared
            .queued_bytes
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |queued| {
                Some(queued.saturating_sub(bytes))
            });
    }
}

async fn runtime_prefetch_send_with_wait(
    sender: &SyncSender<RuntimePrefetchChunk>,
    shared: &RuntimePrefetchSharedState,
    chunk: RuntimePrefetchChunk,
) -> RuntimePrefetchSendOutcome {
    let started_at = Instant::now();
    let retry_delay = Duration::from_millis(shared.config.retry_delay_ms);
    let timeout = Duration::from_millis(shared.config.timeout_ms);
    let buffered_limit = shared.config.max_buffered_bytes.max(1);
    let mut pending = chunk;
    let mut retries = 0usize;
    loop {
        let chunk_bytes = match &pending {
            RuntimePrefetchChunk::Data(bytes) => bytes.len(),
            RuntimePrefetchChunk::End | RuntimePrefetchChunk::Error(_, _) => 0,
        };
        if let Some(outcome) = runtime_prefetch_wait_for_buffer_capacity(
            shared,
            started_at,
            retry_delay,
            timeout,
            buffered_limit,
            chunk_bytes,
            &mut retries,
        )
        .await
        {
            return outcome;
        }
        if chunk_bytes > 0 {
            shared.queued_bytes.fetch_add(chunk_bytes, Ordering::SeqCst);
        }
        match sender.try_send(pending) {
            Ok(()) => {
                return RuntimePrefetchSendOutcome::Sent {
                    wait_ms: started_at.elapsed().as_millis(),
                    retries,
                };
            }
            Err(TrySendError::Disconnected(_)) => {
                runtime_prefetch_release_queued_bytes(shared, chunk_bytes);
                return RuntimePrefetchSendOutcome::Disconnected;
            }
            Err(TrySendError::Full(returned)) => {
                runtime_prefetch_release_queued_bytes(shared, chunk_bytes);
                match runtime_prefetch_retry_after_full(
                    returned,
                    started_at,
                    retry_delay,
                    timeout,
                    &mut retries,
                )
                .await
                {
                    Ok(next) => pending = next,
                    Err(outcome) => return outcome,
                }
            }
        }
    }
}

async fn runtime_prefetch_wait_for_buffer_capacity(
    shared: &RuntimePrefetchSharedState,
    started_at: Instant,
    retry_delay: Duration,
    timeout: Duration,
    buffered_limit: usize,
    chunk_bytes: usize,
    retries: &mut usize,
) -> Option<RuntimePrefetchSendOutcome> {
    loop {
        let queued_bytes = shared.queued_bytes.load(Ordering::SeqCst);
        if (chunk_bytes > buffered_limit && queued_bytes == 0)
            || queued_bytes.saturating_add(chunk_bytes) <= buffered_limit
        {
            return None;
        }
        if started_at.elapsed() >= timeout {
            return Some(RuntimePrefetchSendOutcome::TimedOut {
                message: format!(
                    "runtime prefetch buffered bytes exceeded safe limit ({} > {})",
                    queued_bytes.saturating_add(chunk_bytes),
                    buffered_limit
                ),
            });
        }
        *retries = retries.saturating_add(1);
        runtime_prefetch_sleep_before_retry(started_at, retry_delay, timeout).await;
    }
}

async fn runtime_prefetch_retry_after_full(
    returned: RuntimePrefetchChunk,
    started_at: Instant,
    retry_delay: Duration,
    timeout: Duration,
    retries: &mut usize,
) -> Result<RuntimePrefetchChunk, RuntimePrefetchSendOutcome> {
    if started_at.elapsed() >= timeout {
        return Err(RuntimePrefetchSendOutcome::TimedOut {
            message: format!(
                "runtime prefetch backlog exceeded bounded capacity ({})",
                RUNTIME_PROXY_PREFETCH_QUEUE_CAPACITY
            ),
        });
    }
    *retries = retries.saturating_add(1);
    runtime_prefetch_sleep_before_retry(started_at, retry_delay, timeout).await;
    Ok(returned)
}

async fn runtime_prefetch_sleep_before_retry(
    started_at: Instant,
    retry_delay: Duration,
    timeout: Duration,
) {
    let remaining = timeout.saturating_sub(started_at.elapsed());
    let sleep_for = retry_delay.min(remaining);
    if !sleep_for.is_zero() {
        tokio::time::sleep(sleep_for).await;
    }
}

pub(crate) async fn runtime_prefetch_response_chunks(
    mut response: reqwest::Response,
    sender: SyncSender<RuntimePrefetchChunk>,
    shared: Arc<RuntimePrefetchSharedState>,
    log_path: PathBuf,
    request_id: u64,
) {
    let mut saw_data = false;
    let chunk_idle_timeout = Duration::from_millis(shared.config.stream_idle_timeout_ms.max(1));
    loop {
        match tokio::time::timeout(chunk_idle_timeout, response.chunk()).await {
            Err(_) => {
                let error = "runtime upstream stream idle timed out".to_string();
                runtime_prefetch_set_terminal_error(
                    &shared,
                    io::ErrorKind::TimedOut,
                    error.clone(),
                );
                runtime_proxy_log_to_path(
                    &log_path,
                    &runtime_proxy_structured_log_message(
                        "upstream_stream_idle_timeout",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("transport", "http"),
                            runtime_proxy_log_field(
                                "timeout_ms",
                                shared.config.stream_idle_timeout_ms.max(1).to_string(),
                            ),
                        ],
                    ),
                );
                let _ = runtime_prefetch_send_with_wait(
                    &sender,
                    &shared,
                    RuntimePrefetchChunk::Error(io::ErrorKind::TimedOut, error),
                )
                .await;
                break;
            }
            Ok(Ok(None)) => {
                runtime_proxy_log_to_path(
                    &log_path,
                    &runtime_proxy_structured_log_message(
                        "upstream_stream_end",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("transport", "http"),
                            runtime_proxy_log_field("saw_data", saw_data.to_string()),
                        ],
                    ),
                );
                let _ =
                    runtime_prefetch_send_with_wait(&sender, &shared, RuntimePrefetchChunk::End)
                        .await;
                break;
            }
            Ok(Ok(Some(chunk))) => {
                if !runtime_prefetch_forward_chunk(
                    chunk,
                    &sender,
                    &shared,
                    &log_path,
                    request_id,
                    &mut saw_data,
                )
                .await
                {
                    break;
                }
            }
            Ok(Err(err)) => {
                let kind = runtime_reqwest_error_kind(&err);
                let error = runtime_prefetch_error_log_value(&err.to_string());
                runtime_prefetch_set_terminal_error(&shared, kind, error.clone());
                runtime_proxy_log_to_path(
                    &log_path,
                    &runtime_proxy_structured_log_message(
                        "upstream_stream_error",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("transport", "http"),
                            runtime_proxy_log_field("kind", format!("{kind:?}")),
                            runtime_proxy_log_field("error", error.as_str()),
                        ],
                    ),
                );
                let _ = runtime_prefetch_send_with_wait(
                    &sender,
                    &shared,
                    RuntimePrefetchChunk::Error(kind, error),
                )
                .await;
                break;
            }
        }
    }
}

async fn runtime_prefetch_forward_chunk(
    chunk: bytes::Bytes,
    sender: &SyncSender<RuntimePrefetchChunk>,
    shared: &RuntimePrefetchSharedState,
    log_path: &Path,
    request_id: u64,
    saw_data: &mut bool,
) -> bool {
    if !*saw_data {
        *saw_data = true;
        runtime_proxy_log_to_path(
            log_path,
            &runtime_proxy_structured_log_message(
                "first_upstream_chunk",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("bytes", chunk.len().to_string()),
                ],
            ),
        );
    }
    if chunk.len() > RUNTIME_PROXY_PREFETCH_MAX_CHUNK_BYTES {
        let message = format!(
            "runtime upstream chunk exceeded prefetch limit ({} > {})",
            chunk.len(),
            RUNTIME_PROXY_PREFETCH_MAX_CHUNK_BYTES
        );
        runtime_prefetch_set_terminal_error(shared, io::ErrorKind::InvalidData, message.clone());
        runtime_proxy_log_to_path(
            log_path,
            &format!(
                "request={request_id} transport=http prefetch_chunk_too_large bytes={} limit={} error={message}",
                chunk.len(),
                RUNTIME_PROXY_PREFETCH_MAX_CHUNK_BYTES,
            ),
        );
        let _ = runtime_prefetch_send_with_wait(
            sender,
            shared,
            RuntimePrefetchChunk::Error(io::ErrorKind::InvalidData, message),
        )
        .await;
        return false;
    }
    let chunk_bytes = chunk.len();
    match runtime_prefetch_send_with_wait(
        sender,
        shared,
        RuntimePrefetchChunk::Data(chunk.to_vec()),
    )
    .await
    {
        RuntimePrefetchSendOutcome::Sent { wait_ms, retries } => {
            if retries > 0 {
                runtime_proxy_log_to_path(
                    log_path,
                    &format!(
                        "request={request_id} transport=http prefetch_backpressure_recovered bytes={chunk_bytes} retries={retries} wait_ms={wait_ms}",
                    ),
                );
            }
            true
        }
        RuntimePrefetchSendOutcome::TimedOut { message } => {
            runtime_prefetch_set_terminal_error(shared, io::ErrorKind::WouldBlock, message.clone());
            runtime_proxy_log_to_path(
                log_path,
                &format!(
                    "request={request_id} transport=http prefetch_backpressure_timeout bytes={chunk_bytes} capacity={} error={message}",
                    RUNTIME_PROXY_PREFETCH_QUEUE_CAPACITY,
                ),
            );
            false
        }
        RuntimePrefetchSendOutcome::Disconnected => {
            runtime_proxy_log_to_path(
                log_path,
                &format!("request={request_id} transport=http prefetch_receiver_disconnected"),
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RuntimePrefetchConfig;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn prefetch_error_log_value_redacts_secret_like_material() {
        let message = runtime_prefetch_error_log_value(
            "prefetch failed\nAuthorization: Bearer prefetch-token\napi_key=prefetch-key",
        );

        assert!(!message.contains('\n'));
        assert!(message.contains("Authorization: Bearer <redacted>"));
        assert!(message.contains("api_key=<redacted>"));
        assert!(!message.contains("prefetch-token"));
        assert!(!message.contains("prefetch-key"));
    }

    #[test]
    fn prefetch_sends_one_valid_chunk_larger_than_buffer_limit() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("prefetch test runtime should build");
        let shared = RuntimePrefetchSharedState {
            config: RuntimePrefetchConfig {
                retry_delay_ms: 1,
                timeout_ms: 20,
                max_buffered_bytes: 4,
                ..RuntimePrefetchConfig::default()
            },
            ..RuntimePrefetchSharedState::default()
        };
        let (sender, receiver) = mpsc::sync_channel(2);

        let outcome = runtime.block_on(runtime_prefetch_send_with_wait(
            &sender,
            &shared,
            RuntimePrefetchChunk::Data(vec![b'x'; 8]),
        ));

        assert!(matches!(
            outcome,
            RuntimePrefetchSendOutcome::Sent { retries: 0, .. }
        ));
        assert!(matches!(
            receiver.recv().expect("oversized chunk should be queued"),
            RuntimePrefetchChunk::Data(bytes) if bytes == vec![b'x'; 8]
        ));
        assert_eq!(shared.queued_bytes.load(Ordering::SeqCst), 8);
        runtime_prefetch_release_queued_bytes(&shared, 8);
        assert_eq!(shared.queued_bytes.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn prefetch_disconnected_send_releases_reserved_bytes() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("prefetch test runtime should build");
        let shared = RuntimePrefetchSharedState::default();
        let (sender, receiver) = mpsc::sync_channel(1);
        drop(receiver);

        let outcome = runtime.block_on(runtime_prefetch_send_with_wait(
            &sender,
            &shared,
            RuntimePrefetchChunk::Data(vec![b'x'; 8]),
        ));

        assert_eq!(outcome, RuntimePrefetchSendOutcome::Disconnected);
        assert_eq!(shared.queued_bytes.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn reqwest_chunk_idle_timeout_uses_configured_stream_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("mock upstream should bind");
        let address = listener
            .local_addr()
            .expect("mock upstream address should be available");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("mock upstream should accept");
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n5\r\ndata:\r\n",
                )
                .expect("mock upstream should send the first chunk");
            stream.flush().expect("mock upstream should flush");
            thread::sleep(Duration::from_millis(80));
        });

        let log_path = std::env::temp_dir().join(format!(
            "prodex-prefetch-timeout-{}-{}.log",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after unix epoch")
                .as_nanos()
        ));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("prefetch test runtime should build");
        runtime.block_on(async {
            let response = reqwest::Client::new()
                .get(format!("http://{address}/v1/responses"))
                .send()
                .await
                .expect("mock upstream response should arrive");
            let shared = Arc::new(RuntimePrefetchSharedState {
                config: RuntimePrefetchConfig {
                    retry_delay_ms: 1,
                    timeout_ms: 100,
                    max_buffered_bytes: 1024,
                    lookahead_timeout_ms: 100,
                    stream_idle_timeout_ms: 20,
                },
                ..RuntimePrefetchSharedState::default()
            });
            let (sender, receiver) = mpsc::sync_channel(2);
            runtime_prefetch_response_chunks(
                response,
                sender,
                Arc::clone(&shared),
                log_path.clone(),
                7,
            )
            .await;

            assert!(matches!(
                receiver.recv().expect("first chunk should be queued"),
                RuntimePrefetchChunk::Data(bytes) if bytes.as_slice() == b"data:"
            ));
            assert!(matches!(
                receiver.recv().expect("timeout should be queued"),
                RuntimePrefetchChunk::Error(io::ErrorKind::TimedOut, message)
                    if message == "runtime upstream stream idle timed out"
            ));
            assert_eq!(
                runtime_prefetch_terminal_error(&shared),
                Some((
                    io::ErrorKind::TimedOut,
                    "runtime upstream stream idle timed out".to_string()
                ))
            );
        });
        server.join().expect("mock upstream should finish");
        let _ = std::fs::remove_file(log_path);
    }
}
