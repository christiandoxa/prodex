use anyhow::Result;

#[cfg(test)]
pub(crate) use prodex_core::select_default_codex_home;
pub(crate) use prodex_core::{absolutize, default_codex_home};

pub(crate) fn audit_log_event(
    _component: &str,
    _action: &str,
    _outcome: &str,
    _details: serde_json::Value,
) -> Result<()> {
    Ok(())
}

pub(crate) fn print_launch_status(message: &str) {
    eprintln!("Prodex launch: {message}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn audit_log_event_reports_persistence_failure_without_failing_operation() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "prodex-audit-failure-test-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let blocker = root.join("not-a-directory");
        fs::write(&blocker, "blocked").expect("blocking file should be written");
        let _audit_dir = crate::TestEnvVarGuard::set(
            "PRODEX_AUDIT_LOG_DIR",
            blocker.to_str().expect("test path should be UTF-8"),
        );

        audit_log_event("profile", "add", "success", serde_json::json!({}))
            .expect("local audit persistence is best effort");
        let _ = fs::remove_dir_all(root);
    }
}
