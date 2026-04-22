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

    fn recv_timeout(
        &mut self,
        timeout: Duration,
    ) -> std::result::Result<RuntimePrefetchChunk, RecvTimeoutError> {
        if let Some(chunk) = self.backlog.pop_front() {
            return Ok(chunk);
        }
        let chunk = self
            .receiver
            .as_ref()
            .expect("runtime prefetch receiver should remain available")
            .recv_timeout(timeout)?;
        if let RuntimePrefetchChunk::Data(bytes) = &chunk {
            runtime_prefetch_release_queued_bytes(&self.shared, bytes.len());
        }
        Ok(chunk)
    }

    fn push_backlog(&mut self, chunk: RuntimePrefetchChunk) {
        self.backlog.push_back(chunk);
    }

    pub(super) fn into_reader(mut self, prelude: Vec<u8>) -> RuntimePrefetchReader {
        RuntimePrefetchReader {
            receiver: self
                .receiver
                .take()
                .expect("runtime prefetch receiver should remain available"),
            shared: Arc::clone(&self.shared),
            backlog: std::mem::take(&mut self.backlog),
            pending: Cursor::new(prelude),
            finished: false,
            worker_abort: self
                .worker_abort
                .take()
                .expect("runtime prefetch abort handle should remain available"),
        }
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
                    &format!(
                        "request={request_id} transport=http upstream_stream_end saw_data={saw_data}"
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
                        &format!(
                            "request={request_id} transport=http first_upstream_chunk bytes={}",
                            chunk.len()
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
                    &format!(
                        "request={request_id} transport=http upstream_stream_error kind={kind:?} error={err}"
                    ),
                );
                let _ = sender.try_send(RuntimePrefetchChunk::Error(kind, err.to_string()));
                break;
            }
        }
    }
}

pub(crate) fn inspect_runtime_sse_lookahead(
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
        match prefetch.recv_timeout(remaining) {
            Ok(RuntimePrefetchChunk::Data(chunk)) => {
                buffered.extend_from_slice(&chunk);
                match inspect_runtime_sse_buffer(&buffered)? {
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

    match inspect_runtime_sse_buffer(&buffered)? {
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

pub(crate) fn inspect_runtime_sse_buffer(buffered: &[u8]) -> Result<RuntimeSseInspectionProgress> {
    let mut line = Vec::new();
    let mut data_lines = Vec::new();
    let mut response_ids = BTreeSet::new();
    let mut saw_commit_ready_event = false;
    let mut turn_state = None::<String>;
    let mut process_event = |event: RuntimeParsedSseEvent| {
        if event.quota_blocked {
            return Some(RuntimeSseInspectionProgress::QuotaBlocked);
        }
        if event.previous_response_not_found {
            return Some(RuntimeSseInspectionProgress::PreviousResponseNotFound);
        }
        response_ids.extend(event.response_ids);
        if event.turn_state.is_some() {
            turn_state = event.turn_state;
        }
        if !event
            .event_type
            .as_deref()
            .is_some_and(runtime_proxy_precommit_hold_event_kind)
        {
            saw_commit_ready_event = true;
        }
        None
    };
    let mut terminal = None;
    runtime_sse_consume_chunk(&mut line, &mut data_lines, buffered, |event| {
        if terminal.is_none() {
            terminal = process_event(event);
        }
    });
    if terminal.is_none() {
        runtime_sse_finish_pending(&mut line, &mut data_lines, |event| {
            if terminal.is_none() {
                terminal = process_event(event);
            }
        });
    }
    if let Some(progress) = terminal {
        return Ok(progress);
    }

    if saw_commit_ready_event {
        Ok(RuntimeSseInspectionProgress::Commit {
            response_ids: response_ids.into_iter().collect(),
            turn_state,
        })
    } else {
        Ok(RuntimeSseInspectionProgress::Hold {
            response_ids: response_ids.into_iter().collect(),
            turn_state,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspect_runtime_sse_buffer_handles_comments_crlf_and_partial_tail() {
        let progress = inspect_runtime_sse_buffer(
            concat!(
                ": keep-alive\r\n",
                "data: {\"type\":\"response.completed\",\"response_id\":\"resp-1\",\"turn_state\":\"ts-1\"}\r\n",
                "\r\n",
                "data: {\"type\":\"response.in_progress\",\"response_id\":\"resp-2\"}"
            )
            .as_bytes(),
        )
        .expect("buffer should inspect");

        match progress {
            RuntimeSseInspectionProgress::Commit {
                response_ids,
                turn_state,
            } => {
                assert_eq!(
                    response_ids,
                    vec!["resp-1".to_string(), "resp-2".to_string()]
                );
                assert_eq!(turn_state.as_deref(), Some("ts-1"));
            }
            other => panic!("expected commit progress, got {other:?}"),
        }
    }

    #[test]
    fn inspect_runtime_sse_buffer_detects_previous_response_not_found_from_partial_event() {
        let progress = inspect_runtime_sse_buffer(
            b"data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"previous_response_not_found\",\"message\":\"missing\"}}}",
        )
        .expect("buffer should inspect");

        assert!(matches!(
            progress,
            RuntimeSseInspectionProgress::PreviousResponseNotFound
        ));
    }
}
