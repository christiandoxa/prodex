use crate::{
    ProcessRow, RuntimeBrokerHealth, RuntimeBrokerRegistry, RuntimeProdexBinaryIdentity,
    parse_prodex_version_output,
};
use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
#[cfg(any(target_os = "macos", test))]
use std::thread;
#[cfg(test)]
use std::time::{Duration, Instant};

const RUNTIME_PRODEX_EXECUTABLE_HASH_MAX_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeProcessTerminationOutcome {
    NotRunning,
    OwnershipUnproven,
    OwnershipChanged,
    Terminated,
    StillRunning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
pub(crate) enum RuntimeProcessIdentityOutcome {
    Absent,
    Proven,
    OwnershipChanged,
    OwnershipUnproven,
}

#[derive(Debug, Clone)]
struct RuntimeProcessVersionResolution {
    executable_path: Option<PathBuf>,
    version: Option<String>,
    executable_sha256: Option<String>,
}

#[path = "platform.rs"]
mod platform;
#[cfg(target_os = "macos")]
use self::platform::RuntimeProcessMacos;
#[cfg(not(target_os = "macos"))]
use self::platform::runtime_process_row;
use self::platform::{RuntimeProcessPlatform, RuntimeProcessPlatformImpl};

#[cfg(any(target_os = "macos", test))]
fn runtime_process_executable_paths_match(actual: &Path, expected: &Path) -> bool {
    actual == expected
        || fs::canonicalize(actual)
            .ok()
            .zip(fs::canonicalize(expected).ok())
            .is_some_and(|(actual, expected)| actual == expected)
}

#[cfg(any(target_os = "macos", test))]
fn runtime_process_identity_outcome_for<P: RuntimeProcessPlatform>(
    pid: u32,
    expected_birth_identity: Option<&str>,
    expected_executable_path: Option<&Path>,
) -> RuntimeProcessIdentityOutcome {
    if P::process_absence_proven(pid) {
        return RuntimeProcessIdentityOutcome::Absent;
    }
    let (Some(expected_birth_identity), Some(expected_executable_path)) =
        (expected_birth_identity, expected_executable_path)
    else {
        return RuntimeProcessIdentityOutcome::OwnershipUnproven;
    };
    match P::process_birth_identity(pid) {
        Some(actual) if actual == expected_birth_identity => {}
        Some(_) => return RuntimeProcessIdentityOutcome::OwnershipChanged,
        None if P::process_absence_proven(pid) => return RuntimeProcessIdentityOutcome::Absent,
        None => return RuntimeProcessIdentityOutcome::OwnershipUnproven,
    }
    match P::executable_path(pid) {
        Some(actual)
            if runtime_process_executable_paths_match(&actual, expected_executable_path) =>
        {
            // Re-read the start identity after the path lookup so a same-path PID reuse
            // cannot pass both checks.
            match P::process_birth_identity(pid) {
                Some(actual) if actual == expected_birth_identity => {
                    RuntimeProcessIdentityOutcome::Proven
                }
                Some(_) => RuntimeProcessIdentityOutcome::OwnershipChanged,
                None if P::process_absence_proven(pid) => RuntimeProcessIdentityOutcome::Absent,
                None => RuntimeProcessIdentityOutcome::OwnershipUnproven,
            }
        }
        Some(_) => RuntimeProcessIdentityOutcome::OwnershipChanged,
        None if P::process_absence_proven(pid) => RuntimeProcessIdentityOutcome::Absent,
        None => RuntimeProcessIdentityOutcome::OwnershipUnproven,
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn runtime_process_identity_outcome(
    pid: u32,
    expected_birth_identity: Option<&str>,
    expected_executable_path: Option<&Path>,
) -> RuntimeProcessIdentityOutcome {
    runtime_process_identity_outcome_for::<RuntimeProcessMacos>(
        pid,
        expected_birth_identity,
        expected_executable_path,
    )
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn runtime_process_identity_outcome(
    pid: u32,
    _expected_birth_identity: Option<&str>,
    _expected_executable_path: Option<&Path>,
) -> RuntimeProcessIdentityOutcome {
    if RuntimeProcessPlatformImpl::process_absence_proven(pid) {
        RuntimeProcessIdentityOutcome::Absent
    } else {
        RuntimeProcessIdentityOutcome::Proven
    }
}

#[cfg(any(target_os = "macos", test))]
fn terminate_runtime_process_with_platform<P: RuntimeProcessPlatform>(
    pid: u32,
    expected_birth_identity: Option<&str>,
    expected_executable_path: Option<&Path>,
) -> RuntimeProcessTerminationOutcome {
    match runtime_process_identity_outcome_for::<P>(
        pid,
        expected_birth_identity,
        expected_executable_path,
    ) {
        RuntimeProcessIdentityOutcome::Absent => RuntimeProcessTerminationOutcome::NotRunning,
        RuntimeProcessIdentityOutcome::OwnershipChanged => {
            RuntimeProcessTerminationOutcome::OwnershipChanged
        }
        RuntimeProcessIdentityOutcome::OwnershipUnproven => {
            RuntimeProcessTerminationOutcome::OwnershipUnproven
        }
        RuntimeProcessIdentityOutcome::Proven => {
            P::terminate(pid, expected_birth_identity, expected_executable_path)
        }
    }
}

pub(crate) fn runtime_current_prodex_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub(crate) fn runtime_executable_sha256(path: &Path) -> Result<String> {
    if cfg!(debug_assertions) && env::var_os("PRODEX_TEST_SKIP_BINARY_SHA256").is_some() {
        return Ok("test-skip-sha256".to_string());
    }
    let metadata =
        fs::metadata(path).with_context(|| format!("failed to inspect {}", path.display()))?;
    if metadata.len() > RUNTIME_PRODEX_EXECUTABLE_HASH_MAX_BYTES {
        bail!(
            "{} exceeds executable hash size limit ({} bytes)",
            path.display(),
            RUNTIME_PRODEX_EXECUTABLE_HASH_MAX_BYTES
        );
    }

    let mut file = prodex_core::open_regular_file_no_follow(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    if !prodex_core::opened_file_matches_path(&metadata, path, &file)
        .with_context(|| format!("failed to inspect {}", path.display()))?
    {
        bail!("{} changed while hashing", path.display());
    }

    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut read_bytes = 0_u64;
    loop {
        let len = file
            .read(&mut buffer)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if len == 0 {
            break;
        }
        read_bytes = read_bytes.saturating_add(len as u64);
        if read_bytes > RUNTIME_PRODEX_EXECUTABLE_HASH_MAX_BYTES {
            bail!(
                "{} exceeds executable hash size limit ({} bytes)",
                path.display(),
                RUNTIME_PRODEX_EXECUTABLE_HASH_MAX_BYTES
            );
        }
        hasher.update(&buffer[..len]);
    }
    let digest = hasher.finalize();
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub(crate) fn runtime_current_binary_identity() -> (Option<String>, Option<String>) {
    let identity = runtime_current_prodex_binary_identity();
    (
        identity
            .executable_path
            .map(|path| path.display().to_string()),
        identity.executable_sha256,
    )
}

pub(crate) fn runtime_process_pid_alive(pid: u32) -> bool {
    if RuntimeProcessPlatformImpl::pid_alive(pid) {
        return true;
    }
    #[cfg(target_os = "macos")]
    return false;
    #[cfg(not(target_os = "macos"))]
    runtime_process_row(pid).is_some()
}

pub(crate) fn runtime_process_absence_proven(pid: u32) -> bool {
    RuntimeProcessPlatformImpl::process_absence_proven(pid)
}

pub(crate) fn runtime_process_birth_identity(pid: u32) -> Option<String> {
    RuntimeProcessPlatformImpl::process_birth_identity(pid)
}

pub(crate) fn read_prodex_sha256_from_executable(executable: &Path) -> Result<String> {
    runtime_executable_sha256(executable)
}

pub(crate) fn read_prodex_version_from_executable(executable: &Path) -> Result<String> {
    let mut command = Command::new(executable);
    command.arg("--version");
    let output = crate::command_probe_output(&mut command, "Prodex version probe")
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
    #[cfg(not(target_os = "macos"))]
    if let Some(row) = row {
        for arg in &row.args {
            let path = PathBuf::from(arg);
            if path.exists() {
                push_runtime_process_candidate(&mut candidates, path);
            }
        }
    }
    #[cfg(target_os = "macos")]
    let _ = row;
    candidates
}

fn runtime_process_version_resolution(pid: u32) -> RuntimeProcessVersionResolution {
    #[cfg(not(target_os = "macos"))]
    let row = runtime_process_row(pid);
    #[cfg(target_os = "macos")]
    let row = None;
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
    static IDENTITY: OnceLock<RuntimeProdexBinaryIdentity> = OnceLock::new();
    IDENTITY
        .get_or_init(|| {
            let executable_path = env::current_exe().ok();
            let executable_sha256 = executable_path
                .as_ref()
                .and_then(|path| read_prodex_sha256_from_executable(path).ok());
            RuntimeProdexBinaryIdentity {
                prodex_version: Some(runtime_current_prodex_version().to_string()),
                executable_path,
                executable_sha256,
            }
        })
        .clone()
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

pub(crate) fn terminate_runtime_process(
    pid: u32,
    expected_birth_identity: Option<&str>,
    expected_executable_path: Option<&Path>,
) -> RuntimeProcessTerminationOutcome {
    #[cfg(target_os = "macos")]
    {
        return terminate_runtime_process_with_platform::<RuntimeProcessMacos>(
            pid,
            expected_birth_identity,
            expected_executable_path,
        );
    }
    #[cfg(not(target_os = "macos"))]
    RuntimeProcessPlatformImpl::terminate(pid, expected_birth_identity, expected_executable_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

    const FAKE_ABSENT: u8 = 0;
    const FAKE_MATCH_TERMINATES: u8 = 1;
    const FAKE_BIRTH_CHANGED: u8 = 2;
    const FAKE_PATH_CHANGED: u8 = 3;
    const FAKE_PERMISSION_DENIED: u8 = 4;
    const FAKE_STILL_RUNNING: u8 = 5;
    const FAKE_FORCED_TERMINATES: u8 = 6;
    const FAKE_BIRTH_CHANGED_AFTER_PATH: u8 = 7;
    static FAKE_PROCESS_STATE: AtomicU8 = AtomicU8::new(FAKE_ABSENT);
    static FAKE_BIRTH_READS: AtomicUsize = AtomicUsize::new(0);
    static FAKE_TERMINATE_CALLS: AtomicUsize = AtomicUsize::new(0);

    struct FakeRuntimeProcess;

    impl RuntimeProcessPlatform for FakeRuntimeProcess {
        fn pid_alive(_pid: u32) -> bool {
            FAKE_PROCESS_STATE.load(Ordering::SeqCst) != FAKE_ABSENT
        }

        fn process_absence_proven(_pid: u32) -> bool {
            FAKE_PROCESS_STATE.load(Ordering::SeqCst) == FAKE_ABSENT
        }

        fn process_birth_identity(_pid: u32) -> Option<String> {
            match FAKE_PROCESS_STATE.load(Ordering::SeqCst) {
                FAKE_PERMISSION_DENIED => None,
                FAKE_BIRTH_CHANGED => Some("birth-reused".to_string()),
                FAKE_BIRTH_CHANGED_AFTER_PATH => {
                    if FAKE_BIRTH_READS.fetch_add(1, Ordering::SeqCst) == 0 {
                        Some("birth-expected".to_string())
                    } else {
                        Some("birth-reused".to_string())
                    }
                }
                _ => Some("birth-expected".to_string()),
            }
        }

        fn executable_path(_pid: u32) -> Option<PathBuf> {
            match FAKE_PROCESS_STATE.load(Ordering::SeqCst) {
                FAKE_PERMISSION_DENIED => None,
                FAKE_PATH_CHANGED => Some(PathBuf::from("/opt/other/bin/worker")),
                _ => Some(PathBuf::from("/opt/prodex/bin/prodex")),
            }
        }

        fn terminate(
            _pid: u32,
            _expected_birth_identity: Option<&str>,
            _expected_executable_path: Option<&Path>,
        ) -> RuntimeProcessTerminationOutcome {
            let signals = if FAKE_PROCESS_STATE.load(Ordering::SeqCst) == FAKE_FORCED_TERMINATES {
                2
            } else {
                1
            };
            FAKE_TERMINATE_CALLS.fetch_add(signals, Ordering::SeqCst);
            if FAKE_PROCESS_STATE.load(Ordering::SeqCst) == FAKE_STILL_RUNNING {
                RuntimeProcessTerminationOutcome::StillRunning
            } else {
                RuntimeProcessTerminationOutcome::Terminated
            }
        }
    }

    #[test]
    fn runtime_executable_sha256_hashes_small_files() {
        if cfg!(debug_assertions) && env::var_os("PRODEX_TEST_SKIP_BINARY_SHA256").is_some() {
            return;
        }
        let path = runtime_process_test_path("small-sha256");
        let mut file = fs::File::create(&path).expect("test executable should be created");
        file.write_all(b"abc")
            .expect("test executable should be written");

        let digest = runtime_executable_sha256(&path).expect("test executable should hash");

        assert_eq!(
            digest,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn runtime_executable_sha256_rejects_oversized_files_before_reading() {
        if cfg!(debug_assertions) && env::var_os("PRODEX_TEST_SKIP_BINARY_SHA256").is_some() {
            return;
        }
        let path = runtime_process_test_path("oversized-sha256");
        fs::File::create(&path)
            .expect("test executable should be created")
            .set_len(RUNTIME_PRODEX_EXECUTABLE_HASH_MAX_BYTES + 1)
            .expect("test executable should be made oversized");

        let err = runtime_executable_sha256(&path)
            .expect_err("oversized executable should not be hashed");

        assert!(format!("{err:#}").contains("exceeds executable hash size limit"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn termination_requires_proven_process_ownership() {
        let pid = std::process::id();
        let executable = env::current_exe().expect("current executable should be known");

        assert_eq!(
            terminate_runtime_process(pid, None, Some(&executable)),
            RuntimeProcessTerminationOutcome::OwnershipUnproven
        );
        assert_eq!(
            terminate_runtime_process(pid, Some("not-this-process"), Some(&executable)),
            if cfg!(any(target_os = "linux", target_os = "macos", windows)) {
                RuntimeProcessTerminationOutcome::OwnershipChanged
            } else {
                RuntimeProcessTerminationOutcome::OwnershipUnproven
            }
        );
        assert!(runtime_process_pid_alive(pid));
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    #[test]
    fn current_process_has_a_birth_identity() {
        assert!(runtime_process_birth_identity(std::process::id()).is_some());
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    #[test]
    fn current_process_absence_is_not_proven() {
        assert!(!runtime_process_absence_proven(std::process::id()));
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    #[test]
    fn native_child_smoke_uses_stable_process_reference() {
        if env::var_os("PRODEX_RUNTIME_PROCESS_CHILD").is_some() {
            loop {
                thread::sleep(Duration::from_secs(60));
            }
        }

        let mut child = Command::new(env::current_exe().expect("test executable should exist"))
            .args([
                "native_child_smoke_uses_stable_process_reference",
                "--nocapture",
            ])
            .env("PRODEX_RUNTIME_PROCESS_CHILD", "1")
            .spawn()
            .expect("native child should spawn");
        let started_at = Instant::now();
        let expected_birth_identity = loop {
            if let Some(identity) = runtime_process_birth_identity(child.id()) {
                break identity;
            }
            if child
                .try_wait()
                .expect("native child status should be readable")
                .is_some()
            {
                panic!("native child exited before becoming observable");
            }
            assert!(
                started_at.elapsed() < Duration::from_secs(5),
                "native child birth identity should become observable"
            );
            thread::sleep(Duration::from_millis(10));
        };

        let expected_executable = RuntimeProcessPlatformImpl::executable_path(child.id())
            .expect("native child executable path should be observable");
        let outcome = terminate_runtime_process(
            child.id(),
            Some(&expected_birth_identity),
            Some(&expected_executable),
        );
        assert!(
            matches!(
                outcome,
                RuntimeProcessTerminationOutcome::Terminated
                    | RuntimeProcessTerminationOutcome::NotRunning
                    | RuntimeProcessTerminationOutcome::OwnershipUnproven
            ),
            "native child termination outcome: {outcome:?}"
        );
        if outcome == RuntimeProcessTerminationOutcome::OwnershipUnproven {
            child.kill().expect("owned native child should terminate");
        }
        let status = child.wait().expect("native child should be reapable");
        assert!(!status.success(), "native child should be terminated");
    }

    #[test]
    fn fake_probe_never_signals_absent_unrelated_or_unproven_processes() {
        let expected_path = Path::new("/opt/prodex/bin/prodex");
        let terminate = |state| {
            FAKE_PROCESS_STATE.store(state, Ordering::SeqCst);
            FAKE_BIRTH_READS.store(0, Ordering::SeqCst);
            FAKE_TERMINATE_CALLS.store(0, Ordering::SeqCst);
            let outcome = terminate_runtime_process_with_platform::<FakeRuntimeProcess>(
                4242,
                Some("birth-expected"),
                Some(expected_path),
            );
            (outcome, FAKE_TERMINATE_CALLS.load(Ordering::SeqCst))
        };

        assert_eq!(
            terminate(FAKE_ABSENT),
            (RuntimeProcessTerminationOutcome::NotRunning, 0)
        );
        assert_eq!(
            terminate(FAKE_BIRTH_CHANGED),
            (RuntimeProcessTerminationOutcome::OwnershipChanged, 0)
        );
        assert_eq!(
            terminate(FAKE_PATH_CHANGED),
            (RuntimeProcessTerminationOutcome::OwnershipChanged, 0)
        );
        assert_eq!(
            terminate(FAKE_BIRTH_CHANGED_AFTER_PATH),
            (RuntimeProcessTerminationOutcome::OwnershipChanged, 0)
        );
        assert_eq!(
            terminate(FAKE_PERMISSION_DENIED),
            (RuntimeProcessTerminationOutcome::OwnershipUnproven, 0)
        );
        assert_eq!(
            terminate(FAKE_MATCH_TERMINATES),
            (RuntimeProcessTerminationOutcome::Terminated, 1)
        );
        assert_eq!(
            terminate(FAKE_FORCED_TERMINATES),
            (RuntimeProcessTerminationOutcome::Terminated, 2)
        );
        assert_eq!(
            terminate(FAKE_STILL_RUNNING),
            (RuntimeProcessTerminationOutcome::StillRunning, 1)
        );
        FAKE_PROCESS_STATE.store(FAKE_ABSENT, Ordering::SeqCst);
    }

    fn runtime_process_test_path(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "prodex-runtime-process-{name}-{}-{nanos}",
            std::process::id()
        ))
    }
}
