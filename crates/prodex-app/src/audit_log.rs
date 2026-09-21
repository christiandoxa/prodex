use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;

pub(super) fn audit_log_path() -> PathBuf {
    prodex_audit_log::audit_log_path(&crate::runtime_proxy_log_dir())
}

pub(super) fn append_audit_event(
    component: &str,
    action: &str,
    outcome: &str,
    details: Value,
) -> Result<()> {
    prodex_audit_log::append_audit_event(&audit_log_path(), component, action, outcome, details)
}

pub(super) fn format_audit_logs_summary() -> String {
    prodex_audit_log::format_audit_logs_summary(&audit_log_path())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn append_and_read_summary_use_the_same_override_path() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "prodex-audit-parity-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("test directory");
        let _guard =
            crate::TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", root.to_str().expect("utf8"));
        append_audit_event(
            "parity",
            "write",
            "success",
            serde_json::json!({"secret": "sk-test-secret"}),
        )
        .expect("append audit event");
        let path = audit_log_path();
        assert!(path.exists());
        assert!(format_audit_logs_summary().ends_with("(exists)"));
        let query = prodex_audit_log::AuditLogQuery {
            tail: 10,
            component: Some("parity".to_string()),
            ..Default::default()
        };
        let events =
            prodex_audit_log::read_recent_audit_events(&path, &query).expect("read audit events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, "write");
        let raw = fs::read_to_string(&path).expect("audit contents");
        assert!(!raw.contains("sk-test-secret"));
        fs::remove_dir_all(root).expect("cleanup");
    }
}
