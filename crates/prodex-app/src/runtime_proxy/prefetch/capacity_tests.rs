use super::*;

const OVERLOAD: &[u8] = b"data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"server_is_overloaded\"}}}\r\n\r\n";
const CREATED: &[u8] =
    b"data: {\"type\":\"response.created\",\"response\":{\"id\":\"held-response\"}}\r\n\r\n";
const OUTPUT: &[u8] =
    b"data: {\"type\":\"response.output_text.delta\",\"delta\":\"visible\"}\r\n\r\n";

fn delayed_stream(
    prefix: Vec<u8>,
    tail: Vec<u8>,
    delay_ms: u64,
    deadline_ms: u64,
    byte_limit: usize,
) -> Result<RuntimeSseInspection> {
    TokioRuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let (sender, receiver) = mpsc::sync_channel(4);
            let shared = Arc::new(RuntimePrefetchSharedState {
                config: RuntimePrefetchConfig {
                    lookahead_timeout_ms: 5,
                    stream_idle_timeout_ms: deadline_ms,
                    max_buffered_bytes: byte_limit,
                    ..RuntimePrefetchConfig::default()
                },
                ..RuntimePrefetchSharedState::default()
            });
            if !prefix.is_empty() {
                shared
                    .queued_bytes
                    .fetch_add(prefix.len(), Ordering::SeqCst);
                sender.send(RuntimePrefetchChunk::Data(prefix)).unwrap();
            }
            let producer_shared = Arc::clone(&shared);
            let producer = tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                producer_shared
                    .queued_bytes
                    .fetch_add(tail.len(), Ordering::SeqCst);
                let _ = sender.send(RuntimePrefetchChunk::Data(tail));
                let _ = sender.send(RuntimePrefetchChunk::End);
            });
            let stream = RuntimePrefetchStream {
                receiver: Some(receiver),
                shared,
                backlog: VecDeque::new(),
                worker_abort: Some(producer.abort_handle()),
            };
            let log = env::temp_dir().join(format!(
                "prodex-sse-capacity-{}-{}.log",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let result = inspect_runtime_sse_lookahead_async(stream, log.clone(), 906).await;
            let _ = fs::remove_file(log);
            result.map(|(inspection, _stream)| inspection)
        })
}

#[test]
fn capacity_waits_beyond_polling_slice_without_committing_headers() {
    assert!(matches!(
        delayed_stream(Vec::new(), OVERLOAD.to_vec(), 40, 1_000, 65_536).unwrap(),
        RuntimeSseInspection::Overloaded(_)
    ));
}

#[test]
fn capacity_metadata_and_partial_error_remain_replayable() {
    let mut prefix = CREATED.to_vec();
    prefix.extend_from_slice(&OVERLOAD[..30]);
    assert!(matches!(
        delayed_stream(prefix, OVERLOAD[30..].to_vec(), 40, 1_000, 65_536).unwrap(),
        RuntimeSseInspection::Overloaded(_)
    ));
}

#[test]
fn capacity_large_metadata_does_not_authorize_commit_at_old_eight_kib_limit() {
    let prefix = format!(
        "data: {{\"type\":\"response.created\",\"padding\":\"{}\"}}\r\n\r\n",
        "x".repeat(9_000)
    )
    .into_bytes();
    assert!(matches!(
        delayed_stream(prefix, OVERLOAD.to_vec(), 40, 1_000, 65_536).unwrap(),
        RuntimeSseInspection::Overloaded(_)
    ));
}

#[test]
fn capacity_after_visible_output_is_not_replayed() {
    let mut prefix = OUTPUT.to_vec();
    prefix.extend_from_slice(OVERLOAD);
    assert!(matches!(
        delayed_stream(prefix, Vec::new(), 300, 20, 65_536).unwrap(),
        RuntimeSseInspection::Commit { .. }
    ));
}

#[test]
fn capacity_hold_deadline_and_memory_bounds_fail_without_commit() {
    assert!(delayed_stream(CREATED.to_vec(), OVERLOAD.to_vec(), 300, 20, 65_536).is_err());
    assert!(delayed_stream(CREATED.to_vec(), OVERLOAD.to_vec(), 40, 1_000, 32).is_err());
}

#[test]
fn capacity_metadata_only_eof_does_not_claim_success() {
    assert!(delayed_stream(CREATED.to_vec(), Vec::new(), 5, 1_000, 65_536).is_err());
}
