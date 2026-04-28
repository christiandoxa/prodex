use crate::{
    RuntimeRotationProxyShared, RuntimeWebsocketTcpAttemptResult, runtime_proxy_log_to_path,
};
use std::collections::VecDeque;
use std::env;
use std::io;
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

type RuntimeWebsocketTcpConnectJob = Box<dyn FnOnce() + Send + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeWebsocketLocalPressureKind {
    DnsResolveTimeout,
    DnsResolveExecutorOverflow,
    TcpConnectExecutorOverflow,
}

impl RuntimeWebsocketLocalPressureKind {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::DnsResolveTimeout => "dns_resolve_timeout",
            Self::DnsResolveExecutorOverflow => "dns_resolve_executor_overflow",
            Self::TcpConnectExecutorOverflow => "tcp_connect_executor_overflow",
        }
    }
}

#[derive(Debug)]
struct RuntimeWebsocketLocalPressureError {
    kind: RuntimeWebsocketLocalPressureKind,
    message: String,
}

impl RuntimeWebsocketLocalPressureError {
    fn new(kind: RuntimeWebsocketLocalPressureKind, message: String) -> Self {
        Self { kind, message }
    }
}

impl std::fmt::Display for RuntimeWebsocketLocalPressureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RuntimeWebsocketLocalPressureError {}

pub(super) fn runtime_websocket_local_pressure_io_error(
    kind: RuntimeWebsocketLocalPressureKind,
    message: impl Into<String>,
) -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        RuntimeWebsocketLocalPressureError::new(kind, message.into()),
    )
}

pub(super) fn runtime_websocket_local_pressure_kind_from_io_error(
    err: &io::Error,
) -> Option<RuntimeWebsocketLocalPressureKind> {
    err.get_ref()
        .and_then(|source| source.downcast_ref::<RuntimeWebsocketLocalPressureError>())
        .map(|err| err.kind)
}

#[derive(Clone, Copy)]
enum RuntimeWebsocketTcpConnectTaskKind {
    TcpConnect,
    DnsResolve,
}

impl RuntimeWebsocketTcpConnectTaskKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::TcpConnect => "tcp_connect",
            Self::DnsResolve => "dns_resolve",
        }
    }

    fn worker_thread_prefix(self) -> &'static str {
        match self {
            Self::TcpConnect => "prodex-ws-connect",
            Self::DnsResolve => "prodex-ws-dns",
        }
    }

    fn dispatcher_thread_name(self) -> &'static str {
        match self {
            Self::TcpConnect => "prodex-ws-connect-dispatch",
            Self::DnsResolve => "prodex-ws-dns-dispatch",
        }
    }

    fn overflow_enqueue_event(self) -> &'static str {
        match self {
            Self::TcpConnect => "websocket_connect_overflow_enqueue",
            Self::DnsResolve => "websocket_dns_overflow_enqueue",
        }
    }

    fn overflow_dispatch_event(self) -> &'static str {
        match self {
            Self::TcpConnect => "websocket_connect_overflow_dispatch",
            Self::DnsResolve => "websocket_dns_overflow_dispatch",
        }
    }

    fn overflow_reject_event(self) -> &'static str {
        match self {
            Self::TcpConnect => "websocket_connect_overflow_reject",
            Self::DnsResolve => "websocket_dns_overflow_reject",
        }
    }
}

#[derive(Clone)]
struct RuntimeWebsocketTcpConnectTaskObservability {
    log_path: Option<PathBuf>,
    request_id: Option<u64>,
    kind: RuntimeWebsocketTcpConnectTaskKind,
    addr: Option<SocketAddr>,
    host: Option<String>,
    port: Option<u16>,
}

struct RuntimeWebsocketTcpConnectTask {
    job: RuntimeWebsocketTcpConnectJob,
    observability: RuntimeWebsocketTcpConnectTaskObservability,
}

impl RuntimeWebsocketTcpConnectTask {
    fn new_tcp_connect(
        job: RuntimeWebsocketTcpConnectJob,
        log_path: Option<&Path>,
        request_id: Option<u64>,
        addr: Option<SocketAddr>,
    ) -> Self {
        Self {
            job,
            observability: RuntimeWebsocketTcpConnectTaskObservability {
                log_path: log_path.map(Path::to_path_buf),
                request_id,
                kind: RuntimeWebsocketTcpConnectTaskKind::TcpConnect,
                addr,
                host: None,
                port: None,
            },
        }
    }

    fn new_dns_resolve(
        job: RuntimeWebsocketTcpConnectJob,
        log_path: Option<&Path>,
        request_id: Option<u64>,
        host: &str,
        port: u16,
    ) -> Self {
        Self {
            job,
            observability: RuntimeWebsocketTcpConnectTaskObservability {
                log_path: log_path.map(Path::to_path_buf),
                request_id,
                kind: RuntimeWebsocketTcpConnectTaskKind::DnsResolve,
                addr: None,
                host: Some(host.to_string()),
                port: Some(port),
            },
        }
    }

    fn run(self) {
        (self.job)();
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct RuntimeWebsocketTcpConnectOverflowSnapshot {
    pub(super) pending_jobs: usize,
    pub(super) max_pending_jobs: usize,
    pub(super) total_enqueued: usize,
    pub(super) total_dispatched: usize,
    pub(super) total_rejected: usize,
}

#[derive(Default)]
struct RuntimeWebsocketTcpConnectOverflowState {
    jobs: VecDeque<RuntimeWebsocketTcpConnectTask>,
    total_enqueued: usize,
    total_dispatched: usize,
    total_rejected: usize,
    max_pending_jobs: usize,
}

#[derive(Default)]
struct RuntimeWebsocketTcpConnectOverflowQueue {
    state: Mutex<RuntimeWebsocketTcpConnectOverflowState>,
    work_available: Condvar,
}

impl RuntimeWebsocketTcpConnectOverflowQueue {
    fn snapshot_from_state(
        state: &RuntimeWebsocketTcpConnectOverflowState,
    ) -> RuntimeWebsocketTcpConnectOverflowSnapshot {
        RuntimeWebsocketTcpConnectOverflowSnapshot {
            pending_jobs: state.jobs.len(),
            max_pending_jobs: state.max_pending_jobs,
            total_enqueued: state.total_enqueued,
            total_dispatched: state.total_dispatched,
            total_rejected: state.total_rejected,
        }
    }

    fn push(
        &self,
        task: RuntimeWebsocketTcpConnectTask,
        overflow_capacity: usize,
    ) -> std::result::Result<
        RuntimeWebsocketTcpConnectOverflowSnapshot,
        RuntimeWebsocketTcpConnectOverflowSnapshot,
    > {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.jobs.len() >= overflow_capacity {
            state.total_rejected = state.total_rejected.saturating_add(1);
            return Err(Self::snapshot_from_state(&state));
        }
        state.jobs.push_back(task);
        state.total_enqueued = state.total_enqueued.saturating_add(1);
        state.max_pending_jobs = state.max_pending_jobs.max(state.jobs.len());
        let snapshot = Self::snapshot_from_state(&state);
        self.work_available.notify_one();
        Ok(snapshot)
    }

    fn pop(
        &self,
    ) -> (
        RuntimeWebsocketTcpConnectTask,
        RuntimeWebsocketTcpConnectOverflowSnapshot,
    ) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        loop {
            if let Some(task) = state.jobs.pop_front() {
                state.total_dispatched = state.total_dispatched.saturating_add(1);
                return (task, Self::snapshot_from_state(&state));
            }
            state = self
                .work_available
                .wait(state)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }

    #[cfg(test)]
    fn snapshot(&self) -> RuntimeWebsocketTcpConnectOverflowSnapshot {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Self::snapshot_from_state(&state)
    }
}

enum RuntimeWebsocketTcpConnectExecutorMode {
    Bounded {
        sender: SyncSender<RuntimeWebsocketTcpConnectTask>,
        overflow: Arc<RuntimeWebsocketTcpConnectOverflowQueue>,
    },
    Inline,
}

pub(super) struct RuntimeWebsocketTcpConnectExecutor {
    mode: RuntimeWebsocketTcpConnectExecutorMode,
    worker_count: usize,
    queue_capacity: usize,
    overflow_capacity: usize,
}

impl RuntimeWebsocketTcpConnectExecutor {
    pub(super) fn global() -> &'static Self {
        static EXECUTOR: OnceLock<RuntimeWebsocketTcpConnectExecutor> = OnceLock::new();
        EXECUTOR.get_or_init(|| {
            let worker_count = runtime_websocket_tcp_connect_worker_count();
            let queue_capacity = runtime_websocket_tcp_connect_queue_capacity(worker_count);
            RuntimeWebsocketTcpConnectExecutor::new(worker_count, queue_capacity)
        })
    }

    pub(super) fn new(worker_count: usize, queue_capacity: usize) -> Self {
        let worker_count = worker_count.max(1);
        let queue_capacity = queue_capacity.max(worker_count).max(1);
        let overflow_capacity =
            runtime_websocket_tcp_connect_overflow_capacity(worker_count, queue_capacity);
        Self::new_with_overflow_capacity(worker_count, queue_capacity, overflow_capacity)
    }

    pub(super) fn new_with_overflow_capacity(
        worker_count: usize,
        queue_capacity: usize,
        overflow_capacity: usize,
    ) -> Self {
        Self::new_for_kind(
            RuntimeWebsocketTcpConnectTaskKind::TcpConnect,
            worker_count,
            queue_capacity,
            overflow_capacity,
        )
    }

    fn new_for_kind(
        task_kind: RuntimeWebsocketTcpConnectTaskKind,
        worker_count: usize,
        queue_capacity: usize,
        overflow_capacity: usize,
    ) -> Self {
        Self::new_for_kind_with_spawner(
            task_kind,
            worker_count,
            queue_capacity,
            overflow_capacity,
            |name, job| thread::Builder::new().name(name).spawn(job).is_ok(),
        )
    }

    fn new_for_kind_with_spawner<F>(
        task_kind: RuntimeWebsocketTcpConnectTaskKind,
        worker_count: usize,
        queue_capacity: usize,
        overflow_capacity: usize,
        mut spawn_thread: F,
    ) -> Self
    where
        F: FnMut(String, Box<dyn FnOnce() + Send + 'static>) -> bool,
    {
        let worker_count = worker_count.max(1);
        let queue_capacity = queue_capacity.max(worker_count).max(1);
        let (sender, receiver) =
            mpsc::sync_channel::<RuntimeWebsocketTcpConnectTask>(queue_capacity);
        let receiver = Arc::new(Mutex::new(receiver));
        let overflow = Arc::new(RuntimeWebsocketTcpConnectOverflowQueue::default());
        let mut started_workers = 0usize;

        for index in 0..worker_count {
            let receiver = Arc::clone(&receiver);
            let name = format!("{}-{index}", task_kind.worker_thread_prefix());
            if spawn_thread(
                name,
                Box::new(move || runtime_websocket_tcp_connect_worker_loop(receiver)),
            ) {
                started_workers += 1;
            }
        }

        if started_workers == 0 {
            return Self {
                mode: RuntimeWebsocketTcpConnectExecutorMode::Inline,
                worker_count,
                queue_capacity,
                overflow_capacity,
            };
        }

        let actual_worker_count = started_workers;
        let overflow_sender = sender.clone();
        let overflow_queue = Arc::clone(&overflow);
        let dispatcher_started = spawn_thread(
            task_kind.dispatcher_thread_name().to_string(),
            Box::new(move || {
                runtime_websocket_tcp_connect_dispatcher_loop(
                    overflow_queue,
                    overflow_sender,
                    actual_worker_count,
                    queue_capacity,
                    overflow_capacity,
                )
            }),
        );
        if !dispatcher_started {
            return Self {
                mode: RuntimeWebsocketTcpConnectExecutorMode::Inline,
                worker_count,
                queue_capacity,
                overflow_capacity,
            };
        }

        Self {
            mode: RuntimeWebsocketTcpConnectExecutorMode::Bounded { sender, overflow },
            worker_count: actual_worker_count,
            queue_capacity,
            overflow_capacity,
        }
    }

    #[cfg(test)]
    pub(super) fn new_with_spawn_outcome_for_test(
        worker_count: usize,
        queue_capacity: usize,
        overflow_capacity: usize,
        started_worker_count: usize,
        dispatcher_started: bool,
    ) -> Self {
        let worker_prefix = RuntimeWebsocketTcpConnectTaskKind::TcpConnect.worker_thread_prefix();
        let dispatcher_name =
            RuntimeWebsocketTcpConnectTaskKind::TcpConnect.dispatcher_thread_name();
        Self::new_for_kind_with_spawner(
            RuntimeWebsocketTcpConnectTaskKind::TcpConnect,
            worker_count,
            queue_capacity,
            overflow_capacity,
            |name, job| {
                let accepted = if name == dispatcher_name {
                    dispatcher_started
                } else {
                    name.strip_prefix(worker_prefix)
                        .and_then(|suffix| suffix.strip_prefix('-'))
                        .and_then(|index| index.parse::<usize>().ok())
                        .is_some_and(|index| index < started_worker_count)
                };
                if !accepted {
                    return false;
                }
                thread::Builder::new().name(name).spawn(job).is_ok()
            },
        )
    }

    #[cfg(test)]
    pub(super) fn spawn<F>(&self, job: F) -> bool
    where
        F: FnOnce() + Send + 'static,
    {
        self.spawn_observed(None, None, None, job)
    }

    pub(super) fn spawn_observed<F>(
        &self,
        log_path: Option<&Path>,
        request_id: Option<u64>,
        addr: Option<SocketAddr>,
        job: F,
    ) -> bool
    where
        F: FnOnce() + Send + 'static,
    {
        self.spawn_boxed(RuntimeWebsocketTcpConnectTask::new_tcp_connect(
            Box::new(job),
            log_path,
            request_id,
            addr,
        ))
    }

    fn spawn_boxed(&self, task: RuntimeWebsocketTcpConnectTask) -> bool {
        match &self.mode {
            RuntimeWebsocketTcpConnectExecutorMode::Bounded { sender, overflow } => {
                if let Err(err) = sender.try_send(task) {
                    match err {
                        mpsc::TrySendError::Full(task) => {
                            let observability = task.observability.clone();
                            match overflow.push(task, self.overflow_capacity) {
                                Ok(snapshot) => {
                                    runtime_websocket_tcp_connect_log_overflow_enqueue(
                                        &observability,
                                        self.worker_count,
                                        self.queue_capacity,
                                        self.overflow_capacity,
                                        snapshot,
                                        "queue_full",
                                    );
                                    true
                                }
                                Err(snapshot) => {
                                    runtime_websocket_tcp_connect_log_overflow_reject(
                                        &observability,
                                        self.worker_count,
                                        self.queue_capacity,
                                        self.overflow_capacity,
                                        snapshot,
                                        "overflow_capacity_reached",
                                    );
                                    false
                                }
                            }
                        }
                        mpsc::TrySendError::Disconnected(task) => {
                            let observability = task.observability.clone();
                            match overflow.push(task, self.overflow_capacity) {
                                Ok(snapshot) => {
                                    runtime_websocket_tcp_connect_log_overflow_enqueue(
                                        &observability,
                                        self.worker_count,
                                        self.queue_capacity,
                                        self.overflow_capacity,
                                        snapshot,
                                        "executor_disconnected",
                                    );
                                    true
                                }
                                Err(snapshot) => {
                                    runtime_websocket_tcp_connect_log_overflow_reject(
                                        &observability,
                                        self.worker_count,
                                        self.queue_capacity,
                                        self.overflow_capacity,
                                        snapshot,
                                        "overflow_capacity_reached",
                                    );
                                    false
                                }
                            }
                        }
                    }
                } else {
                    true
                }
            }
            RuntimeWebsocketTcpConnectExecutorMode::Inline => {
                task.run();
                true
            }
        }
    }

    #[cfg(test)]
    pub(super) fn overflow_snapshot(&self) -> Option<RuntimeWebsocketTcpConnectOverflowSnapshot> {
        match &self.mode {
            RuntimeWebsocketTcpConnectExecutorMode::Bounded { overflow, .. } => {
                Some(overflow.snapshot())
            }
            RuntimeWebsocketTcpConnectExecutorMode::Inline => None,
        }
    }
}

pub(super) struct RuntimeWebsocketDnsResolveExecutor {
    inner: RuntimeWebsocketTcpConnectExecutor,
}

impl RuntimeWebsocketDnsResolveExecutor {
    pub(super) fn global() -> &'static Self {
        static EXECUTOR: OnceLock<RuntimeWebsocketDnsResolveExecutor> = OnceLock::new();
        EXECUTOR.get_or_init(|| {
            let worker_count = runtime_websocket_dns_resolve_worker_count();
            let queue_capacity = runtime_websocket_dns_resolve_queue_capacity(worker_count);
            RuntimeWebsocketDnsResolveExecutor::new(worker_count, queue_capacity)
        })
    }

    pub(super) fn new(worker_count: usize, queue_capacity: usize) -> Self {
        let worker_count = worker_count.max(1);
        let queue_capacity = queue_capacity.max(worker_count).max(1);
        let overflow_capacity =
            runtime_websocket_dns_resolve_overflow_capacity(worker_count, queue_capacity);
        Self::new_with_overflow_capacity(worker_count, queue_capacity, overflow_capacity)
    }

    pub(super) fn new_with_overflow_capacity(
        worker_count: usize,
        queue_capacity: usize,
        overflow_capacity: usize,
    ) -> Self {
        Self {
            inner: RuntimeWebsocketTcpConnectExecutor::new_for_kind(
                RuntimeWebsocketTcpConnectTaskKind::DnsResolve,
                worker_count,
                queue_capacity,
                overflow_capacity,
            ),
        }
    }

    fn spawn_observed<F>(
        &self,
        log_path: Option<&Path>,
        request_id: Option<u64>,
        host: &str,
        port: u16,
        job: F,
    ) -> bool
    where
        F: FnOnce() + Send + 'static,
    {
        self.inner
            .spawn_boxed(RuntimeWebsocketTcpConnectTask::new_dns_resolve(
                Box::new(job),
                log_path,
                request_id,
                host,
                port,
            ))
    }

    #[cfg(test)]
    pub(super) fn overflow_snapshot(&self) -> Option<RuntimeWebsocketTcpConnectOverflowSnapshot> {
        self.inner.overflow_snapshot()
    }
}

fn runtime_websocket_tcp_connect_worker_count() -> usize {
    runtime_websocket_tcp_connect_env_usize(
        "PRODEX_RUNTIME_WEBSOCKET_CONNECT_WORKER_COUNT",
        thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(4)
            .clamp(4, 16),
    )
    .max(1)
}

fn runtime_websocket_tcp_connect_queue_capacity(worker_count: usize) -> usize {
    runtime_websocket_tcp_connect_env_usize(
        "PRODEX_RUNTIME_WEBSOCKET_CONNECT_QUEUE_CAPACITY",
        worker_count.saturating_mul(8).clamp(32, 128),
    )
    .max(worker_count)
    .max(1)
}

fn runtime_websocket_tcp_connect_overflow_capacity(
    worker_count: usize,
    queue_capacity: usize,
) -> usize {
    let default_capacity = queue_capacity
        .saturating_mul(4)
        .max(worker_count)
        .clamp(32, 512);
    runtime_websocket_tcp_connect_env_usize_allow_zero(
        "PRODEX_RUNTIME_WEBSOCKET_CONNECT_OVERFLOW_CAPACITY",
        default_capacity,
    )
}

fn runtime_websocket_dns_resolve_worker_count() -> usize {
    runtime_websocket_tcp_connect_env_usize(
        "PRODEX_RUNTIME_WEBSOCKET_DNS_WORKER_COUNT",
        thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(2)
            .clamp(2, 8),
    )
    .max(1)
}

fn runtime_websocket_dns_resolve_queue_capacity(worker_count: usize) -> usize {
    runtime_websocket_tcp_connect_env_usize(
        "PRODEX_RUNTIME_WEBSOCKET_DNS_QUEUE_CAPACITY",
        worker_count.saturating_mul(4).clamp(16, 64),
    )
    .max(worker_count)
    .max(1)
}

fn runtime_websocket_dns_resolve_overflow_capacity(
    worker_count: usize,
    queue_capacity: usize,
) -> usize {
    let default_capacity = queue_capacity
        .saturating_mul(2)
        .max(worker_count)
        .clamp(16, 128);
    runtime_websocket_tcp_connect_env_usize_allow_zero(
        "PRODEX_RUNTIME_WEBSOCKET_DNS_OVERFLOW_CAPACITY",
        default_capacity,
    )
}

fn runtime_websocket_tcp_connect_env_usize(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn runtime_websocket_tcp_connect_env_usize_allow_zero(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(default)
}

fn runtime_websocket_tcp_connect_dispatcher_loop(
    overflow: Arc<RuntimeWebsocketTcpConnectOverflowQueue>,
    sender: SyncSender<RuntimeWebsocketTcpConnectTask>,
    worker_count: usize,
    queue_capacity: usize,
    overflow_capacity: usize,
) {
    loop {
        let (task, snapshot) = overflow.pop();
        let observability = task.observability.clone();
        match sender.send(task) {
            Ok(()) => runtime_websocket_tcp_connect_log_overflow_dispatch(
                &observability,
                worker_count,
                queue_capacity,
                overflow_capacity,
                snapshot,
                "queue_available",
            ),
            Err(err) => {
                runtime_websocket_tcp_connect_log_overflow_dispatch(
                    &err.0.observability,
                    worker_count,
                    queue_capacity,
                    overflow_capacity,
                    snapshot,
                    "executor_disconnected",
                );
                err.0.run();
            }
        }
    }
}

fn runtime_websocket_tcp_connect_worker_loop(
    receiver: Arc<Mutex<Receiver<RuntimeWebsocketTcpConnectTask>>>,
) {
    loop {
        let job = {
            let receiver = receiver
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            receiver.recv()
        };

        let Ok(job) = job else {
            break;
        };
        job.run();
    }
}

fn runtime_websocket_tcp_connect_log_overflow_enqueue(
    observability: &RuntimeWebsocketTcpConnectTaskObservability,
    worker_count: usize,
    queue_capacity: usize,
    overflow_capacity: usize,
    snapshot: RuntimeWebsocketTcpConnectOverflowSnapshot,
    reason: &str,
) {
    runtime_websocket_tcp_connect_log_overflow_event(
        observability,
        worker_count,
        queue_capacity,
        overflow_capacity,
        snapshot,
        observability.kind.overflow_enqueue_event(),
        reason,
    );
}

fn runtime_websocket_tcp_connect_log_overflow_dispatch(
    observability: &RuntimeWebsocketTcpConnectTaskObservability,
    worker_count: usize,
    queue_capacity: usize,
    overflow_capacity: usize,
    snapshot: RuntimeWebsocketTcpConnectOverflowSnapshot,
    reason: &str,
) {
    runtime_websocket_tcp_connect_log_overflow_event(
        observability,
        worker_count,
        queue_capacity,
        overflow_capacity,
        snapshot,
        observability.kind.overflow_dispatch_event(),
        reason,
    );
}

fn runtime_websocket_tcp_connect_log_overflow_reject(
    observability: &RuntimeWebsocketTcpConnectTaskObservability,
    worker_count: usize,
    queue_capacity: usize,
    overflow_capacity: usize,
    snapshot: RuntimeWebsocketTcpConnectOverflowSnapshot,
    reason: &str,
) {
    runtime_websocket_tcp_connect_log_overflow_event(
        observability,
        worker_count,
        queue_capacity,
        overflow_capacity,
        snapshot,
        observability.kind.overflow_reject_event(),
        reason,
    );
}

fn runtime_websocket_tcp_connect_log_overflow_event(
    observability: &RuntimeWebsocketTcpConnectTaskObservability,
    worker_count: usize,
    queue_capacity: usize,
    overflow_capacity: usize,
    snapshot: RuntimeWebsocketTcpConnectOverflowSnapshot,
    event: &str,
    reason: &str,
) {
    let Some(log_path) = observability.log_path.as_ref() else {
        return;
    };

    let request = observability
        .request_id
        .map(|request_id| format!("request={request_id} "))
        .unwrap_or_default();
    let task = observability.kind.as_str();
    let addr = observability
        .addr
        .map(|addr| format!(" addr={addr}"))
        .unwrap_or_default();
    let host = observability
        .host
        .as_ref()
        .map(|host| format!(" host={host}"))
        .unwrap_or_default();
    let port = observability
        .port
        .map(|port| format!(" port={port}"))
        .unwrap_or_default();
    runtime_proxy_log_to_path(
        log_path,
        &format!(
            "{request}transport=websocket {event} reason={reason} task={task}{addr}{host}{port} overflow_pending={} overflow_max_pending={} overflow_total_enqueued={} overflow_total_dispatched={} overflow_total_rejected={} worker_count={worker_count} queue_capacity={queue_capacity} overflow_capacity={overflow_capacity}",
            snapshot.pending_jobs,
            snapshot.max_pending_jobs,
            snapshot.total_enqueued,
            snapshot.total_dispatched,
            snapshot.total_rejected,
        ),
    );
}

pub(super) fn runtime_resolve_websocket_tcp_addrs_with_executor<F>(
    executor: &RuntimeWebsocketDnsResolveExecutor,
    log_path: Option<&Path>,
    request_id: Option<u64>,
    host: String,
    port: u16,
    timeout: Duration,
    resolver: F,
) -> io::Result<Vec<SocketAddr>>
where
    F: FnOnce(String, u16) -> io::Result<Vec<SocketAddr>> + Send + 'static,
{
    let (sender, receiver) = mpsc::channel::<io::Result<Vec<SocketAddr>>>();
    let observed_host = host.clone();
    let accepted = executor.spawn_observed(log_path, request_id, &observed_host, port, move || {
        let result = resolver(host, port);
        let _ = sender.send(result);
    });
    if !accepted {
        return Err(runtime_websocket_local_pressure_io_error(
            RuntimeWebsocketLocalPressureKind::DnsResolveExecutorOverflow,
            format!(
                "websocket DNS resolution executor overflow for {}:{}",
                observed_host, port
            ),
        ));
    }

    match receiver.recv_timeout(timeout) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            runtime_websocket_tcp_connect_log_dns_resolve_timeout(
                log_path,
                request_id,
                &observed_host,
                port,
                timeout,
            );
            Err(runtime_websocket_local_pressure_io_error(
                RuntimeWebsocketLocalPressureKind::DnsResolveTimeout,
                format!(
                    "websocket DNS resolution timed out after {}ms for {}:{}",
                    timeout.as_millis(),
                    observed_host,
                    port
                ),
            ))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            format!(
                "websocket DNS resolution worker disconnected for {}:{}",
                observed_host, port
            ),
        )),
    }
}

fn runtime_websocket_tcp_connect_log_dns_resolve_timeout(
    log_path: Option<&Path>,
    request_id: Option<u64>,
    host: &str,
    port: u16,
    timeout: Duration,
) {
    let Some(log_path) = log_path else {
        return;
    };
    let request = request_id
        .map(|request_id| format!("request={request_id} "))
        .unwrap_or_default();
    runtime_proxy_log_to_path(
        log_path,
        &format!(
            "{request}transport=websocket websocket_dns_resolve_timeout host={host} port={port} timeout_ms={}",
            timeout.as_millis()
        ),
    );
}

pub(super) fn runtime_launch_websocket_tcp_connect_attempt(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    sender: mpsc::Sender<RuntimeWebsocketTcpAttemptResult>,
    addr: SocketAddr,
    connect_timeout: Duration,
) {
    let result_sender = sender.clone();
    let accepted = RuntimeWebsocketTcpConnectExecutor::global().spawn_observed(
        Some(shared.log_path.as_path()),
        Some(request_id),
        Some(addr),
        move || {
            let result = TcpStream::connect_timeout(&addr, connect_timeout);
            let _ = result_sender.send(RuntimeWebsocketTcpAttemptResult { addr, result });
        },
    );
    if !accepted {
        let _ = sender.send(RuntimeWebsocketTcpAttemptResult {
            addr,
            result: Err(runtime_websocket_local_pressure_io_error(
                RuntimeWebsocketLocalPressureKind::TcpConnectExecutorOverflow,
                format!("websocket TCP connect executor overflow for {addr}"),
            )),
        });
    }
}
