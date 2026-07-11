use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;

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
