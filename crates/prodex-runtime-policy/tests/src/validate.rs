use super::{RuntimePolicyFile, validate_runtime_policy_file};
use std::path::Path;

fn parse_policy(input: &str) -> RuntimePolicyFile {
    toml::from_str(input).expect("policy TOML should parse")
}

#[test]
fn gateway_request_constraints_parse_and_default() {
    let defaults = parse_policy("version = 1");
    assert_eq!(
        defaults.gateway.request_constraints,
        Default::default(),
        "request constraints stay opt-in"
    );

    let policy = parse_policy(
        r#"
version = 1

[gateway.request_constraints]
enabled = true
unknown_context = "reject"
safe_window_tokens = 65536
oversized_output = "clamp_with_notice"
"#,
    );
    validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect("explicit request constraints should be valid");
    let constraints = policy.gateway.request_constraints;
    assert_eq!(constraints.enabled, Some(true));
    assert_eq!(constraints.unknown_context.as_deref(), Some("reject"));
    assert_eq!(constraints.safe_window_tokens, Some(65_536));
    assert_eq!(
        constraints.oversized_output.as_deref(),
        Some("clamp_with_notice")
    );
}

#[test]
fn gateway_request_constraints_reject_invalid_values() {
    for (field, value) in [
        ("unknown_context", "guess"),
        ("oversized_output", "truncate"),
    ] {
        let policy = parse_policy(&format!(
            "version = 1\n[gateway.request_constraints]\n{field} = \"{value}\"\n"
        ));
        let err = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
            .expect_err("unknown request constraint policy should be rejected");
        assert!(
            err.to_string()
                .contains(&format!("gateway.request_constraints.{field}"))
        );
    }
}

#[test]
fn gateway_request_constraints_reject_zero_safe_window() {
    let policy = parse_policy(
        r#"
version = 1

[gateway.request_constraints]
safe_window_tokens = 0
"#,
    );
    let err = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("zero safe window should be rejected");
    assert!(
        err.to_string()
            .contains("gateway.request_constraints.safe_window_tokens")
    );
}

#[test]
fn validate_runtime_policy_allows_zero_websocket_executor_overflow_capacities() {
    let policy = parse_policy(
        r#"
version = 1

[runtime_proxy]
websocket_connect_worker_count = 4
websocket_connect_queue_capacity = 16
websocket_connect_overflow_capacity = 0
websocket_dns_worker_count = 2
websocket_dns_queue_capacity = 8
websocket_dns_overflow_capacity = 0
"#,
    );

    validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect("zero websocket overflow capacities should be valid");
}

#[test]
fn validate_runtime_policy_rejects_zero_websocket_executor_non_overflow_values() {
    let policy = parse_policy(
        r#"
version = 1

[runtime_proxy]
websocket_connect_worker_count = 0
websocket_connect_overflow_capacity = 0
websocket_dns_overflow_capacity = 0
"#,
    );

    let err = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("zero websocket executor worker count should be rejected");
    assert!(
        err.to_string()
            .contains("runtime_proxy.websocket_connect_worker_count")
    );
}

#[test]
fn validate_runtime_policy_rejects_empty_gateway_route_alias_models() {
    let policy = parse_policy(
        r#"
version = 1

[[gateway.route_aliases]]
alias = "prodex-fast"
models = []
"#,
    );

    let err = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("empty gateway route alias models should be rejected");
    assert!(err.to_string().contains("gateway.route_aliases[0].models"));
}

#[test]
fn validate_runtime_policy_rejects_empty_gateway_allowed_models() {
    let policy = parse_policy(
        r#"
version = 1

[gateway.guardrails]
allowed_models = ["prodex-fast", ""]
"#,
    );

    let err = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("empty gateway allowed model should be rejected");
    assert!(
        err.to_string()
            .contains("gateway.guardrails.allowed_models[1]")
    );
}

#[test]
fn validate_runtime_policy_rejects_invalid_adaptive_routing_values() {
    let policy = parse_policy(
        r#"
version = 1

[gateway.adaptive_routing]
window_size = 0
min_samples = 1
exploration_rate = 0.25
"#,
    );

    let err = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("zero adaptive window should be rejected");
    assert!(
        err.to_string()
            .contains("gateway.adaptive_routing.window_size")
    );

    let policy = parse_policy(
        r#"
version = 1

[gateway.adaptive_routing]
window_size = 64
min_samples = 1
exploration_rate = 1.5
"#,
    );

    let err = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("adaptive exploration rate should be bounded");
    assert!(
        err.to_string()
            .contains("gateway.adaptive_routing.exploration_rate")
    );
}

#[test]
fn validate_runtime_policy_rejects_unknown_gateway_route_strategy() {
    let policy = parse_policy(
        r#"
version = 1

[[gateway.route_aliases]]
alias = "prodex-fast"
models = ["gpt-5-mini"]
strategy = "magic"
"#,
    );

    let err = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("unknown gateway route strategy should be rejected");
    assert!(
        err.to_string()
            .contains("gateway.route_aliases[0].strategy")
    );
}

#[test]
fn validate_runtime_policy_rejects_gateway_route_metric_unknown_model() {
    let policy = parse_policy(
        r#"
version = 1

[[gateway.route_aliases]]
alias = "prodex-fast"
models = ["gpt-5-mini"]

[[gateway.route_aliases.model_metrics]]
model = "gpt-5-nano"
rpm_limit = 60
"#,
    );

    let err = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("gateway route metric model must match alias models");
    assert!(
        err.to_string()
            .contains("gateway.route_aliases[0].model_metrics[0].model")
    );
}

#[test]
fn validate_runtime_policy_rejects_invalid_gateway_observability_http_endpoint() {
    let policy = parse_policy(
        r#"
version = 1

[gateway.observability]
http_endpoint = "ftp://example.com/events"
"#,
    );

    let err = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("invalid gateway observability endpoint should be rejected");
    assert!(
        err.to_string()
            .contains("gateway.observability.http_endpoint")
    );
}

#[test]
fn validate_runtime_policy_rejects_invalid_gateway_observability_http_schema() {
    let policy = parse_policy(
        r#"
version = 1

[gateway.observability]
http_schema = "unknown"
"#,
    );

    let err = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("invalid gateway observability schema should be rejected");
    assert!(
        err.to_string()
            .contains("gateway.observability.http_schema")
    );
}

#[test]
fn validate_runtime_policy_rejects_empty_gateway_blocked_output_keywords() {
    let policy = parse_policy(
        r#"
version = 1

[gateway.guardrails]
blocked_output_keywords = [""]
"#,
    );

    let err = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("empty gateway output keyword should be rejected");
    assert!(
        err.to_string()
            .contains("gateway.guardrails.blocked_output_keywords[0]")
    );
}

#[test]
fn validate_runtime_policy_rejects_invalid_gateway_guardrail_webhook_phase() {
    let policy = parse_policy(
        r#"
version = 1

[gateway.guardrails]
webhook_url = "https://guardrails.example/check"
webhook_phases = ["middle"]
"#,
    );

    let err = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("invalid gateway guardrail webhook phase should be rejected");
    assert!(
        err.to_string()
            .contains("gateway.guardrails.webhook_phases[0]")
    );
}
