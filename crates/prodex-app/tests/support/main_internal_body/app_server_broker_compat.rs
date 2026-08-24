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
fn codex_0147_through_0149_replay_fixtures_are_byte_preserving() {
    for (version, fixture) in [
        (
            "0.147",
            include_str!("../../fixtures/compat_replay/codex-0.147/app-server.jsonl"),
        ),
        (
            "0.148",
            include_str!("../../fixtures/compat_replay/codex-0.148/app-server.jsonl"),
        ),
        (
            "0.149",
            include_str!("../../fixtures/compat_replay/codex-0.149/app-server.jsonl"),
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
fn codex_0149_permission_fields_are_forwarded_without_translation() {
    for params in [
        serde_json::json!({"threadId": "thr_149", "permissions": "dev"}),
        serde_json::json!({"threadId": "thr_149", "permissionProfile": "obsolete"}),
    ] {
        let frame = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "permission-selection",
            "method": "thread/resume",
            "params": params,
        });
        let input = format!("{frame}\n");
        let mut forwarded = Vec::new();
        let mut diagnostics = Vec::new();

        app_server_broker_write_stdio_passthrough_preview_stream(
            std::io::Cursor::new(input.as_bytes()),
            &mut forwarded,
            &mut diagnostics,
        )
        .expect("permission fields should remain upstream-owned");

        assert_eq!(forwarded, input.as_bytes());
    }
}

#[test]
fn codex_01491_thread_sources_are_byte_preserved_without_resume_or_affinity_rewrites() {
    let fixture = include_str!("../../fixtures/compat_replay/codex-0.149/app-server.jsonl");
    let frames = fixture
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    let frame = |method: &str| {
        frames
            .iter()
            .find(|frame| frame["method"] == method)
            .unwrap_or_else(|| panic!("missing {method} fixture"))
    };

    assert_eq!(
        frame("thread/start")["params"]["threadSource"],
        "automated_review"
    );
    assert_eq!(
        frame("thread/fork")["params"]["threadSource"],
        "release_validation"
    );
    assert!(
        frame("thread/resume")["params"]
            .get("threadSource")
            .is_none()
    );
    assert!(app_server_broker_affinity_keys(frame("thread/start")).is_empty());
    assert_eq!(
        app_server_broker_affinity_keys(frame("thread/fork"))
            .into_iter()
            .map(|key| (key.kind, key.value))
            .collect::<Vec<_>>(),
        [(AppServerBrokerAffinityKeyKind::Thread, "thr_149".to_string())]
    );

    let mut forwarded = Vec::new();
    let mut diagnostics = Vec::new();
    app_server_broker_write_stdio_passthrough_preview_stream(
        std::io::Cursor::new(fixture.as_bytes()),
        &mut forwarded,
        &mut diagnostics,
    )
    .expect("Codex 0.149.1 source frames should be accepted");
    assert_eq!(forwarded, fixture.as_bytes());

    for source in ["user", "memory_consolidation", "future_feature_source"] {
        let input = format!(
            "{}\n",
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": source,
                "method": "thread/start",
                "params": {"threadSource": source},
            })
        );
        let mut forwarded = Vec::new();
        app_server_broker_write_stdio_passthrough_preview_stream(
            std::io::Cursor::new(input.as_bytes()),
            &mut forwarded,
            &mut Vec::new(),
        )
        .expect("string thread sources should remain forward compatible");
        assert_eq!(forwarded, input.as_bytes());
    }
}

#[test]
fn codex_01491_thread_source_schema_rejects_non_string_shapes() {
    for schema in [
        include_str!("../../fixtures/compat_replay/upstream_codex_schema/ThreadStartParams.json"),
        include_str!("../../fixtures/compat_replay/upstream_codex_schema/ThreadForkParams.json"),
    ] {
        let schema: serde_json::Value = serde_json::from_str(schema).unwrap();
        assert_eq!(schema["definitions"]["ThreadSource"]["type"], "string");
        assert_eq!(
            schema["properties"]["threadSource"]["anyOf"][0]["$ref"],
            "#/definitions/ThreadSource"
        );
    }
    let resume: serde_json::Value = serde_json::from_str(include_str!(
        "../../fixtures/compat_replay/upstream_codex_schema/ThreadResumeParams.json"
    ))
    .unwrap();
    assert!(resume["properties"].get("threadSource").is_none());
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
