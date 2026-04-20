use super::*;
use sha2::{Digest, Sha256};

pub(crate) fn runtime_broker_key(upstream_base_url: &str, include_code_review: bool) -> String {
    let mut hasher = DefaultHasher::new();
    upstream_base_url.hash(&mut hasher);
    include_code_review.hash(&mut hasher);
    RUNTIME_PROXY_OPENAI_MOUNT_PATH.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
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

#[derive(Debug, Clone)]
struct RuntimeProcessVersionResolution {
    executable_path: Option<PathBuf>,
    version: Option<String>,
    executable_sha256: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RuntimeProdexBinaryIdentity {
    pub(crate) prodex_version: Option<String>,
    pub(crate) executable_path: Option<PathBuf>,
    pub(crate) executable_sha256: Option<String>,
}

impl RuntimeProdexBinaryIdentity {
    pub(crate) fn is_present(&self) -> bool {
        self.prodex_version.is_some()
            || self.executable_path.is_some()
            || self.executable_sha256.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeBrokerVersionGuardOutcome {
    Compatible,
    Replaced,
    DeferredActiveRequests,
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
    fn pid_alive(_pid: u32) -> bool {
        false
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

pub(crate) fn runtime_process_pid_alive(pid: u32) -> bool {
    if RuntimeProcessPlatformImpl::pid_alive(pid) {
        return true;
    }
    runtime_process_row(pid).is_some()
}

pub(crate) fn runtime_random_token(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = STATE_SAVE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{}-{nanos:x}-{sequence:x}", std::process::id())
}

pub(crate) fn runtime_broker_startup_grace_seconds() -> i64 {
    let ready_timeout_seconds = runtime_broker_ready_timeout_ms().div_ceil(1_000) as i64;
    ready_timeout_seconds
        .saturating_add(1)
        .max(RUNTIME_BROKER_IDLE_GRACE_SECONDS)
}

pub(crate) fn load_runtime_broker_registry(
    paths: &AppPaths,
    broker_key: &str,
) -> Result<Option<RuntimeBrokerRegistry>> {
    let path = runtime_broker_registry_file_path(paths, broker_key);
    let backup_path = runtime_broker_registry_last_good_file_path(paths, broker_key);
    if !path.exists() && !backup_path.exists() {
        return Ok(None);
    }
    match load_json_file_with_backup::<RuntimeBrokerRegistry>(&path, &backup_path) {
        Ok(loaded) => Ok(Some(loaded.value)),
        Err(_err) if !path.exists() && !backup_path.exists() => Ok(None),
        Err(err) => Err(err),
    }
}

pub(crate) fn save_runtime_broker_registry(
    paths: &AppPaths,
    broker_key: &str,
    registry: &RuntimeBrokerRegistry,
) -> Result<()> {
    let path = runtime_broker_registry_file_path(paths, broker_key);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(registry)
        .context("failed to serialize runtime broker registry")?;
    write_json_file_with_backup(
        &path,
        &runtime_broker_registry_last_good_file_path(paths, broker_key),
        &json,
        |content| {
            let _: RuntimeBrokerRegistry = serde_json::from_str(content)
                .context("failed to validate runtime broker registry")?;
            Ok(())
        },
    )
}

pub(crate) fn remove_runtime_broker_registry_if_token_matches(
    paths: &AppPaths,
    broker_key: &str,
    instance_token: &str,
) {
    let Ok(Some(existing)) = load_runtime_broker_registry(paths, broker_key) else {
        return;
    };
    if existing.instance_token != instance_token {
        return;
    }
    for path in [
        runtime_broker_registry_file_path(paths, broker_key),
        runtime_broker_registry_last_good_file_path(paths, broker_key),
    ] {
        let _ = fs::remove_file(path);
    }
}

pub(crate) fn legacy_runtime_proxy_openai_mount_path(version: &str) -> String {
    format!("{LEGACY_RUNTIME_PROXY_OPENAI_MOUNT_PATH_PREFIX}{version}")
}

pub(crate) fn parse_prodex_version_output(output: &str) -> Option<String> {
    let mut parts = output.split_whitespace();
    let binary_name = parts.next()?;
    let version = parts.next()?;
    if binary_name == "prodex" && !version.is_empty() {
        return Some(version.to_string());
    }
    None
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

pub(crate) fn runtime_process_prodex_binary_identity(pid: u32) -> RuntimeProdexBinaryIdentity {
    let resolution = runtime_process_version_resolution(pid);
    RuntimeProdexBinaryIdentity {
        prodex_version: resolution.version,
        executable_path: resolution.executable_path,
        executable_sha256: resolution.executable_sha256,
    }
}

pub(crate) fn runtime_registry_prodex_binary_identity(
    registry: &RuntimeBrokerRegistry,
) -> RuntimeProdexBinaryIdentity {
    RuntimeProdexBinaryIdentity {
        prodex_version: registry.prodex_version.clone(),
        executable_path: registry.executable_path.clone().map(PathBuf::from),
        executable_sha256: registry.executable_sha256.clone(),
    }
}

pub(crate) fn runtime_health_prodex_binary_identity(
    health: &RuntimeBrokerHealth,
) -> RuntimeProdexBinaryIdentity {
    RuntimeProdexBinaryIdentity {
        prodex_version: health.prodex_version.clone(),
        executable_path: health.executable_path.clone().map(PathBuf::from),
        executable_sha256: health.executable_sha256.clone(),
    }
}

pub(crate) fn runtime_prodex_binary_identity_matches(
    current: &RuntimeProdexBinaryIdentity,
    other: &RuntimeProdexBinaryIdentity,
) -> bool {
    if let (Some(current_sha256), Some(other_sha256)) = (
        current.executable_sha256.as_deref(),
        other.executable_sha256.as_deref(),
    ) {
        return current_sha256 == other_sha256;
    }
    if let (Some(current_version), Some(other_version)) = (
        current.prodex_version.as_deref(),
        other.prodex_version.as_deref(),
    ) {
        return current_version == other_version;
    }
    false
}

fn runtime_broker_replacement_reason(
    current: &RuntimeProdexBinaryIdentity,
    observed: &RuntimeProdexBinaryIdentity,
) -> &'static str {
    match (
        current.executable_sha256.as_deref(),
        observed.executable_sha256.as_deref(),
    ) {
        (Some(current_sha256), Some(observed_sha256)) if current_sha256 != observed_sha256 => {
            "sha256_mismatch"
        }
        _ => match (
            current.prodex_version.as_deref(),
            observed.prodex_version.as_deref(),
        ) {
            (Some(current_version), Some(observed_version))
                if current_version != observed_version =>
            {
                "version_mismatch"
            }
            _ if observed.is_present() => "identity_mismatch",
            _ => "identity_unresolved",
        },
    }
}

fn runtime_broker_observed_binary_identity(
    registry: &RuntimeBrokerRegistry,
    health: Option<&RuntimeBrokerHealth>,
) -> RuntimeProdexBinaryIdentity {
    health
        .filter(|health| health.instance_token == registry.instance_token)
        .map(runtime_health_prodex_binary_identity)
        .filter(RuntimeProdexBinaryIdentity::is_present)
        .or_else(|| {
            let identity = runtime_registry_prodex_binary_identity(registry);
            identity.is_present().then_some(identity)
        })
        .or_else(|| {
            let identity = runtime_process_prodex_binary_identity(registry.pid);
            identity.is_present().then_some(identity)
        })
        .unwrap_or_default()
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

pub(crate) fn replace_runtime_broker_if_version_mismatch_with_health(
    paths: &AppPaths,
    broker_key: &str,
    registry: &RuntimeBrokerRegistry,
    health: Option<&RuntimeBrokerHealth>,
) -> RuntimeBrokerVersionGuardOutcome {
    if !runtime_process_pid_alive(registry.pid) {
        return RuntimeBrokerVersionGuardOutcome::Compatible;
    }

    let current_identity = runtime_current_prodex_binary_identity();
    let observed_identity = runtime_broker_observed_binary_identity(registry, health);
    if observed_identity.is_present()
        && runtime_prodex_binary_identity_matches(&current_identity, &observed_identity)
    {
        return RuntimeBrokerVersionGuardOutcome::Compatible;
    }

    if health.is_some_and(|health| {
        health.instance_token == registry.instance_token && health.active_requests > 0
    }) {
        return RuntimeBrokerVersionGuardOutcome::DeferredActiveRequests;
    }

    let replacement_reason =
        runtime_broker_replacement_reason(&current_identity, &observed_identity);
    audit_log_event_best_effort(
        "runtime_broker",
        "replace_stale_broker",
        "success",
        serde_json::json!({
            "reason": replacement_reason,
            "broker_key": broker_key,
            "pid": registry.pid,
            "listen_addr": registry.listen_addr,
            "started_at": registry.started_at,
            "instance_token": registry.instance_token,
            "upstream_base_url": registry.upstream_base_url,
            "include_code_review": registry.include_code_review,
            "current_prodex_version": current_identity.prodex_version,
            "current_executable_path": current_identity
                .executable_path
                .map(|path| path.display().to_string()),
            "current_executable_sha256": current_identity.executable_sha256,
            "detected_prodex_version": observed_identity.prodex_version,
            "detected_executable_sha256": observed_identity.executable_sha256,
            "executable_path": observed_identity
                .executable_path
                .map(|path| path.display().to_string()),
            "active_requests": health.map(|health| health.active_requests),
            "platform": env::consts::OS,
        }),
    );
    terminate_runtime_process(registry.pid);
    remove_runtime_broker_registry_if_token_matches(paths, broker_key, &registry.instance_token);
    RuntimeBrokerVersionGuardOutcome::Replaced
}

pub(crate) fn runtime_broker_openai_mount_path(registry: &RuntimeBrokerRegistry) -> Result<String> {
    if let Some(openai_mount_path) = registry.openai_mount_path.as_deref() {
        return Ok(openai_mount_path.to_string());
    }

    let version = runtime_process_prodex_version(registry.pid).with_context(|| {
        format!(
            "failed to resolve prodex version for runtime broker pid {}",
            registry.pid
        )
    })?;
    Ok(legacy_runtime_proxy_openai_mount_path(&version))
}

pub(crate) fn create_runtime_broker_lease(
    paths: &AppPaths,
    broker_key: &str,
) -> Result<RuntimeBrokerLease> {
    let lease_dir = runtime_broker_lease_dir(paths, broker_key);
    create_runtime_broker_lease_in_dir_for_pid(&lease_dir, std::process::id())
}

pub(crate) fn create_runtime_broker_lease_in_dir_for_pid(
    lease_dir: &Path,
    pid: u32,
) -> Result<RuntimeBrokerLease> {
    fs::create_dir_all(lease_dir)
        .with_context(|| format!("failed to create {}", lease_dir.display()))?;
    let path = lease_dir.join(format!("{}-{}.lease", pid, runtime_random_token("lease")));
    fs::write(&path, format!("pid={pid}\n"))
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(RuntimeBrokerLease { path })
}

pub(crate) fn cleanup_runtime_broker_stale_leases(paths: &AppPaths, broker_key: &str) -> usize {
    let lease_dir = runtime_broker_lease_dir(paths, broker_key);
    let Ok(entries) = fs::read_dir(&lease_dir) else {
        return 0;
    };
    let mut live = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let pid = file_name
            .split('-')
            .next()
            .and_then(|value| value.parse::<u32>().ok());
        if pid.is_some_and(runtime_process_pid_alive) {
            live += 1;
        } else {
            let _ = fs::remove_file(path);
        }
    }
    live
}

pub(crate) fn runtime_proxy_endpoint_from_registry(
    paths: &AppPaths,
    broker_key: &str,
    registry: &RuntimeBrokerRegistry,
) -> Result<RuntimeProxyEndpoint> {
    let lease = create_runtime_broker_lease(paths, broker_key)?;
    let lease_dir = runtime_broker_lease_dir(paths, broker_key);
    let listen_addr = registry.listen_addr.parse().with_context(|| {
        format!(
            "invalid runtime broker listen address {}",
            registry.listen_addr
        )
    })?;
    Ok(RuntimeProxyEndpoint {
        listen_addr,
        openai_mount_path: runtime_broker_openai_mount_path(registry)?,
        lease_dir,
        _lease: Some(lease),
    })
}
