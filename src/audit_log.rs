use anyhow::{Context, Result};
use chrono::Local;
use serde::Serialize;
use serde_json::Value;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

const AUDIT_LOG_FILE_NAME: &str = "prodex-audit.log";

#[derive(Debug, Serialize)]
struct AuditEvent<'a> {
    recorded_at: String,
    recorded_at_epoch: i64,
    pid: u32,
    component: &'a str,
    action: &'a str,
    outcome: &'a str,
    details: Value,
}

pub(super) fn audit_log_dir() -> PathBuf {
    env::var_os("PRODEX_AUDIT_LOG_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(super::runtime_proxy_log_dir)
}

pub(super) fn audit_log_path() -> PathBuf {
    audit_log_dir().join(AUDIT_LOG_FILE_NAME)
}

pub(super) fn append_audit_event(
    component: &str,
    action: &str,
    outcome: &str,
    details: Value,
) -> Result<()> {
    let path = audit_log_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let event = AuditEvent {
        recorded_at: Local::now().to_rfc3339(),
        recorded_at_epoch: Local::now().timestamp(),
        pid: std::process::id(),
        component,
        action,
        outcome,
        details,
    };
    let line = serde_json::to_string(&event).context("failed to serialize audit event")?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    writeln!(file, "{line}").with_context(|| format!("failed to append {}", path.display()))?;
    Ok(())
}

pub(super) fn format_audit_logs_summary() -> String {
    let path = audit_log_path();
    format!(
        "{} ({})",
        path.display(),
        if path.exists() { "exists" } else { "missing" }
    )
}

pub(super) fn audit_logs_json_value() -> Value {
    let path = audit_log_path();
    serde_json::json!({
        "directory": audit_log_dir().display().to_string(),
        "path": path.display().to_string(),
        "exists": path.exists(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct AuditLogEnvGuard {
        previous: Option<std::ffi::OsString>,
    }

    impl AuditLogEnvGuard {
        fn set(name: &str, value: &str) -> Self {
            let previous = env::var_os(name);
            // SAFETY: tests serialize on a single thread when they mutate process env.
            unsafe { env::set_var(name, value) };
            Self { previous }
        }
    }

    impl Drop for AuditLogEnvGuard {
        fn drop(&mut self) {
            match self.previous.as_ref() {
                Some(value) => {
                    // SAFETY: tests serialize on a single thread when they mutate process env.
                    unsafe { env::set_var("PRODEX_AUDIT_LOG_DIR", value) };
                }
                None => {
                    // SAFETY: tests serialize on a single thread when they mutate process env.
                    unsafe { env::remove_var("PRODEX_AUDIT_LOG_DIR") };
                }
            }
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = env::temp_dir().join(format!(
            "prodex-audit-log-{name}-{}-{nanos:x}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn audit_log_path_uses_env_override() {
        let dir = temp_dir("path");
        let _guard = AuditLogEnvGuard::set("PRODEX_AUDIT_LOG_DIR", &dir.display().to_string());

        assert_eq!(audit_log_path(), dir.join(AUDIT_LOG_FILE_NAME));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn append_audit_event_writes_json_line() {
        let dir = temp_dir("append");
        let _guard = AuditLogEnvGuard::set("PRODEX_AUDIT_LOG_DIR", &dir.display().to_string());

        append_audit_event(
            "profile",
            "add",
            "success",
            serde_json::json!({
                "profile_name": "main",
                "managed": true,
            }),
        )
        .unwrap();

        let content = fs::read_to_string(audit_log_path()).unwrap();
        let line = content.lines().next().unwrap();
        let value: Value = serde_json::from_str(line).unwrap();
        assert_eq!(
            value.get("component").and_then(Value::as_str),
            Some("profile")
        );
        assert_eq!(value.get("action").and_then(Value::as_str), Some("add"));
        assert_eq!(
            value.get("outcome").and_then(Value::as_str),
            Some("success")
        );
        assert_eq!(
            value
                .pointer("/details/profile_name")
                .and_then(Value::as_str),
            Some("main")
        );

        let _ = fs::remove_dir_all(dir);
    }
}
