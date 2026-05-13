use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::{
    ProcessRow, RuntimeBrokerHealth, RuntimeBrokerRegistry, RuntimeProdexBinaryIdentity,
    collect_process_rows, parse_prodex_version_output,
};

#[derive(Debug, Clone)]
struct RuntimeProcessVersionResolution {
    executable_path: Option<PathBuf>,
    version: Option<String>,
    executable_sha256: Option<String>,
}

trait RuntimeProcessPlatform {
    fn pid_alive(pid: u32) -> bool;
    fn executable_path(pid: u32) -> Option<PathBuf>;
    fn terminate_step(pid_value: &str, force: bool);
}

#[cfg(target_os = "linux")]
struct RuntimeProcessLinux;

#[cfg(windows)]
struct RuntimeProcessWindows;

#[cfg(not(any(target_os = "linux", windows)))]
struct RuntimeProcessFallback;

type RuntimeProcessPlatformImpl = RuntimeProcessPlatformForTarget;

#[cfg(target_os = "linux")]
type RuntimeProcessPlatformForTarget = RuntimeProcessLinux;

#[cfg(windows)]
type RuntimeProcessPlatformForTarget = RuntimeProcessWindows;

#[cfg(not(any(target_os = "linux", windows)))]
type RuntimeProcessPlatformForTarget = RuntimeProcessFallback;

fn runtime_process_row(pid: u32) -> Option<ProcessRow> {
    collect_process_rows()
        .into_iter()
        .find(|row| row.pid == pid)
}

#[cfg(target_os = "linux")]
impl RuntimeProcessPlatform for RuntimeProcessLinux {
    fn pid_alive(pid: u32) -> bool {
        PathBuf::from(format!("/proc/{pid}")).exists()
    }

    fn executable_path(pid: u32) -> Option<PathBuf> {
        fs::read_link(format!("/proc/{pid}/exe")).ok()
    }

    fn terminate_step(pid_value: &str, force: bool) {
        let signal = if force { "-KILL" } else { "-TERM" };
        let _ = Command::new("kill")
            .args([signal, pid_value])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

#[cfg(windows)]
impl RuntimeProcessPlatform for RuntimeProcessWindows {
    fn pid_alive(pid: u32) -> bool {
        let Ok(output) = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
        else {
            return false;
        };
        if !output.status.success() {
            return false;
        }
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                trimmed
                    .strip_prefix('"')
                    .and_then(|value| value.split("\",\"").nth(1))
                    .and_then(|value| value.parse::<u32>().ok())
            })
            .any(|listed_pid| listed_pid == pid)
    }

    fn executable_path(pid: u32) -> Option<PathBuf> {
        for shell in ["powershell", "pwsh"] {
            let Ok(output) = Command::new(shell)
                .args([
                    "-NoProfile",
                    "-Command",
                    &format!("(Get-Process -Id {pid} -ErrorAction SilentlyContinue).Path"),
                ])
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()
            else {
                continue;
            };
            if !output.status.success() {
                continue;
            }
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
        None
    }

    fn terminate_step(pid_value: &str, force: bool) {
        let mut command = Command::new("taskkill");
        command.args(["/PID", pid_value, "/T"]);
        if force {
            command.arg("/F");
        }
        let _ = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
impl RuntimeProcessPlatform for RuntimeProcessFallback {
    fn pid_alive(pid: u32) -> bool {
        let pid_value = pid.to_string();
        Command::new("kill")
            .arg("-0")
            .arg(pid_value)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    fn executable_path(_pid: u32) -> Option<PathBuf> {
        None
    }

    fn terminate_step(pid_value: &str, force: bool) {
        let signal = if force { "-KILL" } else { "-TERM" };
        let _ = Command::new("kill")
            .args([signal, pid_value])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

pub(crate) fn runtime_current_prodex_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub(crate) fn runtime_executable_sha256(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let digest = Sha256::digest(&bytes);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub(crate) fn runtime_current_binary_identity() -> (Option<String>, Option<String>) {
    let path = env::current_exe().ok();
    let sha256 = path
        .as_deref()
        .and_then(|path| runtime_executable_sha256(path).ok());
    (path.map(|path| path.display().to_string()), sha256)
}

pub(crate) fn runtime_process_pid_alive(pid: u32) -> bool {
    if RuntimeProcessPlatformImpl::pid_alive(pid) {
        return true;
    }
    runtime_process_row(pid).is_some()
}

pub(crate) fn read_prodex_sha256_from_executable(executable: &Path) -> Result<String> {
    runtime_executable_sha256(executable)
}

pub(crate) fn read_prodex_version_from_executable(executable: &Path) -> Result<String> {
    let output = Command::new(executable)
        .arg("--version")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .with_context(|| format!("failed to run {} --version", executable.display()))?;
    if !output.status.success() {
        bail!(
            "{} --version exited with status {}",
            executable.display(),
            output
                .status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "signal".to_string())
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_prodex_version_output(&stdout).with_context(|| {
        format!(
            "failed to parse prodex version output from {}",
            executable.display()
        )
    })
}

fn resolve_prodex_executable_identity(
    executable_candidates: &[PathBuf],
) -> (Option<PathBuf>, Option<String>, Option<String>) {
    let mut first_candidate = None;
    let mut first_sha256 = None;
    for executable in executable_candidates {
        if first_candidate.is_none() {
            first_candidate = Some(executable.clone());
        }
        let candidate_sha256 = read_prodex_sha256_from_executable(executable).ok();
        if first_sha256.is_none() {
            first_sha256 = candidate_sha256.clone();
        }
        if let Ok(version) = read_prodex_version_from_executable(executable) {
            return (
                Some(executable.clone()),
                Some(version),
                candidate_sha256.or(first_sha256),
            );
        }
    }
    (first_candidate, None, first_sha256)
}

fn push_runtime_process_candidate(candidates: &mut Vec<PathBuf>, path: PathBuf) {
    if !candidates.iter().any(|candidate| candidate == &path) {
        candidates.push(path);
    }
}

fn runtime_process_executable_candidates(pid: u32, row: Option<&ProcessRow>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(executable) = RuntimeProcessPlatformImpl::executable_path(pid) {
        push_runtime_process_candidate(&mut candidates, executable);
    }
    if let Some(row) = row {
        for arg in &row.args {
            let path = PathBuf::from(arg);
            if path.exists() {
                push_runtime_process_candidate(&mut candidates, path);
            }
        }
    }
    candidates
}

fn runtime_process_version_resolution(pid: u32) -> RuntimeProcessVersionResolution {
    let row = runtime_process_row(pid);
    let executable_candidates = runtime_process_executable_candidates(pid, row.as_ref());
    let (executable_path, version, executable_sha256) =
        resolve_prodex_executable_identity(&executable_candidates);
    RuntimeProcessVersionResolution {
        executable_path,
        version,
        executable_sha256,
    }
}

pub(crate) fn runtime_current_prodex_binary_identity() -> RuntimeProdexBinaryIdentity {
    let executable_path = env::current_exe().ok();
    let executable_sha256 = executable_path
        .as_ref()
        .and_then(|path| read_prodex_sha256_from_executable(path).ok());
    RuntimeProdexBinaryIdentity {
        prodex_version: Some(runtime_current_prodex_version().to_string()),
        executable_path,
        executable_sha256,
    }
}

pub(crate) fn runtime_current_prodex_version_identity() -> RuntimeProdexBinaryIdentity {
    RuntimeProdexBinaryIdentity {
        prodex_version: Some(runtime_current_prodex_version().to_string()),
        executable_path: env::current_exe().ok(),
        executable_sha256: None,
    }
}

pub(crate) fn runtime_process_prodex_binary_identity(pid: u32) -> RuntimeProdexBinaryIdentity {
    let resolution = runtime_process_version_resolution(pid);
    RuntimeProdexBinaryIdentity {
        prodex_version: resolution.version,
        executable_path: resolution.executable_path,
        executable_sha256: resolution.executable_sha256,
    }
}

pub(super) fn runtime_broker_observed_binary_identity(
    registry: &RuntimeBrokerRegistry,
    health: Option<&RuntimeBrokerHealth>,
) -> RuntimeProdexBinaryIdentity {
    prodex_runtime_broker::runtime_broker_observed_known_binary_identity(registry, health)
        .unwrap_or_else(|| runtime_process_prodex_binary_identity(registry.pid))
}

pub(crate) fn runtime_process_prodex_version(pid: u32) -> Option<String> {
    runtime_process_version_resolution(pid).version
}

pub(crate) fn terminate_runtime_process(pid: u32) {
    if !runtime_process_pid_alive(pid) {
        return;
    }

    let pid_value = pid.to_string();
    let wait_for_exit = |timeout_ms: u64| -> bool {
        let started_at = Instant::now();
        while started_at.elapsed() < Duration::from_millis(timeout_ms) {
            if !runtime_process_pid_alive(pid) {
                return true;
            }
            thread::sleep(Duration::from_millis(20));
        }
        !runtime_process_pid_alive(pid)
    };

    RuntimeProcessPlatformImpl::terminate_step(&pid_value, false);
    if wait_for_exit(500) {
        return;
    }

    RuntimeProcessPlatformImpl::terminate_step(&pid_value, true);
    let _ = wait_for_exit(250);
}
