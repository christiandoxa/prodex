use super::*;
use crate::TestEnvVarGuard;
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
fn runtime_config_snapshots_gemini_hot_path_environment() {
    let policy_dir = with_test_policy_dir("version = 1\n");
    let paths = test_app_paths(policy_dir.root.clone());
    let values = BTreeMap::from([
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
