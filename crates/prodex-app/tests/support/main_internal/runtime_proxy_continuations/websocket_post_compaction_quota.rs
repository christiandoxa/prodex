use super::*;

#[test]
fn runtime_proxy_websocket_post_compaction_turn_state_quota_replays_without_user_error() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let (_connect_timeout_guard, _progress_timeout_guard) =
        ci_runtime_proxy_websocket_timeout_guards();
    let now = Local::now().timestamp();
    let turn_state = "turn-post-compact-quota";
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket(),
        "main",
        &["main", "second"],
        &[],
        Vec::new(),
    )
    .restart_with_journal_continuations(RuntimeContinuationStore {
        turn_state_bindings: BTreeMap::from([(
            turn_state.to_string(),
            ResponseProfileBinding {
                binding_identity: None,
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        ..RuntimeContinuationStore::default()
    });

    let headers = [runtime_continuation_header("x-codex-turn-state", turn_state)];
    let request = serde_json::json!({
        "type": "response.create",
        "model": "gpt-6-luna",
        "input": [
            {"type": "message", "role": "user", "content": "compacted history"},
            {"type": "message", "role": "assistant", "content": "completed work"},
            {"type": "message", "role": "user", "content": "continue"},
        ],
        "client_metadata": {
            "x-codex-turn-state": turn_state,
        },
    });

    let mut socket =
        fixture.connect_websocket_with_headers("backend-api/prodex/responses", &headers);
    send_runtime_websocket_json(&mut socket, request);
    let (frames, completed) = read_runtime_websocket_until(&mut socket, |text| {
        text.contains(r#""type":"response.completed""#)
            || text.contains("insufficient_quota")
            || text.contains("usage_limit_reached")
            || text.contains("usage limit")
            || text.contains("previous_response_not_found")
    });
    let _ = socket.close(None);

    assert!(
        completed.contains(r#""response":{"id":"resp-second"}"#),
        "post-compaction quota should rotate to the ready profile without a user-visible retry error: {frames:?}"
    );
    assert!(
        frames.iter().all(|frame| {
            !frame.contains("insufficient_quota")
                && !frame.contains("usage_limit_reached")
                && !frame.contains("usage limit")
                && !frame.contains("previous_response_not_found")
        }),
        "quota and replay-control errors must stay behind the proxy when full context is replayable: {frames:?}"
    );
    assert_eq!(
        fixture.backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()],
        "quota owner should be attempted once, followed by the ready profile"
    );

    let upstream_headers = fixture.backend.responses_headers();
    assert_eq!(upstream_headers.len(), 2, "{upstream_headers:?}");
    assert!(
        !upstream_headers[1].contains_key("x-codex-turn-state"),
        "dead turn-state must not be forwarded to the rotated profile: {upstream_headers:?}"
    );
    let upstream_requests = fixture.backend.websocket_requests();
    assert_eq!(upstream_requests.len(), 2, "{upstream_requests:?}");
    let replay_request: serde_json::Value =
        serde_json::from_str(&upstream_requests[1]).expect("replay request should be JSON");
    assert_eq!(
        replay_request
            .get("client_metadata")
            .and_then(|metadata| metadata.get("x-codex-turn-state")),
        None,
        "dead turn-state must also be removed from response.create client_metadata"
    );

    let log = fixture.wait_for_log(|log| {
        log.contains("quota_blocked_turn_state_full_context_replay")
            && log.contains("committed profile=second")
    });
    assert!(
        log.contains("quota_blocked_turn_state_full_context_replay"),
        "{log}"
    );
    assert!(
        !log.contains("upstream_usage_limit_passthrough"),
        "recoverable post-compaction quota must never take the passthrough branch: {log}"
    );
}
