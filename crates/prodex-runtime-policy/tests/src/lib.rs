use super::{
    RuntimeLogFormat, clear_runtime_policy_cache, load_runtime_policy_from_root,
    runtime_policy_path, runtime_policy_proxy,
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
