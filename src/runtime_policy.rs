use super::{AppPaths, secret_store::SecretBackendKind};

mod cache;
mod load;
mod paths;
mod types;
mod validate;

#[cfg(test)]
pub(super) use self::cache::clear_runtime_policy_cache;
#[cfg(test)]
pub(super) use self::load::load_runtime_policy_from_root;
pub(super) use self::load::{
    ensure_runtime_policy_valid, runtime_policy_proxy, runtime_policy_runtime,
    runtime_policy_secrets, runtime_policy_summary,
};
#[cfg(test)]
pub(super) use self::paths::runtime_policy_path;
pub(super) use self::types::{RuntimeLogFormat, RuntimePolicySummary};

#[cfg(test)]
mod tests {
    use super::*;
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
}
