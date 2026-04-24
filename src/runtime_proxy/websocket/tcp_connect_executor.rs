use crate::{
    RuntimeRotationProxyShared, RuntimeWebsocketTcpAttemptResult, runtime_proxy_log_to_path,
};
use std::collections::VecDeque;
use std::env;
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

type RuntimeWebsocketTcpConnectJob = Box<dyn FnOnce() + Send + 'static>;

#[derive(Clone)]
struct RuntimeWebsocketTcpConnectTaskObservability {
    log_path: Option<PathBuf>,
    request_id: Option<u64>,
    addr: Option<SocketAddr>,
}

struct RuntimeWebsocketTcpConnectTask {
    job: RuntimeWebsocketTcpConnectJob,
    observability: RuntimeWebsocketTcpConnectTaskObservability,
}

impl RuntimeWebsocketTcpConnectTask {
    fn new(
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
                addr,
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
}

#[derive(Default)]
struct RuntimeWebsocketTcpConnectOverflowState {
    jobs: VecDeque<RuntimeWebsocketTcpConnectTask>,
    total_enqueued: usize,
    total_dispatched: usize,
    max_pending_jobs: usize,
}

#[derive(Default)]
struct RuntimeWebsocketTcpConnectOverflowQueue {
    state: Mutex<RuntimeWebsocketTcpConnectOverflowState>,
    work_available: Condvar,
}

impl RuntimeWebsocketTcpConnectOverflowQueue {
    fn push(
        &self,
        task: RuntimeWebsocketTcpConnectTask,
    ) -> RuntimeWebsocketTcpConnectOverflowSnapshot {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.jobs.push_back(task);
        state.total_enqueued = state.total_enqueued.saturating_add(1);
        state.max_pending_jobs = state.max_pending_jobs.max(state.jobs.len());
        let snapshot = RuntimeWebsocketTcpConnectOverflowSnapshot {
            pending_jobs: state.jobs.len(),
            max_pending_jobs: state.max_pending_jobs,
            total_enqueued: state.total_enqueued,
            total_dispatched: state.total_dispatched,
        };
        self.work_available.notify_one();
        snapshot
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
                return (
                    task,
                    RuntimeWebsocketTcpConnectOverflowSnapshot {
                        pending_jobs: state.jobs.len(),
                        max_pending_jobs: state.max_pending_jobs,
                        total_enqueued: state.total_enqueued,
                        total_dispatched: state.total_dispatched,
                    },
                );
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
        RuntimeWebsocketTcpConnectOverflowSnapshot {
            pending_jobs: state.jobs.len(),
            max_pending_jobs: state.max_pending_jobs,
            total_enqueued: state.total_enqueued,
            total_dispatched: state.total_dispatched,
        }
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
}

impl RuntimeWebsocketTcpConnectExecutor {
    fn global() -> &'static Self {
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
        let (sender, receiver) =
            mpsc::sync_channel::<RuntimeWebsocketTcpConnectTask>(queue_capacity);
        let receiver = Arc::new(Mutex::new(receiver));
        let overflow = Arc::new(RuntimeWebsocketTcpConnectOverflowQueue::default());
        let mut started_workers = 0usize;

        for index in 0..worker_count {
            let receiver = Arc::clone(&receiver);
            let builder = thread::Builder::new().name(format!("prodex-ws-connect-{index}"));
            if builder
                .spawn(move || runtime_websocket_tcp_connect_worker_loop(receiver))
                .is_ok()
            {
                started_workers += 1;
            }
        }

        if started_workers == 0 {
            return Self {
                mode: RuntimeWebsocketTcpConnectExecutorMode::Inline,
                worker_count,
                queue_capacity,
            };
        }

        let overflow_sender = sender.clone();
        let overflow_queue = Arc::clone(&overflow);
        let dispatcher_started = thread::Builder::new()
            .name("prodex-ws-connect-dispatch".to_string())
            .spawn(move || {
                runtime_websocket_tcp_connect_dispatcher_loop(
                    overflow_queue,
                    overflow_sender,
                    worker_count,
                    queue_capacity,
                )
            })
            .is_ok();
        if !dispatcher_started {
            return Self {
                mode: RuntimeWebsocketTcpConnectExecutorMode::Inline,
                worker_count,
                queue_capacity,
            };
        }

        Self {
            mode: RuntimeWebsocketTcpConnectExecutorMode::Bounded { sender, overflow },
            worker_count,
            queue_capacity,
        }
    }

    #[cfg(test)]
    pub(super) fn spawn<F>(&self, job: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.spawn_observed(None, None, None, job);
    }

    pub(super) fn spawn_observed<F>(
        &self,
        log_path: Option<&Path>,
        request_id: Option<u64>,
        addr: Option<SocketAddr>,
        job: F,
    ) where
        F: FnOnce() + Send + 'static,
    {
        self.spawn_boxed(RuntimeWebsocketTcpConnectTask::new(
            Box::new(job),
            log_path,
            request_id,
            addr,
        ));
    }

    fn spawn_boxed(&self, task: RuntimeWebsocketTcpConnectTask) {
        match &self.mode {
            RuntimeWebsocketTcpConnectExecutorMode::Bounded { sender, overflow } => {
                if let Err(err) = sender.try_send(task) {
                    match err {
                        mpsc::TrySendError::Full(task) => {
                            let observability = task.observability.clone();
                            let snapshot = overflow.push(task);
                            runtime_websocket_tcp_connect_log_overflow_enqueue(
                                &observability,
                                self.worker_count,
                                self.queue_capacity,
                                snapshot,
                                "queue_full",
                            );
                        }
                        mpsc::TrySendError::Disconnected(task) => {
                            let observability = task.observability.clone();
                            let snapshot = overflow.push(task);
                            runtime_websocket_tcp_connect_log_overflow_enqueue(
                                &observability,
                                self.worker_count,
                                self.queue_capacity,
                                snapshot,
                                "executor_disconnected",
                            );
                        }
                    }
                }
            }
            RuntimeWebsocketTcpConnectExecutorMode::Inline => task.run(),
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

fn runtime_websocket_tcp_connect_env_usize(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn runtime_websocket_tcp_connect_dispatcher_loop(
    overflow: Arc<RuntimeWebsocketTcpConnectOverflowQueue>,
    sender: SyncSender<RuntimeWebsocketTcpConnectTask>,
    worker_count: usize,
    queue_capacity: usize,
) {
    loop {
        let (task, snapshot) = overflow.pop();
        let observability = task.observability.clone();
        match sender.send(task) {
            Ok(()) => runtime_websocket_tcp_connect_log_overflow_dispatch(
                &observability,
                worker_count,
                queue_capacity,
                snapshot,
                "queue_available",
            ),
            Err(err) => {
                runtime_websocket_tcp_connect_log_overflow_dispatch(
                    &err.0.observability,
                    worker_count,
                    queue_capacity,
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
    snapshot: RuntimeWebsocketTcpConnectOverflowSnapshot,
    reason: &str,
) {
    runtime_websocket_tcp_connect_log_overflow_event(
        observability,
        worker_count,
        queue_capacity,
        snapshot,
        "websocket_connect_overflow_enqueue",
        reason,
    );
}

fn runtime_websocket_tcp_connect_log_overflow_dispatch(
    observability: &RuntimeWebsocketTcpConnectTaskObservability,
    worker_count: usize,
    queue_capacity: usize,
    snapshot: RuntimeWebsocketTcpConnectOverflowSnapshot,
    reason: &str,
) {
    runtime_websocket_tcp_connect_log_overflow_event(
        observability,
        worker_count,
        queue_capacity,
        snapshot,
        "websocket_connect_overflow_dispatch",
        reason,
    );
}

fn runtime_websocket_tcp_connect_log_overflow_event(
    observability: &RuntimeWebsocketTcpConnectTaskObservability,
    worker_count: usize,
    queue_capacity: usize,
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
    let addr = observability
        .addr
        .map(|addr| format!(" addr={addr}"))
        .unwrap_or_default();
    runtime_proxy_log_to_path(
        log_path,
        &format!(
            "{request}transport=websocket {event} reason={reason}{addr} overflow_pending={} overflow_max_pending={} overflow_total_enqueued={} overflow_total_dispatched={} worker_count={worker_count} queue_capacity={queue_capacity}",
            snapshot.pending_jobs,
            snapshot.max_pending_jobs,
            snapshot.total_enqueued,
            snapshot.total_dispatched,
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
    RuntimeWebsocketTcpConnectExecutor::global().spawn_observed(
        Some(shared.log_path.as_path()),
        Some(request_id),
        Some(addr),
        move || {
            let result = TcpStream::connect_timeout(&addr, connect_timeout);
            let _ = sender.send(RuntimeWebsocketTcpAttemptResult { addr, result });
        },
    );
}
