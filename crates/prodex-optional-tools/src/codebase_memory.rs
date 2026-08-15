use std::path::Path;
use std::time::Duration;

const SHARED_DAEMON_MIN_VERSION: &str = "0.9.1-rc.1";
const DAEMON_STATUS_ARGS: [&str; 2] = ["daemon", "status"];
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn validate_version(version_line: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        has_shared_daemon(version_line),
        "codebase-memory-mcp lacks shared daemon coordination; update to {SHARED_DAEMON_MIN_VERSION} or newer"
    );
    Ok(())
}

pub(super) fn validate_daemon(program: &Path) -> anyhow::Result<()> {
    let output = crate::process::probe_command(program, &DAEMON_STATUS_ARGS, PROBE_TIMEOUT)?;
    let status_line = super::probe_first_line(&output)?;
    anyhow::ensure!(
        daemon_status_is_accepted(output.status.success(), &status_line),
        "codebase-memory-mcp daemon status check exited with {}: {status_line}",
        output.status
    );
    Ok(())
}

fn has_shared_daemon(version_line: &str) -> bool {
    let Some(version) = version_line.strip_prefix("codebase-memory-mcp ") else {
        return false;
    };
    if version == "dev" {
        return true;
    }
    let (Ok(version), Ok(minimum)) = (
        semver::Version::parse(version),
        semver::Version::parse(SHARED_DAEMON_MIN_VERSION),
    ) else {
        return false;
    };
    version >= minimum
}

fn daemon_status_is_accepted(success: bool, status_line: &str) -> bool {
    (status_line.starts_with("daemon: active") && success)
        || status_line.starts_with("daemon: not running")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_shared_daemon_release() {
        assert!(has_shared_daemon("codebase-memory-mcp 0.9.1-rc.1"));
        assert!(has_shared_daemon("codebase-memory-mcp 0.9.1"));
        assert!(has_shared_daemon("codebase-memory-mcp dev"));
        assert!(!has_shared_daemon("codebase-memory-mcp 0.9.0"));
        assert!(!has_shared_daemon("unexpected output"));
    }

    #[test]
    fn accepts_active_or_initially_stopped_daemon() {
        assert!(daemon_status_is_accepted(
            true,
            "daemon: active (permanent)"
        ));
        assert!(daemon_status_is_accepted(false, "daemon: not running"));
        assert!(!daemon_status_is_accepted(true, "usage: daemon <command>"));
        assert!(!daemon_status_is_accepted(
            false,
            "daemon: active (permanent)"
        ));
    }
}
