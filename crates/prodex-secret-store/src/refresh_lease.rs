use sha2::{Digest, Sha256};
use std::error::Error as StdError;
use std::fmt;
use std::fs;
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DEFAULT_LEASE_TTL: Duration = Duration::from_secs(30);
const DEFAULT_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_RESULT_TTL: Duration = Duration::from_secs(300);
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone)]
pub struct RefreshLeaseCoordinator {
    root: PathBuf,
    namespace: String,
    lease_ttl: Duration,
    wait_timeout: Duration,
    result_ttl: Duration,
    poll_interval: Duration,
}

impl RefreshLeaseCoordinator {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            namespace: "auth-refresh".to_string(),
            lease_ttl: DEFAULT_LEASE_TTL,
            wait_timeout: DEFAULT_WAIT_TIMEOUT,
            result_ttl: DEFAULT_RESULT_TTL,
            poll_interval: DEFAULT_POLL_INTERVAL,
        }
    }

    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = namespace.into();
        self
    }

    pub fn with_lease_ttl(mut self, ttl: Duration) -> Self {
        self.lease_ttl = ttl;
        self
    }

    pub fn with_wait_timeout(mut self, timeout: Duration) -> Self {
        self.wait_timeout = timeout;
        self
    }

    pub fn with_result_ttl(mut self, ttl: Duration) -> Self {
        self.result_ttl = ttl;
        self
    }

    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    pub fn paths_for_key(&self, sensitive_key: impl AsRef<[u8]>) -> RefreshLeasePaths {
        let digest = digest_key(&self.namespace, sensitive_key.as_ref());
        RefreshLeasePaths {
            digest: digest.clone(),
            lock_path: self.root.join(format!("{digest}.lock")),
            result_path: self.root.join(format!("{digest}.result.json")),
        }
    }

    pub fn acquire(
        &self,
        sensitive_key: impl AsRef<[u8]>,
    ) -> Result<RefreshLeaseDecision, RefreshLeaseError> {
        let paths = self.paths_for_key(sensitive_key);
        fs::create_dir_all(&self.root).map_err(|err| RefreshLeaseError::io(&self.root, err))?;
        remove_stale_result(&paths.result_path, self.result_ttl)?;

        if let Some(result_json) = read_fresh_result(&paths.result_path, self.result_ttl)? {
            return Ok(RefreshLeaseDecision::Follower { result_json });
        }

        let started = Instant::now();
        loop {
            cleanup_stale_lock(&paths.lock_path, self.lease_ttl)?;

            match create_lock(&paths.lock_path) {
                Ok(()) => {
                    let mut owner = RefreshLeaseOwner {
                        lock_path: paths.lock_path,
                        result_path: paths.result_path,
                        released: false,
                    };
                    if let Some(result_json) =
                        read_fresh_result(&owner.result_path, self.result_ttl)?
                    {
                        owner.release()?;
                        return Ok(RefreshLeaseDecision::Follower { result_json });
                    }
                    return Ok(RefreshLeaseDecision::Owner(owner));
                }
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
                Err(err) => return Err(RefreshLeaseError::io(&paths.lock_path, err)),
            }

            if let Some(result_json) = read_fresh_result(&paths.result_path, self.result_ttl)? {
                return Ok(RefreshLeaseDecision::Follower { result_json });
            }

            if started.elapsed() >= self.wait_timeout {
                return Ok(RefreshLeaseDecision::Bypass {
                    reason: RefreshLeaseBypassReason::WaitTimeout,
                });
            }

            thread::sleep(next_sleep(self.poll_interval, self.wait_timeout, started));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshLeasePaths {
    digest: String,
    lock_path: PathBuf,
    result_path: PathBuf,
}

impl RefreshLeasePaths {
    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }

    pub fn result_path(&self) -> &Path {
        &self.result_path
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshLeaseRole {
    Owner,
    Follower,
    Bypass,
}

#[derive(Debug)]
pub enum RefreshLeaseDecision {
    Owner(RefreshLeaseOwner),
    Follower { result_json: String },
    Bypass { reason: RefreshLeaseBypassReason },
}

impl RefreshLeaseDecision {
    pub fn role(&self) -> RefreshLeaseRole {
        match self {
            Self::Owner(_) => RefreshLeaseRole::Owner,
            Self::Follower { .. } => RefreshLeaseRole::Follower,
            Self::Bypass { .. } => RefreshLeaseRole::Bypass,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshLeaseBypassReason {
    WaitTimeout,
}

#[derive(Debug)]
pub struct RefreshLeaseOwner {
    lock_path: PathBuf,
    result_path: PathBuf,
    released: bool,
}

impl RefreshLeaseOwner {
    pub fn commit_result(mut self, result_json: impl AsRef<str>) -> Result<(), RefreshLeaseError> {
        write_result(&self.result_path, result_json.as_ref())?;
        self.release()?;
        Ok(())
    }

    pub fn release(&mut self) -> Result<(), RefreshLeaseError> {
        if self.released {
            return Ok(());
        }

        match fs::remove_file(&self.lock_path) {
            Ok(()) => {
                self.released = true;
                Ok(())
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                self.released = true;
                Ok(())
            }
            Err(err) => Err(RefreshLeaseError::io(&self.lock_path, err)),
        }
    }

    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }

    pub fn result_path(&self) -> &Path {
        &self.result_path
    }
}

impl Drop for RefreshLeaseOwner {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshLeaseError {
    Io { path: PathBuf, reason: String },
}

impl RefreshLeaseError {
    fn io(path: impl Into<PathBuf>, error: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            reason: error.to_string(),
        }
    }
}

impl fmt::Display for RefreshLeaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { .. } => write!(f, "refresh lease I/O error"),
        }
    }
}

impl StdError for RefreshLeaseError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshLeaseErrorStatus {
    ServiceUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshLeaseErrorResponsePlan {
    pub status: RefreshLeaseErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_refresh_lease_error_response(
    _error: &RefreshLeaseError,
) -> RefreshLeaseErrorResponsePlan {
    RefreshLeaseErrorResponsePlan {
        status: RefreshLeaseErrorStatus::ServiceUnavailable,
        code: "refresh_lease_unavailable",
        message: "refresh lease coordination is temporarily unavailable",
    }
}

fn digest_key(namespace: &str, sensitive_key: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(namespace.as_bytes());
    hasher.update([0]);
    hasher.update(sensitive_key);
    hex_lower(&hasher.finalize())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn create_lock(path: &Path) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options.open(path)?;
    let content = format!(
        "pid={}\ncreated_unix_ms={}\n",
        std::process::id(),
        unix_millis(SystemTime::now())
    );
    file.write_all(content.as_bytes())
}

fn write_result(path: &Path, result_json: &str) -> Result<(), RefreshLeaseError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| RefreshLeaseError::io(parent, err))?;
    }

    let temp_path = unique_temp_path(path);
    write_private_file(&temp_path, result_json.as_bytes())?;
    match fs::rename(&temp_path, path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
            fs::remove_file(path).map_err(|err| RefreshLeaseError::io(path, err))?;
            fs::rename(&temp_path, path).map_err(|err| RefreshLeaseError::io(path, err))
        }
        Err(err) => Err(RefreshLeaseError::io(path, err)),
    }
}

fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), RefreshLeaseError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options
        .open(path)
        .map_err(|err| RefreshLeaseError::io(path, err))?;
    file.write_all(bytes)
        .map_err(|err| RefreshLeaseError::io(path, err))
}

fn read_fresh_result(path: &Path, ttl: Duration) -> Result<Option<String>, RefreshLeaseError> {
    if is_path_stale(path, ttl)? {
        let _ = fs::remove_file(path);
        return Ok(None);
    }

    match fs::read_to_string(path) {
        Ok(value) => Ok(Some(value)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(RefreshLeaseError::io(path, err)),
    }
}

fn remove_stale_result(path: &Path, ttl: Duration) -> Result<(), RefreshLeaseError> {
    if is_path_stale(path, ttl)? {
        let _ = fs::remove_file(path);
    }
    Ok(())
}

fn cleanup_stale_lock(path: &Path, ttl: Duration) -> Result<(), RefreshLeaseError> {
    if is_path_stale(path, ttl)? {
        let _ = fs::remove_file(path);
    }
    Ok(())
}

fn is_path_stale(path: &Path, ttl: Duration) -> Result<bool, RefreshLeaseError> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(RefreshLeaseError::io(path, err)),
    };

    let modified = match metadata.modified() {
        Ok(modified) => modified,
        Err(_) => return Ok(false),
    };

    Ok(SystemTime::now()
        .duration_since(modified)
        .map(|age| age > ttl)
        .unwrap_or(false))
}

fn next_sleep(poll_interval: Duration, wait_timeout: Duration, started: Instant) -> Duration {
    let remaining = wait_timeout.saturating_sub(started.elapsed());
    if remaining.is_zero() {
        return Duration::ZERO;
    }
    poll_interval.min(remaining)
}

fn unique_temp_path(path: &Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    path.with_extension(format!("{pid}.{nanos:x}.tmp"))
}

fn unix_millis(time: SystemTime) -> u128 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
