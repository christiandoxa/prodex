use anyhow::Result;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::RuntimeRotationProxyShared;

pub(super) use prodex_audit_log::{
    AuditLogQuery, AuditLogReadResult, render_audit_events_human_with_scope,
};

pub(super) fn audit_log_path() -> PathBuf {
    prodex_audit_log::audit_log_path(&super::runtime_proxy_log_dir())
}

pub(super) fn append_audit_event(
    component: &str,
    action: &str,
    outcome: &str,
    details: Value,
) -> Result<()> {
    let path = audit_log_path();
    prodex_audit_log::append_audit_event(&path, component, action, outcome, details)
}

pub(super) fn append_runtime_audit_event_best_effort(
    runtime_shared: &RuntimeRotationProxyShared,
    component: &str,
    action: &str,
    outcome: &str,
    details: Value,
) {
    let default_log_dir = runtime_shared
        .log_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let path = prodex_audit_log::audit_log_path(default_log_dir);
    append_runtime_audit_event_to_paths_best_effort(
        &runtime_shared.log_path,
        &path,
        component,
        action,
        outcome,
        details,
    );
}

fn append_runtime_audit_event_to_paths_best_effort(
    runtime_log_path: &Path,
    audit_log_path: &Path,
    component: &str,
    action: &str,
    outcome: &str,
    details: Value,
) {
    if prodex_audit_log::append_audit_event(audit_log_path, component, action, outcome, details)
        .is_err()
    {
        super::runtime_proxy_log_to_path(
            runtime_log_path,
            &runtime_proxy_structured_log_message(
                "gateway_audit_append_failed",
                [
                    runtime_proxy_log_field("component", component),
                    runtime_proxy_log_field("action", action),
                    runtime_proxy_log_field("error_kind", "gateway_audit_persistence_failed"),
                ],
            ),
        );
    }
}

pub(super) fn format_audit_logs_summary() -> String {
    let path = audit_log_path();
    prodex_audit_log::format_audit_logs_summary(&path)
}

pub(super) fn audit_logs_json_value() -> Value {
    prodex_audit_log::audit_logs_json_value(&super::runtime_proxy_log_dir())
}

pub(super) fn read_recent_audit_events_with_scope(
    query: &AuditLogQuery,
) -> Result<AuditLogReadResult> {
    prodex_audit_log::read_recent_audit_events_with_scope(&audit_log_path(), query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn runtime_admin_activate_and_release_audit_failures_are_visible() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "prodex-runtime-audit-failure-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("test directory should be created");
        let blocked_parent = dir.join("not-a-directory");
        fs::write(&blocked_parent, b"blocked").expect("blocking file should be written");
        let runtime_log_path = dir.join("runtime.log");
        crate::runtime_core_shared::prepare_runtime_proxy_test_log_path(&runtime_log_path);

        for action in ["activate_profile", "release_session_affinity"] {
            append_runtime_audit_event_to_paths_best_effort(
                &runtime_log_path,
                &blocked_parent.join("prodex-audit.jsonl"),
                "runtime_broker",
                action,
                "success",
                serde_json::json!({"secret": "not-logged"}),
            );
        }

        let log = crate::read_runtime_proxy_test_log(&runtime_log_path);
        assert!(log.contains("gateway_audit_append_failed"));
        assert!(log.contains("component=runtime_broker"));
        assert!(log.contains("action=activate_profile"));
        assert!(log.contains("action=release_session_affinity"));
        assert!(log.contains("error_kind=gateway_audit_persistence_failed"));
        assert!(!log.contains("not-a-directory"));
        assert!(!log.contains("not-logged"));
        fs::remove_dir_all(dir).expect("test directory should be removed");
    }
}
