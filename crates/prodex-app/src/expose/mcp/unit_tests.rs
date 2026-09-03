use super::protocol::{mcp_json_nesting_within_limit, mcp_origin_allowed};
use super::tools::{
    apply_provider_override, event_page_limit, mcp_tools, optional_string, validate_tool_arguments,
};
use super::{ExposeMcpEndpoint, PublicMcpEndpoint, expose_instance_id};
use serde_json::{Value, json};
use std::sync::Arc;

fn endpoint(capability: &str) -> Arc<ExposeMcpEndpoint> {
    let crate::Commands::Super(args) =
        crate::parse_cli_command_from(["prodex", "s"]).expect("Super args should parse")
    else {
        panic!("expected Super args");
    };
    ExposeMcpEndpoint::new(
        capability,
        "pdxi_test".to_string(),
        std::env::temp_dir(),
        "test".to_string(),
        "test".to_string(),
        args,
    )
}

#[test]
fn public_url_has_only_one_opaque_capability_segment() {
    let url =
        PublicMcpEndpoint::new("https://name.trycloudflare.com", "opaque-capability").unwrap();
    assert_eq!(
        url.as_str(),
        "https://name.trycloudflare.com/pdx/v1/opaque-capability/mcp"
    );
    assert!(!url.as_str().contains('?'));
    assert!(!url.as_str().contains("model"));
    assert_eq!(
        url.as_str()
            .bytes()
            .filter(|byte| *byte == 10 || *byte == 13)
            .count(),
        0
    );
}

#[test]
fn public_url_rejects_newlines_and_keeps_long_capability_atomic() {
    assert!(PublicMcpEndpoint::new("https://example.test\n", "capability").is_err());
    assert!(PublicMcpEndpoint::new("https://example.test", "capability\r").is_err());
    for origin in [
        "https://user:password@example.test",
        "https://example.test/path",
        "https://example.test?capability=secret",
        "https://example.test/#fragment",
    ] {
        assert!(
            PublicMcpEndpoint::new(origin, "capability").is_err(),
            "accepted {origin}"
        );
    }
    let capability = "a".repeat(512);
    let url = PublicMcpEndpoint::new("https://example.test", &capability).unwrap();
    assert_eq!(url.as_str().lines().count(), 1);
    assert!(url.as_str().ends_with("/mcp"));
}

#[test]
fn invalid_path_does_not_match_even_when_json_is_malformed() {
    let endpoint = endpoint("abcdefghijklmnopqrstuvwxyz012345");
    assert!(!endpoint.matches_target("/pdx/v1/abcdefghijklmnopqrstuvwxyz012345/mcp?x=1"));
    assert!(!endpoint.matches_target("/pdx/v1/abcdefghijklmnopqrstuvwxyz012345/mcp/extra"));
    assert!(!endpoint.matches_target("/pdx/v1/abcdefghijklmnopqrstuvwxyz012345%2Fmcp"));
}

#[test]
fn json_nesting_is_bounded_before_deserialization() {
    assert!(mcp_json_nesting_within_limit(
        br#"{"items":[1,{"ok":true}]}"#,
        4
    ));
    assert!(!mcp_json_nesting_within_limit(br#"[[[[[0]]]]]"#, 4));
    assert!(!mcp_json_nesting_within_limit(
        br#"{"unterminated":"value}"#,
        64
    ));
}

#[test]
fn tool_list_is_focused_and_annotations_are_present() {
    let tools = mcp_tools();
    assert_eq!(tools.len(), 8);
    assert!(tools.iter().all(|tool| tool.get("annotations").is_some()));
    assert!(tools.iter().all(|tool| {
        tool.get("name")
            .and_then(Value::as_str)
            .is_some_and(|name| !name.contains("shell"))
    }));
}

#[test]
fn origins_are_exact_and_optional_for_server_to_server_clients() {
    assert!(mcp_origin_allowed("name.trycloudflare.com", None));
    assert!(mcp_origin_allowed(
        "name.trycloudflare.com",
        Some("https://chatgpt.com/")
    ));
    assert!(!mcp_origin_allowed(
        "name.trycloudflare.com",
        Some("https://evil.openai.com/")
    ));
    assert!(!mcp_origin_allowed(
        "name.trycloudflare.com",
        Some("https://name.trycloudflare.com.evil.example/")
    ));
}

#[test]
fn null_optional_tool_values_use_instance_defaults() {
    assert_eq!(
        optional_string(&json!({"model": null}), "model", 32),
        Ok(None)
    );
}

#[test]
fn optional_model_values_reject_control_characters() {
    assert!(optional_string(&json!({"model": "model\nnext"}), "model", 32).is_err());
    assert!(optional_string(&json!({"model": "model\t"}), "model", 32).is_err());
    assert_eq!(
        optional_string(&json!({"model": "model-1"}), "model", 32),
        Ok(Some("model-1".to_string()))
    );
}

#[test]
fn tool_arguments_reject_unknown_schema_keys() {
    assert!(
        validate_tool_arguments(
            "prodex_super_start",
            &json!({"task": "review", "unexpected": true})
        )
        .is_err()
    );
    assert!(
        validate_tool_arguments(
            "prodex_super_start",
            &json!({"task": "review", "model": null})
        )
        .is_ok()
    );
    assert!(validate_tool_arguments("prodex_super_list", &json!({})).is_ok());
    assert!(validate_tool_arguments("unknown_tool", &json!({"unexpected": true})).is_ok());
}

#[test]
fn event_page_limit_matches_its_schema_bounds() {
    assert_eq!(event_page_limit(&json!({})), Ok(64));
    assert_eq!(event_page_limit(&json!({"limit": 1})), Ok(1));
    assert_eq!(event_page_limit(&json!({"limit": 64})), Ok(64));
    assert!(event_page_limit(&json!({"limit": 0})).is_err());
    assert!(event_page_limit(&json!({"limit": 65})).is_err());
}

#[test]
fn start_rejects_unsafe_profiles_and_native_provider_conflicts_before_queueing() {
    let crate::Commands::Super(args) =
        crate::parse_cli_command_from(["prodex", "s", "--cli", "gemini", "--provider", "gemini"])
            .expect("native Super args should parse")
    else {
        panic!("expected Super command");
    };
    let endpoint = ExposeMcpEndpoint::new(
        "abcdefghijklmnopqrstuvwxyz0123456789ABCDEFG",
        "pdxi_native_validation".to_string(),
        std::env::temp_dir(),
        "test".to_string(),
        "test".to_string(),
        args,
    );

    let profile = endpoint
        .start_tool(&json!({"task": "review", "profile": "../escape"}))
        .unwrap_err();
    assert_eq!(profile, "profile is invalid");

    let provider = endpoint
        .start_tool(&json!({"task": "review", "provider": "openai"}))
        .unwrap_err();
    assert!(provider.contains("incompatible with the selected native Gemini"));
}

#[test]
fn provider_override_replaces_the_frozen_model_provider_override() {
    let crate::Commands::Super(mut args) =
        crate::parse_cli_command_from(["prodex", "s"]).expect("Super args should parse")
    else {
        panic!("expected Super args");
    };
    args.api_key = Some("test-provider-key".to_string());
    args.codex_args = vec![
        "-c".into(),
        "model_provider=\"openai\"".into(),
        "--config=model_provider=\"openai\"".into(),
    ];
    apply_provider_override(&mut args, "copilot").expect("provider override should parse");
    assert_eq!(
        args.provider,
        Some(prodex_cli::SuperExternalProvider::Copilot)
    );
    assert!(args.api_key.is_none());
    assert!(crate::codex_cli_config_override_value(&args.codex_args, "model_provider").is_none());
}

#[test]
fn provider_override_keeps_same_provider_credentials() {
    let crate::Commands::Super(mut args) =
        crate::parse_cli_command_from(["prodex", "s", "--provider", "copilot"])
            .expect("Super args should parse")
    else {
        panic!("expected Super args");
    };
    args.api_key = Some("same-provider-key".to_string());
    apply_provider_override(&mut args, "copilot").expect("provider override should parse");
    assert_eq!(args.api_key.as_deref(), Some("same-provider-key"));
}

#[test]
fn provider_override_clears_model_and_effort_when_provider_changes() {
    let crate::Commands::Super(mut args) =
        crate::parse_cli_command_from(["prodex", "s", "--no-sub-agent"])
            .expect("Super args should parse")
    else {
        panic!("expected Super args");
    };
    args.local_model = Some("openai-model".to_string());
    args.codex_args = vec!["-c".into(), "model_reasoning_effort=max".into()];
    apply_provider_override(&mut args, "copilot").expect("provider override should parse");
    assert!(args.local_model.is_none());
    assert!(crate::codex_cli_config_override_value(&args.codex_args, "model").is_none());
    assert!(
        crate::codex_cli_config_override_value(&args.codex_args, "model_reasoning_effort")
            .is_none()
    );
}

#[test]
fn endpoint_debug_redacts_capability() {
    let capability = "abcdefghijklmnopqrstuvwxyz0123456789ABCDEFG";
    let debug = format!("{:?}", endpoint(capability));
    assert!(!debug.contains(capability));
    assert!(debug.contains("<redacted>"));
}

#[test]
fn instance_ids_are_random_and_not_capabilities() {
    let first = expose_instance_id().unwrap();
    let second = expose_instance_id().unwrap();
    assert_ne!(first, second);
    assert!(first.starts_with("pdxi_"));
    assert_eq!(first.len(), 27);
}
