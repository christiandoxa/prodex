use crate::secure_file::{self, FileSecurity, SecureDirectory};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
#[cfg(test)]
use std::collections::HashSet;
use std::error::Error as StdError;
use std::fmt;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, mpsc};
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
const HEARTBEAT_COMMAND_CAPACITY: usize = 64;
const MAX_ACTIVE_HEARTBEATS: usize = HEARTBEAT_COMMAND_CAPACITY;
const HEARTBEAT_WORKER_COUNT: usize = 2;
const HEARTBEAT_WORK_QUEUE_CAPACITY: usize = 1;
const HEARTBEAT_SCHEDULER_TICK: Duration = Duration::from_millis(10);
const HEARTBEAT_COMMAND_DEADLINE: Duration = Duration::from_millis(100);
const HEARTBEAT_WRITE_DEADLINE: Duration = Duration::from_millis(100);
const HEARTBEAT_SHUTDOWN_DEADLINE: Duration = Duration::from_millis(250);
const LOCK_RECORD_MAGIC: &str = "prodex-refresh-lease";
const RESULT_RECORD_MAGIC: &str = "prodex-refresh-result";
static FENCE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static HEARTBEAT_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static HEARTBEAT_COMMANDS: OnceLock<Result<mpsc::SyncSender<HeartbeatCommand>, io::ErrorKind>> =
    OnceLock::new();
#[cfg(test)]
static HEARTBEAT_TEST_WRITE_CALLS: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
static HEARTBEAT_TEST_ACTIVE_WRITES: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
static HEARTBEAT_TEST_MAX_ACTIVE_WRITES: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
static HEARTBEAT_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
#[cfg(test)]
static HEARTBEAT_TEST_STALL_IDS: OnceLock<Mutex<HashSet<u64>>> = OnceLock::new();

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
                Err(err) if refresh_lease_contention_error(&err) => {}
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
        self.stop_heartbeat()?;
        self.verify_ownership()?;
        write_result(&self.result_path, &self.fence_token, result_json.as_ref())?;
        self.release()?;
        Ok(())
    }

    pub fn release(&mut self) -> Result<(), RefreshLeaseError> {
        if self.released {
            return Ok(());
        }

        let heartbeat_error = self.stop_heartbeat().err();
        match secure_file::delete_private_verified(&self.lock_path, &self.lock_file) {
            Ok(()) => {
                self.released = true;
                heartbeat_error.map_or(Ok(()), Err)
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

    fn stop_heartbeat(&mut self) -> Result<(), RefreshLeaseError> {
        if let Some(heartbeat) = self.heartbeat.take() {
            heartbeat
                .stop()
                .map_err(|error| RefreshLeaseError::io(&self.lock_path, error))
        } else {
            Ok(())
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeartbeatFailure {
    SchedulerUnavailable,
    SchedulerDeadline,
    CommandQueueFull,
    CommandDisconnected,
    CapacityExhausted,
    WorkerQueueFull,
    WorkerUnavailable,
    WriteTimedOut,
    WriteFailed,
    ShutdownDeadline,
    SchedulerStopped,
}

impl HeartbeatFailure {
    fn error(self) -> io::Error {
        let (kind, message) = match self {
            Self::SchedulerUnavailable => (
                io::ErrorKind::Other,
                "refresh lease heartbeat scheduler unavailable",
            ),
            Self::SchedulerDeadline => (
                io::ErrorKind::TimedOut,
                "refresh lease heartbeat scheduler deadline exceeded",
            ),
            Self::CommandQueueFull => (
                io::ErrorKind::WouldBlock,
                "refresh lease heartbeat command queue is full",
            ),
            Self::CommandDisconnected => (
                io::ErrorKind::BrokenPipe,
                "refresh lease heartbeat scheduler stopped",
            ),
            Self::CapacityExhausted => (
                io::ErrorKind::WouldBlock,
                "refresh lease heartbeat capacity is exhausted",
            ),
            Self::WorkerQueueFull => (
                io::ErrorKind::WouldBlock,
                "refresh lease heartbeat worker queue is full",
            ),
            Self::WorkerUnavailable => (
                io::ErrorKind::BrokenPipe,
                "refresh lease heartbeat worker unavailable",
            ),
            Self::WriteTimedOut => (
                io::ErrorKind::TimedOut,
                "refresh lease heartbeat write deadline exceeded",
            ),
            Self::WriteFailed => (io::ErrorKind::Other, "refresh lease heartbeat write failed"),
            Self::ShutdownDeadline => (
                io::ErrorKind::TimedOut,
                "refresh lease heartbeat shutdown deadline exceeded",
            ),
            Self::SchedulerStopped => (
                io::ErrorKind::BrokenPipe,
                "refresh lease heartbeat scheduler stopped",
            ),
        };
        io::Error::new(kind, message)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeartbeatState {
    Pending,
    Running,
    StopRequested,
    Stopped,
    Cancelled,
    Failed(HeartbeatFailure),
}

struct HeartbeatControl {
    state: Mutex<HeartbeatState>,
    changed: Condvar,
}

impl HeartbeatControl {
    fn new() -> Self {
        Self {
            state: Mutex::new(HeartbeatState::Pending),
            changed: Condvar::new(),
        }
    }

    fn state(&self) -> HeartbeatState {
        *self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn mark_running(&self) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *state != HeartbeatState::Pending {
            return false;
        }
        *state = HeartbeatState::Running;
        self.changed.notify_all();
        true
    }

    fn request_stop(&self) -> HeartbeatState {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *state == HeartbeatState::Pending {
            *state = HeartbeatState::Cancelled;
            self.changed.notify_all();
        } else if *state == HeartbeatState::Running {
            *state = HeartbeatState::StopRequested;
            self.changed.notify_all();
        }
        *state
    }

    fn mark_stopped(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if matches!(
            *state,
            HeartbeatState::Running | HeartbeatState::StopRequested
        ) {
            *state = HeartbeatState::Stopped;
            self.changed.notify_all();
        }
    }

    fn fail(&self, failure: HeartbeatFailure) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if matches!(
            *state,
            HeartbeatState::Pending | HeartbeatState::Running | HeartbeatState::StopRequested
        ) {
            *state = HeartbeatState::Failed(failure);
            self.changed.notify_all();
        }
    }

    fn wait_for_start(&self) -> io::Result<()> {
        let deadline = Instant::now() + HEARTBEAT_COMMAND_DEADLINE;
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        loop {
            match *state {
                HeartbeatState::Running => return Ok(()),
                HeartbeatState::Failed(failure) => return Err(failure.error()),
                HeartbeatState::Cancelled
                | HeartbeatState::Stopped
                | HeartbeatState::StopRequested => {
                    return Err(HeartbeatFailure::SchedulerDeadline.error());
                }
                HeartbeatState::Pending => {}
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                *state = HeartbeatState::Cancelled;
                self.changed.notify_all();
                return Err(HeartbeatFailure::SchedulerDeadline.error());
            }
            let (next_state, _) = self
                .changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state = next_state;
        }
    }

    fn wait_for_stop(&self) -> io::Result<()> {
        let deadline = Instant::now() + HEARTBEAT_SHUTDOWN_DEADLINE;
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        loop {
            match *state {
                HeartbeatState::Stopped => return Ok(()),
                HeartbeatState::Failed(failure) => return Err(failure.error()),
                HeartbeatState::Pending | HeartbeatState::Cancelled => {
                    return Err(HeartbeatFailure::SchedulerDeadline.error());
                }
                HeartbeatState::Running | HeartbeatState::StopRequested => {}
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(HeartbeatFailure::ShutdownDeadline.error());
            }
            let (next_state, _) = self
                .changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state = next_state;
        }
    }
}

struct RefreshLeaseHeartbeat {
    id: u64,
    commands: mpsc::SyncSender<HeartbeatCommand>,
    control: Arc<HeartbeatControl>,
}

impl RefreshLeaseHeartbeat {
    fn start(lock_file: &File, interval: Duration) -> io::Result<Self> {
        let lock_file = lock_file.try_clone()?;
        let id = HEARTBEAT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let commands = heartbeat_commands()?.clone();
        let control = Arc::new(HeartbeatControl::new());
        commands
            .try_send(HeartbeatCommand::Add {
                id,
                lock_file,
                interval,
                control: Arc::clone(&control),
            })
            .map_err(map_heartbeat_command_error)?;
        control.wait_for_start()?;
        Ok(Self {
            id,
            commands,
            control,
        })
    }

    fn stop(self) -> io::Result<()> {
        match self.control.request_stop() {
            HeartbeatState::Stopped => return Ok(()),
            HeartbeatState::Failed(failure) => return Err(failure.error()),
            HeartbeatState::Pending | HeartbeatState::Cancelled => {
                return Err(HeartbeatFailure::SchedulerDeadline.error());
            }
            HeartbeatState::Running | HeartbeatState::StopRequested => {}
        }

        self.commands
            .try_send(HeartbeatCommand::Remove { id: self.id })
            .map_err(map_heartbeat_command_error)?;
        self.control.wait_for_stop()
    }
}

enum HeartbeatCommand {
    Add {
        id: u64,
        lock_file: File,
        interval: Duration,
        control: Arc<HeartbeatControl>,
    },
    Remove {
        id: u64,
    },
    WriteFinished {
        id: u64,
        result: Result<(), HeartbeatFailure>,
    },
}

struct HeartbeatJob {
    id: u64,
    lock_file: Arc<File>,
    deadline: Instant,
}

struct ActiveHeartbeat {
    lock_file: Arc<File>,
    interval: Duration,
    next: Instant,
    deadline: Instant,
    in_flight: bool,
    control: Arc<HeartbeatControl>,
}

fn map_heartbeat_command_error<T>(error: mpsc::TrySendError<T>) -> io::Error {
    match error {
        mpsc::TrySendError::Full(_) => HeartbeatFailure::CommandQueueFull.error(),
        mpsc::TrySendError::Disconnected(_) => HeartbeatFailure::CommandDisconnected.error(),
    }
}

fn heartbeat_commands() -> io::Result<&'static mpsc::SyncSender<HeartbeatCommand>> {
    HEARTBEAT_COMMANDS
        .get_or_init(start_heartbeat_runtime)
        .as_ref()
        .map_err(|_| HeartbeatFailure::SchedulerUnavailable.error())
}

fn start_heartbeat_runtime() -> Result<mpsc::SyncSender<HeartbeatCommand>, io::ErrorKind> {
    let (commands, received) = mpsc::sync_channel(HEARTBEAT_COMMAND_CAPACITY);
    let mut worker_queues = Vec::with_capacity(HEARTBEAT_WORKER_COUNT);

    for worker_index in 0..HEARTBEAT_WORKER_COUNT {
        let (jobs, received_jobs) = mpsc::sync_channel(HEARTBEAT_WORK_QUEUE_CAPACITY);
        let worker_commands = commands.clone();
        thread::Builder::new()
            .name(format!("prodex-refresh-lease-heartbeat-{worker_index}"))
            .spawn(move || heartbeat_worker(received_jobs, worker_commands))
            .map_err(|error| error.kind())?;
        worker_queues.push(jobs);
    }

    thread::Builder::new()
        .name("prodex-refresh-lease-heartbeat-scheduler".to_string())
        .spawn(move || heartbeat_scheduler(received, worker_queues))
        .map_err(|error| error.kind())?;

    Ok(commands)
}

fn heartbeat_worker(
    jobs: mpsc::Receiver<HeartbeatJob>,
    commands: mpsc::SyncSender<HeartbeatCommand>,
) {
    while let Ok(job) = jobs.recv() {
        let result = if Instant::now() >= job.deadline {
            Err(HeartbeatFailure::WriteTimedOut)
        } else {
            perform_heartbeat_write(job.id, &job.lock_file)
                .map(|_| ())
                .map_err(|_| HeartbeatFailure::WriteFailed)
        };
        let _ = commands.try_send(HeartbeatCommand::WriteFinished { id: job.id, result });
    }
}

fn perform_heartbeat_write(_id: u64, lock_file: &File) -> io::Result<()> {
    #[cfg(test)]
    {
        let active = HEARTBEAT_TEST_ACTIVE_WRITES.fetch_add(1, Ordering::SeqCst) + 1;
        HEARTBEAT_TEST_MAX_ACTIVE_WRITES.fetch_max(active, Ordering::SeqCst);
    }

    #[cfg(test)]
    if heartbeat_test_should_stall(_id) {
        HEARTBEAT_TEST_WRITE_CALLS.fetch_add(1, Ordering::SeqCst);
        while heartbeat_test_should_stall(_id) {
            thread::sleep(Duration::from_millis(1));
        }
    }

    let times = fs::FileTimes::new().set_modified(SystemTime::now());
    // Scheduler deadline does not cancel an OS filesystem call; stalled worker stays fixed.
    let result = lock_file.set_times(times);

    #[cfg(test)]
    HEARTBEAT_TEST_ACTIVE_WRITES.fetch_sub(1, Ordering::SeqCst);

    result
}

#[cfg(test)]
fn heartbeat_test_should_stall(id: u64) -> bool {
    HEARTBEAT_TEST_STALL_IDS
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .contains(&id)
}

#[cfg(test)]
fn heartbeat_test_set_stalled_ids(ids: impl IntoIterator<Item = u64>) {
    let mut stalled = HEARTBEAT_TEST_STALL_IDS
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    stalled.clear();
    stalled.extend(ids);
}

fn heartbeat_scheduler(
    commands: mpsc::Receiver<HeartbeatCommand>,
    worker_queues: Vec<mpsc::SyncSender<HeartbeatJob>>,
) {
    let mut active = HashMap::<u64, ActiveHeartbeat>::new();
    let mut next_worker = 0;

    loop {
        if !receive_heartbeat_command(&commands, &mut active) {
            break;
        }
        process_active_heartbeats(&mut active, &worker_queues, &mut next_worker);
    }
}

fn receive_heartbeat_command(
    commands: &mpsc::Receiver<HeartbeatCommand>,
    active: &mut HashMap<u64, ActiveHeartbeat>,
) -> bool {
    match commands.recv_timeout(heartbeat_scheduler_timeout(active)) {
        Ok(command) => handle_heartbeat_command(command, active),
        Err(mpsc::RecvTimeoutError::Timeout) => {}
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            for heartbeat in active.values() {
                heartbeat.control.fail(HeartbeatFailure::SchedulerStopped);
            }
            return false;
        }
    }
    true
}

#[derive(Clone, Copy)]
enum HeartbeatDisposition {
    Keep,
    Stop,
    Fail(HeartbeatFailure),
}

fn process_active_heartbeats(
    active: &mut HashMap<u64, ActiveHeartbeat>,
    worker_queues: &[mpsc::SyncSender<HeartbeatJob>],
    next_worker: &mut usize,
) {
    let now = Instant::now();
    let mut stopped = Vec::new();
    let mut failed = Vec::new();
    for (&id, heartbeat) in active.iter_mut() {
        match heartbeat_disposition(id, heartbeat, worker_queues, next_worker, now) {
            HeartbeatDisposition::Keep => {}
            HeartbeatDisposition::Stop => stopped.push(id),
            HeartbeatDisposition::Fail(failure) => failed.push((id, failure)),
        }
    }
    for id in stopped {
        if let Some(heartbeat) = active.remove(&id) {
            heartbeat.control.mark_stopped();
        }
    }
    for (id, failure) in failed {
        if let Some(heartbeat) = active.remove(&id) {
            heartbeat.control.fail(failure);
        }
    }
}

fn heartbeat_disposition(
    id: u64,
    heartbeat: &mut ActiveHeartbeat,
    worker_queues: &[mpsc::SyncSender<HeartbeatJob>],
    next_worker: &mut usize,
    now: Instant,
) -> HeartbeatDisposition {
    match heartbeat.control.state() {
        HeartbeatState::StopRequested => heartbeat_stopping_disposition(heartbeat, now),
        HeartbeatState::Running => {
            heartbeat_running_disposition(id, heartbeat, worker_queues, next_worker, now)
        }
        HeartbeatState::Failed(failure) => HeartbeatDisposition::Fail(failure),
        HeartbeatState::Pending | HeartbeatState::Stopped | HeartbeatState::Cancelled => {
            HeartbeatDisposition::Stop
        }
    }
}

fn heartbeat_stopping_disposition(
    heartbeat: &ActiveHeartbeat,
    now: Instant,
) -> HeartbeatDisposition {
    if !heartbeat.in_flight {
        return HeartbeatDisposition::Stop;
    }
    if now >= heartbeat.deadline {
        HeartbeatDisposition::Fail(HeartbeatFailure::WriteTimedOut)
    } else {
        HeartbeatDisposition::Keep
    }
}

fn heartbeat_running_disposition(
    id: u64,
    heartbeat: &mut ActiveHeartbeat,
    worker_queues: &[mpsc::SyncSender<HeartbeatJob>],
    next_worker: &mut usize,
    now: Instant,
) -> HeartbeatDisposition {
    if heartbeat.in_flight {
        return if now >= heartbeat.deadline {
            HeartbeatDisposition::Fail(HeartbeatFailure::WriteTimedOut)
        } else {
            HeartbeatDisposition::Keep
        };
    }
    if heartbeat.next > now {
        return HeartbeatDisposition::Keep;
    }
    let deadline = now + HEARTBEAT_WRITE_DEADLINE;
    let job = HeartbeatJob {
        id,
        lock_file: Arc::clone(&heartbeat.lock_file),
        deadline,
    };
    match send_heartbeat_job(worker_queues, next_worker, job) {
        Ok(()) => {
            heartbeat.in_flight = true;
            heartbeat.deadline = deadline;
            HeartbeatDisposition::Keep
        }
        Err(HeartbeatFailure::WorkerQueueFull) => {
            heartbeat.next = now + HEARTBEAT_SCHEDULER_TICK;
            HeartbeatDisposition::Keep
        }
        Err(failure) => HeartbeatDisposition::Fail(failure),
    }
}

fn heartbeat_scheduler_timeout(active: &HashMap<u64, ActiveHeartbeat>) -> Duration {
    let now = Instant::now();
    active
        .values()
        .map(|heartbeat| {
            let deadline = if heartbeat.in_flight {
                heartbeat.deadline
            } else {
                heartbeat.next
            };
            deadline
                .saturating_duration_since(now)
                .min(HEARTBEAT_SCHEDULER_TICK)
        })
        .min()
        .unwrap_or(HEARTBEAT_SCHEDULER_TICK)
}

fn handle_heartbeat_command(command: HeartbeatCommand, active: &mut HashMap<u64, ActiveHeartbeat>) {
    match command {
        HeartbeatCommand::Add {
            id,
            lock_file,
            interval,
            control,
        } => add_heartbeat(active, id, lock_file, interval, control),
        HeartbeatCommand::Remove { id } => remove_heartbeat(active, id),
        HeartbeatCommand::WriteFinished { id, result } => {
            finish_heartbeat_write(active, id, result)
        }
    }
}

fn add_heartbeat(
    active: &mut HashMap<u64, ActiveHeartbeat>,
    id: u64,
    lock_file: File,
    interval: Duration,
    control: Arc<HeartbeatControl>,
) {
    if active.len() >= MAX_ACTIVE_HEARTBEATS || active.contains_key(&id) {
        control.fail(HeartbeatFailure::CapacityExhausted);
        return;
    }
    if control.mark_running() {
        let now = Instant::now();
        active.insert(
            id,
            ActiveHeartbeat {
                lock_file: Arc::new(lock_file),
                interval,
                next: now + interval,
                deadline: now,
                in_flight: false,
                control,
            },
        );
    }
}

fn remove_heartbeat(active: &mut HashMap<u64, ActiveHeartbeat>, id: u64) {
    if active.get(&id).is_some_and(|heartbeat| heartbeat.in_flight) {
        return;
    }
    if let Some(heartbeat) = active.remove(&id) {
        heartbeat.control.mark_stopped();
    }
}

fn finish_heartbeat_write(
    active: &mut HashMap<u64, ActiveHeartbeat>,
    id: u64,
    result: Result<(), HeartbeatFailure>,
) {
    let Some(heartbeat) = active.get_mut(&id) else {
        return;
    };
    if !heartbeat.in_flight {
        return;
    }
    heartbeat.in_flight = false;
    let mut stop = false;
    let failure = match result {
        Err(failure) => Some(failure),
        Ok(()) if Instant::now() >= heartbeat.deadline => Some(HeartbeatFailure::WriteTimedOut),
        Ok(()) => {
            stop = heartbeat.control.state() == HeartbeatState::StopRequested;
            if !stop {
                heartbeat.next = Instant::now() + heartbeat.interval;
            }
            None
        }
    };
    if let Some(failure) = failure {
        if let Some(heartbeat) = active.remove(&id) {
            heartbeat.control.fail(failure);
        }
    } else if stop && let Some(heartbeat) = active.remove(&id) {
        heartbeat.control.mark_stopped();
    }
}

fn send_heartbeat_job(
    worker_queues: &[mpsc::SyncSender<HeartbeatJob>],
    next_worker: &mut usize,
    job: HeartbeatJob,
) -> Result<(), HeartbeatFailure> {
    if worker_queues.is_empty() {
        return Err(HeartbeatFailure::WorkerUnavailable);
    }

    let mut job = Some(job);
    let mut disconnected = 0;
    for offset in 0..worker_queues.len() {
        let index = (*next_worker + offset) % worker_queues.len();
        match worker_queues[index].try_send(job.take().expect("heartbeat job is retained")) {
            Ok(()) => {
                *next_worker = (index + 1) % worker_queues.len();
                return Ok(());
            }
            Err(mpsc::TrySendError::Full(returned)) => job = Some(returned),
            Err(mpsc::TrySendError::Disconnected(returned)) => {
                disconnected += 1;
                job = Some(returned);
            }
        }
    }

    if disconnected == worker_queues.len() {
        Err(HeartbeatFailure::WorkerUnavailable)
    } else {
        Err(HeartbeatFailure::WorkerQueueFull)
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
            return match secure_file::remove_untrusted_entry(path) {
                Ok(()) => Ok(()),
                Err(error) if refresh_lease_contention_error(&error) => Ok(()),
                Err(error) => Err(RefreshLeaseError::io(path, error)),
            };
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

fn refresh_lease_contention_error(error: &io::Error) -> bool {
    if matches!(
        error.kind(),
        io::ErrorKind::AlreadyExists | io::ErrorKind::WouldBlock
    ) {
        return true;
    }

    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{
            ERROR_ACCESS_DENIED, ERROR_LOCK_VIOLATION, ERROR_SHARING_VIOLATION,
        };

        matches!(
            error.raw_os_error(),
            Some(code)
                if code == ERROR_ACCESS_DENIED as i32
                    || code == ERROR_LOCK_VIOLATION as i32
                    || code == ERROR_SHARING_VIOLATION as i32
        )
    }

    #[cfg(not(windows))]
    {
        false
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

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;

    fn wait_for_test_writes_to_finish() {
        let deadline = Instant::now() + Duration::from_secs(1);
        while HEARTBEAT_TEST_ACTIVE_WRITES.load(Ordering::SeqCst) != 0 && Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(HEARTBEAT_TEST_ACTIVE_WRITES.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn stalled_heartbeat_write_returns_bounded_unhealthy_error() {
        let _test_lock = HEARTBEAT_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = std::env::temp_dir().join(format!(
            "prodex-secret-store-heartbeat-stall-{}-{}",
            std::process::id(),
            FENCE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let coordinator = RefreshLeaseCoordinator::new(&root)
            .with_lease_ttl(Duration::from_millis(30))
            .with_wait_timeout(Duration::ZERO);
        let owner = match coordinator.acquire("heartbeat-stall-test") {
            Ok(RefreshLeaseDecision::Owner(owner)) => owner,
            other => panic!("expected owner, got {other:?}"),
        };
        let id = owner.heartbeat.as_ref().expect("heartbeat is active").id;
        HEARTBEAT_TEST_WRITE_CALLS.store(0, Ordering::SeqCst);
        heartbeat_test_set_stalled_ids([id]);
        thread::sleep(HEARTBEAT_WRITE_DEADLINE + HEARTBEAT_SCHEDULER_TICK * 3);

        let (finished, received) = mpsc::sync_channel(1);
        let handle = thread::spawn(move || {
            finished
                .send(owner.commit_result("{\"access_token\":\"stall\"}"))
                .unwrap();
        });
        let elapsed_start = Instant::now();
        let result = received.recv_timeout(Duration::from_secs(1));
        heartbeat_test_set_stalled_ids([]);
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                let _ = handle.join();
                panic!("stalled heartbeat shutdown exceeded bound: {error}");
            }
        };
        handle.join().unwrap();
        wait_for_test_writes_to_finish();

        assert!(elapsed_start.elapsed() < Duration::from_secs(1));
        assert!(matches!(
            result,
            Err(RefreshLeaseError::Io {
                kind: RefreshLeaseIoKind::Generic,
                ..
            })
        ));
        assert_eq!(HEARTBEAT_TEST_WRITE_CALLS.load(Ordering::SeqCst), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stalled_heartbeat_writes_cannot_accumulate_workers() {
        let _test_lock = HEARTBEAT_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = std::env::temp_dir().join(format!(
            "prodex-secret-store-heartbeat-workers-{}-{}",
            std::process::id(),
            FENCE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let coordinator = RefreshLeaseCoordinator::new(&root)
            .with_lease_ttl(Duration::from_millis(3))
            .with_wait_timeout(Duration::ZERO);
        wait_for_test_writes_to_finish();
        HEARTBEAT_TEST_ACTIVE_WRITES.store(0, Ordering::SeqCst);
        HEARTBEAT_TEST_MAX_ACTIVE_WRITES.store(0, Ordering::SeqCst);
        HEARTBEAT_TEST_WRITE_CALLS.store(0, Ordering::SeqCst);
        let mut owners = Vec::new();
        for index in 0..8 {
            match coordinator.acquire(format!("heartbeat-workers-test-{index}")) {
                Ok(RefreshLeaseDecision::Owner(owner)) => owners.push(owner),
                other => panic!("expected owner, got {other:?}"),
            }
        }
        heartbeat_test_set_stalled_ids(
            owners
                .iter()
                .map(|owner| owner.heartbeat.as_ref().expect("heartbeat is active").id),
        );

        thread::sleep(HEARTBEAT_WRITE_DEADLINE + HEARTBEAT_SCHEDULER_TICK * 3);
        let maximum_active_writes = HEARTBEAT_TEST_MAX_ACTIVE_WRITES.load(Ordering::SeqCst);

        heartbeat_test_set_stalled_ids([]);
        for mut owner in owners {
            let _ = owner.release();
        }
        wait_for_test_writes_to_finish();
        assert!(
            (1..=HEARTBEAT_WORKER_COUNT as u64).contains(&maximum_active_writes),
            "stalled writes should occupy only the fixed worker pool: {maximum_active_writes}"
        );
        let _ = fs::remove_dir_all(root);
    }
}
