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
fn governance_defaults_preserve_personal_mode() {
    let policy = parse_policy("version = 1");
    assert_eq!(
        policy.governance.mode,
        crate::RuntimeGovernanceMode::Personal
    );
    validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect("personal governance defaults should remain compatible");
}

#[test]
fn governance_authority_tenants_are_typed_unique_and_bounded() {
    let duplicate = parse_policy(
        r#"
version = 1
[governance]
authority_tenants = [
  "00000000-0000-7000-8000-000000000001",
  "00000000-0000-7000-8000-000000000001",
]
"#,
    );
    let error = validate_runtime_policy_file(&duplicate, Path::new("policy.toml"))
        .expect_err("duplicate authority tenants must be rejected");
    assert!(error.to_string().contains("duplicates"), "{error:#}");

    let tenants = (0..=crate::MAX_GOVERNANCE_AUTHORITY_TENANTS)
        .map(|index| format!("\"00000000-0000-7000-8000-{index:012x}\""))
        .collect::<Vec<_>>()
        .join(",");
    let error = validate_runtime_policy_file(
        &parse_policy(&format!(
            "version = 1\n[governance]\nauthority_tenants = [{tenants}]"
        )),
        Path::new("policy.toml"),
    )
    .expect_err("authority tenant count must be bounded");
    assert!(error.to_string().contains("tenant count"), "{error:#}");
}

#[test]
fn governance_policy_rules_are_strict_typed_and_bounded() {
    let valid = parse_policy(
        r#"
version = 1
[[governance.policy_rules]]
id = "deny.revoked.api"
effect = "deny"
obligations = []
reason_code = "policy.session_revoked"

[governance.policy_rules.condition]
channel = "api"
team_id = "platform"
project_id = "gateway"
user_id = "service-account"
credential_scope = "data_plane"
session_revoked = true
minimum_authentication_strength = 2
requested_capability = "tools"
requested_model = "model-a"
requested_tool = "shell"
requested_modality = "audio"
break_glass_required = true
break_glass_scope = "incident-response"
quota_has_headroom = false
"#,
    );
    validate_runtime_policy_file(&valid, Path::new("policy.toml"))
        .expect("strict typed policy conditions should validate");

    let missing_effect = r#"
version = 1
[[governance.policy_rules]]
id = "missing.effect"
obligations = []
reason_code = "policy.invalid"
[governance.policy_rules.condition]
"#;
    assert!(toml::from_str::<RuntimePolicyFile>(missing_effect).is_err());

    let deny_with_obligation = parse_policy(
        r#"
version = 1
[[governance.policy_rules]]
id = "deny.with.obligation"
effect = "deny"
obligations = [{ kind = "require_mfa" }]
reason_code = "policy.invalid"
[governance.policy_rules.condition]
"#,
    );
    assert!(validate_runtime_policy_file(&deny_with_obligation, Path::new("policy.toml")).is_err());

    let reserved = parse_policy(
        r#"
version = 1
[[governance.policy_rules]]
id = "builtin.confidential-controls"
effect = "allow"
obligations = []
reason_code = "policy.invalid"
[governance.policy_rules.condition]
"#,
    );
    assert!(validate_runtime_policy_file(&reserved, Path::new("policy.toml")).is_err());

    let non_api_channel = parse_policy(
        r#"
version = 1
[[governance.policy_rules]]
id = "deny.cli"
effect = "deny"
obligations = []
reason_code = "policy.invalid_channel"
[governance.policy_rules.condition]
channel = "cli"
"#,
    );
    let error = validate_runtime_policy_file(&non_api_channel, Path::new("policy.toml"))
        .expect_err("gateway policy publication must reject non-API channel selectors");
    assert!(error.to_string().contains("must be api"), "{error:#}");

    let mut invalid_selector = valid.clone();
    invalid_selector.governance.policy_rules[0]
        .condition
        .team_id = Some("x".repeat(129));
    let error = validate_runtime_policy_file(&invalid_selector, Path::new("policy.toml"))
        .expect_err("attribute selectors must be bounded");
    assert!(
        error.to_string().contains("attribute selector"),
        "{error:#}"
    );

    let mut too_many = parse_policy("version = 1");
    too_many.governance.policy_rules = (0..=prodex_domain::MAX_GOVERNANCE_POLICY_RULES)
        .map(|index| crate::RuntimeGovernancePolicyRule {
            id: format!("rule-{index}"),
            condition: crate::RuntimeGovernancePolicyRuleCondition::default(),
            effect: crate::RuntimeGovernancePolicyEffect::Allow,
            obligations: Vec::new(),
            reason_code: format!("policy.rule-{index}"),
        })
        .collect();
    assert!(validate_runtime_policy_file(&too_many, Path::new("policy.toml")).is_err());
}

#[path = "validate_organization.rs"]
mod organization;

#[test]
fn governance_inspection_patterns_enforce_bounded_tenant_scoped_globs() {
    let valid = parse_policy(
        r#"
version = 1

[[governance.inspection_patterns]]
tenant_id = "00000000-0000-7000-8000-000000000001"
id = "customer-code"
pattern = "customer-*secret"
"#,
    );
    validate_runtime_policy_file(&valid, Path::new("policy.toml"))
        .expect("bounded interior-star tenant patterns should validate");

    for pattern in ["*secret", "secret*", "secret**value", "secret\nvalue"] {
        let input = format!(
            r#"
version = 1

[[governance.inspection_patterns]]
tenant_id = "00000000-0000-7000-8000-000000000001"
id = "customer-code"
pattern = {pattern:?}
"#
        );
        let error = validate_runtime_policy_file(&parse_policy(&input), Path::new("policy.toml"))
            .expect_err("unbounded or malformed tenant patterns must fail closed");
        assert!(error.to_string().contains("bounded literal"), "{error:#}");
    }
}

#[test]
fn enforcing_governance_requires_all_stages() {
    let policy = parse_policy(
        r#"
version = 1

[governance]
mode = "enterprise_enforce"
inspection = "enforce"
classification = "observe"
policy = "enforce"
routing = "enforce"
mandatory_audit = true
"#,
    );
    let error = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("partial governance enforcement must fail closed");
    assert!(
        error
            .to_string()
            .contains("requires inspection, classification, policy, and routing enforcement")
    );
}

#[test]
fn enforcing_governance_accepts_complete_immutable_snapshots() {
    let policy = parse_policy(
        r#"
version = 1

[governance]
mode = "enterprise_enforce"
inspection = "enforce"
classification = "enforce"
policy = "enforce"
routing = "enforce"
mandatory_audit = true
anonymous_data_plane = false
raw_secret_sources = false
policy_revision = "00000000-0000-7000-8000-000000000001"
active_policy_revision = "00000000-0000-7000-8000-000000000001"
policy_valid_until_unix_ms = 4102444800000
classification_default = "confidential"
classification_unknown = "deny"
policy_failure_mode = "closed"
classification_revision = "classification-v1"
classification_checksum = "sha256-test-v1"
provider_registry_revision = 1
routing_score_revision = 1

[governance.session]
absolute_timeout_seconds = 3600
idle_timeout_seconds = 900
max_concurrent = 10

[governance.provider]
descriptor_revision = 1
trust_tier = "enterprise"
maximum_classification = "confidential"
regions = ["us-east"]
retention_seconds = 0
training_use = false
"#,
    );

    validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect("complete immutable governance snapshots should validate");
}

#[test]
fn enforcing_governance_rejects_missing_snapshot_or_provider_revision() {
    let policy = parse_policy(
        r#"
version = 1

[governance]
mode = "enterprise_enforce"
inspection = "enforce"
classification = "enforce"
policy = "enforce"
routing = "enforce"
mandatory_audit = true
"#,
    );
    let error = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("missing immutable revisions must fail closed");
    assert!(error.to_string().contains("immutable snapshot revisions"));
}

#[test]
fn bank_governance_requires_audit_identity_and_secret_references() {
    for (field, mandatory_audit, anonymous_data_plane, raw_secret_sources) in [
        ("mandatory audit", false, false, false),
        ("anonymous data-plane", true, true, false),
        ("secret references", true, false, true),
    ] {
        let input = format!(
            r#"
version = 1

[governance]
mode = "bank_enforce"
inspection = "enforce"
classification = "enforce"
policy = "enforce"
routing = "enforce"
mandatory_audit = {mandatory_audit}
anonymous_data_plane = {anonymous_data_plane}
raw_secret_sources = {raw_secret_sources}
"#
        );
        let error = validate_runtime_policy_file(&parse_policy(&input), Path::new("policy.toml"))
            .expect_err("bank governance guardrail must fail closed");
        assert!(error.to_string().contains(field), "{error:#}");
    }
}

fn complete_bank_policy() -> String {
    r#"
version = 1

[secrets]
production = true
projected_root = "/run/secrets/prodex"
projected_provider = "kubernetes"

[gateway]
listen_addr = "10.0.0.10:4317"
expected_host = "gateway.example.com"
restricted_egress = true
replica_count = 2
require_multi_replica_accounting_checks = true
provider = "gemini"
require_auth = true
provider_api_key_ref = { provider = "kubernetes", name = "PROVIDER_KEY" }
auth_token_ref = { provider = "kubernetes", name = "GATEWAY_TOKEN" }
trusted_proxies = ["10.0.0.2"]

[gateway.state]
backend = "postgres"
postgres_url_ref = { provider = "kubernetes", name = "POSTGRES_URL" }
redis_url_ref = { provider = "kubernetes", name = "REDIS_URL" }
postgres_tls_mode = "verify-full"

[gateway.sso]
remote_human = true
oidc_issuer = "https://idp.example.com"
oidc_audience = "prodex-bank"
required_scope = "control_plane"
authentication_strength = "phishing_resistant"

[gateway.observability]
sinks = ["siem"]
siem_endpoint = "https://siem.example.com/events"
siem_bearer_token_ref = { provider = "kubernetes", name = "SIEM_TOKEN" }
siem_mtls_identity_ref = { provider = "kubernetes", name = "SIEM_MTLS" }
siem_signing_key_ref = { provider = "kubernetes", name = "SIEM_SIGNING" }
siem_max_batch_events = 64
siem_max_batch_bytes = 262144
siem_max_attempts = 5
siem_retry_base_ms = 1000
siem_retry_max_ms = 30000
siem_max_lag_ms = 60000

[governance]
config_version = 1
mode = "bank_enforce"
inspection = "enforce"
classification = "enforce"
policy = "enforce"
routing = "enforce"
mandatory_audit = true
anonymous_data_plane = false
raw_secret_sources = false
classification_default = "restricted"
classification_unknown = "deny"
policy_failure_mode = "closed"
policy_revision = "00000000-0000-7000-8000-000000000001"
active_policy_revision = "00000000-0000-7000-8000-000000000001"
policy_valid_until_unix_ms = 4102444800000
classification_revision = "classification-v1"
classification_checksum = "sha256-test-v1"
provider_registry_revision = 1
routing_score_revision = 1

[governance.session]
absolute_timeout_seconds = 3600
idle_timeout_seconds = 900
max_concurrent = 10

[governance.provider]
descriptor_revision = 1
trust_tier = "restricted_approved"
local_execution = true
maximum_classification = "restricted"
regions = ["us-east"]
retention_seconds = 0
training_use = false
"#
    .to_string()
}

#[test]
fn bank_governance_deployment_matrix_fails_closed() {
    let valid = complete_bank_policy();
    validate_runtime_policy_file(&parse_policy(&valid), Path::new("policy.toml"))
        .expect("complete bank deployment should validate");

    for (from, to, expected) in [
        (
            "expected_host = \"gateway.example.com\"",
            "expected_host = \"gateway.example.com/path\"",
            "bounded exact HTTP authority",
        ),
        (
            "listen_addr = \"10.0.0.10:4317\"",
            "listen_addr = \"0.0.0.0:4317\"",
            "private gateway listen address",
        ),
        (
            "restricted_egress = true",
            "restricted_egress = false",
            "restricted egress",
        ),
        (
            "replica_count = 2",
            "replica_count = 1",
            "highly available gateway",
        ),
        (
            "backend = \"postgres\"",
            "backend = \"sqlite\"",
            "shared PostgreSQL state",
        ),
        (
            "remote_human = true",
            "remote_human = false",
            "remote human OIDC",
        ),
        (
            "required_scope = \"control_plane\"",
            "required_scope = \"data_plane\"",
            "control_plane scope",
        ),
        (
            "authentication_strength = \"phishing_resistant\"",
            "authentication_strength = \"mfa\"",
            "phishing_resistant",
        ),
        (
            "classification_default = \"restricted\"",
            "classification_default = \"confidential\"",
            "restricted default classification",
        ),
        (
            "idle_timeout_seconds = 900",
            "idle_timeout_seconds = 7200",
            "idle_timeout_seconds",
        ),
    ] {
        let candidate = valid.replacen(from, to, 1);
        let error =
            validate_runtime_policy_file(&parse_policy(&candidate), Path::new("policy.toml"))
                .expect_err("insecure bank deployment combination must fail closed");
        assert!(error.to_string().contains(expected), "{error:#}");
    }
}

#[test]
fn identity_edge_config_rejects_incomplete_or_untrusted_configuration() {
    for (input, expected) in [
        (
            "version = 1\n[gateway]\ntrusted_proxies = [\"proxy.example.com\"]\n",
            "exact IP addresses",
        ),
        (
            "version = 1\n[gateway.workload_identity]\nenabled = true\nissuer = \"https://workload.example.com\"\naudience = \"prodex-data-plane\"\nrequired_scope = \"data_plane\"\nmtls_required = true\nmtls_ca_ref = { provider = \"file\", name = \"WORKLOAD_CA\" }\n",
            "tls_identity_ref",
        ),
        (
            "version = 1\n[gateway]\nlisten_addr = \"10.0.0.10:4317\"\n",
            "required for a non-loopback",
        ),
        (
            "version = 1\n[gateway]\nexpected_host = \"https://gateway.example.com\"\n",
            "bounded exact HTTP authority",
        ),
        (
            "version = 1\n[gateway.sso]\nbrowser_flow = true\npkce_method = \"S256\"\n",
            "requires exact OIDC",
        ),
    ] {
        let error = validate_runtime_policy_file(&parse_policy(input), Path::new("policy.toml"))
            .expect_err("unsafe identity edge configuration must fail closed");
        assert!(error.to_string().contains(expected), "{error:#}");
    }
}

#[test]
fn identity_edge_accepts_workload_mtls_and_browser_pkce() {
    let workload = parse_policy(
        r#"
version = 1

[gateway.workload_identity]
enabled = true
issuer = "https://workload.example.com"
audience = "prodex-data-plane"
required_scope = "data_plane"
mtls_required = true
mtls_ca_ref = { provider = "file", name = "WORKLOAD_CA" }
tls_identity_ref = { provider = "file", name = "GATEWAY_TLS" }
"#,
    );
    validate_runtime_policy_file(&workload, Path::new("policy.toml"))
        .expect("verified workload JWT plus mTLS binding should be supported");

    let browser = parse_policy(
        r#"
version = 1

[gateway.sso]
remote_human = true
required_scope = "control_plane"
oidc_issuer = "https://identity.example.com"
oidc_audience = "prodex-console"
browser_flow = true
pkce_method = "S256"
oidc_authorization_url = "https://identity.example.com/oauth2/authorize"
oidc_token_url = "https://identity.example.com/oauth2/token"
oidc_client_id = "prodex-console"
oidc_redirect_uri = "https://gateway.example.com/v1/prodex/gateway/auth/callback"
"#,
    );
    validate_runtime_policy_file(&browser, Path::new("policy.toml"))
        .expect("authorization-code browser OIDC with PKCE S256 should be supported");
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
fn bank_governance_requires_allowlisted_https_guardrail_webhook() {
    let policy = parse_policy(
        r#"
version = 1

[governance]
mode = "bank_enforce"

[gateway.guardrails]
webhook_url = "http://guardrails.example/check"
webhook_phases = ["pre"]
webhook_fail_closed = true
"#,
    );

    let error = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("bank webhook without HTTPS/allowlist must fail");
    assert!(
        error
            .to_string()
            .contains("requires HTTPS and a host allowlist")
    );

    let policy = parse_policy(
        r#"
version = 1

[gateway.guardrails]
webhook_url = "https://guardrails.example/check"
webhook_host_allowlist = ["https://guardrails.example"]
"#,
    );
    let error = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("allowlist entries must be exact hosts");
    assert!(error.to_string().contains("must be an exact DNS host"));
}

#[test]
fn runtime_policy_validation_aggregates_sections_in_stable_order() {
    let policy = parse_policy(
        r#"
version = 2

[runtime]
log_dir = " "

[secrets]
projected_root = ""

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
            "version = 1\n[secrets]\nprojected_root = \"\"\nprojected_provider = \"vault\"",
            "secrets.projected_root in policy.toml cannot be empty",
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
projected_provider = "policy-secret-sentinel invalid"

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
