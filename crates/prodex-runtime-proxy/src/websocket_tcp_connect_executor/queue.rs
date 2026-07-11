use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};

use super::stats::{
    RuntimeWebsocketTcpConnectOverflowSnapshot, RuntimeWebsocketTcpConnectOverflowState,
};
use super::task::RuntimeWebsocketTcpConnectTask;

#[derive(Default)]
struct RuntimeWebsocketTcpConnectOverflowQueueState {
    jobs: VecDeque<RuntimeWebsocketTcpConnectTask>,
    stats: RuntimeWebsocketTcpConnectOverflowState,
}

#[derive(Default)]
pub(super) struct RuntimeWebsocketTcpConnectOverflowQueue {
    state: Mutex<RuntimeWebsocketTcpConnectOverflowQueueState>,
    work_available: Condvar,
}

impl RuntimeWebsocketTcpConnectOverflowQueue {
    pub(super) fn push(
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

    pub(super) fn pop(
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

    pub(super) fn snapshot(&self) -> RuntimeWebsocketTcpConnectOverflowSnapshot {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.stats.snapshot()
    }
}
