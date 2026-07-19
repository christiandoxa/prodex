use super::{
    RuntimeConfig, RuntimeConfigEnvironment, RuntimeGeminiConfig,
    runtime_proxy_profile_inflight_hard_limit, runtime_proxy_profile_inflight_soft_limit,
};
use crate::TestEnvVarGuard;
use crate::{
    AppPaths, AppState, GatewayArgs, SuperExternalProvider, clear_runtime_policy_cache,
    gemini_settings_source_paths_for_config_home, start_runtime_rotation_proxy_with_listen_addr,
};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;

struct TestPolicyDir {
    root: PathBuf,
}

impl TestPolicyDir {
    fn new(policy_toml: &str) -> Self {
        clear_runtime_policy_cache();
        let root = std::env::temp_dir().join(format!(
            "prodex-runtime-tuning-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("policy.toml"), policy_toml).unwrap();
        Self { root }
    }
}

impl Drop for TestPolicyDir {
    fn drop(&mut self) {
        clear_runtime_policy_cache();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn with_test_policy_dir(policy_toml: &str) -> TestPolicyDir {
    TestPolicyDir::new(policy_toml)
}

fn test_app_paths(root: PathBuf) -> AppPaths {
    AppPaths {
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared-codex"),
        legacy_shared_codex_root: root.join("shared"),
        root,
    }
}

fn gateway_args(provider: Option<SuperExternalProvider>) -> GatewayArgs {
    GatewayArgs {
        command: None,
        listen: None,
        provider,
        harness: None,
        base_url: None,
        api_key: None,
        auth_token: None,
        smart_context: false,
        presidio: false,
        no_presidio: false,
    }
}

#[test]
fn runtime_config_reads_each_environment_key_once() {
    let policy_dir = with_test_policy_dir("version = 1\n");
    let paths = test_app_paths(policy_dir.root.clone());
    let mut reads = BTreeMap::<String, usize>::new();
    let environment = RuntimeConfigEnvironment::read_with(|key| {
        *reads.entry(key.to_string()).or_default() += 1;
        None
    });

    RuntimeConfig::from_environment(&paths, environment).expect("default config should parse");

    assert!(!reads.is_empty());
    assert!(reads.values().all(|count| *count == 1), "reads={reads:?}");
}

#[test]
fn runtime_config_default_allows_large_codex_turns() {
    let policy_dir = with_test_policy_dir("version = 1\n");
    let paths = test_app_paths(policy_dir.root.clone());
    let environment = RuntimeConfigEnvironment::read_with(|_| None);

    let config =
        RuntimeConfig::from_environment(&paths, environment).expect("default config should parse");

    assert_eq!(config.max_request_body_bytes, 64 * 1024 * 1024);
}

#[test]
fn runtime_config_carries_validated_governance_mode() {
    let policy_dir = with_test_policy_dir(
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
    let paths = test_app_paths(policy_dir.root.clone());
    let environment = RuntimeConfigEnvironment::read_with(|_| None);

    let config = RuntimeConfig::from_environment(&paths, environment)
        .expect("validated governance settings should load");

    assert_eq!(
        config.governance.mode,
        prodex_config::GovernanceMode::EnterpriseEnforce
    );
    assert_eq!(
        config.governance.routing,
        prodex_config::GovernanceRolloutMode::Enforce
    );
}

#[test]
fn runtime_config_aggregates_errors_without_values() {
    let policy_dir = with_test_policy_dir("version = 1\n");
    let paths = test_app_paths(policy_dir.root.clone());
    let values = BTreeMap::from([
        ("PRODEX_RUNTIME_PROXY_WORKER_COUNT", OsString::from("0")),
        (
            "PRODEX_RUNTIME_PROXY_MAX_REQUEST_BODY_BYTES",
            OsString::from("super-secret-bearer-token"),
        ),
        (
            "PRODEX_RUNTIME_FAULT_UPSTREAM_CONNECT_ERROR_ONCE",
            OsString::from("also-secret"),
        ),
    ]);
    let environment = RuntimeConfigEnvironment::read_with(|key| values.get(key).cloned());

    let errors = RuntimeConfig::from_environment(&paths, environment).unwrap_err();
    let rendered = errors.to_string();

    assert_eq!(
        rendered,
        "runtime configuration is invalid; \
         PRODEX_RUNTIME_PROXY_WORKER_COUNT must be greater than zero; \
         PRODEX_RUNTIME_PROXY_MAX_REQUEST_BODY_BYTES must be an unsigned integer; \
         PRODEX_RUNTIME_FAULT_UPSTREAM_CONNECT_ERROR_ONCE must be an unsigned integer"
    );
    assert!(!rendered.contains("super-secret-bearer-token"));
    assert!(!rendered.contains("also-secret"));
}

#[test]
fn gateway_runtime_config_aggregates_selected_provider_errors_without_secrets() {
    let policy_dir = with_test_policy_dir("version = 1\n");
    let paths = test_app_paths(policy_dir.root.clone());
    let values = BTreeMap::from([
        ("PRODEX_GATEWAY_REPLICA_COUNT", OsString::from("0")),
        (
            "PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS",
            OsString::from("sometimes"),
        ),
        ("PRODEX_DEEPSEEK_STRICT_TOOLS", OsString::from("maybe")),
        ("PRODEX_DEEPSEEK_BETA_BASE_URL", OsString::from("not-a-url")),
        ("PRODEX_DEEPSEEK_WEB_SEARCH_MODE", OsString::from("enabled")),
        ("DEEPSEEK_API_KEY", OsString::from("secret-sentinel")),
    ]);
    let environment =
        RuntimeConfigEnvironment::read_with_gateway(|key| values.get(key).cloned(), true);

    let rendered = RuntimeConfig::from_gateway_environment(
        &paths,
        environment,
        prodex_runtime_policy::RuntimePolicyServiceMode::Gateway,
        &gateway_args(Some(SuperExternalProvider::DeepSeek)),
    )
    .unwrap_err()
    .to_string();

    assert_eq!(
        rendered,
        "runtime configuration is invalid; \
         PRODEX_GATEWAY_REPLICA_COUNT must be at least 1; \
         PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS must be one of true,false,1,0,yes,no,on,off; \
         PRODEX_DEEPSEEK_STRICT_TOOLS must be true or false; \
         PRODEX_DEEPSEEK_BETA_BASE_URL must be an http(s) URL with host and no credentials, query, or fragment; \
         PRODEX_DEEPSEEK_WEB_SEARCH_MODE must be auto, off, openai_chat, or anthropic"
    );
    assert!(!rendered.contains("secret-sentinel"), "{rendered}");
}

#[test]
fn gateway_runtime_config_keeps_only_secret_environment_names() {
    let policy_dir = with_test_policy_dir("version = 1\n");
    let paths = test_app_paths(policy_dir.root.clone());
    let values = BTreeMap::from([
        ("OPENAI_API_KEY", OsString::from("provider-secret-sentinel")),
        (
            "PRODEX_GATEWAY_TOKEN",
            OsString::from("gateway-secret-sentinel"),
        ),
    ]);
    let environment =
        RuntimeConfigEnvironment::read_with_gateway(|key| values.get(key).cloned(), true);
    let config = RuntimeConfig::from_gateway_environment(
        &paths,
        environment,
        prodex_runtime_policy::RuntimePolicyServiceMode::Gateway,
        &gateway_args(None),
    )
    .unwrap();

    let rendered = format!("{config:?}");
    assert!(rendered.contains("OPENAI_API_KEY"), "{rendered}");
    assert!(rendered.contains("PRODEX_GATEWAY_TOKEN"), "{rendered}");
    assert!(!rendered.contains("provider-secret-sentinel"), "{rendered}");
    assert!(!rendered.contains("gateway-secret-sentinel"), "{rendered}");
}

#[test]
fn gateway_runtime_config_rejects_url_secrets_without_rendering_them() {
    let policy_dir = with_test_policy_dir("version = 1\n");
    let paths = test_app_paths(policy_dir.root.clone());
    let values = BTreeMap::from([(
        "PRODEX_DEEPSEEK_BETA_BASE_URL",
        OsString::from(
            "https://deepseek-user:deepseek-password@example.test/beta?deepseek-query#deepseek-fragment",
        ),
    )]);
    let environment =
        RuntimeConfigEnvironment::read_with_gateway(|key| values.get(key).cloned(), true);
    let mut args = gateway_args(Some(SuperExternalProvider::DeepSeek));
    args.base_url = Some(
        "https://upstream-user:upstream-password@example.test/v1?upstream-query#upstream-fragment"
            .to_string(),
    );

    let rendered = RuntimeConfig::from_gateway_environment(
        &paths,
        environment,
        prodex_runtime_policy::RuntimePolicyServiceMode::Gateway,
        &args,
    )
    .unwrap_err()
    .to_string();

    assert!(
        rendered.contains(
            "gateway --base-url must be an http(s) URL with host and no credentials, query, or fragment"
        ),
        "{rendered}"
    );
    assert!(
        rendered.contains(
            "PRODEX_DEEPSEEK_BETA_BASE_URL must be an http(s) URL with host and no credentials, query, or fragment"
        ),
        "{rendered}"
    );
    for secret in [
        "upstream-user",
        "upstream-password",
        "upstream-query",
        "upstream-fragment",
        "deepseek-user",
        "deepseek-password",
        "deepseek-query",
        "deepseek-fragment",
    ] {
        assert!(!rendered.contains(secret), "{rendered}");
    }
}

#[test]
fn gateway_runtime_config_debug_redacts_captured_urls() {
    let policy_dir = with_test_policy_dir("version = 1\n");
    let paths = test_app_paths(policy_dir.root.clone());
    let values = BTreeMap::from([(
        "PRODEX_DEEPSEEK_BETA_BASE_URL",
        OsString::from("https://example.test/deepseek-beta-sentinel"),
    )]);
    let environment =
        RuntimeConfigEnvironment::read_with_gateway(|key| values.get(key).cloned(), true);
    let mut args = gateway_args(Some(SuperExternalProvider::DeepSeek));
    args.base_url = Some("https://example.test/upstream-sentinel".to_string());

    let config = RuntimeConfig::from_gateway_environment(
        &paths,
        environment,
        prodex_runtime_policy::RuntimePolicyServiceMode::Gateway,
        &args,
    )
    .unwrap();

    assert_eq!(
        config.gateway.launch.upstream_base_url(),
        Some("https://example.test/upstream-sentinel")
    );
    assert_eq!(
        config
            .gateway
            .launch
            .deepseek()
            .map(|config| config.beta_base_url.as_str()),
        Some("https://example.test/deepseek-beta-sentinel")
    );
    let rendered = format!("{config:?}");
    assert!(rendered.contains("<redacted>"), "{rendered}");
    assert!(!rendered.contains("upstream-sentinel"), "{rendered}");
    assert!(!rendered.contains("deepseek-beta-sentinel"), "{rendered}");
}

#[test]
fn runtime_config_aggregates_oidc_timing_errors() {
    let policy_dir = with_test_policy_dir("version = 1\n");
    let paths = test_app_paths(policy_dir.root.clone());
    let values = BTreeMap::from([
        (
            "PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS",
            OsString::from("0"),
        ),
        (
            "PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS",
            OsString::from(" 250 "),
        ),
        (
            "PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS",
            OsString::from("credential-secret"),
        ),
        (
            "PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS",
            OsString::from("604801"),
        ),
    ]);
    let environment = RuntimeConfigEnvironment::read_with(|key| values.get(key).cloned());

    let rendered = RuntimeConfig::from_environment(&paths, environment)
        .unwrap_err()
        .to_string();

    assert_eq!(
        rendered,
        "runtime configuration is invalid; \
         PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS must be greater than zero; \
         PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS must not contain whitespace; \
         PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS must be an unsigned integer; \
         PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS must not exceed maximum"
    );
    assert!(!rendered.contains("credential-secret"));
    assert!(!rendered.contains("604801"));
}

#[test]
fn runtime_config_aggregates_gateway_topology_errors() {
    let policy_dir = with_test_policy_dir("version = 1\n");
    let paths = test_app_paths(policy_dir.root.clone());
    let values = BTreeMap::from([
        ("PRODEX_GATEWAY_REPLICA_COUNT", OsString::from("0")),
        (
            "PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS",
            OsString::from("sometimes"),
        ),
    ]);
    let environment = RuntimeConfigEnvironment::read_with(|key| values.get(key).cloned());

    let rendered = RuntimeConfig::from_environment(&paths, environment)
        .unwrap_err()
        .to_string();

    assert_eq!(
        rendered,
        "runtime configuration is invalid; \
         PRODEX_GATEWAY_REPLICA_COUNT must be at least 1; \
         PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS must be one of true,false,1,0,yes,no,on,off"
    );
    assert!(!rendered.contains("sometimes"));
}

#[test]
fn runtime_config_oidc_snapshot_ignores_post_start_environment_changes() {
    let policy_dir = with_test_policy_dir("version = 1\n");
    let paths = test_app_paths(policy_dir.root.clone());
    let _prefetch = TestEnvVarGuard::set("PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS", "125");
    let _cache = TestEnvVarGuard::set("PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS", "250");
    let _backoff = TestEnvVarGuard::set("PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS", "375");
    let _last_known_good =
        TestEnvVarGuard::set("PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS", "500");

    let config = RuntimeConfig::from_env_policy_and_cli(&paths).unwrap();

    let _changed_prefetch = TestEnvVarGuard::set("PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS", "625");
    let _changed_cache = TestEnvVarGuard::set("PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS", "750");
    let _changed_backoff =
        TestEnvVarGuard::set("PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS", "875");
    let _changed_last_known_good =
        TestEnvVarGuard::set("PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS", "1000");

    assert_eq!(
        config.oidc.prefetch_timeout,
        std::time::Duration::from_millis(125)
    );
    assert_eq!(
        config.oidc.http_cache_ttl,
        std::time::Duration::from_secs(250)
    );
    assert_eq!(
        config.oidc.refresh_failure_backoff,
        std::time::Duration::from_millis(375)
    );
    assert_eq!(
        config.oidc.last_known_good_window,
        std::time::Duration::from_secs(500)
    );
}

#[test]
fn runtime_config_snapshots_gemini_hot_path_environment() {
    let policy_dir = with_test_policy_dir("version = 1\n");
    let paths = test_app_paths(policy_dir.root.clone());
    let values = BTreeMap::from([
        ("HOME", OsString::from("/tmp/home-a")),
        ("GEMINI_CLI_HOME", OsString::from("/tmp/gemini-home-a")),
        (
            "GEMINI_CLI_SYSTEM_SETTINGS_PATH",
            OsString::from("/tmp/system-a/settings.json"),
        ),
        (
            "GEMINI_CLI_SYSTEM_DEFAULTS_PATH",
            OsString::from("/tmp/system-a/defaults.json"),
        ),
        (
            "PRODEX_GEMINI_EXTENSION_DIRS",
            std::env::join_paths(["/tmp/extensions-a", "/tmp/extensions-b"]).unwrap(),
        ),
        ("PRODEX_GEMINI_EXTENSIONS", OsString::from("alpha,beta")),
        (
            "PRODEX_GEMINI_EXPORT_FILE",
            OsString::from("/tmp/checkpoint.json"),
        ),
        (
            "PRODEX_GEMINI_SESSION_FILE",
            std::env::join_paths(["/tmp/session-a.json", "/tmp/session-b.json"]).unwrap(),
        ),
        (
            "PRODEX_GEMINI_TOOL_OUTPUT_MASK_THRESHOLD",
            OsString::from("123"),
        ),
        (
            "PRODEX_GEMINI_TOOL_OUTPUT_DIR",
            OsString::from("/tmp/tool-output"),
        ),
        (
            "PRODEX_GEMINI_DISABLE_CONTEXT_FILES",
            OsString::from("true"),
        ),
        ("PRODEX_GEMINI_LOAD_MEMORY", OsString::from("false")),
        (
            "PRODEX_GEMINI_EXTENSION_MEMORY",
            std::env::join_paths(["/tmp/extension-memory.md"]).unwrap(),
        ),
        (
            "PRODEX_GEMINI_LIVE_URL",
            OsString::from("wss://live.example.test/ws"),
        ),
        (
            "PRODEX_GEMINI_LIVE_MODEL",
            OsString::from("gemini-live-test"),
        ),
        ("PRODEX_GEMINI_STICKY_FRESH_OAUTH", OsString::from("off")),
    ]);
    let environment = RuntimeConfigEnvironment::read_with(|key| values.get(key).cloned());

    let config = RuntimeConfig::from_environment(&paths, environment).unwrap();

    assert_eq!(
        config.gemini.home_dir.as_deref(),
        Some(std::path::Path::new("/tmp/home-a"))
    );
    assert_eq!(
        config.gemini.config_dir.as_deref(),
        Some(std::path::Path::new("/tmp/gemini-home-a/.gemini"))
    );
    assert_eq!(
        config.gemini.system_settings_path.as_deref(),
        Some(std::path::Path::new("/tmp/system-a/settings.json"))
    );
    assert_eq!(
        config.gemini.system_defaults_path.as_deref(),
        Some(std::path::Path::new("/tmp/system-a/defaults.json"))
    );
    assert_eq!(
        config.gemini.extension_dirs,
        ["/tmp/extensions-a", "/tmp/extensions-b"]
            .into_iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        config.gemini.extension_enabled_override("alpha"),
        Some(true)
    );
    assert_eq!(
        config.gemini.extension_enabled_override("gamma"),
        Some(false)
    );
    assert_eq!(
        config.gemini.export_checkpoint_path.as_deref(),
        Some(std::path::Path::new("/tmp/checkpoint.json"))
    );
    assert_eq!(config.gemini.import_paths.len(), 2);
    assert_eq!(config.gemini.tool_output_mask_threshold, 123);
    assert!(config.gemini.memory_files_disabled);
    assert!(!config.gemini.memory_files_default);
    assert_eq!(config.gemini.extension_memory_paths.len(), 1);
    assert_eq!(
        config.gemini.live_url.as_deref(),
        Some("wss://live.example.test/ws")
    );
    assert_eq!(
        config.gemini.live_model.as_deref(),
        Some("gemini-live-test")
    );
    assert!(!config.gemini.sticky_fresh_oauth);
}

#[test]
fn runtime_config_gemini_paths_ignore_post_start_environment_changes() {
    let policy_dir = with_test_policy_dir("version = 1\n");
    let paths = test_app_paths(policy_dir.root.clone());
    let _home = TestEnvVarGuard::set("HOME", "/tmp/home-before-start");
    let _gemini_home = TestEnvVarGuard::set("GEMINI_CLI_HOME", "/tmp/gemini-before-start");
    let _system = TestEnvVarGuard::set(
        "GEMINI_CLI_SYSTEM_SETTINGS_PATH",
        "/tmp/system-before-start/settings.json",
    );
    let _defaults = TestEnvVarGuard::set(
        "GEMINI_CLI_SYSTEM_DEFAULTS_PATH",
        "/tmp/system-before-start/defaults.json",
    );
    let config = RuntimeConfig::from_env_policy_and_cli(&paths).unwrap();

    let _changed_home = TestEnvVarGuard::set("HOME", "/tmp/home-after-start");
    let _changed_gemini_home = TestEnvVarGuard::set("GEMINI_CLI_HOME", "/tmp/gemini-after-start");
    let _changed_system = TestEnvVarGuard::set(
        "GEMINI_CLI_SYSTEM_SETTINGS_PATH",
        "/tmp/system-after-start/settings.json",
    );
    let _changed_defaults = TestEnvVarGuard::set(
        "GEMINI_CLI_SYSTEM_DEFAULTS_PATH",
        "/tmp/system-after-start/defaults.json",
    );

    assert_eq!(
        config.gemini.home_dir.as_deref(),
        Some(std::path::Path::new("/tmp/home-before-start"))
    );
    let planned = gemini_settings_source_paths_for_config_home(
        config.gemini.config_dir.as_deref(),
        None,
        config.gemini.system_settings_path.as_deref(),
        config.gemini.system_defaults_path.as_deref(),
    );
    assert_eq!(
        planned,
        vec![
            (
                "system-defaults".to_string(),
                PathBuf::from("/tmp/system-before-start/defaults.json"),
            ),
            (
                "global".to_string(),
                PathBuf::from("/tmp/gemini-before-start/.gemini/settings.json"),
            ),
            (
                "system".to_string(),
                PathBuf::from("/tmp/system-before-start/settings.json"),
            ),
        ]
    );
}

#[test]
fn runtime_config_gateway_topology_ignores_post_start_environment_changes() {
    let policy_dir = with_test_policy_dir("version = 1\n");
    let paths = test_app_paths(policy_dir.root.clone());
    let _replicas = TestEnvVarGuard::set("PRODEX_GATEWAY_REPLICA_COUNT", "3");
    let _gate = TestEnvVarGuard::set("PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS", "true");
    let config = RuntimeConfig::from_env_policy_and_cli(&paths).unwrap();

    let _changed_replicas = TestEnvVarGuard::set("PRODEX_GATEWAY_REPLICA_COUNT", "1");
    let _changed_gate =
        TestEnvVarGuard::set("PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS", "false");

    assert_eq!(config.gateway.replica_count, 3);
    assert!(config.gateway.require_multi_replica_accounting_checks);
}

#[test]
fn runtime_config_preserves_invalid_gemini_threshold_compatibility_default() {
    let policy_dir = with_test_policy_dir("version = 1\n");
    let paths = test_app_paths(policy_dir.root.clone());
    let environment = RuntimeConfigEnvironment::read_with(|key| {
        (key == "PRODEX_GEMINI_TOOL_OUTPUT_MASK_THRESHOLD").then(|| OsString::from("not-a-number"))
    });

    let config = RuntimeConfig::from_environment(&paths, environment).unwrap();

    assert_eq!(
        config.gemini.tool_output_mask_threshold,
        RuntimeGeminiConfig::DEFAULT_TOOL_OUTPUT_MASK_THRESHOLD
    );
    assert!(
        config
            .compatibility_defaults()
            .contains(&"PRODEX_GEMINI_TOOL_OUTPUT_MASK_THRESHOLD")
    );
}

#[test]
fn runtime_config_failure_precedes_listener_bind() {
    let policy_dir = with_test_policy_dir("version = 1\n");
    let paths = test_app_paths(policy_dir.root.clone());
    let _worker_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_WORKER_COUNT", "0");

    let result = start_runtime_rotation_proxy_with_listen_addr(
        &paths,
        &AppState::default(),
        "main",
        "http://127.0.0.1:1".to_string(),
        false,
        true,
        Some("not-a-listener-address"),
    );
    let error = match result {
        Ok(_) => panic!("invalid runtime config unexpectedly started a listener"),
        Err(error) => error.to_string(),
    };

    assert!(error.contains("PRODEX_RUNTIME_PROXY_WORKER_COUNT"));
    assert!(
        !error.contains("bind"),
        "unexpected listener error: {error}"
    );
}

#[test]
fn rotation_proxy_rejects_enterprise_observe_before_listener_bind() {
    let policy_dir = with_test_policy_dir(
        r#"
version = 1

[governance]
mode = "enterprise_observe"
"#,
    );
    let paths = test_app_paths(policy_dir.root.clone());

    let result = start_runtime_rotation_proxy_with_listen_addr(
        &paths,
        &AppState::default(),
        "main",
        "http://127.0.0.1:1".to_string(),
        false,
        true,
        Some("not-a-listener-address"),
    );
    let error = match result {
        Ok(_) => panic!("enterprise observe unexpectedly started the anonymous proxy"),
        Err(error) => error.to_string(),
    };

    assert!(
        error.contains("enterprise governance modes require the authenticated unified gateway")
    );
    assert!(
        !error.contains("bind"),
        "unexpected listener error: {error}"
    );
}

#[test]
fn profile_inflight_limits_read_from_policy_and_env_overrides_policy() {
    let policy_dir = with_test_policy_dir(
        r#"
version = 1

[runtime_proxy]
profile_inflight_soft_limit = 7
profile_inflight_hard_limit = 11
"#,
    );
    let _home_guard = TestEnvVarGuard::set(
        "PRODEX_HOME",
        policy_dir.root.to_str().expect("policy dir path"),
    );
    let _soft_unset_guard =
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_SOFT_LIMIT");
    let _hard_unset_guard =
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_HARD_LIMIT");

    assert_eq!(runtime_proxy_profile_inflight_soft_limit(), 7);
    assert_eq!(runtime_proxy_profile_inflight_hard_limit(), 11);

    let _soft_env_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_SOFT_LIMIT", "13");
    let _hard_env_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_HARD_LIMIT", "17");
    assert_eq!(runtime_proxy_profile_inflight_soft_limit(), 13);
    assert_eq!(runtime_proxy_profile_inflight_hard_limit(), 17);
}

#[test]
fn profile_inflight_limits_reject_zero_env_values() {
    let policy_dir = with_test_policy_dir(
        r#"
version = 1

[runtime_proxy]
profile_inflight_soft_limit = 7
profile_inflight_hard_limit = 11
"#,
    );
    let _home_guard = TestEnvVarGuard::set(
        "PRODEX_HOME",
        policy_dir.root.to_str().expect("policy dir path"),
    );
    let _soft_env_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_SOFT_LIMIT", "0");
    let _hard_env_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_HARD_LIMIT", "0");

    let result = std::panic::catch_unwind(runtime_proxy_profile_inflight_soft_limit);
    assert!(result.is_err(), "zero soft limit should fail closed");
    let result = std::panic::catch_unwind(runtime_proxy_profile_inflight_hard_limit);
    assert!(result.is_err(), "zero hard limit should fail closed");
}
