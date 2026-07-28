use super::*;

#[test]
fn app_server_broker_write_stdio_validate_stream_does_not_hint_response_errors() {
    let replay = "{\"jsonrpc\":\"2.0\",\"id\":\"req-thread-start\",\"method\":\"thread/start\",\"params\":{\"cwd\":\"/workspace\",\"model\":\"gpt-5\",\"modelProvider\":\"openai\",\"approvalPolicy\":\"never\",\"approvalsReviewer\":\"user\",\"ephemeral\":false}}\n\
{\"jsonrpc\":\"2.0\",\"id\":\"req-thread-start\",\"error\":{\"code\":-32000,\"message\":\"failed\"}}\n";
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
        .expect("response-error replay should still validate");

    let diagnostics_text = String::from_utf8(diagnostics).unwrap();
    let lines: Vec<&str> = diagnostics_text.lines().collect();
    assert_eq!(lines.len(), 3);
    let request_preview: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    let response_preview: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    let report: serde_json::Value = serde_json::from_str(lines[2]).unwrap();

    assert_eq!(
        request_preview["preview"]["summary"]["lifecycle_schema_file"],
        serde_json::Value::String("ThreadStartParams.json".to_string())
    );
    assert_eq!(response_preview["preview"]["summary"]["frame_kind"], "response");
    assert_eq!(
        response_preview["preview"]["summary"]["lifecycle_schema_file"],
        serde_json::Value::Null
    );
    assert_eq!(report["report"]["error_count"].as_u64(), Some(0));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_accepts_valid_schema_replay() {
    let replay = app_server_broker_upstream_schema_replay_fixture();
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
        .expect("valid schema replay should validate");

    let rendered = String::from_utf8(diagnostics).unwrap();
    assert_eq!(
        rendered,
        app_server_broker_upstream_schema_expected_stream_fixture()
    );
}

#[test]
fn app_server_broker_write_stdio_validate_stream_rejects_malformed_replay() {
    let replay = app_server_broker_stdio_preview_malformed_replay_fixture();
    let mut diagnostics = Vec::new();

    let err =
        app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
            .expect_err("malformed replay should fail closed");

    assert!(
        err.to_string()
            .contains("app-server broker validation failed"),
        "{err}"
    );
    assert_eq!(
        String::from_utf8(diagnostics).unwrap(),
        app_server_broker_stdio_preview_malformed_expected_stream_fixture()
    );
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_accepts_valid_schema_replay() {
    let replay = app_server_broker_upstream_schema_replay_fixture();
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect("valid schema replay should validate before passthrough");

    assert_eq!(
        String::from_utf8(passthrough).unwrap(),
        app_server_broker_upstream_schema_passthrough_expected_stdout_fixture()
    );
    assert_eq!(
        String::from_utf8(diagnostics).unwrap(),
        app_server_broker_upstream_schema_passthrough_expected_stderr_fixture()
    );
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_blocks_malformed_replay() {
    let replay = "\
{\"jsonrpc\":\"2.0\",\"id\":\"req-1\",\"method\":\"custom/ping\",\"params\":{}}\n\
\n\
{\"jsonrpc\":\"2.0\"\n\
{\"jsonrpc\":\"2.0\",\"id\":\"resp-1\",\"result\":{\"ok\":true}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("malformed replay should fail before passthrough");

    assert!(
        err.chain().any(|cause| cause
            .to_string()
            .contains("app-server broker validation failed before passthrough")),
        "{err}"
    );
    assert_eq!(
        String::from_utf8(passthrough).unwrap(),
        "{\"jsonrpc\":\"2.0\",\"id\":\"req-1\",\"method\":\"custom/ping\",\"params\":{}}\n\n"
    );
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"error\":\"invalid_json\""));
    assert!(rendered.contains("\"line\":3"));
    assert!(!rendered.contains("resp-1"));
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_blocks_invalid_frame() {
    let replay = "{\"jsonrpc\":\"2.0\",\"id\":\"req-1\",\"method\":\"turn/start\",\"result\":{}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("invalid JSON-RPC frame should fail before passthrough");

    assert!(err.to_string().contains("invalid_frame_count=1"), "{err}");
    assert!(passthrough.is_empty());
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"frame_kind\":\"invalid\""));
    assert!(rendered.contains("\"method_with_result_or_error\""));
}

#[test]
fn app_server_broker_validates_jsonrpc_batches_before_exact_passthrough() {
    let replay = "\
[{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"custom/ping\",\"params\":{}},{\"jsonrpc\":\"2.0\",\"id\":\"two\",\"method\":\"custom/ping\",\"params\":{}}]\n\
[{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"ok\":true}},{\"jsonrpc\":\"2.0\",\"id\":\"two\",\"result\":{\"ok\":true}}]\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect("matching batch requests and responses should validate");

    assert_eq!(String::from_utf8(passthrough).unwrap(), replay);
    let diagnostics = String::from_utf8(diagnostics).unwrap();
    assert_eq!(diagnostics.lines().count(), 5);
    assert!(diagnostics.contains("\"batch_index\":0"));
    assert!(diagnostics.contains("\"batch_index\":1"));
    let report: serde_json::Value =
        serde_json::from_str(diagnostics.lines().last().unwrap()).unwrap();
    assert_eq!(report["report"]["line_count"], 2);
    assert_eq!(report["report"]["parsed_count"], 2);
    assert_eq!(report["report"]["frame_kind_counts"]["batch"], 2);
}

#[test]
fn app_server_broker_rejects_invalid_jsonrpc_batches_before_passthrough() {
    for (replay, reason) in [
        ("[]\n", "empty_batch"),
        ("[[{\"jsonrpc\":\"2.0\",\"method\":\"custom/ping\"}]]\n", "nested_batch"),
        ("[\"not-a-frame\"]\n", "invalid_batch_member"),
    ] {
        let preview = app_server_broker_preview_line(replay.trim());
        assert_eq!(preview["summary"]["frame_kind"], "invalid");
        assert_eq!(preview["summary"]["invalid_reason"], reason);

        let mut passthrough = Vec::new();
        let mut diagnostics = Vec::new();
        app_server_broker_write_stdio_validate_passthrough_stream(
            std::io::Cursor::new(replay),
            &mut passthrough,
            &mut diagnostics,
        )
        .expect_err("invalid batch should fail closed");
        assert!(passthrough.is_empty());
        assert!(String::from_utf8(diagnostics).unwrap().contains(reason));
    }
}
