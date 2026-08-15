use super::{
    RecoveredLoad, runtime_proxy_log_field, runtime_proxy_log_to_path,
    runtime_proxy_structured_log_message,
};
use anyhow::Result;

pub(super) fn runtime_startup_recovery_or_default<T>(
    log_path: &std::path::Path,
    section: &'static str,
    result: Result<RecoveredLoad<T>>,
    fallback: &'static str,
    default: T,
) -> RecoveredLoad<T> {
    match result {
        Ok(loaded) => loaded,
        Err(_error) => {
            runtime_proxy_log_to_path(
                log_path,
                &runtime_proxy_structured_log_message(
                    "runtime_proxy_recovery_fallback",
                    [
                        runtime_proxy_log_field("section", section),
                        runtime_proxy_log_field("fallback", fallback),
                        runtime_proxy_log_field(
                            "reason",
                            "persistent load failed; using safe default",
                        ),
                    ],
                ),
            );
            RecoveredLoad {
                value: default,
                recovered_from_backup: false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::runtime_startup_recovery_or_default;
    use std::collections::BTreeMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn startup_recovery_fallback_is_logged_and_marked() {
        let root = std::env::temp_dir().join(format!(
            "prodex-runtime-recovery-fallback-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        ));
        std::fs::create_dir_all(&root).expect("runtime log directory should be created");
        let log_path = root.join("runtime.log");
        std::fs::write(&log_path, "").expect("runtime log should be created");
        let loaded = runtime_startup_recovery_or_default(
            &log_path,
            "continuations",
            Err(anyhow::anyhow!("malformed sidecar")),
            "empty",
            BTreeMap::<String, i64>::new(),
        );

        assert!(loaded.value.is_empty());
        assert!(!loaded.recovered_from_backup);
        let log = crate::read_runtime_proxy_test_log(&log_path);
        assert!(log.contains("runtime_proxy_recovery_fallback"));
        assert!(log.contains("section=continuations"));
        assert!(log.contains("fallback=empty"));
        assert!(log.contains("reason=\"persistent load failed; using safe default\""));
        let _ = std::fs::remove_dir_all(root);
    }
}
