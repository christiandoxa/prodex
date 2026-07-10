use super::*;
use crate::TestEnvVarGuard;
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
