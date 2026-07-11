fn compat_replay_fixture_text(name: &str) -> &'static str {
    match name {
        "claude_mcp_expected_surface.json" => {
            include_str!("../../fixtures/compat_replay/claude_mcp_expected_surface.json")
        }
        "claude_mcp_expected_translation.json" => {
            include_str!("../../fixtures/compat_replay/claude_mcp_expected_translation.json")
        }
        "claude_mcp_request.json" => {
            include_str!("../../fixtures/compat_replay/claude_mcp_request.json")
        }
        "codex_sse_expected_inspection.json" => {
            include_str!("../../fixtures/compat_replay/codex_sse_expected_inspection.json")
        }
        "codex_sse_expected_payloads.json" => {
            include_str!("../../fixtures/compat_replay/codex_sse_expected_payloads.json")
        }
        "codex_sse_expected_surface.json" => {
            include_str!("../../fixtures/compat_replay/codex_sse_expected_surface.json")
        }
        "codex_sse_request.json" => {
            include_str!("../../fixtures/compat_replay/codex_sse_request.json")
        }
        "codex_sse_stream.txt" => include_str!("../../fixtures/compat_replay/codex_sse_stream.txt"),
        "codex_websocket_expected_metadata.json" => {
            include_str!("../../fixtures/compat_replay/codex_websocket_expected_metadata.json")
        }
        "codex_websocket_expected_surface.json" => {
            include_str!("../../fixtures/compat_replay/codex_websocket_expected_surface.json")
        }
        "codex_websocket_replay.json" => {
            include_str!("../../fixtures/compat_replay/codex_websocket_replay.json")
        }
        "sample_capture_replay.json" => {
            include_str!("../../fixtures/compat_replay/sample_capture_replay.json")
        }
        other => panic!("unknown compat replay fixture {other}"),
    }
}

fn compat_replay_fixture_json(name: &str) -> Value {
    serde_json::from_str(compat_replay_fixture_text(name))
        .unwrap_or_else(|err| panic!("compat replay fixture {name} should be valid JSON: {err}"))
}
