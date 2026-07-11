#[test]
fn claude_picker_uses_builtin_aliases_for_primary_gpt_models() {
    assert_eq!(runtime_proxy_claude_picker_model("gpt-5.4"), "opus");
    assert_eq!(runtime_proxy_claude_picker_model("gpt-5.4-mini"), "haiku");
    assert_eq!(runtime_proxy_claude_picker_model("gpt-5.3-codex"), "sonnet");
    assert_eq!(runtime_proxy_claude_picker_model("gpt-5.2"), "gpt-5.2");
}

#[test]
fn claude_picker_descriptor_accepts_aliases_and_legacy_native_placeholders() {
    assert_eq!(
        runtime_proxy_claude_picker_model_descriptor("opus").map(|descriptor| descriptor.id),
        Some("gpt-5.4")
    );
    assert_eq!(
        runtime_proxy_claude_picker_model_descriptor("sonnet").map(|descriptor| descriptor.id),
        Some("gpt-5.3-codex")
    );
    assert_eq!(
        runtime_proxy_claude_picker_model_descriptor("haiku").map(|descriptor| descriptor.id),
        Some("gpt-5.4-mini")
    );
    assert_eq!(
        runtime_proxy_claude_picker_model_descriptor("claude-opus-4-6").map(|descriptor| descriptor.id),
        Some("gpt-5.4")
    );
    assert_eq!(
        runtime_proxy_claude_picker_model_descriptor("claude-haiku-4-5")
            .map(|descriptor| descriptor.id),
        Some("gpt-5.4-mini")
    );
    assert_eq!(
        runtime_proxy_claude_picker_model_descriptor("claude-sonnet-4-6")
            .map(|descriptor| descriptor.id),
        Some("gpt-5.3-codex")
    );
}

#[test]
fn anthropic_model_descriptor_advertises_max_effort_for_xhigh_models() {
    let gpt_54 = runtime_proxy_anthropic_model_descriptor("gpt-5.4");
    let gpt_5 = runtime_proxy_anthropic_model_descriptor("gpt-5");

    assert_eq!(
        gpt_54.get("supportsEffort"),
        Some(&serde_json::Value::Bool(true))
    );
    assert_eq!(
        gpt_54.get("supportedEffortLevels"),
        Some(&serde_json::json!(["low", "medium", "high", "max"]))
    );
    assert_eq!(
        gpt_5.get("supportedEffortLevels"),
        Some(&serde_json::json!(["low", "medium", "high"]))
    );
}

#[test]
fn claude_additional_model_options_cache_omits_alias_backed_primary_gpt_entries() {
    let entries = runtime_proxy_claude_additional_model_option_entries();
    assert!(!entries.iter().any(|entry| {
        entry.get("label").and_then(serde_json::Value::as_str) == Some("gpt-5.4")
    }));

    let gpt_52 = entries
        .into_iter()
        .find(|entry| entry.get("label").and_then(serde_json::Value::as_str) == Some("gpt-5.2"))
        .expect("gpt-5.2 model option should exist");

    assert_eq!(
        gpt_52.get("value").and_then(serde_json::Value::as_str),
        Some("gpt-5.2")
    );
    assert_eq!(
        gpt_52.get("supportsEffort"),
        Some(&serde_json::Value::Bool(true))
    );
    assert_eq!(
        gpt_52.get("supportedEffortLevels"),
        Some(&serde_json::json!(["low", "medium", "high", "max"]))
    );
}

#[test]
fn claude_managed_model_option_value_recognizes_current_and_legacy_entries() {
    assert!(runtime_proxy_claude_managed_model_option_value("gpt-5.4"));
    assert!(runtime_proxy_claude_managed_model_option_value("claude-opus-4-6"));
    assert!(!runtime_proxy_claude_managed_model_option_value("custom-provider/model"));
}
