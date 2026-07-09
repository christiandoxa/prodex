use super::*;

mod lookahead;
mod worker;

pub(crate) use lookahead::inspect_runtime_sse_lookahead_async;
pub(crate) use worker::{
    runtime_prefetch_release_queued_bytes, runtime_prefetch_response_chunks,
    runtime_prefetch_terminal_error,
};

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
    fn prefetch_lookahead_rate_limits_before_quota_stays_retryable() {
        let (inspection, _prefetch) = block_on_lookahead(
            vec![
                RuntimePrefetchChunk::Data(
                    b"data: {\"type\":\"codex.rate_limits\",\"rate_limits\":[]}\r\n\r\n".to_vec(),
                ),
                RuntimePrefetchChunk::Data(
                    b"data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"insufficient_quota\",\"message\":\"quota exhausted\"}}}\r\n\r\n".to_vec(),
                ),
            ],
            "rate-limits-before-quota",
        )
        .expect("lookahead should inspect");

        match inspection {
            RuntimeSseInspection::QuotaBlocked(prelude) => {
                let prelude = String::from_utf8_lossy(&prelude);
                assert!(prelude.contains("codex.rate_limits"));
                assert!(prelude.contains("insufficient_quota"));
            }
            other => panic!("expected quota blocked after sideband prelude, got {other:?}"),
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
