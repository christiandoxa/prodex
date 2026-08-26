use super::protocol::{mcp_json_nesting_within_limit, mcp_origin_allowed};
use super::tools::{apply_provider_override, mcp_tools, optional_string};
use super::{ExposeMcpEndpoint, expose_instance_id, mcp_public_url};
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
    let url = mcp_public_url("https://name.trycloudflare.com", "opaque-capability");
    assert_eq!(
        url,
        "https://name.trycloudflare.com/pdx/v1/opaque-capability/mcp"
    );
    assert!(!url.contains('?'));
    assert!(!url.contains("model"));
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
    assert_eq!(tools.len(), 6);
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
fn provider_override_replaces_the_frozen_model_provider_override() {
    let crate::Commands::Super(mut args) =
        crate::parse_cli_command_from(["prodex", "s"]).expect("Super args should parse")
    else {
        panic!("expected Super args");
    };
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
    assert!(crate::codex_cli_config_override_value(&args.codex_args, "model_provider").is_none());
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
