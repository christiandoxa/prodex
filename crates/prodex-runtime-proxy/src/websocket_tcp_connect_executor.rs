use std::collections::VecDeque;
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use prodex_runtime_tuning::{
    runtime_websocket_dns_resolve_overflow_capacity_default,
    runtime_websocket_tcp_connect_overflow_capacity_default,
};

pub type RuntimeWebsocketLogToPath = fn(&Path, &str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeWebsocketLocalPressureKind {
    DnsResolveTimeout,
    DnsResolveExecutorOverflow,
    TcpConnectExecutorOverflow,
}

impl RuntimeWebsocketLocalPressureKind {
    pub fn as_str(self) -> &'static str {
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

pub fn runtime_websocket_local_pressure_io_error(
    kind: RuntimeWebsocketLocalPressureKind,
    message: impl Into<String>,
) -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        RuntimeWebsocketLocalPressureError::new(kind, message.into()),
    )
}

pub fn runtime_websocket_local_pressure_kind_from_io_error(
    err: &io::Error,
) -> Option<RuntimeWebsocketLocalPressureKind> {
    err.get_ref()
        .and_then(|source| source.downcast_ref::<RuntimeWebsocketLocalPressureError>())
        .map(|err| err.kind)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeWebsocketTcpConnectTaskKind {
    TcpConnect,
    DnsResolve,
}

impl RuntimeWebsocketTcpConnectTaskKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TcpConnect => "tcp_connect",
            Self::DnsResolve => "dns_resolve",
        }
    }

    pub fn worker_thread_prefix(self) -> &'static str {
        match self {
            Self::TcpConnect => "prodex-ws-connect",
            Self::DnsResolve => "prodex-ws-dns",
        }
    }

    pub fn dispatcher_thread_name(self) -> &'static str {
        match self {
            Self::TcpConnect => "prodex-ws-connect-dispatch",
            Self::DnsResolve => "prodex-ws-dns-dispatch",
        }
    }

    pub fn overflow_enqueue_event(self) -> &'static str {
        match self {
            Self::TcpConnect => "websocket_connect_overflow_enqueue",
            Self::DnsResolve => "websocket_dns_overflow_enqueue",
        }
    }

    pub fn overflow_dispatch_event(self) -> &'static str {
        match self {
            Self::TcpConnect => "websocket_connect_overflow_dispatch",
            Self::DnsResolve => "websocket_dns_overflow_dispatch",
        }
    }

    pub fn overflow_reject_event(self) -> &'static str {
        match self {
            Self::TcpConnect => "websocket_connect_overflow_reject",
            Self::DnsResolve => "websocket_dns_overflow_reject",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuntimeWebsocketTcpConnectOverflowSnapshot {
    pub pending_jobs: usize,
    pub max_pending_jobs: usize,
    pub total_enqueued: usize,
    pub total_dispatched: usize,
    pub total_rejected: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RuntimeWebsocketTcpConnectOverflowState {
    pending_jobs: usize,
    max_pending_jobs: usize,
    total_enqueued: usize,
    total_dispatched: usize,
    total_rejected: usize,
}

impl RuntimeWebsocketTcpConnectOverflowState {
    pub fn snapshot(&self) -> RuntimeWebsocketTcpConnectOverflowSnapshot {
        RuntimeWebsocketTcpConnectOverflowSnapshot {
            pending_jobs: self.pending_jobs,
            max_pending_jobs: self.max_pending_jobs,
            total_enqueued: self.total_enqueued,
            total_dispatched: self.total_dispatched,
            total_rejected: self.total_rejected,
        }
    }

    pub fn try_enqueue(
        &mut self,
        overflow_capacity: usize,
    ) -> Result<
        RuntimeWebsocketTcpConnectOverflowSnapshot,
        RuntimeWebsocketTcpConnectOverflowSnapshot,
    > {
        if self.pending_jobs >= overflow_capacity {
            self.total_rejected = self.total_rejected.saturating_add(1);
            return Err(self.snapshot());
        }

        self.pending_jobs = self.pending_jobs.saturating_add(1);
        self.total_enqueued = self.total_enqueued.saturating_add(1);
        self.max_pending_jobs = self.max_pending_jobs.max(self.pending_jobs);
        Ok(self.snapshot())
    }

    pub fn dispatch(&mut self) -> RuntimeWebsocketTcpConnectOverflowSnapshot {
        self.pending_jobs = self.pending_jobs.saturating_sub(1);
        self.total_dispatched = self.total_dispatched.saturating_add(1);
        self.snapshot()
    }
}

type RuntimeWebsocketTcpConnectJob = Box<dyn FnOnce() + Send + 'static>;

#[derive(Clone)]
struct RuntimeWebsocketTcpConnectTaskObservability {
    log_path: Option<PathBuf>,
    log_to_path: Option<RuntimeWebsocketLogToPath>,
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
        log_to_path: Option<RuntimeWebsocketLogToPath>,
    ) -> Self {
        Self {
            job,
            observability: RuntimeWebsocketTcpConnectTaskObservability {
                log_path: log_path.map(Path::to_path_buf),
                log_to_path,
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
        log_to_path: Option<RuntimeWebsocketLogToPath>,
    ) -> Self {
        Self {
            job,
            observability: RuntimeWebsocketTcpConnectTaskObservability {
                log_path: log_path.map(Path::to_path_buf),
                log_to_path,
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

#[derive(Default)]
struct RuntimeWebsocketTcpConnectOverflowQueueState {
    jobs: VecDeque<RuntimeWebsocketTcpConnectTask>,
    stats: RuntimeWebsocketTcpConnectOverflowState,
}

#[derive(Default)]
struct RuntimeWebsocketTcpConnectOverflowQueue {
    state: Mutex<RuntimeWebsocketTcpConnectOverflowQueueState>,
    work_available: Condvar,
}

impl RuntimeWebsocketTcpConnectOverflowQueue {
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
        let snapshot = state.stats.try_enqueue(overflow_capacity)?;
        state.jobs.push_back(task);
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
                return (task, state.stats.dispatch());
            }
            state = self
                .work_available
                .wait(state)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }

    fn snapshot(&self) -> RuntimeWebsocketTcpConnectOverflowSnapshot {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.stats.snapshot()
    }
}

enum RuntimeWebsocketTcpConnectExecutorMode {
    Bounded {
        sender: SyncSender<RuntimeWebsocketTcpConnectTask>,
        overflow: Arc<RuntimeWebsocketTcpConnectOverflowQueue>,
    },
    Inline,
}

pub struct RuntimeWebsocketTcpConnectExecutor {
    mode: RuntimeWebsocketTcpConnectExecutorMode,
    worker_count: usize,
    queue_capacity: usize,
    overflow_capacity: usize,
}

impl RuntimeWebsocketTcpConnectExecutor {
    pub fn new(worker_count: usize, queue_capacity: usize) -> Self {
        let worker_count = worker_count.max(1);
        let queue_capacity = queue_capacity.max(worker_count).max(1);
        let overflow_capacity =
            runtime_websocket_tcp_connect_overflow_capacity_default(worker_count, queue_capacity);
        Self::new_with_overflow_capacity(worker_count, queue_capacity, overflow_capacity)
    }

    pub fn new_with_overflow_capacity(
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

    pub fn new_with_spawn_outcome_for_test(
        worker_count: usize,
        queue_capacity: usize,
        overflow_capacity: usize,
        started_worker_count: usize,
        dispatcher_started: bool,
    ) -> Self {
        Self::new_for_kind_with_spawn_outcome_for_test(
            RuntimeWebsocketTcpConnectTaskKind::TcpConnect,
            worker_count,
            queue_capacity,
            overflow_capacity,
            started_worker_count,
            dispatcher_started,
        )
    }

    fn new_for_kind_with_spawn_outcome_for_test(
        task_kind: RuntimeWebsocketTcpConnectTaskKind,
        worker_count: usize,
        queue_capacity: usize,
        overflow_capacity: usize,
        started_worker_count: usize,
        dispatcher_started: bool,
    ) -> Self {
        let worker_prefix = task_kind.worker_thread_prefix();
        let dispatcher_name = task_kind.dispatcher_thread_name();
        Self::new_for_kind_with_spawner(
            task_kind,
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

    pub fn spawn<F>(&self, job: F) -> bool
    where
        F: FnOnce() + Send + 'static,
    {
        self.spawn_observed(None, None, None, None, job)
    }

    pub fn spawn_observed<F>(
        &self,
        log_path: Option<&Path>,
        request_id: Option<u64>,
        addr: Option<SocketAddr>,
        log_to_path: Option<RuntimeWebsocketLogToPath>,
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
            log_to_path,
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

    pub fn overflow_snapshot(&self) -> Option<RuntimeWebsocketTcpConnectOverflowSnapshot> {
        match &self.mode {
            RuntimeWebsocketTcpConnectExecutorMode::Bounded { overflow, .. } => {
                Some(overflow.snapshot())
            }
            RuntimeWebsocketTcpConnectExecutorMode::Inline => None,
        }
    }
}

pub struct RuntimeWebsocketDnsResolveExecutor {
    inner: RuntimeWebsocketTcpConnectExecutor,
}

impl RuntimeWebsocketDnsResolveExecutor {
    pub fn new(worker_count: usize, queue_capacity: usize) -> Self {
        let worker_count = worker_count.max(1);
        let queue_capacity = queue_capacity.max(worker_count).max(1);
        let overflow_capacity =
            runtime_websocket_dns_resolve_overflow_capacity_default(worker_count, queue_capacity);
        Self::new_with_overflow_capacity(worker_count, queue_capacity, overflow_capacity)
    }

    pub fn new_with_overflow_capacity(
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

    pub fn new_with_spawn_outcome_for_test(
        worker_count: usize,
        queue_capacity: usize,
        overflow_capacity: usize,
        started_worker_count: usize,
        dispatcher_started: bool,
    ) -> Self {
        Self {
            inner: RuntimeWebsocketTcpConnectExecutor::new_for_kind_with_spawn_outcome_for_test(
                RuntimeWebsocketTcpConnectTaskKind::DnsResolve,
                worker_count,
                queue_capacity,
                overflow_capacity,
                started_worker_count,
                dispatcher_started,
            ),
        }
    }

    fn spawn_observed<F>(
        &self,
        log_path: Option<&Path>,
        request_id: Option<u64>,
        host: &str,
        port: u16,
        log_to_path: Option<RuntimeWebsocketLogToPath>,
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
                log_to_path,
            ))
    }

    pub fn overflow_snapshot(&self) -> Option<RuntimeWebsocketTcpConnectOverflowSnapshot> {
        self.inner.overflow_snapshot()
    }
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
    let (Some(log_path), Some(log_to_path)) =
        (observability.log_path.as_ref(), observability.log_to_path)
    else {
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
    log_to_path(
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

#[derive(Clone, Copy)]
pub struct RuntimeWebsocketDnsResolveRequest<'a> {
    pub executor: &'a RuntimeWebsocketDnsResolveExecutor,
    pub log_path: Option<&'a Path>,
    pub request_id: Option<u64>,
    pub port: u16,
    pub timeout: Duration,
    pub log_to_path: Option<RuntimeWebsocketLogToPath>,
}

pub fn runtime_resolve_websocket_tcp_addrs_with_executor<F>(
    request: RuntimeWebsocketDnsResolveRequest<'_>,
    host: String,
    resolver: F,
) -> io::Result<Vec<SocketAddr>>
where
    F: FnOnce(String, u16) -> io::Result<Vec<SocketAddr>> + Send + 'static,
{
    let RuntimeWebsocketDnsResolveRequest {
        executor,
        log_path,
        request_id,
        port,
        timeout,
        log_to_path,
    } = request;
    let (sender, receiver) = mpsc::channel::<io::Result<Vec<SocketAddr>>>();
    let observed_host = host.clone();
    let accepted = executor.spawn_observed(
        log_path,
        request_id,
        &observed_host,
        port,
        log_to_path,
        move || {
            let result = resolver(host, port);
            let _ = sender.send(result);
        },
    );
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
                log_to_path,
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
    log_to_path: Option<RuntimeWebsocketLogToPath>,
) {
    let (Some(log_path), Some(log_to_path)) = (log_path, log_to_path) else {
        return;
    };
    let request = request_id
        .map(|request_id| format!("request={request_id} "))
        .unwrap_or_default();
    log_to_path(
        log_path,
        &format!(
            "{request}transport=websocket websocket_dns_resolve_timeout host={host} port={port} timeout_ms={}",
            timeout.as_millis()
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_pressure_error_round_trips_tagged_kind() {
        let err = runtime_websocket_local_pressure_io_error(
            RuntimeWebsocketLocalPressureKind::TcpConnectExecutorOverflow,
            "websocket TCP connect executor overflow for 127.0.0.1:443",
        );

        assert_eq!(err.kind(), io::ErrorKind::WouldBlock);
        assert_eq!(
            runtime_websocket_local_pressure_kind_from_io_error(&err),
            Some(RuntimeWebsocketLocalPressureKind::TcpConnectExecutorOverflow)
        );

        let plain = io::Error::new(io::ErrorKind::WouldBlock, "plain local backpressure");
        assert_eq!(
            runtime_websocket_local_pressure_kind_from_io_error(&plain),
            None
        );
    }

    #[test]
    fn task_kind_names_stay_stable_for_observability() {
        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::TcpConnect.as_str(),
            "tcp_connect"
        );
        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::TcpConnect.worker_thread_prefix(),
            "prodex-ws-connect"
        );
        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::TcpConnect.dispatcher_thread_name(),
            "prodex-ws-connect-dispatch"
        );
        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::TcpConnect.overflow_enqueue_event(),
            "websocket_connect_overflow_enqueue"
        );
        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::TcpConnect.overflow_dispatch_event(),
            "websocket_connect_overflow_dispatch"
        );
        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::TcpConnect.overflow_reject_event(),
            "websocket_connect_overflow_reject"
        );

        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::DnsResolve.as_str(),
            "dns_resolve"
        );
        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::DnsResolve.worker_thread_prefix(),
            "prodex-ws-dns"
        );
        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::DnsResolve.dispatcher_thread_name(),
            "prodex-ws-dns-dispatch"
        );
        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::DnsResolve.overflow_enqueue_event(),
            "websocket_dns_overflow_enqueue"
        );
        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::DnsResolve.overflow_dispatch_event(),
            "websocket_dns_overflow_dispatch"
        );
        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::DnsResolve.overflow_reject_event(),
            "websocket_dns_overflow_reject"
        );
    }

    #[test]
    fn overflow_state_tracks_bounded_queue_transitions() {
        let mut state = RuntimeWebsocketTcpConnectOverflowState::default();

        let first = state.try_enqueue(2).expect("first enqueue should fit");
        assert_eq!(
            first,
            RuntimeWebsocketTcpConnectOverflowSnapshot {
                pending_jobs: 1,
                max_pending_jobs: 1,
                total_enqueued: 1,
                total_dispatched: 0,
                total_rejected: 0,
            }
        );

        let second = state.try_enqueue(2).expect("second enqueue should fit");
        assert_eq!(second.pending_jobs, 2);
        assert_eq!(second.max_pending_jobs, 2);
        assert_eq!(second.total_enqueued, 2);

        let rejected = state
            .try_enqueue(2)
            .expect_err("full overflow state should reject");
        assert_eq!(rejected.pending_jobs, 2);
        assert_eq!(rejected.total_rejected, 1);

        let dispatched = state.dispatch();
        assert_eq!(dispatched.pending_jobs, 1);
        assert_eq!(dispatched.max_pending_jobs, 2);
        assert_eq!(dispatched.total_dispatched, 1);
    }
}
