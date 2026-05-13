use super::*;

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
        shared.queued_bytes.fetch_sub(bytes, Ordering::SeqCst);
    }
}

async fn runtime_prefetch_send_with_wait(
    sender: &SyncSender<RuntimePrefetchChunk>,
    shared: &RuntimePrefetchSharedState,
    chunk: Vec<u8>,
) -> RuntimePrefetchSendOutcome {
    let started_at = Instant::now();
    let retry_delay = Duration::from_millis(runtime_proxy_prefetch_backpressure_retry_ms());
    let timeout = Duration::from_millis(runtime_proxy_prefetch_backpressure_timeout_ms());
    let buffered_limit = runtime_proxy_prefetch_max_buffered_bytes().max(1);
    let mut pending = RuntimePrefetchChunk::Data(chunk);
    let mut retries = 0usize;
    loop {
        let chunk_bytes = match &pending {
            RuntimePrefetchChunk::Data(bytes) => bytes.len(),
            RuntimePrefetchChunk::End | RuntimePrefetchChunk::Error(_, _) => 0,
        };
        let queued_bytes = shared.queued_bytes.load(Ordering::SeqCst);
        if queued_bytes.saturating_add(chunk_bytes) > buffered_limit {
            if started_at.elapsed() >= timeout {
                return RuntimePrefetchSendOutcome::TimedOut {
                    message: format!(
                        "runtime prefetch buffered bytes exceeded safe limit ({} > {})",
                        queued_bytes.saturating_add(chunk_bytes),
                        buffered_limit
                    ),
                };
            }
            retries = retries.saturating_add(1);
            let remaining = timeout.saturating_sub(started_at.elapsed());
            let sleep_for = retry_delay.min(remaining);
            if !sleep_for.is_zero() {
                tokio::time::sleep(sleep_for).await;
            }
            continue;
        }
        match sender.try_send(pending) {
            Ok(()) => {
                if chunk_bytes > 0 {
                    shared.queued_bytes.fetch_add(chunk_bytes, Ordering::SeqCst);
                }
                return RuntimePrefetchSendOutcome::Sent {
                    wait_ms: started_at.elapsed().as_millis(),
                    retries,
                };
            }
            Err(TrySendError::Disconnected(_)) => {
                return RuntimePrefetchSendOutcome::Disconnected;
            }
            Err(TrySendError::Full(returned)) => {
                if started_at.elapsed() >= timeout {
                    return RuntimePrefetchSendOutcome::TimedOut {
                        message: format!(
                            "runtime prefetch backlog exceeded bounded capacity ({})",
                            RUNTIME_PROXY_PREFETCH_QUEUE_CAPACITY
                        ),
                    };
                }
                pending = returned;
                retries = retries.saturating_add(1);
                let remaining = timeout.saturating_sub(started_at.elapsed());
                let sleep_for = retry_delay.min(remaining);
                if !sleep_for.is_zero() {
                    tokio::time::sleep(sleep_for).await;
                }
            }
        }
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
    loop {
        match response.chunk().await {
            Ok(None) => {
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
                let _ = sender.try_send(RuntimePrefetchChunk::End);
                break;
            }
            Ok(Some(chunk)) => {
                if !saw_data {
                    saw_data = true;
                    runtime_proxy_log_to_path(
                        &log_path,
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
                    runtime_prefetch_set_terminal_error(
                        &shared,
                        io::ErrorKind::InvalidData,
                        message.clone(),
                    );
                    runtime_proxy_log_to_path(
                        &log_path,
                        &format!(
                            "request={request_id} transport=http prefetch_chunk_too_large bytes={} limit={} error={message}",
                            chunk.len(),
                            RUNTIME_PROXY_PREFETCH_MAX_CHUNK_BYTES,
                        ),
                    );
                    let _ = sender.try_send(RuntimePrefetchChunk::Error(
                        io::ErrorKind::InvalidData,
                        message,
                    ));
                    break;
                }
                let chunk_bytes = chunk.len();
                match runtime_prefetch_send_with_wait(&sender, &shared, chunk.to_vec()).await {
                    RuntimePrefetchSendOutcome::Sent { wait_ms, retries } => {
                        if retries > 0 {
                            runtime_proxy_log_to_path(
                                &log_path,
                                &format!(
                                    "request={request_id} transport=http prefetch_backpressure_recovered bytes={chunk_bytes} retries={retries} wait_ms={wait_ms}",
                                ),
                            );
                        }
                    }
                    RuntimePrefetchSendOutcome::TimedOut { message } => {
                        runtime_prefetch_set_terminal_error(
                            &shared,
                            io::ErrorKind::WouldBlock,
                            message.clone(),
                        );
                        runtime_proxy_log_to_path(
                            &log_path,
                            &format!(
                                "request={request_id} transport=http prefetch_backpressure_timeout bytes={chunk_bytes} capacity={} error={message}",
                                RUNTIME_PROXY_PREFETCH_QUEUE_CAPACITY,
                            ),
                        );
                        break;
                    }
                    RuntimePrefetchSendOutcome::Disconnected => {
                        runtime_proxy_log_to_path(
                            &log_path,
                            &format!(
                                "request={request_id} transport=http prefetch_receiver_disconnected"
                            ),
                        );
                        break;
                    }
                }
            }
            Err(err) => {
                let kind = runtime_reqwest_error_kind(&err);
                runtime_prefetch_set_terminal_error(&shared, kind, err.to_string());
                runtime_proxy_log_to_path(
                    &log_path,
                    &runtime_proxy_structured_log_message(
                        "upstream_stream_error",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("transport", "http"),
                            runtime_proxy_log_field("kind", format!("{kind:?}")),
                            runtime_proxy_log_field("error", err.to_string()),
                        ],
                    ),
                );
                let _ = sender.try_send(RuntimePrefetchChunk::Error(kind, err.to_string()));
                break;
            }
        }
    }
}
