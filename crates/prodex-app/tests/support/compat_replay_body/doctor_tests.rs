#[test]
fn compat_replay_doctor_fields_surface_compat_warnings() {
    let tail = [
        "[2026-04-10 00:00:00.000 +07:00] request=1 compat_request_surface stage=request family=claude_code client=claude_code route=anthropic_messages transport=http stream=streaming tool_surface=mcp continuation=none approval=false origin=external user_agent=claude-code/1.2.3",
        "[2026-04-10 00:00:01.000 +07:00] request=1 compat_warning stage=request family=claude_code client=claude_code route=anthropic_messages transport=http warning=anthropic_mcp_servers_without_toolset",
    ]
    .join("\n");
    let summary = summarize_runtime_log_tail(tail.as_bytes());
    let fields = runtime_doctor_fields_for_summary(
        &summary,
        std::path::Path::new("/tmp/prodex-runtime-latest.path"),
    )
    .into_iter()
    .collect::<BTreeMap<_, _>>();
    let value = runtime_doctor_json_value(&summary);

    assert_eq!(summary.compat_warning_count, 1);
    assert_eq!(
        summary.top_client_family.as_deref(),
        Some("claude_code (2)")
    );
    assert_eq!(summary.top_client.as_deref(), Some("claude_code (2)"));
    assert_eq!(summary.top_tool_surface.as_deref(), Some("mcp (1)"));
    assert_eq!(
        summary.top_compat_warning.as_deref(),
        Some("anthropic_mcp_servers_without_toolset (1)")
    );
    assert_eq!(fields.get("Compat samples").map(String::as_str), Some("1"));
    assert_eq!(fields.get("Compat warnings").map(String::as_str), Some("1"));
    assert_eq!(
        fields.get("Client family").map(String::as_str),
        Some("claude_code (2)")
    );
    assert_eq!(
        fields.get("Top client").map(String::as_str),
        Some("claude_code (2)")
    );
    assert_eq!(
        fields.get("Tool surface").map(String::as_str),
        Some("mcp (1)")
    );
    assert_eq!(
        fields.get("Compat warning").map(String::as_str),
        Some("anthropic_mcp_servers_without_toolset (1)")
    );
    assert_eq!(value["compat_warning_count"], 1);
    assert_eq!(value["top_client_family"], "claude_code (2)");
    assert_eq!(value["top_client"], "claude_code (2)");
    assert_eq!(value["top_tool_surface"], "mcp (1)");
    assert_eq!(
        value["top_compat_warning"],
        "anthropic_mcp_servers_without_toolset (1)"
    );
}

#[test]
fn compat_replay_doctor_classifies_context_loss_warning_from_log_text() {
    let tail = [
        "[2026-04-10 00:00:00.000 +07:00] request=7 compat_request_surface stage=message family=codex client=codex_cli route=responses transport=websocket stream=streaming tool_surface=none continuation=previous_response origin=external approval=false user_agent=codex-cli/test-fixture",
        "[2026-04-10 00:00:00.005 +07:00] request=7 compat_warning stage=message family=codex client=codex_cli route=responses transport=websocket warning=websocket_previous_response_without_turn_state",
    ]
    .join("\n");
    let mut summary = summarize_runtime_log_tail(tail.as_bytes());
    summary.pointer_exists = true;
    summary.log_exists = true;
    runtime_doctor_finalize_summary(&mut summary);
    let fields = runtime_doctor_fields_for_summary(
        &summary,
        std::path::Path::new("/tmp/prodex-runtime-latest.path"),
    )
    .into_iter()
    .collect::<BTreeMap<_, _>>();

    assert_eq!(summary.compat_warning_count, 1);
    assert_eq!(summary.top_client_family.as_deref(), Some("codex (2)"));
    assert_eq!(summary.top_client.as_deref(), Some("codex_cli (2)"));
    assert_eq!(
        summary.top_compat_warning.as_deref(),
        Some("websocket_previous_response_without_turn_state (1)")
    );
    assert_eq!(
        summary.diagnosis,
        "Recent compatibility warnings were observed for codex_cli (2): websocket_previous_response_without_turn_state (1)."
    );
    assert_eq!(fields.get("Compat warnings").map(String::as_str), Some("1"));
    assert_eq!(
        fields.get("Compat warning").map(String::as_str),
        Some("websocket_previous_response_without_turn_state (1)")
    );
}
