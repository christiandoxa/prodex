use super::{
    RuntimePolicyFile, RuntimePolicyValidationErrors, RuntimePolicyValidationSection,
    validate_runtime_policy_file,
};
use crate::RuntimePolicyServiceMode;
use std::path::Path;

fn parse_policy(input: &str) -> RuntimePolicyFile {
    toml::from_str(input).expect("policy TOML should parse")
}

fn control_plane_policy() -> RuntimePolicyFile {
    parse_policy(
        r#"
version = 1
service_mode = "control-plane"

[secrets]
production = true
projected_root = "/run/secrets/prodex"
projected_provider = "kubernetes"

[gateway.state]
backend = "postgres"
postgres_url_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_POSTGRES_URL" }

[[gateway.admin_tokens]]
name = "operations"
token_ref = { provider = "kubernetes", name = "PRODEX_CONTROL_PLANE_ADMIN_TOKEN" }
role = "admin"
"#,
    )
}

#[test]
fn service_mode_defaults_to_gateway_and_accepts_explicit_control_plane() {
    assert_eq!(
        parse_policy("version = 1").service_mode,
        RuntimePolicyServiceMode::Gateway
    );
    validate_runtime_policy_file(&control_plane_policy(), Path::new("policy.toml"))
        .expect("projected admin auth and shared state should satisfy control-plane mode");
}

#[test]
fn control_plane_mode_rejects_data_plane_capabilities() {
    for (field, configure) in [
        ("gateway.provider", "provider = \"anthropic\""),
        (
            "gateway.provider_api_key_ref",
            "provider_api_key_ref = { provider = \"kubernetes\", name = \"PROVIDER_KEY\" }",
        ),
        (
            "gateway.auth_token_ref",
            "auth_token_ref = { provider = \"kubernetes\", name = \"GATEWAY_TOKEN\" }",
        ),
        (
            "gateway.observability",
            "[gateway.observability]\nhttp_endpoint = \"https://telemetry.example.com\"",
        ),
        (
            "gateway.sso",
            "[gateway.sso]\noidc_issuer = \"https://idp.example.com\"\noidc_audience = \"prodex\"",
        ),
    ] {
        let mut input = String::from(
            r#"version = 1
service_mode = "control-plane"

[secrets]
production = true
projected_root = "/run/secrets/prodex"
projected_provider = "kubernetes"

[gateway]
"#,
        );
        input.push_str(configure);
        input.push_str(
            r#"

[gateway.state]
backend = "postgres"
postgres_url_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_POSTGRES_URL" }

[[gateway.admin_tokens]]
name = "operations"
token_ref = { provider = "kubernetes", name = "PRODEX_CONTROL_PLANE_ADMIN_TOKEN" }
role = "admin"
"#,
        );
        let error = validate_runtime_policy_file(&parse_policy(&input), Path::new("policy.toml"))
            .expect_err("control-plane data-plane capability should fail closed");
        assert!(error.to_string().contains(field), "{error:#}");
    }
}

#[test]
fn control_plane_mode_requires_production_projected_admin_and_shared_state() {
    let cases = [
        (
            "production mode",
            "production = true",
            "production = false",
            "secrets.production",
        ),
        (
            "admin role",
            "role = \"admin\"",
            "role = \"viewer\"",
            "projected admin-role token",
        ),
        (
            "shared state",
            "backend = \"postgres\"",
            "backend = \"file\"",
            "must configure postgres or redis",
        ),
    ];
    let valid = r#"
version = 1
service_mode = "control-plane"

[secrets]
production = true
projected_root = "/run/secrets/prodex"
projected_provider = "kubernetes"

[gateway.state]
backend = "postgres"
postgres_url_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_POSTGRES_URL" }

[[gateway.admin_tokens]]
name = "operations"
token_ref = { provider = "kubernetes", name = "PRODEX_CONTROL_PLANE_ADMIN_TOKEN" }
role = "admin"
"#;
    for (name, from, to, expected) in cases {
        let policy = parse_policy(&valid.replacen(from, to, 1));
        let error = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
            .expect_err("unsafe control-plane policy should fail closed");
        assert!(error.to_string().contains(expected), "{name}: {error:#}");
    }
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

#[test]
fn runtime_policy_validation_aggregates_sections_in_stable_order() {
    let policy = parse_policy(
        r#"
version = 2

[runtime]
log_dir = " "

[secrets]
backend = "policy-secret-sentinel"

[runtime_proxy]
worker_count = 0

[gateway]
base_url = ""

[gateway.state]
backend = "unknown"

[[gateway.admin_tokens]]
name = ""
token_env = "PRODEX_GATEWAY_ADMIN_TOKEN"

[gateway.sso]
oidc_audience = "prodex"

[[gateway.route_aliases]]
alias = "prodex-fast"
models = []

[[gateway.virtual_keys]]
name = ""
token_env = "PRODEX_GATEWAY_VIRTUAL_KEY"

[gateway.observability]
http_endpoint = "ftp://example.com/events"

[gateway.guardrails]
blocked_keywords = [""]
"#,
    );

    let first = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("independent invalid sections should be aggregated");
    let second = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("validation order should be deterministic");
    assert_eq!(first.to_string(), second.to_string());

    let errors = first
        .downcast_ref::<RuntimePolicyValidationErrors>()
        .expect("validation should expose structured errors");
    let sections = errors
        .issues()
        .iter()
        .map(|issue| issue.section())
        .collect::<Vec<_>>();
    assert_eq!(
        sections,
        [
            RuntimePolicyValidationSection::Version,
            RuntimePolicyValidationSection::Runtime,
            RuntimePolicyValidationSection::Secrets,
            RuntimePolicyValidationSection::RuntimeProxy,
            RuntimePolicyValidationSection::Gateway,
            RuntimePolicyValidationSection::GatewayState,
            RuntimePolicyValidationSection::GatewayAdminTokens,
            RuntimePolicyValidationSection::GatewaySso,
            RuntimePolicyValidationSection::GatewayRouting,
            RuntimePolicyValidationSection::GatewayVirtualKeys,
            RuntimePolicyValidationSection::GatewayObservability,
            RuntimePolicyValidationSection::GatewayGuardrails,
        ]
    );
    assert!(
        errors
            .issues()
            .iter()
            .all(|issue| issue.message().contains("policy.toml"))
    );
}

#[test]
fn runtime_policy_validation_single_error_rendering_stays_compatible() {
    for (name, input, expected) in [
        (
            "version",
            "version = 2",
            "unsupported prodex policy version 2 in policy.toml; expected 1",
        ),
        (
            "runtime",
            "version = 1\n[runtime]\nlog_dir = \" \"",
            "runtime.log_dir in policy.toml cannot be empty",
        ),
        (
            "secrets",
            "version = 1\n[secrets]\nbackend = \"unknown\"",
            "invalid secrets.backend in policy.toml",
        ),
        (
            "runtime proxy",
            "version = 1\n[runtime_proxy]\nworker_count = 0",
            "runtime_proxy.worker_count in policy.toml must be greater than 0",
        ),
        (
            "gateway",
            "version = 1\n[gateway]\nbase_url = \"\"",
            "gateway.base_url in policy.toml cannot be empty",
        ),
        (
            "gateway state",
            "version = 1\n[gateway.state]\nbackend = \"unknown\"",
            "gateway.state.backend in policy.toml is invalid",
        ),
        (
            "gateway admin",
            "version = 1\n[[gateway.admin_tokens]]\nname = \"\"\ntoken_env = \"TOKEN\"",
            "gateway.admin_tokens[0].name in policy.toml must be non-empty without whitespace",
        ),
        (
            "gateway sso",
            "version = 1\n[gateway.sso]\ndefault_role = \"unknown\"",
            "gateway.sso.default_role in policy.toml is invalid",
        ),
        (
            "gateway routing",
            "version = 1\n[[gateway.route_aliases]]\nalias = \"alias\"\nmodels = []",
            "gateway.route_aliases[0].models in policy.toml cannot be empty",
        ),
        (
            "gateway virtual key",
            "version = 1\n[[gateway.virtual_keys]]\nname = \"\"\ntoken_env = \"TOKEN\"",
            "gateway.virtual_keys[0].name in policy.toml must be non-empty without whitespace",
        ),
        (
            "gateway observability",
            "version = 1\n[gateway.observability]\nhttp_endpoint = \"ftp://example.com\"",
            "gateway.observability.http_endpoint in policy.toml must be an http(s) URL with host",
        ),
        (
            "gateway guardrails",
            "version = 1\n[gateway.guardrails]\nblocked_keywords = [\"\"]",
            "gateway.guardrails.blocked_keywords[0] in policy.toml cannot be empty",
        ),
    ] {
        let policy = parse_policy(input);
        let error = validate_runtime_policy_file(&policy, Path::new("policy.toml")).unwrap_err();
        assert_eq!(error.to_string(), expected, "{name}");
    }
}

#[test]
fn runtime_policy_validation_formatting_redacts_invalid_values() {
    let policy = parse_policy(
        r#"
version = 1

[secrets]
backend = "policy-secret-sentinel"

[gateway]
base_url = "https://policy-secret-sentinel invalid"
"#,
    );
    let error = validate_runtime_policy_file(&policy, Path::new("policy.toml")).unwrap_err();

    for formatted in [
        error.to_string(),
        format!("{error:?}"),
        format!("{error:#}"),
    ] {
        assert!(!formatted.contains("policy-secret-sentinel"), "{formatted}");
    }
}
