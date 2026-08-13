use std::collections::HashMap;
#[cfg(test)]
use std::collections::HashSet;
use std::fs::{self, File};
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, mpsc};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

#[cfg(test)]
use super::{
    FENCE_SEQUENCE, RefreshLeaseCoordinator, RefreshLeaseDecision, RefreshLeaseError,
    RefreshLeaseIoKind,
};

const MIN_HEARTBEAT_INTERVAL: Duration = Duration::from_millis(1);
const MAX_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(60);
const HEARTBEAT_COMMAND_CAPACITY: usize = 64;
const MAX_ACTIVE_HEARTBEATS: usize = HEARTBEAT_COMMAND_CAPACITY;
const HEARTBEAT_WORKER_COUNT: usize = 2;
const HEARTBEAT_WORK_QUEUE_CAPACITY: usize = 1;
const HEARTBEAT_SCHEDULER_TICK: Duration = Duration::from_millis(10);
const HEARTBEAT_COMMAND_DEADLINE: Duration = Duration::from_millis(100);
const HEARTBEAT_WRITE_DEADLINE: Duration = Duration::from_secs(1);
const HEARTBEAT_SHUTDOWN_DEADLINE: Duration = Duration::from_millis(250);
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
#[cfg(test)]
pub(crate) fn lock_heartbeat_test_state() -> std::sync::MutexGuard<'static, ()> {
    HEARTBEAT_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
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

pub(super) struct RefreshLeaseHeartbeat {
    id: u64,
    commands: mpsc::SyncSender<HeartbeatCommand>,
    control: Arc<HeartbeatControl>,
}

impl RefreshLeaseHeartbeat {
    pub(super) fn start(lock_file: &File, interval: Duration) -> io::Result<Self> {
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

    pub(super) fn stop(self) -> io::Result<()> {
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

pub(super) fn heartbeat_interval(lease_ttl: Duration) -> Duration {
    (lease_ttl / 3).clamp(MIN_HEARTBEAT_INTERVAL, MAX_HEARTBEAT_INTERVAL)
}

#[cfg(test)]
mod tests;
