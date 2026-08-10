use anyhow::{Context, Result, bail};
use std::env;
use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const PROBE_OUTPUT_LIMIT: usize = 64 * 1024;
const PROBE_OUTPUT_DRAIN_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Debug)]
pub(crate) struct ProbeOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub truncated: bool,
}

pub(crate) fn probe_command(
    program: &Path,
    args: &[impl AsRef<OsStr>],
    timeout: Duration,
) -> Result<ProbeOutput> {
    probe_command_inner(program, args, timeout, false)
}

pub(crate) fn probe_command_without_secrets(
    program: &Path,
    args: &[impl AsRef<OsStr>],
    timeout: Duration,
) -> Result<ProbeOutput> {
    probe_command_inner(program, args, timeout, true)
}

fn probe_command_inner(
    program: &Path,
    args: &[impl AsRef<OsStr>],
    timeout: Duration,
    sanitize_environment: bool,
) -> Result<ProbeOutput> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    if sanitize_environment {
        command.env_clear();
        for key in [
            "PATH",
            "HOME",
            "TMPDIR",
            "TMP",
            "TEMP",
            "XDG_CACHE_HOME",
            "NPM_CONFIG_CACHE",
            "npm_config_cache",
        ] {
            if let Some(value) = env::var_os(key) {
                command.env(key, value);
            }
        }
        command.env(
            "npm_config_userconfig",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        );
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to execute {}", program.display()))?;
    let stdout = child
        .stdout
        .take()
        .context("probe stdout pipe is missing")?;
    let stderr = child
        .stderr
        .take()
        .context("probe stderr pipe is missing")?;
    let stdout_reader = thread::spawn(move || read_bounded(stdout));
    let stderr_reader = thread::spawn(move || read_bounded(stderr));
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .with_context(|| format!("failed to wait for {}", program.display()))?
        {
            break status;
        }
        if Instant::now() >= deadline {
            terminate_probe_process_tree(&mut child);
            let _ = child.wait();
            bail!(
                "{} health check timed out after {timeout:?}",
                program.display()
            );
        }
        thread::sleep(Duration::from_millis(10));
    };
    if !probe_readers_finished(&stdout_reader, &stderr_reader) {
        let deadline = Instant::now() + PROBE_OUTPUT_DRAIN_TIMEOUT;
        while !probe_readers_finished(&stdout_reader, &stderr_reader) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
    }
    if !probe_readers_finished(&stdout_reader, &stderr_reader) {
        terminate_probe_process_tree(&mut child);
        let deadline = Instant::now() + PROBE_OUTPUT_DRAIN_TIMEOUT;
        while !probe_readers_finished(&stdout_reader, &stderr_reader) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
    }
    if !probe_readers_finished(&stdout_reader, &stderr_reader) {
        bail!("{} health check output did not close", program.display());
    }
    let (stdout, stdout_truncated) = stdout_reader
        .join()
        .map_err(|_| anyhow::anyhow!("{} stdout reader panicked", program.display()))??;
    let (stderr, stderr_truncated) = stderr_reader
        .join()
        .map_err(|_| anyhow::anyhow!("{} stderr reader panicked", program.display()))??;
    Ok(ProbeOutput {
        status,
        stdout,
        stderr,
        truncated: stdout_truncated || stderr_truncated,
    })
}

fn probe_readers_finished<T, U>(
    stdout: &thread::JoinHandle<T>,
    stderr: &thread::JoinHandle<U>,
) -> bool {
    stdout.is_finished() && stderr.is_finished()
}

fn terminate_probe_process_tree(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        let pid = child.id() as libc::pid_t;
        if pid > 0 {
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
        }
    }
    let _ = child.kill();
}

fn read_bounded(mut reader: impl std::io::Read) -> std::io::Result<(Vec<u8>, bool)> {
    let mut retained = Vec::new();
    let mut buffer = [0_u8; 8 * 1024];
    let mut truncated = false;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let remaining = PROBE_OUTPUT_LIMIT.saturating_sub(retained.len());
        let keep = remaining.min(read);
        retained.extend_from_slice(&buffer[..keep]);
        truncated |= keep < read;
    }
    Ok((retained, truncated))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::sync::{Mutex, OnceLock};

    static TEST_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct EnvGuard {
        key: &'static str,
        previous: Option<OsString>,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let lock = TEST_ENV_LOCK
                .get_or_init(|| Mutex::new(()))
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let previous = env::var_os(key);
            // SAFETY: the shared test lock serializes this mutation and its restoration.
            unsafe { env::set_var(key, value) };
            Self {
                key,
                previous,
                _lock: lock,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: the shared test lock is held for the complete guard lifetime.
            unsafe {
                match self.previous.as_ref() {
                    Some(value) => env::set_var(self.key, value),
                    None => env::remove_var(self.key),
                }
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn probe_reports_exit_and_bounds_output() {
        let output = probe_command(
            Path::new("/bin/sh"),
            &["-c", "yes x | head -c 70000; exit 7"],
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(output.status.code(), Some(7));
        assert_eq!(output.stdout.len(), PROBE_OUTPUT_LIMIT);
        assert!(output.truncated);
    }

    #[cfg(unix)]
    #[test]
    fn probe_times_out() {
        let started = Instant::now();
        let error = probe_command(
            Path::new("/bin/sh"),
            &["-c", "sleep 5"],
            Duration::from_millis(20),
        )
        .unwrap_err();
        assert!(error.to_string().contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[cfg(unix)]
    #[test]
    fn probe_does_not_wait_for_inherited_output_pipes() {
        let started = Instant::now();
        let output = probe_command(
            Path::new("/bin/sh"),
            &["-c", "(sleep 5) & exit 0"],
            Duration::from_secs(2),
        )
        .unwrap();

        assert!(output.status.success());
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[cfg(unix)]
    #[test]
    fn sanitized_probe_does_not_inherit_untrusted_environment() {
        const SECRET: &str = "PRODEX_PROBE_TEST_SECRET";
        let _env_guard = EnvGuard::set(SECRET, "sentinel");
        let output = probe_command_without_secrets(
            Path::new("/bin/sh"),
            &["-c", "printf '%s' \"${PRODEX_PROBE_TEST_SECRET:-}\""],
            Duration::from_secs(2),
        );

        assert!(output.unwrap().stdout.is_empty());
    }
}
