#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeDoctorTuningLaneLimits {
    pub responses: usize,
    pub compact: usize,
    pub websocket: usize,
    pub standard: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeDoctorTuningSnapshot {
    pub active_request_limit: usize,
    pub lane_limits: RuntimeDoctorTuningLaneLimits,
    pub admission_wait_budget_ms: u64,
    pub pressure_admission_wait_budget_ms: u64,
    pub websocket_connect_worker_count: usize,
    pub websocket_connect_queue_capacity: usize,
    pub websocket_connect_overflow_capacity: usize,
    pub websocket_dns_worker_count: usize,
    pub websocket_dns_queue_capacity: usize,
    pub websocket_dns_overflow_capacity: usize,
    pub profile_inflight_soft_limit: usize,
    pub profile_inflight_hard_limit: usize,
}

impl Default for RuntimeDoctorTuningSnapshot {
    fn default() -> Self {
        Self {
            active_request_limit: 8,
            lane_limits: RuntimeDoctorTuningLaneLimits {
                responses: 6,
                compact: 1,
                websocket: 1,
                standard: 2,
            },
            admission_wait_budget_ms: 0,
            pressure_admission_wait_budget_ms: 250,
            websocket_connect_worker_count: 4,
            websocket_connect_queue_capacity: 8,
            websocket_connect_overflow_capacity: 16,
            websocket_dns_worker_count: 4,
            websocket_dns_queue_capacity: 8,
            websocket_dns_overflow_capacity: 16,
            profile_inflight_soft_limit: 2,
            profile_inflight_hard_limit: 4,
        }
    }
}
