use super::*;

impl RuntimePrefetchStream {
    pub(super) fn spawn(
        response: reqwest::Response,
        async_runtime: Arc<TokioRuntime>,
        log_path: PathBuf,
        request_id: u64,
    ) -> Self {
        let (sender, receiver) =
            mpsc::sync_channel::<RuntimePrefetchChunk>(RUNTIME_PROXY_PREFETCH_QUEUE_CAPACITY);
        let shared = Arc::new(RuntimePrefetchSharedState::default());
        let worker_shared = Arc::clone(&shared);
        let worker = async_runtime.spawn(async move {
            runtime_prefetch_response_chunks(response, sender, worker_shared, log_path, request_id)
                .await;
        });
        let worker_abort = worker.abort_handle();
        Self {
            receiver: Some(receiver),
            shared,
            backlog: VecDeque::new(),
            worker_abort: Some(worker_abort),
        }
    }

    async fn recv_timeout_async(
        &mut self,
        timeout: Duration,
    ) -> std::result::Result<RuntimePrefetchChunk, RecvTimeoutError> {
        let started_at = Instant::now();
        let retry_delay =
            Duration::from_millis(runtime_proxy_prefetch_backpressure_retry_ms().max(1));

        loop {
            if let Some(chunk) = self.backlog.pop_front() {
                return Ok(chunk);
            }
            let Some(receiver) = self.receiver.as_ref() else {
                return Err(RecvTimeoutError::Disconnected);
            };
            match receiver.try_recv() {
                Ok(chunk) => {
                    if let RuntimePrefetchChunk::Data(bytes) = &chunk {
                        runtime_prefetch_release_queued_bytes(&self.shared, bytes.len());
                    }
                    return Ok(chunk);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err(RecvTimeoutError::Disconnected);
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
            let elapsed = started_at.elapsed();
            if elapsed >= timeout {
                return Err(RecvTimeoutError::Timeout);
            }
            let sleep_for = retry_delay.min(timeout.saturating_sub(elapsed));
            if !sleep_for.is_zero() {
                tokio::time::sleep(sleep_for).await;
            }
        }
    }

    fn push_backlog(&mut self, chunk: RuntimePrefetchChunk) {
        self.backlog.push_back(chunk);
    }

    pub(super) fn into_reader(mut self, prelude: Vec<u8>) -> Result<RuntimePrefetchReader> {
        let receiver = self
            .receiver
            .take()
            .context("runtime prefetch receiver missing before stream handoff")?;
        let worker_abort = self
            .worker_abort
            .take()
            .context("runtime prefetch abort handle missing before stream handoff")?;
        Ok(RuntimePrefetchReader {
            receiver,
            shared: Arc::clone(&self.shared),
            backlog: std::mem::take(&mut self.backlog),
            pending: Cursor::new(prelude),
            finished: false,
            worker_abort,
        })
    }
}

impl Drop for RuntimePrefetchStream {
    fn drop(&mut self) {
        if let Some(worker_abort) = self.worker_abort.take() {
            worker_abort.abort();
        }
    }
}

pub(crate) fn runtime_prefetch_set_terminal_error(
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

pub(crate) async fn runtime_prefetch_send_with_wait(
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

pub(crate) async fn inspect_runtime_sse_lookahead(
    prefetch: &mut RuntimePrefetchStream,
    log_path: &Path,
    request_id: u64,
) -> Result<RuntimeSseInspection> {
    let deadline = Instant::now() + Duration::from_millis(runtime_proxy_sse_lookahead_timeout_ms());
    let mut buffered = Vec::new();

    loop {
        if buffered.len() >= RUNTIME_PROXY_SSE_LOOKAHEAD_BYTES {
            break;
        }
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        let remaining = deadline.saturating_duration_since(now);
        match prefetch.recv_timeout_async(remaining).await {
            Ok(RuntimePrefetchChunk::Data(chunk)) => {
                buffered.extend_from_slice(&chunk);
                match inspect_runtime_sse_buffer(&buffered) {
                    RuntimeSseInspectionProgress::Commit {
                        response_ids,
                        turn_state,
                    } => {
                        runtime_proxy_log_to_path(
                            log_path,
                            &format!(
                                "request={request_id} transport=http lookahead_commit bytes={} response_ids={}",
                                buffered.len(),
                                response_ids.len()
                            ),
                        );
                        return Ok(RuntimeSseInspection::Commit {
                            prelude: buffered,
                            response_ids,
                            turn_state,
                        });
                    }
                    RuntimeSseInspectionProgress::Hold { .. } => {}
                    RuntimeSseInspectionProgress::QuotaBlocked => {
                        runtime_proxy_log_to_path(
                            log_path,
                            &format!(
                                "request={request_id} transport=http lookahead_retryable_signal bytes={}",
                                buffered.len()
                            ),
                        );
                        return Ok(RuntimeSseInspection::QuotaBlocked(buffered));
                    }
                    RuntimeSseInspectionProgress::PreviousResponseNotFound => {
                        runtime_proxy_log_to_path(
                            log_path,
                            &format!(
                                "request={request_id} transport=http lookahead_retryable_signal bytes={}",
                                buffered.len()
                            ),
                        );
                        return Ok(RuntimeSseInspection::PreviousResponseNotFound(buffered));
                    }
                }
            }
            Ok(RuntimePrefetchChunk::End) => break,
            Ok(RuntimePrefetchChunk::Error(kind, message)) => {
                if buffered.is_empty() {
                    runtime_proxy_log_to_path(
                        log_path,
                        &format!(
                            "request={request_id} transport=http lookahead_error_before_bytes kind={kind:?} error={message}"
                        ),
                    );
                    return Err(anyhow::Error::new(io::Error::new(kind, message))
                        .context("failed to inspect runtime auto-rotate SSE stream"));
                }
                prefetch.push_backlog(RuntimePrefetchChunk::Error(kind, message));
                break;
            }
            Err(RecvTimeoutError::Timeout) => {
                runtime_proxy_log_to_path(
                    log_path,
                    &format!(
                        "request={request_id} transport=http lookahead_timeout bytes={}",
                        buffered.len()
                    ),
                );
                break;
            }
            Err(RecvTimeoutError::Disconnected) => {
                runtime_proxy_log_to_path(
                    log_path,
                    &format!(
                        "request={request_id} transport=http lookahead_channel_disconnected bytes={}",
                        buffered.len()
                    ),
                );
                break;
            }
        }
    }

    match inspect_runtime_sse_buffer(&buffered) {
        RuntimeSseInspectionProgress::Commit {
            response_ids,
            turn_state,
        }
        | RuntimeSseInspectionProgress::Hold {
            response_ids,
            turn_state,
        } => {
            if !buffered.is_empty() {
                runtime_proxy_log_to_path(
                    log_path,
                    &format!(
                        "request={request_id} transport=http lookahead_budget_exhausted bytes={} response_ids={}",
                        buffered.len(),
                        response_ids.len()
                    ),
                );
            }
            Ok(RuntimeSseInspection::Commit {
                prelude: buffered,
                response_ids,
                turn_state,
            })
        }
        RuntimeSseInspectionProgress::QuotaBlocked => {
            Ok(RuntimeSseInspection::QuotaBlocked(buffered))
        }
        RuntimeSseInspectionProgress::PreviousResponseNotFound => {
            Ok(RuntimeSseInspection::PreviousResponseNotFound(buffered))
        }
    }
}

pub(crate) async fn inspect_runtime_sse_lookahead_async(
    mut prefetch: RuntimePrefetchStream,
    log_path: PathBuf,
    request_id: u64,
) -> Result<(RuntimeSseInspection, RuntimePrefetchStream)> {
    let inspection = inspect_runtime_sse_lookahead(&mut prefetch, &log_path, request_id).await?;
    Ok((inspection, prefetch))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_log_path(name: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "prodex-prefetch-{name}-{}-{}.log",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after unix epoch")
                .as_nanos()
        ))
    }

    fn test_prefetch_stream(chunks: Vec<RuntimePrefetchChunk>) -> RuntimePrefetchStream {
        let (sender, receiver) =
            mpsc::sync_channel::<RuntimePrefetchChunk>(RUNTIME_PROXY_PREFETCH_QUEUE_CAPACITY);
        let shared = Arc::new(RuntimePrefetchSharedState::default());
        for chunk in chunks {
            if let RuntimePrefetchChunk::Data(bytes) = &chunk {
                shared.queued_bytes.fetch_add(bytes.len(), Ordering::SeqCst);
            }
            sender.send(chunk).expect("test prefetch chunk should send");
        }
        drop(sender);
        RuntimePrefetchStream {
            receiver: Some(receiver),
            shared,
            backlog: VecDeque::new(),
            worker_abort: None,
        }
    }

    fn block_on_lookahead(
        chunks: Vec<RuntimePrefetchChunk>,
        name: &str,
    ) -> Result<(RuntimeSseInspection, RuntimePrefetchStream)> {
        let prefetch = test_prefetch_stream(chunks);
        let log_path = test_log_path(name);
        TokioRuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
            .block_on(inspect_runtime_sse_lookahead_async(prefetch, log_path, 7))
    }

    #[test]
    fn prefetch_lookahead_commits_response_ids_and_turn_state() {
        let (inspection, prefetch) = block_on_lookahead(
            vec![RuntimePrefetchChunk::Data(
                concat!(
                    ": keep-alive\r\n",
                    "data: {\"type\":\"response.completed\",\"response_id\":\"resp-1\",\"turn_state\":\"ts-1\"}\r\n",
                    "\r\n"
                )
                .as_bytes()
                .to_vec(),
            )],
            "commit",
        )
        .expect("lookahead should inspect");

        match inspection {
            RuntimeSseInspection::Commit {
                prelude,
                response_ids,
                turn_state,
            } => {
                assert!(String::from_utf8_lossy(&prelude).contains("response.completed"));
                assert_eq!(response_ids, vec!["resp-1".to_string()]);
                assert_eq!(turn_state.as_deref(), Some("ts-1"));
            }
            other => panic!("expected commit, got {other:?}"),
        }
        assert_eq!(prefetch.shared.queued_bytes.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn prefetch_lookahead_detects_quota_blocked_before_commit() {
        let (inspection, _prefetch) = block_on_lookahead(
            vec![RuntimePrefetchChunk::Data(
                b"data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"insufficient_quota\",\"message\":\"quota exhausted\"}}}\r\n\r\n".to_vec(),
            )],
            "quota",
        )
        .expect("lookahead should inspect");

        match inspection {
            RuntimeSseInspection::QuotaBlocked(prelude) => {
                assert!(String::from_utf8_lossy(&prelude).contains("insufficient_quota"));
            }
            other => panic!("expected quota blocked, got {other:?}"),
        }
    }

    #[test]
    fn prefetch_lookahead_detects_previous_response_not_found_before_commit() {
        let (inspection, _prefetch) = block_on_lookahead(
            vec![RuntimePrefetchChunk::Data(
                b"data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"previous_response_not_found\",\"message\":\"missing\"}}}".to_vec(),
            )],
            "previous-response-not-found",
        )
        .expect("lookahead should inspect");

        match inspection {
            RuntimeSseInspection::PreviousResponseNotFound(prelude) => {
                assert!(String::from_utf8_lossy(&prelude).contains("previous_response_not_found"));
            }
            other => panic!("expected previous response not found, got {other:?}"),
        }
    }

    #[test]
    fn prefetch_lookahead_timeout_with_partial_hold_commits_buffered_prelude() {
        let (inspection, _prefetch) = block_on_lookahead(
            vec![RuntimePrefetchChunk::Data(
                b"data: {\"type\":\"response.in_progress\",\"response_id\":\"resp-partial\"}"
                    .to_vec(),
            )],
            "partial-timeout",
        )
        .expect("lookahead should inspect");

        match inspection {
            RuntimeSseInspection::Commit {
                prelude,
                response_ids,
                turn_state,
            } => {
                assert!(String::from_utf8_lossy(&prelude).contains("resp-partial"));
                assert_eq!(response_ids, vec!["resp-partial".to_string()]);
                assert_eq!(turn_state, None);
            }
            other => panic!("expected commit after hold timeout, got {other:?}"),
        }
    }

    #[test]
    fn prefetch_lookahead_error_before_bytes_returns_error() {
        let err = match block_on_lookahead(
            vec![RuntimePrefetchChunk::Error(
                io::ErrorKind::TimedOut,
                "upstream timed out".to_string(),
            )],
            "error-before-bytes",
        ) {
            Ok(_) => panic!("lookahead should fail before any prelude bytes"),
            Err(err) => err,
        };

        assert!(
            err.to_string()
                .contains("failed to inspect runtime auto-rotate SSE stream")
        );
    }

    #[test]
    fn prefetch_lookahead_error_after_prelude_preserves_error_backlog() {
        let (inspection, prefetch) = block_on_lookahead(
            vec![
                RuntimePrefetchChunk::Data(
                    b"data: {\"type\":\"response.in_progress\",\"response_id\":\"resp-before-error\"}"
                        .to_vec(),
                ),
                RuntimePrefetchChunk::Error(
                    io::ErrorKind::ConnectionReset,
                    "connection reset".to_string(),
                ),
            ],
            "error-after-prelude",
        )
        .expect("lookahead should keep partial prelude");

        match inspection {
            RuntimeSseInspection::Commit {
                prelude,
                response_ids,
                turn_state,
            } => {
                assert!(String::from_utf8_lossy(&prelude).contains("resp-before-error"));
                assert_eq!(response_ids, vec!["resp-before-error".to_string()]);
                assert_eq!(turn_state, None);
            }
            other => panic!("expected partial commit, got {other:?}"),
        }
        assert!(matches!(
            prefetch.backlog.front(),
            Some(RuntimePrefetchChunk::Error(io::ErrorKind::ConnectionReset, message))
                if message == "connection reset"
        ));
    }
}
