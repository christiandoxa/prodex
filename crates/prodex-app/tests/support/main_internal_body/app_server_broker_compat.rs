use super::*;

#[test]
fn codex_0148_thread_queue_and_revert_frames_keep_thread_affinity() {
    for payload in [
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "queue-add",
            "method": "thread/queue/add",
            "params": {"threadId": "thr-0148", "items": []}
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "method": "thread/queue/changed",
            "params": {"threadId": "thr-0148"}
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "thread-revert",
            "method": "thread/revert",
            "params": {"threadId": "thr-0148"}
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "thread-archive",
            "method": "thread/archive",
            "params": {"threadId": "thr-0148"}
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "thread-unarchive",
            "method": "thread/unarchive",
            "params": {"threadId": "thr-0148"}
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "method": "thread/archived",
            "params": {"threadId": "thr-0148"}
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "method": "thread/unarchived",
            "params": {"threadId": "thr-0148"}
        }),
    ] {
        let keys = app_server_broker_affinity_keys(&payload);
        assert!(keys.iter().any(|key| {
            key.kind == AppServerBrokerAffinityKeyKind::Thread && key.value == "thr-0148"
        }));
        assert_eq!(
            app_server_broker_continuation_decision(&payload),
            AppServerBrokerContinuationDecision::ContinueThread
        );
    }
}

#[test]
fn codex_0147_and_0148_replay_fixtures_are_byte_preserving() {
    for (version, fixture) in [
        (
            "0.147",
            include_str!("../../fixtures/compat_replay/codex-0.147/app-server.jsonl"),
        ),
        (
            "0.148",
            include_str!("../../fixtures/compat_replay/codex-0.148/app-server.jsonl"),
        ),
    ] {
        let mut forwarded = Vec::new();
        let mut diagnostics = Vec::new();
        app_server_broker_write_stdio_passthrough_preview_stream(
            std::io::Cursor::new(fixture.as_bytes()),
            &mut forwarded,
            &mut diagnostics,
        )
        .unwrap_or_else(|error| panic!("{version} replay should be accepted: {error}"));
        assert_eq!(forwarded, fixture.as_bytes(), "{version} stdout must be unchanged");
        for line in fixture.lines().filter(|line| !line.trim().is_empty()) {
            let value: serde_json::Value =
                serde_json::from_str(line).unwrap_or_else(|error| panic!("{version}: {error}"));
            assert!(value.is_object(), "{version} replay frame must be an object");
        }
    }
}

#[test]
fn codex_0148_live_app_server_smoke_fixture_is_byte_preserving() {
    let fixture = include_str!("../../fixtures/compat_replay/codex-0.148/app-server-live-smoke.jsonl");
    let mut forwarded = Vec::new();
    let mut diagnostics = Vec::new();
    app_server_broker_write_stdio_passthrough_preview_stream(
        std::io::Cursor::new(fixture.as_bytes()),
        &mut forwarded,
        &mut diagnostics,
    )
    .expect("the sanitized 0.148 binary smoke capture should be accepted");
    assert_eq!(forwarded, fixture.as_bytes());
    assert!(fixture.contains("\"multiAgentVersion\":\"v2\""));
    assert!(fixture.contains("\"remoteControl/status/changed\""));
}

#[test]
fn codex_0148_model_list_variants_and_unknown_future_values_are_forwarded() {
    for multi_agent_version in [
        serde_json::Value::Null,
        serde_json::json!("disabled"),
        serde_json::json!("v1"),
        serde_json::json!("v2"),
        serde_json::json!("future"),
    ] {
        let frame = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "model-list",
            "result": {
                "data": [{
                    "id": "gpt-5.6-luna",
                    "multiAgentVersion": multi_agent_version,
                }],
                "nextCursor": null,
            },
        });
        let input = format!("{frame}\n");
        let mut forwarded = Vec::new();
        let mut diagnostics = Vec::new();
        app_server_broker_write_stdio_passthrough_preview_stream(
            std::io::Cursor::new(input.as_bytes()),
            &mut forwarded,
            &mut diagnostics,
        )
        .expect("model/list compatibility frame should be accepted");
        assert_eq!(forwarded, input.as_bytes());
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&forwarded).unwrap()["result"]["data"][0]
                ["multiAgentVersion"],
            frame["result"]["data"][0]["multiAgentVersion"]
        );
    }
}
