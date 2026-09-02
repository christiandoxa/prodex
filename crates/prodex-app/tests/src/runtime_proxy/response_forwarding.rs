use super::*;

struct SharedBufferWriter {
    bytes: Arc<Mutex<Vec<u8>>>,
}

struct ChunkedReader {
    chunks: VecDeque<Vec<u8>>,
}

impl ChunkedReader {
    fn new(chunks: impl IntoIterator<Item = &'static [u8]>) -> Self {
        Self {
            chunks: chunks
                .into_iter()
                .map(|chunk| chunk.to_vec())
                .collect::<VecDeque<_>>(),
        }
    }
}

impl Read for ChunkedReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let Some(mut chunk) = self.chunks.pop_front() else {
            return Ok(0);
        };
        let read = chunk.len().min(buf.len());
        buf[..read].copy_from_slice(&chunk[..read]);
        if read < chunk.len() {
            chunk.drain(..read);
            self.chunks.push_front(chunk);
        }
        Ok(read)
    }
}

impl Write for SharedBufferWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bytes
            .lock()
            .map_err(|_| io::Error::other("test writer buffer is poisoned"))?
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn test_runtime_streaming_shared(log_path: PathBuf) -> RuntimeRotationProxyShared {
    let root = env::temp_dir().join(format!(
        "prodex-response-forwarding-test-{}",
        std::process::id()
    ));
    let paths = AppPaths {
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared-codex"),
        legacy_shared_codex_root: root.join("shared"),
        root,
    };

    RuntimeRotationProxyShared {
        smart_context_engine: std::sync::Arc::new(crate::RuntimeSmartContextEngine::default()),
        runtime_config: Arc::new(crate::RuntimeConfig::compatibility_current()),
        upstream_no_proxy: false,
        auto_redeem_enabled: false,
        compact_client: reqwest::Client::new(),
        async_client: reqwest::Client::new(),
        async_runtime: Arc::new(
            TokioRuntimeBuilder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime"),
        ),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths,
            state: AppState::default(),
            upstream_base_url: "http://127.0.0.1".to_string(),
            include_code_review: false,
            current_profile: "test".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_backoff_updated_at: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        })),
        log_path,
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: 8,
        runtime_state_lock_wait_counters:
            RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
        lane_admission: RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
            responses: 8,
            compact: 8,
            websocket: 8,
            standard: 8,
        }),
    }
}

#[test]
fn sse_tap_reader_observes_split_events_without_event_buffering() {
    let _guard = acquire_test_runtime_lock();
    let log_path = env::temp_dir().join(format!(
        "prodex-response-forwarding-tap-test-{}.log",
        std::process::id()
    ));
    let shared = test_runtime_streaming_shared(log_path);
    let chunks: [&'static [u8]; 4] = [
        b"data: {\"type\":\"response.created\",\"response_id\":\"resp-",
        b"split\"}\r\n\r\n",
        b"data: {\"type\":\"response.in_progress\",\"turn_state\":\"ts-",
        b"split\"}\r\n\r\n",
    ];
    let mut reader = RuntimeSseTapReader::new(
        ChunkedReader::new(chunks),
        RuntimeSseTapReaderInit {
            shared: shared.clone(),
            profile_name: "test".to_string(),
            prelude: &[],
            remembered_response_ids: &[],
            request_previous_response_id: None,
            turn_state: None,
            request_id: 1,
            prompt_cache_key: None,
            model_name: None,
        },
    );

    let mut sink = Vec::new();
    reader
        .read_to_end(&mut sink)
        .expect("tap reader should read");

    let runtime = shared.runtime.lock().expect("runtime state");
    assert_eq!(
        runtime
            .state
            .response_profile_bindings
            .get("resp-split")
            .map(|binding| binding.profile_name.as_str()),
        Some("test"),
    );
    assert_eq!(
        runtime
            .state
            .response_profile_bindings
            .get(&runtime_response_turn_state_lineage_key(
                "resp-split",
                "ts-split"
            ))
            .map(|binding| binding.profile_name.as_str()),
        Some("test"),
    );
    assert_eq!(sink, chunks.concat());
}

#[test]
fn sse_tap_logs_completion_usage_without_delta_usage() {
    let _guard = acquire_test_runtime_lock();
    let log_path = env::temp_dir().join(format!(
        "prodex-response-forwarding-throughput-test-{}.log",
        std::process::id()
    ));
    prepare_runtime_proxy_test_log_path(&log_path);
    let shared = test_runtime_streaming_shared(log_path.clone());
    let mut reader = RuntimeSseTapReader::new(
        ChunkedReader::new([
            b"data: {\"type\":\"response.created\"}\r\n\r\n".as_slice(),
            b"data: {\"type\":\"response.output_text.delta\",\"delta\":\"hello\"}\r\n\r\n",
            b"data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-sse\",\"usage\":{\"input_tokens\":3,\"output_tokens\":12}}}\r\n\r\n",
        ]),
        RuntimeSseTapReaderInit {
            shared: shared.clone(),
            profile_name: "test".to_string(),
            prelude: &[],
            remembered_response_ids: &[],
            request_previous_response_id: None,
            turn_state: None,
            request_id: 501,
            prompt_cache_key: None,
            model_name: None,
        },
    );
    let mut buf = [0; 4096];
    assert!(reader.read(&mut buf).expect("created event should read") > 0);
    assert!(reader.read(&mut buf).expect("output delta should read") > 0);
    thread::sleep(Duration::from_millis(10));
    assert!(reader.read(&mut buf).expect("completion event should read") > 0);
    assert_eq!(reader.read(&mut buf).expect("SSE should reach EOF"), 0);

    let log = crate::read_runtime_proxy_test_log(&shared.log_path);
    let line = log
        .lines()
        .find(|line| line.contains("] token_usage request=501"))
        .expect("completion usage should be logged");
    let generation_ms = line
        .split_whitespace()
        .find_map(|field| field.strip_prefix("generation_ms="))
        .and_then(|value| value.parse::<u64>().ok());
    assert!(generation_ms.is_some_and(|value| value > 0), "{line}");
    assert!(line.contains("output_tokens=12"));
    let output_tokens_per_second = line
        .split_whitespace()
        .find_map(|field| field.strip_prefix("output_tokens_per_second="))
        .and_then(|value| value.parse::<f64>().ok());
    assert!(
        output_tokens_per_second.is_some_and(|value| value > 0.0),
        "{line}"
    );
    assert!(!log.contains("token_usage_progress request=501"));
    let _ = std::fs::remove_file(log_path);
}

#[test]
fn sse_tap_times_tool_call_first_completion_usage() {
    let _guard = acquire_test_runtime_lock();
    let log_path = env::temp_dir().join(format!(
        "prodex-response-forwarding-tool-throughput-test-{}.log",
        std::process::id()
    ));
    prepare_runtime_proxy_test_log_path(&log_path);
    let shared = test_runtime_streaming_shared(log_path.clone());
    let mut reader = RuntimeSseTapReader::new(
        ChunkedReader::new([
            b"data: {\"type\":\"response.created\"}\r\n\r\n".as_slice(),
            b"data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"function_call\",\"name\":\"exec\"}}\r\n\r\n",
            b"data: {\"type\":\"response.function_call_arguments.delta\",\"delta\":\"{}\"}\r\n\r\n",
            b"data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-tool\",\"usage\":{\"input_tokens\":2,\"output_tokens\":7}}}\r\n\r\n",
        ]),
        RuntimeSseTapReaderInit {
            shared: shared.clone(),
            profile_name: "test".to_string(),
            prelude: &[],
            remembered_response_ids: &[],
            request_previous_response_id: None,
            turn_state: None,
            request_id: 502,
            prompt_cache_key: None,
            model_name: None,
        },
    );
    let mut buf = [0; 4096];
    assert!(reader.read(&mut buf).expect("created event should read") > 0);
    assert!(reader.read(&mut buf).expect("tool item should read") > 0);
    thread::sleep(Duration::from_millis(10));
    assert!(reader.read(&mut buf).expect("tool delta should read") > 0);
    assert!(reader.read(&mut buf).expect("completion event should read") > 0);
    assert_eq!(reader.read(&mut buf).expect("SSE should reach EOF"), 0);

    let log = crate::read_runtime_proxy_test_log(&shared.log_path);
    let line = log
        .lines()
        .find(|line| line.contains("] token_usage request=502"))
        .expect("tool-call completion usage should be logged");
    let generation_ms = line
        .split_whitespace()
        .find_map(|field| field.strip_prefix("generation_ms="))
        .and_then(|value| value.parse::<u64>().ok());
    assert!(generation_ms.is_some_and(|value| value > 0), "{line}");
    assert!(line.contains("output_tokens=7"));
    let output_tokens_per_second = line
        .split_whitespace()
        .find_map(|field| field.strip_prefix("output_tokens_per_second="))
        .and_then(|value| value.parse::<f64>().ok());
    assert!(
        output_tokens_per_second.is_some_and(|value| value > 0.0),
        "{line}"
    );
    assert!(!log.contains("token_usage_progress request=502"));
    let _ = std::fs::remove_file(log_path);
}

#[test]
fn untimed_unary_usage_does_not_emit_generation_rate() {
    let _guard = acquire_test_runtime_lock();
    let log_path = env::temp_dir().join(format!(
        "prodex-response-forwarding-unary-throughput-test-{}.log",
        std::process::id()
    ));
    prepare_runtime_proxy_test_log_path(&log_path);
    let shared = test_runtime_streaming_shared(log_path.clone());
    log_runtime_token_usage(RuntimeTokenUsageLog {
        shared: &shared,
        request_id: 503,
        transport: "http",
        profile_name: "test",
        source: "responses_unary",
        prompt_cache_key: None,
        model_name: None,
        usage: Some(RuntimeTokenUsage {
            input_tokens: 2,
            output_tokens: 5,
            ..RuntimeTokenUsage::default()
        }),
        generation_ms: None,
    });

    let log = crate::read_runtime_proxy_test_log(&shared.log_path);
    let line = log
        .lines()
        .find(|line| line.contains("] token_usage request=503"))
        .expect("unary usage should still be logged");
    assert!(!line.contains("generation_ms="));
    assert!(!line.contains("output_tokens_per_second="));
    let _ = std::fs::remove_file(log_path);
}

#[test]
fn zero_output_usage_does_not_emit_generation_rate() {
    let _guard = acquire_test_runtime_lock();
    let log_path = env::temp_dir().join(format!(
        "prodex-response-forwarding-zero-output-throughput-test-{}.log",
        std::process::id()
    ));
    prepare_runtime_proxy_test_log_path(&log_path);
    let shared = test_runtime_streaming_shared(log_path.clone());
    log_runtime_token_usage(RuntimeTokenUsageLog {
        shared: &shared,
        request_id: 504,
        transport: "websocket",
        profile_name: "test",
        source: "responses_websocket",
        prompt_cache_key: None,
        model_name: None,
        usage: Some(RuntimeTokenUsage {
            input_tokens: 2,
            ..RuntimeTokenUsage::default()
        }),
        generation_ms: Some(1_000),
    });

    let log = crate::read_runtime_proxy_test_log(&shared.log_path);
    let line = log
        .lines()
        .find(|line| line.contains("] token_usage request=504"))
        .expect("zero-output usage should still be logged");
    assert!(!line.contains("generation_ms="));
    assert!(!line.contains("output_tokens_per_second="));
    let _ = std::fs::remove_file(log_path);
}

#[test]
fn streaming_sse_previous_response_error_passes_through_after_commit() {
    let event = concat!(
        "event: response.failed\r\n",
        "data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"previous_response_not_found\",\"message\":\"Previous response with id 'resp-123' not found.\"}}}\r\n",
        "\r\n"
    );
    let output = Arc::new(Mutex::new(Vec::new()));
    let writer = SharedBufferWriter {
        bytes: Arc::clone(&output),
    };
    let response = RuntimeStreamingResponse {
        status: 200,
        headers: vec![
            ("content-type".to_string(), "text/event-stream".to_string()),
            (
                "date".to_string(),
                "Thu, 01 Jan 1970 00:00:00 GMT".to_string(),
            ),
            ("server".to_string(), "upstream".to_string()),
        ],
        body: Box::new(Cursor::new(event.as_bytes().to_vec())),
        request_id: 1,
        profile_name: "test".to_string(),
        log_path: env::temp_dir().join(format!(
            "prodex-response-forwarding-test-{}.log",
            std::process::id()
        )),
        shared: test_runtime_streaming_shared(env::temp_dir().join(format!(
            "prodex-response-forwarding-shared-test-{}.log",
            std::process::id()
        ))),
        _inflight_guard: None,
    };

    write_runtime_streaming_response(Box::new(writer), response).expect("stream response");
    let text = String::from_utf8(output.lock().expect("output").clone()).expect("utf8");

    assert!(text.contains("date: Thu, 01 Jan 1970 00:00:00 GMT\r\n"));
    assert!(text.contains("server: upstream\r\n"));
    assert!(text.contains("previous_response_not_found"));
    assert!(!text.contains("stale_continuation"));
}
