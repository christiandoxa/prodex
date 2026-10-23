use prodex_mojo_core::optional_tools_policy::{
    codebase_memory_daemon_status_supported, codebase_memory_version_supported,
};
use std::path::Path;
use std::time::Duration;

const DAEMON_STATUS_ARGS: [&str; 2] = ["daemon", "status"];
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn validate_version(version_line: &str) -> anyhow::Result<()> {
    let supported = codebase_memory_version_supported(
        version_line,
        crate::CODEBASE_MEMORY_MINIMUM_SUPPORTED_VERSION,
    )
    .map_err(|error| anyhow::anyhow!("failed to validate shared daemon version: {error:?}"))?;
    anyhow::ensure!(
        supported,
        "codebase-memory-mcp lacks shared daemon coordination; Prodex requires {} or newer. Update to the latest stable release (release-qualified reference: {})",
        crate::CODEBASE_MEMORY_MINIMUM_SUPPORTED_VERSION,
        crate::CODEBASE_MEMORY_LATEST_STABLE_REFERENCE,
    );
    Ok(())
}

pub(super) fn validate_daemon(program: &Path) -> anyhow::Result<()> {
    let output = crate::process::probe_command(program, &DAEMON_STATUS_ARGS, PROBE_TIMEOUT)?;
    let status_line = super::probe_first_line(&output)?;
    let accepted = codebase_memory_daemon_status_supported(output.status.success(), &status_line)
        .map_err(|error| {
        anyhow::anyhow!("failed to validate shared daemon status: {error:?}")
    })?;
    anyhow::ensure!(
        accepted,
        "codebase-memory-mcp daemon status check exited with {}: {status_line}",
        output.status
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_shared_daemon_release() {
        assert!(validate_version("codebase-memory-mcp 0.9.1-rc.1").is_ok());
        assert!(validate_version("codebase-memory-mcp 0.9.1").is_ok());
        assert!(validate_version("codebase-memory-mcp dev").is_ok());
        assert!(validate_version("codebase-memory-mcp 0.9.0").is_err());
        assert!(validate_version("unexpected output").is_err());
    }

    #[test]
    fn accepts_active_or_initially_stopped_daemon() {
        assert!(
            codebase_memory_daemon_status_supported(true, "daemon: active (permanent)").unwrap()
        );
        assert!(codebase_memory_daemon_status_supported(false, "daemon: not running").unwrap());
        assert!(!codebase_memory_daemon_status_supported(true, "usage: daemon <command>").unwrap());
        assert!(
            !codebase_memory_daemon_status_supported(false, "daemon: active (permanent)").unwrap()
        );
    }
}
