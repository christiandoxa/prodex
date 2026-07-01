use super::{
    RuntimeLogFormat, clear_runtime_policy_cache, invalidate_runtime_policy_cache_for,
    load_runtime_policy_cached, load_runtime_policy_from_root,
    plan_runtime_policy_cache_invalidation, reload_runtime_policy_cached,
    resolve_runtime_policy_path, runtime_policy_path, runtime_policy_proxy,
};
use secret_store::SecretBackendKind;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct EnvGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(key);
        // SAFETY: tests using this helper hold env_lock.
        unsafe { std::env::set_var(key, value) };
        Self { key, previous }
    }

    fn unset(key: &'static str) -> Self {
        let previous = std::env::var_os(key);
        // SAFETY: tests using this helper hold env_lock.
        unsafe { std::env::remove_var(key) };
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.as_ref() {
            // SAFETY: tests using this helper hold env_lock.
            unsafe { std::env::set_var(self.key, previous) };
        } else {
            // SAFETY: tests using this helper hold env_lock.
            unsafe { std::env::remove_var(self.key) };
        }
    }
}

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
fn reload_runtime_policy_cached_invalidates_root_entry_before_reading() {
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
    let invalidated = invalidate_runtime_policy_cache_for(&root)
        .expect("expected root cache entry to be invalidated")
        .expect("expected cached policy value");
    assert_eq!(invalidated.runtime_proxy.worker_count, Some(2));
    assert!(invalidate_runtime_policy_cache_for(&root).is_none());

    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
worker_count = 7
"#,
    )
    .unwrap();

    let reloaded = reload_runtime_policy_cached(&root).unwrap().unwrap();
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

#[test]
fn load_runtime_policy_from_root_parses_runtime_proxy_preset() {
    clear_runtime_policy_cache();
    let root = temp_root("preset-parse");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
preset = "many-terminals"
"#,
    )
    .unwrap();

    let loaded = load_runtime_policy_from_root(&root).unwrap().unwrap();
    assert_eq!(
        loaded.runtime_proxy.preset().map(|preset| preset.as_str()),
        Some("many-terminals")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_runtime_proxy_preset() {
    clear_runtime_policy_cache();
    let root = temp_root("preset-exact");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
preset = " many-terminals "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    let detail = format!("{err:#}");
    assert!(detail.contains("unknown variant"), "{detail}");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_policy_proxy_applies_preset_values_and_explicit_overrides() {
    let _lock = env_lock().lock().unwrap();
    clear_runtime_policy_cache();
    let root = temp_root("preset-values");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
preset = "many-terminals"
active_request_limit = 99
"#,
    )
    .unwrap();
    let _home = EnvGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let _preset = EnvGuard::unset("PRODEX_RUNTIME_PROXY_PRESET");

    let loaded = runtime_policy_proxy().unwrap();
    assert_eq!(
        loaded.preset().map(|preset| preset.as_str()),
        Some("many-terminals")
    );
    assert_eq!(loaded.worker_count, Some(12));
    assert_eq!(loaded.long_lived_worker_count, Some(32));
    assert_eq!(loaded.long_lived_queue_capacity, Some(512));
    assert_eq!(loaded.active_request_limit, Some(99));
    assert_eq!(loaded.responses_active_limit, Some(120));
    assert_eq!(loaded.websocket_active_limit, Some(32));
    assert_eq!(loaded.websocket_connect_overflow_capacity, Some(384));

    clear_runtime_policy_cache();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_policy_proxy_uses_env_preset_without_policy_file() {
    let _lock = env_lock().lock().unwrap();
    clear_runtime_policy_cache();
    let root = temp_root("preset-env-no-file");
    let _home = EnvGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let _preset = EnvGuard::set("PRODEX_RUNTIME_PROXY_PRESET", "low");

    let loaded = runtime_policy_proxy().unwrap();
    assert_eq!(loaded.preset().map(|preset| preset.as_str()), Some("low"));
    assert_eq!(loaded.worker_count, Some(4));
    assert_eq!(loaded.active_request_limit, Some(48));
    assert_eq!(loaded.profile_inflight_hard_limit, Some(4));

    clear_runtime_policy_cache();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_policy_proxy_default_preset_keeps_tuning_values_unset() {
    let _lock = env_lock().lock().unwrap();
    clear_runtime_policy_cache();
    let root = temp_root("preset-default");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
preset = "default"
"#,
    )
    .unwrap();
    let _home = EnvGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let _preset = EnvGuard::unset("PRODEX_RUNTIME_PROXY_PRESET");

    let loaded = runtime_policy_proxy().unwrap();
    assert_eq!(
        loaded.preset().map(|preset| preset.as_str()),
        Some("default")
    );
    assert_eq!(loaded.worker_count, None);
    assert_eq!(loaded.long_lived_worker_count, None);
    assert_eq!(loaded.active_request_limit, None);
    assert_eq!(loaded.responses_active_limit, None);

    clear_runtime_policy_cache();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_policy_proxy_env_preset_overrides_configured_preset() {
    let _lock = env_lock().lock().unwrap();
    clear_runtime_policy_cache();
    let root = temp_root("preset-env-override");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
preset = "low"
"#,
    )
    .unwrap();
    let _home = EnvGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let _preset = EnvGuard::set("PRODEX_RUNTIME_PROXY_PRESET", "aggressive");

    let loaded = runtime_policy_proxy().unwrap();
    assert_eq!(
        loaded.preset().map(|preset| preset.as_str()),
        Some("aggressive")
    );
    assert_eq!(loaded.worker_count, Some(24));
    assert_eq!(loaded.long_lived_worker_count, Some(96));
    assert_eq!(loaded.active_request_limit, Some(384));
    assert_eq!(loaded.websocket_dns_overflow_capacity, Some(128));

    clear_runtime_policy_cache();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_unknown_preset() {
    clear_runtime_policy_cache();
    let root = temp_root("preset-unknown");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
preset = "huge"
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(err.to_string().contains("failed to parse"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_policy_proxy_ignores_unknown_env_preset_and_falls_back_to_config() {
    let _lock = env_lock().lock().unwrap();
    clear_runtime_policy_cache();
    let root = temp_root("preset-env-unknown");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
preset = "low"
"#,
    )
    .unwrap();
    let _home = EnvGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let _preset = EnvGuard::set("PRODEX_RUNTIME_PROXY_PRESET", "huge");

    let loaded = runtime_policy_proxy().unwrap();
    assert_eq!(loaded.preset().map(|preset| preset.as_str()), Some("low"));
    assert_eq!(loaded.worker_count, Some(4));
    assert_eq!(loaded.active_request_limit, Some(48));

    clear_runtime_policy_cache();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_unsupported_version() {
    clear_runtime_policy_cache();
    let root = temp_root("version");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 2

[runtime]
log_format = "json"
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string()
            .contains("unsupported prodex policy version")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_parses_secret_settings() {
    clear_runtime_policy_cache();
    let root = temp_root("secrets");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[secrets]
backend = "keyring"
keyring_service = "prodex"
"#,
    )
    .unwrap();

    let loaded = load_runtime_policy_from_root(&root).unwrap().unwrap();
    assert_eq!(loaded.secrets.backend, Some(SecretBackendKind::Keyring));
    assert_eq!(loaded.secrets.keyring_service.as_deref(), Some("prodex"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_keyring_service() {
    clear_runtime_policy_cache();
    let root = temp_root("secrets-keyring-service-exact");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[secrets]
backend = "keyring"
keyring_service = " prodex "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(err.to_string().contains("secrets.keyring_service"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_secret_backend_kind() {
    clear_runtime_policy_cache();
    let root = temp_root("secrets-backend-kind-exact");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[secrets]
backend = " keyring "
keyring_service = "prodex"
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(err.to_string().contains("invalid secrets.backend"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_keyring_backend_without_service() {
    clear_runtime_policy_cache();
    let root = temp_root("secrets-missing-service");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[secrets]
backend = "keyring"
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(err.to_string().contains("secrets.keyring_service"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_zero_profile_inflight_limits() {
    clear_runtime_policy_cache();
    let root = temp_root("inflight-zero");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
profile_inflight_soft_limit = 0
profile_inflight_hard_limit = 1
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string()
            .contains("runtime_proxy.profile_inflight_soft_limit")
    );

    let _ = fs::remove_dir_all(root);
}
