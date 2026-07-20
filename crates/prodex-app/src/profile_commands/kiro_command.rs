use anyhow::{Context, Result, anyhow, bail};
use std::ffi::{OsStr, OsString};
use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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
    process
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .envs(extra_env.iter().cloned());
    if let Some(cwd) = cwd {
        process.current_dir(cwd);
    }
    let mut child = process
        .spawn()
        .context("failed to start Kiro metadata command")?;
    let stdout = child
        .stdout
        .take()
        .context("failed to capture Kiro metadata stdout")?;
    let stderr = child
        .stderr
        .take()
        .context("failed to capture Kiro metadata stderr")?;
    let stdout_reader = bounded_reader(stdout);
    let stderr_reader = bounded_reader(stderr);
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                terminate(&mut child);
                bail!("Kiro metadata command timed out");
            }
            Err(error) => {
                terminate(&mut child);
                return Err(error).context("failed to poll Kiro metadata command");
            }
        }
    };
    let stdout = join_reader(stdout_reader, "stdout")?;
    let stderr = join_reader(stderr_reader, "stderr")?;
    if stdout.len() > KIRO_METADATA_OUTPUT_MAX_BYTES
        || stderr.len() > KIRO_METADATA_OUTPUT_MAX_BYTES
    {
        bail!("Kiro metadata command output exceeded the limit");
    }
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

fn bounded_reader(
    mut reader: impl Read + Send + 'static,
) -> thread::JoinHandle<io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        reader
            .by_ref()
            .take((KIRO_METADATA_OUTPUT_MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        Ok(bytes)
    })
}

fn join_reader(reader: thread::JoinHandle<io::Result<Vec<u8>>>, stream: &str) -> Result<Vec<u8>> {
    reader
        .join()
        .map_err(|_| anyhow!("Kiro metadata {stream} reader panicked"))?
        .with_context(|| format!("failed to read Kiro metadata {stream}"))
}

fn terminate(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

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
}
