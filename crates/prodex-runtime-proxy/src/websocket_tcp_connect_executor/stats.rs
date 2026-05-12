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
