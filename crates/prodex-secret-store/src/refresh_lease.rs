use crate::secure_file::{self, FileSecurity, SecureDirectory};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{OnceLock, mpsc};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use zeroize::{ZeroizeOnDrop, Zeroizing};

mod record;
use record::{create_lock, digest_key, lock_record_matches, read_fresh_result, write_result};

const DEFAULT_LEASE_TTL: Duration = Duration::from_secs(30);
const DEFAULT_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_RESULT_TTL: Duration = Duration::from_secs(300);
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(50);
const REFRESH_LEASE_RESULT_MAX_BYTES: u64 = 1024 * 1024;
const REFRESH_LEASE_RECORD_MAX_BYTES: u64 = REFRESH_LEASE_RESULT_MAX_BYTES + 512;
const REFRESH_LEASE_LOCK_MAX_BYTES: u64 = 4096;
const REFRESH_LEASE_LOCK_RECORD_VERSION: u32 = 1;
const REFRESH_LEASE_RESULT_RECORD_VERSION: u32 = 1;
const MIN_HEARTBEAT_INTERVAL: Duration = Duration::from_millis(1);
const MAX_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(60);
const LOCK_RECORD_MAGIC: &str = "prodex-refresh-lease";
const RESULT_RECORD_MAGIC: &str = "prodex-refresh-result";
static FENCE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static HEARTBEAT_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static HEARTBEAT_COMMANDS: OnceLock<Result<mpsc::Sender<HeartbeatCommand>, io::ErrorKind>> =
    OnceLock::new();

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
                Ok((lock_file, fence_token)) => {
                    let mut owner = RefreshLeaseOwner::new(
                        paths.lock_path,
                        lock_file,
                        paths.result_path,
                        fence_token,
                        heartbeat_interval(self.lease_ttl),
                    )?;
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
    fence_token: String,
    heartbeat: Option<RefreshLeaseHeartbeat>,
    released: bool,
}

impl RefreshLeaseOwner {
    fn new(
        lock_path: PathBuf,
        lock_file: File,
        result_path: PathBuf,
        fence_token: String,
        heartbeat_interval: Duration,
    ) -> Result<Self, RefreshLeaseError> {
        let heartbeat = match RefreshLeaseHeartbeat::start(&lock_file, heartbeat_interval) {
            Ok(heartbeat) => heartbeat,
            Err(error) => {
                let _ = secure_file::delete_private_verified(&lock_path, &lock_file);
                return Err(RefreshLeaseError::io(&lock_path, error));
            }
        };
        Ok(Self {
            lock_path,
            lock_file,
            result_path,
            fence_token,
            heartbeat: Some(heartbeat),
            released: false,
        })
    }

    pub fn commit_result(mut self, result_json: impl AsRef<str>) -> Result<(), RefreshLeaseError> {
        self.stop_heartbeat();
        self.verify_ownership()?;
        write_result(&self.result_path, &self.fence_token, result_json.as_ref())?;
        self.release()?;
        Ok(())
    }

    pub fn release(&mut self) -> Result<(), RefreshLeaseError> {
        if self.released {
            return Ok(());
        }

        self.stop_heartbeat();
        match secure_file::delete_private_verified(&self.lock_path, &self.lock_file) {
            Ok(()) => {
                self.released = true;
                Ok(())
            }
            Err(err) => Err(RefreshLeaseError::ownership(&self.lock_path, err)),
        }
    }

    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }

    pub fn result_path(&self) -> &Path {
        &self.result_path
    }

    fn verify_ownership(&self) -> Result<(), RefreshLeaseError> {
        secure_file::verify_private_file(&self.lock_path, &self.lock_file)
            .map_err(|error| RefreshLeaseError::ownership(&self.lock_path, error))?;
        if lock_record_matches(&self.lock_path, &self.fence_token)? {
            Ok(())
        } else {
            Err(RefreshLeaseError::OwnershipLost)
        }
    }

    fn stop_heartbeat(&mut self) {
        if let Some(heartbeat) = self.heartbeat.take() {
            heartbeat.stop();
        }
    }
}

impl fmt::Debug for RefreshLeaseOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RefreshLeaseOwner")
            .field("lock_path", &"<redacted>")
            .field("result_path", &"<redacted>")
            .field("fence_token", &"<redacted>")
            .field("released", &self.released)
            .finish()
    }
}

impl Drop for RefreshLeaseOwner {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

struct RefreshLeaseHeartbeat {
    id: u64,
    commands: mpsc::Sender<HeartbeatCommand>,
}

impl RefreshLeaseHeartbeat {
    fn start(lock_file: &File, interval: Duration) -> io::Result<Self> {
        let lock_file = lock_file.try_clone()?;
        let id = HEARTBEAT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let commands = heartbeat_commands()?.clone();
        commands
            .send(HeartbeatCommand::Add {
                id,
                lock_file,
                interval,
            })
            .map_err(|_| io::Error::other("refresh lease heartbeat worker stopped"))?;
        Ok(Self { id, commands })
    }

    fn stop(self) {
        let (stopped, wait) = mpsc::channel();
        if self
            .commands
            .send(HeartbeatCommand::Remove {
                id: self.id,
                stopped,
            })
            .is_ok()
        {
            let _ = wait.recv();
        }
    }
}

enum HeartbeatCommand {
    Add {
        id: u64,
        lock_file: File,
        interval: Duration,
    },
    Remove {
        id: u64,
        stopped: mpsc::Sender<()>,
    },
}

struct ActiveHeartbeat {
    lock_file: File,
    interval: Duration,
    next: Instant,
}

fn heartbeat_commands() -> io::Result<&'static mpsc::Sender<HeartbeatCommand>> {
    HEARTBEAT_COMMANDS
        .get_or_init(|| {
            let (commands, received) = mpsc::channel();
            thread::Builder::new()
                .name("prodex-refresh-lease-heartbeat".to_string())
                .spawn(move || heartbeat_worker(received))
                .map(|_| commands)
                .map_err(|error| error.kind())
        })
        .as_ref()
        .map_err(|kind| io::Error::new(*kind, "failed to start refresh lease heartbeat worker"))
}

fn heartbeat_worker(commands: mpsc::Receiver<HeartbeatCommand>) {
    let mut active = HashMap::<u64, ActiveHeartbeat>::new();
    loop {
        let now = Instant::now();
        let timeout = active
            .values()
            .map(|heartbeat| heartbeat.next.saturating_duration_since(now))
            .min()
            .unwrap_or(MAX_HEARTBEAT_INTERVAL);
        match commands.recv_timeout(timeout) {
            Ok(HeartbeatCommand::Add {
                id,
                lock_file,
                interval,
            }) => {
                active.insert(
                    id,
                    ActiveHeartbeat {
                        lock_file,
                        interval,
                        next: Instant::now() + interval,
                    },
                );
            }
            Ok(HeartbeatCommand::Remove { id, stopped }) => {
                active.remove(&id);
                let _ = stopped.send(());
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        let now = Instant::now();
        active.retain(|_, heartbeat| {
            if heartbeat.next > now {
                return true;
            }
            let times = fs::FileTimes::new().set_modified(SystemTime::now());
            if heartbeat.lock_file.set_times(times).is_err() {
                return false;
            }
            heartbeat.next = now + heartbeat.interval;
            true
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshLeaseIoKind {
    Generic,
    ResultTooLarge,
}

#[derive(Clone, PartialEq, Eq)]
pub enum RefreshLeaseError {
    Io {
        kind: RefreshLeaseIoKind,
        reason: Zeroizing<String>,
    },
    OwnershipLost,
}

impl ZeroizeOnDrop for RefreshLeaseError {}

impl RefreshLeaseError {
    pub fn io(_path: impl Into<PathBuf>, error: io::Error) -> Self {
        let reason = Zeroizing::new(error.to_string());
        let kind = if error.kind() == io::ErrorKind::InvalidData
            && reason.contains("exceeds safe size limit")
        {
            RefreshLeaseIoKind::ResultTooLarge
        } else {
            RefreshLeaseIoKind::Generic
        };
        Self::Io { kind, reason }
    }

    fn ownership(path: impl Into<PathBuf>, error: io::Error) -> Self {
        if matches!(
            error.kind(),
            io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
        ) {
            Self::OwnershipLost
        } else {
            Self::io(path, error)
        }
    }

    pub fn is_ownership_lost(&self) -> bool {
        matches!(self, Self::OwnershipLost)
    }
}

impl fmt::Debug for RefreshLeaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { .. } => f.debug_struct("Io").field("reason", &"<redacted>").finish(),
            Self::OwnershipLost => f.write_str("OwnershipLost"),
        }
    }
}

impl fmt::Display for RefreshLeaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                kind: RefreshLeaseIoKind::ResultTooLarge,
                ..
            } => {
                write!(f, "refresh lease result exceeds safe size limit")
            }
            Self::Io { .. } => write!(f, "refresh lease I/O error"),
            Self::OwnershipLost => write!(f, "refresh lease ownership was lost"),
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

fn remove_stale_result(path: &Path, ttl: Duration) -> Result<(), RefreshLeaseError> {
    if is_path_stale(path, ttl)? {
        remove_entry(path)?;
    }
    Ok(())
}

fn cleanup_stale_lock(path: &Path, ttl: Duration) -> Result<(), RefreshLeaseError> {
    let opened = match secure_file::open_file(path, FileSecurity::Private) {
        Ok(Some(opened)) => opened,
        Ok(None) => return Ok(()),
        Err(error) if unsafe_entry_error(&error) => {
            remove_entry(path)?;
            return Ok(());
        }
        Err(error) => return Err(RefreshLeaseError::io(path, error)),
    };
    if !metadata_is_stale(opened.metadata(), ttl) {
        return Ok(());
    }
    let file = opened.into_file();
    match try_lock_refresh_lease(&file) {
        Ok(true) => match secure_file::delete_private_verified(path, &file) {
            Ok(()) => Ok(()),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
                ) =>
            {
                Ok(())
            }
            Err(error) => Err(RefreshLeaseError::io(path, error)),
        },
        Ok(false) => Ok(()),
        Err(error) => Err(RefreshLeaseError::io(path, error)),
    }
}

fn try_lock_refresh_lease(file: &File) -> io::Result<bool> {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle as _;
        use windows_sys::Win32::Foundation::ERROR_LOCK_VIOLATION;
        use windows_sys::Win32::Storage::FileSystem::LockFile;

        // Keep the record readable while reserving one byte beyond any bounded read.
        let offset = REFRESH_LEASE_LOCK_MAX_BYTES + 1;
        // SAFETY: `file` owns a live handle and the locked one-byte range is valid
        // even beyond EOF. Windows releases the range when the handle closes.
        let locked = unsafe {
            LockFile(
                file.as_raw_handle().cast(),
                offset as u32,
                (offset >> 32) as u32,
                1,
                0,
            )
        };
        if locked != 0 {
            return Ok(true);
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_LOCK_VIOLATION as i32) {
            Ok(false)
        } else {
            Err(error)
        }
    }

    #[cfg(not(windows))]
    match file.try_lock() {
        Ok(()) => Ok(true),
        Err(fs::TryLockError::WouldBlock) => Ok(false),
        Err(error) => Err(io::Error::other(error)),
    }
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

fn heartbeat_interval(lease_ttl: Duration) -> Duration {
    (lease_ttl / 3).clamp(MIN_HEARTBEAT_INTERVAL, MAX_HEARTBEAT_INTERVAL)
}

fn unix_millis(time: SystemTime) -> u128 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
