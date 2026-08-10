use anyhow::Result;
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::{Command, Output};
use std::time::Duration;

const KIRO_METADATA_COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
const KIRO_METADATA_OUTPUT_MAX_BYTES: usize = 1024 * 1024;

pub(super) fn run_kiro_metadata_command(
    command: &OsStr,
    args: &[&str],
    cwd: Option<&Path>,
    extra_env: &[(OsString, OsString)],
) -> Result<Output> {
    run_kiro_metadata_command_with_timeout(
        command,
        args,
        cwd,
        extra_env,
        KIRO_METADATA_COMMAND_TIMEOUT,
    )
}

fn run_kiro_metadata_command_with_timeout(
    command: &OsStr,
    args: &[&str],
    cwd: Option<&Path>,
    extra_env: &[(OsString, OsString)],
    timeout: Duration,
) -> Result<Output> {
    let mut process = Command::new(command);
    process.args(args).envs(extra_env.iter().cloned());
    if let Some(cwd) = cwd {
        process.current_dir(cwd);
    }
    crate::command_output_with_timeout(
        &mut process,
        timeout,
        KIRO_METADATA_OUTPUT_MAX_BYTES,
        "Kiro metadata command",
    )
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn metadata_command_timeout_terminates_the_child() {
        let started = Instant::now();
        let error = run_kiro_metadata_command_with_timeout(
            OsStr::new("sh"),
            &["-c", "exec sleep 5"],
            None,
            &[],
            Duration::from_millis(50),
        )
        .unwrap_err();

        assert!(error.to_string().contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn metadata_command_does_not_wait_for_inherited_output_pipes() {
        let started = Instant::now();
        let output = run_kiro_metadata_command_with_timeout(
            OsStr::new("sh"),
            &[
                "-c",
                "(sleep 5) &
exit 0",
            ],
            None,
            &[],
            Duration::from_millis(50),
        )
        .expect("metadata should finish after closing inherited pipes");

        assert!(output.status.success());
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
