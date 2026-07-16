use super::{
    RuntimeLogFormat, RuntimePolicyProxyPreset, clear_runtime_policy_cache,
    load_runtime_policy_cached, load_runtime_policy_from_root,
    plan_runtime_policy_cache_invalidation, reload_runtime_policy_cached_with_invalidation,
    resolve_runtime_policy_path, runtime_policy_path, runtime_policy_proxy_from_root,
};
use secret_store::SecretBackendKind;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_root(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "prodex-runtime-policy-{name}-{}-{nanos:x}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn resolve_runtime_policy_path_preserves_nonblank_path_value() {
    let root = temp_root("resolve-path-exact");

    let resolved = resolve_runtime_policy_path(&root, " runtime logs ").unwrap();

    assert_eq!(resolved, root.join(" runtime logs "));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn resolve_runtime_policy_path_preserves_absolute_path_value() {
    let root = temp_root("resolve-path-absolute-exact");
    let absolute = root.join(" absolute logs ");

    let resolved = resolve_runtime_policy_path(&root, absolute.to_str().unwrap()).unwrap();

    assert_eq!(resolved, absolute);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn resolve_runtime_policy_path_rejects_blank_path_value() {
    let root = temp_root("resolve-path-blank");

    let err = resolve_runtime_policy_path(&root, "   ").unwrap_err();

    assert!(
        err.to_string()
            .contains("policy path values cannot be empty")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_reads_versioned_policy_and_resolves_relative_log_dir() {
    clear_runtime_policy_cache();
    let root = temp_root("loads");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime]
log_format = "json"
log_dir = "runtime-logs"

[secrets]
backend = "file"

[runtime_proxy]
worker_count = 12
active_request_limit = 96
profile_inflight_soft_limit = 5
profile_inflight_hard_limit = 9
"#,
    )
    .unwrap();

    let loaded = load_runtime_policy_from_root(&root).unwrap().unwrap();
    assert_eq!(loaded.version, 1);
    assert_eq!(loaded.runtime.log_format, Some(RuntimeLogFormat::Json));
    assert_eq!(loaded.runtime.log_dir, Some(root.join("runtime-logs")));
    assert_eq!(loaded.secrets.backend, Some(SecretBackendKind::File));
    assert_eq!(loaded.runtime_proxy.worker_count, Some(12));
    assert_eq!(loaded.runtime_proxy.active_request_limit, Some(96));
    assert_eq!(loaded.runtime_proxy.profile_inflight_soft_limit, Some(5));
    assert_eq!(loaded.runtime_proxy.profile_inflight_hard_limit, Some(9));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_preserves_runtime_log_dir_path_value() {
    clear_runtime_policy_cache();
    let root = temp_root("runtime-log-dir-exact");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime]
log_dir = " runtime logs "
"#,
    )
    .unwrap();

    let loaded = load_runtime_policy_from_root(&root).unwrap().unwrap();
    assert_eq!(loaded.runtime.log_dir, Some(root.join(" runtime logs ")));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_cached_preserves_runtime_log_dir_path_value() {
    clear_runtime_policy_cache();
    let root = temp_root("runtime-log-dir-cache-exact");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime]
log_dir = " runtime logs "
"#,
    )
    .unwrap();

    let loaded = load_runtime_policy_cached(&root).unwrap().unwrap();
    assert_eq!(loaded.runtime.log_dir, Some(root.join(" runtime logs ")));
    let cached = load_runtime_policy_cached(&root).unwrap().unwrap();
    assert_eq!(cached.runtime.log_dir, Some(root.join(" runtime logs ")));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_blank_runtime_log_dir_path_value() {
    clear_runtime_policy_cache();
    let root = temp_root("runtime-log-dir-blank");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime]
log_dir = "   "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(err.to_string().contains("runtime.log_dir"), "{err:#}");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn reload_runtime_policy_cached_atomically_replaces_and_reports_previous_entry() {
    clear_runtime_policy_cache();
    let root = temp_root("reload");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
worker_count = 2
"#,
    )
    .unwrap();

    let cached = load_runtime_policy_cached(&root).unwrap().unwrap();
    assert_eq!(cached.runtime_proxy.worker_count, Some(2));
    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
worker_count = 7
"#,
    )
    .unwrap();

    let (invalidation, reloaded) = reload_runtime_policy_cached_with_invalidation(&root).unwrap();
    assert_eq!(invalidation.root, root);
    assert!(invalidation.had_cached_entry);
    assert_eq!(invalidation.cached_policy_version, Some(1));
    let reloaded = reloaded.unwrap();
    assert_eq!(reloaded.runtime_proxy.worker_count, Some(7));
    let cached_after_reload = load_runtime_policy_cached(&root).unwrap().unwrap();
    assert_eq!(cached_after_reload.runtime_proxy.worker_count, Some(7));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_policy_cache_invalidation_plan_is_redacted_and_removes_root_entry() {
    clear_runtime_policy_cache();
    let root = temp_root("invalidation-plan");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
worker_count = 3
"#,
    )
    .unwrap();

    let cached = load_runtime_policy_cached(&root).unwrap().unwrap();
    assert_eq!(cached.runtime_proxy.worker_count, Some(3));

    let plan = plan_runtime_policy_cache_invalidation(&root);

    assert_eq!(plan.root, root);
    assert!(plan.had_cached_entry);
    assert_eq!(plan.cached_policy_version, Some(1));

    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
worker_count = 9
"#,
    )
    .unwrap();
    let reloaded = load_runtime_policy_cached(&root).unwrap().unwrap();
    assert_eq!(reloaded.runtime_proxy.worker_count, Some(9));

    let second_plan = plan_runtime_policy_cache_invalidation(&root);
    assert!(second_plan.had_cached_entry);
    assert_eq!(second_plan.cached_policy_version, Some(1));
    let missing_plan = plan_runtime_policy_cache_invalidation(&root);
    assert!(!missing_plan.had_cached_entry);
    assert_eq!(missing_plan.cached_policy_version, None);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_policy_cache_invalidation_normalizes_root_aliases() {
    clear_runtime_policy_cache();
    let root = temp_root("invalidation-root-alias");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
worker_count = 3
"#,
    )
    .unwrap();

    let alias = root.join(".");
    let cached = load_runtime_policy_cached(&alias).unwrap().unwrap();
    assert_eq!(cached.runtime_proxy.worker_count, Some(3));

    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
worker_count = 9
"#,
    )
    .unwrap();
    let plan = plan_runtime_policy_cache_invalidation(&root);
    assert_eq!(plan.root, root);
    assert!(plan.had_cached_entry);
    assert_eq!(plan.cached_policy_version, Some(1));

    let reloaded = load_runtime_policy_cached(&alias).unwrap().unwrap();
    assert_eq!(reloaded.runtime_proxy.worker_count, Some(9));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_parses_gateway_settings() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway]
listen_addr = "127.0.0.1:4100"
provider = "gemini"
harness = "minimal"
base_url = "https://generativelanguage.googleapis.com/v1beta"
require_auth = true

[gateway.adaptive_routing]
enabled = true
shadow_mode = true
window_size = 64
min_samples = 12
exploration_rate = 0.05

[gateway.state]
backend = "sqlite"
sqlite_path = "gateway-state.sqlite"

[[gateway.admin_tokens]]
name = "ops"
token_env = "PRODEX_GATEWAY_OPS_TOKEN"
role = "admin"

[[gateway.admin_tokens]]
name = "auditor"
token_env = "PRODEX_GATEWAY_AUDITOR_TOKEN"
role = "viewer"
allowed_key_prefixes = ["team-a-", "sandbox-"]
tenant_id = "tenant-a"
team_id = "platform"
project_id = "codex-gateway"
user_id = "alice@example.com"
budget_id = "budget-platform"

[gateway.sso]
proxy_token_env = "PRODEX_GATEWAY_SSO_PROXY_TOKEN"
token_header = "x-prodex-sso-token"
user_header = "x-auth-request-email"
role_header = "x-prodex-role"
key_prefixes_header = "x-prodex-key-prefixes"
tenant_header = "x-prodex-tenant"
oidc_issuer = "https://idp.example"
oidc_audience = "prodex-gateway"
oidc_jwks_url = "https://idp.example/.well-known/jwks.json"
oidc_jwks_origin_allowlist = ["https://keys.idp.example:8443"]
oidc_user_claim = "preferred_username"
oidc_role_claim = "prodex_role"
oidc_tenant_claim = "prodex_tenant"
oidc_key_prefixes_claim = "prodex_key_prefixes"
default_role = "viewer"

[[gateway.route_aliases]]
alias = "prodex-fast"
models = ["gemini-3-flash", "gemini-2.5-flash"]
strategy = "lowest-cost"

[[gateway.route_aliases.model_metrics]]
model = "gemini-3-flash"
input_cost_per_million_microusd = 100
output_cost_per_million_microusd = 200
latency_ms = 300
rpm_limit = 60
tpm_limit = 100000

[[gateway.route_aliases.model_metrics]]
model = "gemini-2.5-flash"
input_cost_per_million_microusd = 50
output_cost_per_million_microusd = 100
latency_ms = 250
rpm_limit = 30
tpm_limit = 50000

[[gateway.virtual_keys]]
name = "team-a"
token_env = "PRODEX_GATEWAY_TEAM_A_TOKEN"
tenant_id = "tenant-a"
team_id = "platform"
project_id = "codex-gateway"
user_id = "alice@example.com"
budget_id = "budget-platform"
allowed_models = ["prodex-fast"]
budget_usd = 12.5
request_budget = 1000
rpm_limit = 60
tpm_limit = 100000

[gateway.observability]
sinks = ["log", "jsonl", "http"]
call_id_header = "x-prodex-call-id"
jsonl_path = "gateway-spend.jsonl"
http_endpoint = "https://otel-collector.example/v1/events"
http_schema = "otel"
http_bearer_token_env = "PRODEX_GATEWAY_OBSERVABILITY_TOKEN"

[gateway.guardrails]
blocked_keywords = ["secret project"]
blocked_output_keywords = ["do not reveal"]
allowed_models = ["prodex-fast"]
presidio_redaction = true
prompt_injection_detection = true
pii_redaction = true
webhook_url = "https://guardrails.example/check"
webhook_phases = ["pre", "post"]
webhook_bearer_token_env = "PRODEX_GATEWAY_GUARDRAIL_TOKEN"
webhook_fail_closed = true
"#,
    )
    .unwrap();

    let loaded = load_runtime_policy_from_root(&root).unwrap().unwrap();
    assert_eq!(
        loaded.gateway.listen_addr.as_deref(),
        Some("127.0.0.1:4100")
    );
    assert_eq!(loaded.gateway.provider.as_deref(), Some("gemini"));
    assert_eq!(
        loaded.gateway.harness,
        Some(prodex_provider_core::HarnessMode::Minimal)
    );
    assert_eq!(loaded.gateway.adaptive_routing.enabled, Some(true));
    assert_eq!(loaded.gateway.adaptive_routing.shadow_mode, Some(true));
    assert_eq!(loaded.gateway.adaptive_routing.window_size, Some(64));
    assert_eq!(loaded.gateway.adaptive_routing.min_samples, Some(12));
    assert_eq!(loaded.gateway.adaptive_routing.exploration_rate, Some(0.05));
    assert_eq!(loaded.gateway.state.backend.as_deref(), Some("sqlite"));
    assert_eq!(
        loaded.gateway.state.sqlite_path.as_deref(),
        Some("gateway-state.sqlite")
    );
    assert_eq!(loaded.gateway.admin_tokens.len(), 2);
    assert_eq!(loaded.gateway.admin_tokens[0].name, "ops");
    assert_eq!(
        loaded.gateway.admin_tokens[1].role.as_deref(),
        Some("viewer")
    );
    assert_eq!(
        loaded.gateway.admin_tokens[1].allowed_key_prefixes,
        vec!["team-a-", "sandbox-"]
    );
    assert_eq!(
        loaded.gateway.admin_tokens[1].tenant_id.as_deref(),
        Some("tenant-a")
    );
    assert_eq!(
        loaded.gateway.admin_tokens[1].team_id.as_deref(),
        Some("platform")
    );
    assert_eq!(
        loaded.gateway.admin_tokens[1].project_id.as_deref(),
        Some("codex-gateway")
    );
    assert_eq!(
        loaded.gateway.admin_tokens[1].user_id.as_deref(),
        Some("alice@example.com")
    );
    assert_eq!(
        loaded.gateway.admin_tokens[1].budget_id.as_deref(),
        Some("budget-platform")
    );
    assert_eq!(
        loaded.gateway.sso.proxy_token_env.as_deref(),
        Some("PRODEX_GATEWAY_SSO_PROXY_TOKEN")
    );
    assert_eq!(
        loaded.gateway.sso.user_header.as_deref(),
        Some("x-auth-request-email")
    );
    assert_eq!(
        loaded.gateway.sso.tenant_header.as_deref(),
        Some("x-prodex-tenant")
    );
    assert_eq!(
        loaded.gateway.sso.oidc_issuer.as_deref(),
        Some("https://idp.example")
    );
    assert_eq!(
        loaded.gateway.sso.oidc_audience.as_deref(),
        Some("prodex-gateway")
    );
    assert_eq!(
        loaded.gateway.sso.oidc_jwks_url.as_deref(),
        Some("https://idp.example/.well-known/jwks.json")
    );
    assert_eq!(
        loaded.gateway.sso.oidc_jwks_origin_allowlist,
        ["https://keys.idp.example:8443"]
    );
    assert_eq!(
        loaded.gateway.sso.oidc_user_claim.as_deref(),
        Some("preferred_username")
    );
    assert_eq!(
        loaded.gateway.sso.oidc_tenant_claim.as_deref(),
        Some("prodex_tenant")
    );
    assert_eq!(loaded.gateway.sso.default_role.as_deref(), Some("viewer"));
    assert_eq!(loaded.gateway.route_aliases[0].alias, "prodex-fast");
    assert_eq!(
        loaded.gateway.route_aliases[0].models,
        vec!["gemini-3-flash", "gemini-2.5-flash"]
    );
    assert_eq!(
        loaded.gateway.route_aliases[0].strategy.as_deref(),
        Some("lowest-cost")
    );
    assert_eq!(loaded.gateway.route_aliases[0].model_metrics.len(), 2);
    assert_eq!(
        loaded.gateway.route_aliases[0].model_metrics[0].rpm_limit,
        Some(60)
    );
    assert_eq!(loaded.gateway.virtual_keys[0].name, "team-a");
    assert_eq!(
        loaded.gateway.virtual_keys[0].token_env,
        "PRODEX_GATEWAY_TEAM_A_TOKEN"
    );
    assert_eq!(
        loaded.gateway.virtual_keys[0].allowed_models,
        vec!["prodex-fast"]
    );
    assert_eq!(
        loaded.gateway.virtual_keys[0].tenant_id.as_deref(),
        Some("tenant-a")
    );
    assert_eq!(
        loaded.gateway.virtual_keys[0].team_id.as_deref(),
        Some("platform")
    );
    assert_eq!(
        loaded.gateway.virtual_keys[0].project_id.as_deref(),
        Some("codex-gateway")
    );
    assert_eq!(
        loaded.gateway.virtual_keys[0].user_id.as_deref(),
        Some("alice@example.com")
    );
    assert_eq!(
        loaded.gateway.virtual_keys[0].budget_id.as_deref(),
        Some("budget-platform")
    );
    assert_eq!(loaded.gateway.virtual_keys[0].budget_usd, Some(12.5));
    assert_eq!(loaded.gateway.virtual_keys[0].request_budget, Some(1000));
    assert_eq!(
        loaded.gateway.observability.sinks,
        vec!["log", "jsonl", "http"]
    );
    assert_eq!(
        loaded.gateway.observability.jsonl_path.as_deref(),
        Some("gateway-spend.jsonl")
    );
    assert_eq!(
        loaded.gateway.observability.http_endpoint.as_deref(),
        Some("https://otel-collector.example/v1/events")
    );
    assert_eq!(
        loaded.gateway.observability.http_schema.as_deref(),
        Some("otel")
    );
    assert_eq!(
        loaded
            .gateway
            .observability
            .http_bearer_token_env
            .as_deref(),
        Some("PRODEX_GATEWAY_OBSERVABILITY_TOKEN")
    );
    assert_eq!(
        loaded.gateway.guardrails.blocked_keywords,
        vec!["secret project"]
    );
    assert_eq!(
        loaded.gateway.guardrails.blocked_output_keywords,
        vec!["do not reveal"]
    );
    assert_eq!(
        loaded.gateway.guardrails.allowed_models,
        vec!["prodex-fast"]
    );
    assert_eq!(loaded.gateway.guardrails.presidio_redaction, Some(true));
    assert_eq!(
        loaded.gateway.guardrails.prompt_injection_detection,
        Some(true)
    );
    assert_eq!(loaded.gateway.guardrails.pii_redaction, Some(true));
    assert_eq!(
        loaded.gateway.guardrails.webhook_url.as_deref(),
        Some("https://guardrails.example/check")
    );
    assert_eq!(
        loaded.gateway.guardrails.webhook_phases,
        vec!["pre", "post"]
    );
    assert_eq!(
        loaded
            .gateway
            .guardrails
            .webhook_bearer_token_env
            .as_deref(),
        Some("PRODEX_GATEWAY_GUARDRAIL_TOKEN")
    );
    assert_eq!(loaded.gateway.guardrails.webhook_fail_closed, Some(true));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_accepts_evaluated_gateway_harness() {
    let root = temp_root("gateway-harness-evaluated");
    fs::write(
        runtime_policy_path(&root),
        "version = 1\n[gateway]\nharness = \"evaluated\"\n",
    )
    .unwrap();

    let loaded = load_runtime_policy_from_root(&root).unwrap().unwrap();
    assert_eq!(
        loaded.gateway.harness,
        Some(prodex_provider_core::HarnessMode::Evaluated)
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_rejects_unknown_gateway_harness() {
    let root = temp_root("gateway-harness-unknown");
    fs::write(
        runtime_policy_path(&root),
        "version = 1\n[gateway]\nharness = \"unknown\"\n",
    )
    .unwrap();

    let error = load_runtime_policy_from_root(&root)
        .unwrap_err()
        .to_string();

    assert!(
        error.contains("unknown variant") || error.contains("unknown"),
        "{error}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_empty_gateway_admin_key_prefix() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-admin-empty-prefix");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[[gateway.admin_tokens]]
name = "scoped"
token_env = "PRODEX_GATEWAY_SCOPED_TOKEN"
allowed_key_prefixes = [""]
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string()
            .contains("gateway.admin_tokens[0].allowed_key_prefixes[0]")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_admin_token_name() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-admin-padded-token-name");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[[gateway.admin_tokens]]
name = " scoped "
token_env = "PRODEX_GATEWAY_SCOPED_TOKEN"
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string().contains("gateway.admin_tokens[0].name"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_admin_token_env() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-admin-padded-token-env");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[[gateway.admin_tokens]]
name = "scoped"
token_env = " PRODEX_GATEWAY_SCOPED_TOKEN "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string()
            .contains("gateway.admin_tokens[0].token_env"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_admin_key_prefix() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-admin-padded-prefix");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[[gateway.admin_tokens]]
name = "scoped"
token_env = "PRODEX_GATEWAY_SCOPED_TOKEN"
allowed_key_prefixes = [" team-a- "]
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string()
            .contains("gateway.admin_tokens[0].allowed_key_prefixes[0]"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_admin_role() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-admin-padded-role");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[[gateway.admin_tokens]]
name = "scoped"
token_env = "PRODEX_GATEWAY_SCOPED_TOKEN"
role = " admin "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string().contains("gateway.admin_tokens[0].role"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_sso_default_role() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-sso-padded-default-role");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.sso]
default_role = " admin "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string().contains("gateway.sso.default_role"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_provider() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-padded-provider");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway]
provider = " gemini "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(err.to_string().contains("gateway.provider"), "{err:#}");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_unknown_gateway_provider() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-unknown-provider");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway]
provider = "unknown-provider"
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(err.to_string().contains("gateway.provider"), "{err:#}");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_base_url() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-padded-base-url");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway]
base_url = " https://api.example/v1 "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(err.to_string().contains("gateway.base_url"), "{err:#}");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_listen_addr() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-padded-listen-addr");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway]
listen_addr = " 127.0.0.1:4000 "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(err.to_string().contains("gateway.listen_addr"), "{err:#}");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_sso_proxy_token_env() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-sso-padded-proxy-token-env");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.sso]
proxy_token_env = " PRODEX_GATEWAY_SSO_PROXY_TOKEN "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string().contains("gateway.sso.proxy_token_env"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_sso_header_and_claim() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-sso-padded-header-claim");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.sso]
user_header = " x-auth-request-email "
oidc_user_claim = " preferred_username "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string().contains("gateway.sso.user_header"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_sso_oidc_claim() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-sso-padded-oidc-claim");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.sso]
oidc_user_claim = " preferred_username "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string().contains("gateway.sso.oidc_user_claim"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_empty_gateway_admin_governance_scope() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-admin-empty-governance-scope");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[[gateway.admin_tokens]]
name = "scoped"
token_env = "PRODEX_GATEWAY_SCOPED_TOKEN"
team_id = ""
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string().contains("gateway.admin_tokens[0].team_id"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_admin_governance_scope() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-admin-padded-governance-scope");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[[gateway.admin_tokens]]
name = "scoped"
token_env = "PRODEX_GATEWAY_SCOPED_TOKEN"
tenant_id = " tenant-a "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string()
            .contains("gateway.admin_tokens[0].tenant_id"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_virtual_key_name() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-key-padded-name");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[[gateway.virtual_keys]]
name = " team-a "
token_env = "PRODEX_GATEWAY_TEAM_A_TOKEN"
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string().contains("gateway.virtual_keys[0].name"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_virtual_key_token_env() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-key-padded-token-env");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[[gateway.virtual_keys]]
name = "team-a"
token_env = " PRODEX_GATEWAY_TEAM_A_TOKEN "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string()
            .contains("gateway.virtual_keys[0].token_env"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_virtual_key_governance_scope() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-key-padded-governance-scope");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[[gateway.virtual_keys]]
name = "team-a"
token_env = "PRODEX_GATEWAY_TEAM_A_TOKEN"
team_id = " team-a "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string().contains("gateway.virtual_keys[0].team_id"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_virtual_key_allowed_model() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-key-padded-allowed-model");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[[gateway.virtual_keys]]
name = "team-a"
token_env = "PRODEX_GATEWAY_TEAM_A_TOKEN"
allowed_models = [" gpt-5 "]
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string()
            .contains("gateway.virtual_keys[0].allowed_models[0]"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_guardrail_allowed_model() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-guardrail-padded-allowed-model");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.guardrails]
allowed_models = [" gpt-5 "]
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string()
            .contains("gateway.guardrails.allowed_models[0]"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_guardrail_bearer_token_env() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-guardrail-padded-bearer-token-env");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.guardrails]
webhook_bearer_token_env = " PRODEX_GATEWAY_GUARDRAIL_TOKEN "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string()
            .contains("gateway.guardrails.webhook_bearer_token_env"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_guardrail_webhook_phase() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-guardrail-padded-webhook-phase");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.guardrails]
webhook_phases = [" pre "]
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string()
            .contains("gateway.guardrails.webhook_phases[0]"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_preserves_gateway_guardrail_keyword_values() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-guardrail-keyword-exact");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.guardrails]
blocked_keywords = [" secret project "]
blocked_output_keywords = [" do not reveal "]
"#,
    )
    .unwrap();

    let loaded = load_runtime_policy_from_root(&root).unwrap().unwrap();
    assert_eq!(
        loaded.gateway.guardrails.blocked_keywords,
        vec![" secret project "]
    );
    assert_eq!(
        loaded.gateway.guardrails.blocked_output_keywords,
        vec![" do not reveal "]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_observability_bearer_token_env() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-observability-padded-bearer-token-env");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.observability]
http_bearer_token_env = " PRODEX_GATEWAY_OBSERVABILITY_TOKEN "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string()
            .contains("gateway.observability.http_bearer_token_env"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_observability_http_schema() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-observability-padded-http-schema");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.observability]
http_schema = " otel "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string()
            .contains("gateway.observability.http_schema"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_observability_http_endpoint() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-observability-padded-http-endpoint");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.observability]
http_endpoint = " https://otel-collector.example/v1/events "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string()
            .contains("gateway.observability.http_endpoint"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_guardrail_webhook_url() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-guardrail-padded-webhook-url");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.guardrails]
webhook_url = " https://guardrails.example/check "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string().contains("gateway.guardrails.webhook_url"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_observability_sink() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-observability-padded-sink");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.observability]
sinks = [" jsonl "]
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string().contains("gateway.observability.sinks[0]"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_observability_call_id_header() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-observability-padded-call-id-header");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.observability]
call_id_header = " x-prodex-call-id "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string()
            .contains("gateway.observability.call_id_header"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_route_alias_model() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-route-alias-padded-model");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[[gateway.route_aliases]]
alias = "fast"
models = [" gpt-5 "]
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string()
            .contains("gateway.route_aliases[0].models[0]"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_route_alias_name() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-route-alias-padded-name");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[[gateway.route_aliases]]
alias = " fast "
models = ["gpt-5"]
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string().contains("gateway.route_aliases[0].alias"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_route_alias_strategy() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-route-alias-padded-strategy");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[[gateway.route_aliases]]
alias = "fast"
models = ["gpt-5"]
strategy = " lowest-cost "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string()
            .contains("gateway.route_aliases[0].strategy"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_route_metric_model() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-route-metric-padded-model");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[[gateway.route_aliases]]
alias = "fast"
models = ["gpt-5"]

[[gateway.route_aliases.model_metrics]]
model = " gpt-5 "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string()
            .contains("gateway.route_aliases[0].model_metrics[0].model"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_incomplete_gateway_oidc_sso() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-oidc-incomplete");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.sso]
oidc_issuer = "https://idp.example"
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string()
            .contains("requires oidc_issuer and oidc_audience"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_allows_explicit_cross_origin_oidc_jwks() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-oidc-jwks-origin-allowlist");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.sso]
oidc_issuer = "https://idp.example"
oidc_audience = "prodex-gateway"
oidc_jwks_url = "https://keys.idp.example:8443/jwks.json"
oidc_jwks_origin_allowlist = ["https://keys.idp.example:8443"]
"#,
    )
    .unwrap();

    let loaded = load_runtime_policy_from_root(&root).unwrap().unwrap();
    assert_eq!(
        loaded.gateway.sso.oidc_jwks_origin_allowlist,
        ["https://keys.idp.example:8443"]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_rejects_unlisted_or_malformed_oidc_jwks_origins() {
    for (name, allowlist) in [
        ("missing", "[]"),
        ("path", "[\"https://keys.idp.example/not-an-origin\"]"),
        ("private", "[\"https://169.254.169.254\"]"),
    ] {
        clear_runtime_policy_cache();
        let root = temp_root(&format!("gateway-oidc-jwks-origin-{name}"));
        let path = runtime_policy_path(&root);
        fs::write(
            &path,
            format!(
                r#"
version = 1

[gateway.sso]
oidc_issuer = "https://idp.example"
oidc_audience = "prodex-gateway"
oidc_jwks_url = "https://keys.idp.example/jwks.json"
oidc_jwks_origin_allowlist = {allowlist}
"#
            ),
        )
        .unwrap();

        let error = load_runtime_policy_from_root(&root).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("gateway.sso.oidc_jwks_origin_allowlist")
                || error.to_string().contains("gateway.sso.oidc_jwks_url"),
            "{error:#}"
        );
        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn load_runtime_policy_bounds_oidc_jwks_origin_allowlist() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-oidc-jwks-origin-count");
    let path = runtime_policy_path(&root);
    let origins = (0..17)
        .map(|index| format!("\"https://keys-{index}.example\""))
        .collect::<Vec<_>>()
        .join(",");
    fs::write(
        &path,
        format!(
            r#"
version = 1

[gateway.sso]
oidc_issuer = "https://idp.example"
oidc_audience = "prodex-gateway"
oidc_jwks_origin_allowlist = [{origins}]
"#
        ),
    )
    .unwrap();

    let error = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        error.to_string().contains("at most 16 entries"),
        "{error:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_plaintext_oidc_jwks_url() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-oidc-http-jwks");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.sso]
oidc_issuer = "https://idp.example"
oidc_audience = "prodex-gateway"
oidc_jwks_url = "http://idp.example/.well-known/jwks.json"
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(err.to_string().contains("must be an https URL"), "{err:#}");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_userinfo_oidc_jwks_url() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-oidc-userinfo-jwks");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.sso]
oidc_issuer = "https://idp.example"
oidc_audience = "prodex-gateway"
oidc_jwks_url = "https://user@idp.example/.well-known/jwks.json"
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string().contains("gateway.sso.oidc_jwks_url"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_oidc_jwks_url() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-oidc-padded-jwks");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.sso]
oidc_issuer = "https://idp.example"
oidc_audience = "prodex-gateway"
oidc_jwks_url = " https://idp.example/.well-known/jwks.json "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string().contains("gateway.sso.oidc_jwks_url"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_plaintext_oidc_issuer() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-oidc-http-issuer");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.sso]
oidc_issuer = "http://idp.example"
oidc_audience = "prodex-gateway"
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(err.to_string().contains("must be an https URL"), "{err:#}");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_userinfo_oidc_issuer() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-oidc-userinfo-issuer");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.sso]
oidc_issuer = "https://user@idp.example"
oidc_audience = "prodex-gateway"
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string().contains("gateway.sso.oidc_issuer"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_oidc_issuer() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-oidc-padded-issuer");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.sso]
oidc_issuer = " https://idp.example "
oidc_audience = "prodex-gateway"
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string().contains("gateway.sso.oidc_issuer"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_oidc_audience() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-oidc-padded-audience");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.sso]
oidc_issuer = "https://idp.example"
oidc_audience = " prodex-gateway "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string().contains("gateway.sso.oidc_audience"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_parses_shared_gateway_state_settings() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-shared-state");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.state]
backend = "postgres"
postgres_url_env = "PRODEX_GATEWAY_POSTGRES_URL"
redis_url_env = "PRODEX_GATEWAY_REDIS_URL"
"#,
    )
    .unwrap();

    let loaded = load_runtime_policy_from_root(&root).unwrap().unwrap();
    assert_eq!(loaded.gateway.state.backend.as_deref(), Some("postgres"));
    assert_eq!(
        loaded.gateway.state.postgres_url_env.as_deref(),
        Some("PRODEX_GATEWAY_POSTGRES_URL")
    );
    assert_eq!(
        loaded.gateway.state.redis_url_env.as_deref(),
        Some("PRODEX_GATEWAY_REDIS_URL")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_state_url_env_refs() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-shared-state-env-exact");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.state]
backend = "postgres"
postgres_url_env = " PRODEX_GATEWAY_POSTGRES_URL "
redis_url_env = " PRODEX_GATEWAY_REDIS_URL "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string().contains("gateway.state.postgres_url_env"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_gateway_state_backend() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-shared-state-backend-exact");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.state]
backend = " redis "
redis_url_env = "PRODEX_GATEWAY_REDIS_URL"
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(err.to_string().contains("gateway.state.backend"), "{err:#}");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_shared_backend_without_url_env() {
    clear_runtime_policy_cache();
    let root = temp_root("gateway-shared-state-missing-env");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[gateway.state]
backend = "redis"
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string().contains("gateway.state.redis_url_env"),
        "{err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[path = "runtime_proxy.rs"]
mod runtime_proxy;
