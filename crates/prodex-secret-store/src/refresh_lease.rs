use crate::secure_file::{self, FileSecurity, SecureDirectory};
use sha2::{Digest, Sha256};
use std::error::Error as StdError;
use std::fmt;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

const DEFAULT_LEASE_TTL: Duration = Duration::from_secs(30);
const DEFAULT_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_RESULT_TTL: Duration = Duration::from_secs(300);
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(50);
const REFRESH_LEASE_RESULT_MAX_BYTES: u64 = 1024 * 1024;

#[derive(Clone)]
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
        SecureDirectory::open(&self.root, true)
            .map_err(|error| RefreshLeaseError::io(&self.root, error))?;
        remove_stale_result(&paths.result_path, self.result_ttl)?;

        if let Some(result_json) = read_fresh_result(&paths.result_path, self.result_ttl)? {
            return Ok(RefreshLeaseDecision::Follower { result_json });
        }

        let started = Instant::now();
        loop {
            cleanup_stale_lock(&paths.lock_path, self.lease_ttl)?;

            match create_lock(&paths.lock_path) {
                Ok(lock_file) => {
                    let mut owner = RefreshLeaseOwner {
                        lock_path: paths.lock_path,
                        lock_file,
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

impl fmt::Debug for RefreshLeaseCoordinator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RefreshLeaseCoordinator")
            .field("root", &"<redacted>")
            .field("namespace", &"<redacted>")
            .field("lease_ttl", &"<redacted>")
            .field("wait_timeout", &"<redacted>")
            .field("result_ttl", &"<redacted>")
            .field("poll_interval", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
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

impl fmt::Debug for RefreshLeasePaths {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RefreshLeasePaths")
            .field("digest", &"<redacted>")
            .field("lock_path", &"<redacted>")
            .field("result_path", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshLeaseRole {
    Owner,
    Follower,
    Bypass,
}

pub enum RefreshLeaseDecision {
    Owner(RefreshLeaseOwner),
    Follower { result_json: Zeroizing<String> },
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

impl fmt::Debug for RefreshLeaseDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Owner(owner) => f.debug_tuple("Owner").field(owner).finish(),
            Self::Follower { .. } => f
                .debug_struct("Follower")
                .field("result_json", &"<redacted>")
                .finish(),
            Self::Bypass { reason } => f.debug_struct("Bypass").field("reason", reason).finish(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshLeaseBypassReason {
    WaitTimeout,
}

pub struct RefreshLeaseOwner {
    lock_path: PathBuf,
    lock_file: File,
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

        match secure_file::delete_private_verified(&self.lock_path, &self.lock_file) {
            Ok(()) => {
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

impl fmt::Debug for RefreshLeaseOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RefreshLeaseOwner")
            .field("lock_path", &"<redacted>")
            .field("result_path", &"<redacted>")
            .field("released", &self.released)
            .finish()
    }
}

impl Drop for RefreshLeaseOwner {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

#[derive(Clone, PartialEq, Eq)]
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

impl fmt::Debug for RefreshLeaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { .. } => f
                .debug_struct("Io")
                .field("path", &"<redacted>")
                .field("reason", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for RefreshLeaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { reason, .. } if reason.contains("exceeds safe size limit") => {
                write!(f, "refresh lease result exceeds safe size limit")
            }
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

fn create_lock(path: &Path) -> io::Result<File> {
    let content = format!(
        "pid={}\ncreated_unix_ms={}\n",
        std::process::id(),
        unix_millis(SystemTime::now())
    );
    secure_file::create_private(path, content.as_bytes())
}

fn write_result(path: &Path, result_json: &str) -> Result<(), RefreshLeaseError> {
    if result_json.len() as u64 > REFRESH_LEASE_RESULT_MAX_BYTES {
        return Err(refresh_result_size_error(path));
    }

    secure_file::write_private_atomic(path, result_json.as_bytes())
        .map_err(|error| RefreshLeaseError::io(path, error))
}

fn read_fresh_result(
    path: &Path,
    ttl: Duration,
) -> Result<Option<Zeroizing<String>>, RefreshLeaseError> {
    let opened = match secure_file::open_file(path, FileSecurity::Private) {
        Ok(Some(opened)) => opened,
        Ok(None) => return Ok(None),
        Err(error) if unsafe_entry_error(&error) => {
            remove_entry(path)?;
            return Ok(None);
        }
        Err(error) => return Err(RefreshLeaseError::io(path, error)),
    };
    if metadata_is_stale(opened.metadata(), ttl)
        || opened.metadata().len() > REFRESH_LEASE_RESULT_MAX_BYTES
    {
        remove_entry(path)?;
        return Ok(None);
    }
    let mut bytes = Zeroizing::new(
        opened
            .read_bounded(REFRESH_LEASE_RESULT_MAX_BYTES)
            .map_err(|error| RefreshLeaseError::io(path, error))?,
    );
    match String::from_utf8(std::mem::take(&mut *bytes)) {
        Ok(value) => Ok(Some(Zeroizing::new(value))),
        Err(error) => {
            drop(Zeroizing::new(error.into_bytes()));
            remove_entry(path)?;
            Ok(None)
        }
    }
}

fn remove_stale_result(path: &Path, ttl: Duration) -> Result<(), RefreshLeaseError> {
    if is_path_stale(path, ttl)? {
        remove_entry(path)?;
    }
    Ok(())
}

fn cleanup_stale_lock(path: &Path, ttl: Duration) -> Result<(), RefreshLeaseError> {
    if is_path_stale(path, ttl)? {
        remove_entry(path)?;
    }
    Ok(())
}

fn is_path_stale(path: &Path, ttl: Duration) -> Result<bool, RefreshLeaseError> {
    let opened = match secure_file::open_file(path, FileSecurity::Private) {
        Ok(Some(opened)) => opened,
        Ok(None) => return Ok(false),
        Err(error) if unsafe_entry_error(&error) => return Ok(true),
        Err(error) => return Err(RefreshLeaseError::io(path, error)),
    };
    Ok(metadata_is_stale(opened.metadata(), ttl))
}

fn metadata_is_stale(metadata: &fs::Metadata, ttl: Duration) -> bool {
    let modified = match metadata.modified() {
        Ok(modified) => modified,
        Err(_) => return false,
    };

    SystemTime::now()
        .duration_since(modified)
        .map(|age| age > ttl)
        .unwrap_or(false)
}

fn unsafe_entry_error(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::InvalidData
            | io::ErrorKind::InvalidInput
            | io::ErrorKind::NotADirectory
            | io::ErrorKind::PermissionDenied
    )
}

fn remove_entry(path: &Path) -> Result<(), RefreshLeaseError> {
    secure_file::remove_untrusted_entry(path).map_err(|error| RefreshLeaseError::io(path, error))
}

fn refresh_result_size_error(path: &Path) -> RefreshLeaseError {
    RefreshLeaseError::io(
        path,
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "refresh lease result exceeds safe size limit ({REFRESH_LEASE_RESULT_MAX_BYTES} bytes)"
            ),
        ),
    )
}

fn next_sleep(poll_interval: Duration, wait_timeout: Duration, started: Instant) -> Duration {
    let remaining = wait_timeout.saturating_sub(started.elapsed());
    if remaining.is_zero() {
        return Duration::ZERO;
    }
    poll_interval.min(remaining)
}

fn unix_millis(time: SystemTime) -> u128 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
