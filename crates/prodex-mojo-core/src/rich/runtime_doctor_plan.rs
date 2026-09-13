use super::{ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address};
use crate::MojoError;

pub const RUNTIME_DOCTOR_PLAN_ABI_VERSION: i64 = 1;
pub const RUNTIME_DOCTOR_PLAN_MAX_COUNT: i64 = 1_000_000;
pub const RUNTIME_DOCTOR_PLAN_MAX_SCALAR: i64 = 4_000_000_000;
pub const RUNTIME_DOCTOR_PLAN_MAX_SUGGESTIONS: usize = 7;
pub const RUNTIME_DOCTOR_PLAN_MAX_SETTINGS: usize = 3;

pub const RUNTIME_DOCTOR_PLAN_OP_PREVIOUS_RESPONSE: i64 = 0;
pub const RUNTIME_DOCTOR_PLAN_OP_COMPACT_FINAL_FAILURE: i64 = 1;
pub const RUNTIME_DOCTOR_PLAN_OP_LANE_PRESSURE: i64 = 2;
pub const RUNTIME_DOCTOR_PLAN_OP_ACTIVE_PRESSURE: i64 = 3;
pub const RUNTIME_DOCTOR_PLAN_OP_PROFILE_INFLIGHT: i64 = 4;
pub const RUNTIME_DOCTOR_PLAN_OP_ROUTE_HEALTH: i64 = 5;
pub const RUNTIME_DOCTOR_PLAN_OP_WEBSOCKET_CONNECT: i64 = 6;
pub const RUNTIME_DOCTOR_PLAN_OP_PROFILE_AUTH: i64 = 7;
pub const RUNTIME_DOCTOR_PLAN_OP_PERSISTENCE: i64 = 8;
pub const RUNTIME_DOCTOR_PLAN_OP_SYNC_PROBE: i64 = 9;
pub const RUNTIME_DOCTOR_PLAN_OP_PROBE_REFRESH: i64 = 10;
pub const RUNTIME_DOCTOR_PLAN_OP_TRANSPORT: i64 = 11;
pub const RUNTIME_DOCTOR_PLAN_OP_QUOTA: i64 = 12;
pub const RUNTIME_DOCTOR_PLAN_OP_PRECOMMIT: i64 = 13;
pub const RUNTIME_DOCTOR_PLAN_OP_POLICY_SUGGESTIONS: i64 = 14;

pub const RUNTIME_DOCTOR_PLAN_LANE_MISSING: i64 = 0;
pub const RUNTIME_DOCTOR_PLAN_LANE_RESPONSES: i64 = 1;
pub const RUNTIME_DOCTOR_PLAN_LANE_COMPACT: i64 = 2;
pub const RUNTIME_DOCTOR_PLAN_LANE_WEBSOCKET: i64 = 3;
pub const RUNTIME_DOCTOR_PLAN_LANE_STANDARD: i64 = 4;
pub const RUNTIME_DOCTOR_PLAN_LANE_OTHER: i64 = 5;

pub const RUNTIME_DOCTOR_PLAN_COMPACT_REASON_UNKNOWN: i64 = 0;
pub const RUNTIME_DOCTOR_PLAN_COMPACT_REASON_QUOTA: i64 = 1;
pub const RUNTIME_DOCTOR_PLAN_COMPACT_REASON_OVERLOAD: i64 = 2;
pub const RUNTIME_DOCTOR_PLAN_COMPACT_REASON_TRANSPORT: i64 = 3;
pub const RUNTIME_DOCTOR_PLAN_COMPACT_REASON_INFLIGHT: i64 = 4;

pub const RUNTIME_DOCTOR_PLAN_NEXT_NONE: i64 = 0;
pub const RUNTIME_DOCTOR_PLAN_NEXT_CONTEXT_DEPENDENT: i64 = 1;
pub const RUNTIME_DOCTOR_PLAN_NEXT_COMPACT_PRESSURE: i64 = 2;
pub const RUNTIME_DOCTOR_PLAN_NEXT_COMPACT_QUOTA: i64 = 3;
pub const RUNTIME_DOCTOR_PLAN_NEXT_COMPACT_OVERLOAD: i64 = 4;
pub const RUNTIME_DOCTOR_PLAN_NEXT_COMPACT_TRANSPORT: i64 = 5;
pub const RUNTIME_DOCTOR_PLAN_NEXT_COMPACT_INFLIGHT: i64 = 6;
pub const RUNTIME_DOCTOR_PLAN_NEXT_LANE_RESPONSES: i64 = 7;
pub const RUNTIME_DOCTOR_PLAN_NEXT_PROFILE_HARD_LIMIT: i64 = 8;
pub const RUNTIME_DOCTOR_PLAN_NEXT_WEBSOCKET_REJECTED: i64 = 9;
pub const RUNTIME_DOCTOR_PLAN_NEXT_WEBSOCKET_DISPATCH: i64 = 10;
pub const RUNTIME_DOCTOR_PLAN_NEXT_WEBSOCKET_ENQUEUE: i64 = 11;
pub const RUNTIME_DOCTOR_PLAN_NEXT_AUTH_FAILED: i64 = 12;
pub const RUNTIME_DOCTOR_PLAN_NEXT_AUTH_RECOVERED: i64 = 13;
pub const RUNTIME_DOCTOR_PLAN_NEXT_SYNC_JOBS: i64 = 14;
pub const RUNTIME_DOCTOR_PLAN_NEXT_SYNC_PROFILES: i64 = 15;
pub const RUNTIME_DOCTOR_PLAN_NEXT_QUOTA_SYNC: i64 = 16;
pub const RUNTIME_DOCTOR_PLAN_NEXT_QUOTA_PROBE: i64 = 17;
pub const RUNTIME_DOCTOR_PLAN_NEXT_QUOTA_STALE: i64 = 18;
pub const RUNTIME_DOCTOR_PLAN_NEXT_PRECOMMIT_COMPACT: i64 = 19;
pub const RUNTIME_DOCTOR_PLAN_NEXT_PRECOMMIT_GENERAL: i64 = 20;
pub const RUNTIME_DOCTOR_PLAN_NEXT_ACTIVE_PRESSURE: i64 = 21;
pub const RUNTIME_DOCTOR_PLAN_NEXT_ROUTE_HEALTH: i64 = 22;
pub const RUNTIME_DOCTOR_PLAN_NEXT_PROBE_REFRESH: i64 = 23;

pub const RUNTIME_DOCTOR_PLAN_MARKER_NONE: i64 = 0;
pub const RUNTIME_DOCTOR_PLAN_MARKER_WEBSOCKET_REJECTED: i64 = 1;
pub const RUNTIME_DOCTOR_PLAN_MARKER_WEBSOCKET_REJECT: i64 = 2;
pub const RUNTIME_DOCTOR_PLAN_MARKER_WEBSOCKET_ENQUEUE: i64 = 3;
pub const RUNTIME_DOCTOR_PLAN_MARKER_WEBSOCKET_DISPATCH: i64 = 4;
pub const RUNTIME_DOCTOR_PLAN_MARKER_AUTH_FAILED: i64 = 5;
pub const RUNTIME_DOCTOR_PLAN_MARKER_AUTH_RECOVERED: i64 = 6;
pub const RUNTIME_DOCTOR_PLAN_MARKER_TRANSPORT_BACKOFF: i64 = 7;
pub const RUNTIME_DOCTOR_PLAN_MARKER_PROFILE_TRANSPORT_FAILURE: i64 = 8;
pub const RUNTIME_DOCTOR_PLAN_MARKER_STREAM_READ_ERROR: i64 = 9;
pub const RUNTIME_DOCTOR_PLAN_MARKER_CONNECT_TIMEOUT: i64 = 10;
pub const RUNTIME_DOCTOR_PLAN_MARKER_CONNECT_ERROR: i64 = 11;
pub const RUNTIME_DOCTOR_PLAN_MARKER_DNS_ERROR: i64 = 12;
pub const RUNTIME_DOCTOR_PLAN_MARKER_TLS_ERROR: i64 = 13;

pub const RUNTIME_DOCTOR_PLAN_SOURCE_NONE: i64 = 0;
pub const RUNTIME_DOCTOR_PLAN_SOURCE_STATE: i64 = 1;
pub const RUNTIME_DOCTOR_PLAN_SOURCE_JOURNAL: i64 = 2;
pub const RUNTIME_DOCTOR_PLAN_SOURCE_JOBS: i64 = 3;
pub const RUNTIME_DOCTOR_PLAN_SOURCE_PROFILES: i64 = 4;
pub const RUNTIME_DOCTOR_PLAN_SOURCE_QUOTA: i64 = 5;
pub const RUNTIME_DOCTOR_PLAN_SOURCE_RESPONSES_SKIP: i64 = 6;
pub const RUNTIME_DOCTOR_PLAN_SOURCE_WEBSOCKET_SKIP: i64 = 7;

pub const RUNTIME_DOCTOR_PLAN_SUGGESTION_LANE: i64 = 1;
pub const RUNTIME_DOCTOR_PLAN_SUGGESTION_ACTIVE: i64 = 2;
pub const RUNTIME_DOCTOR_PLAN_SUGGESTION_PROFILE_INFLIGHT: i64 = 3;
pub const RUNTIME_DOCTOR_PLAN_SUGGESTION_WEBSOCKET_CONNECT: i64 = 4;
pub const RUNTIME_DOCTOR_PLAN_SUGGESTION_WEBSOCKET_DNS: i64 = 5;
pub const RUNTIME_DOCTOR_PLAN_SUGGESTION_PERSISTENCE: i64 = 6;
pub const RUNTIME_DOCTOR_PLAN_SUGGESTION_ROUTE_HEALTH: i64 = 7;

pub const RUNTIME_DOCTOR_PLAN_SEVERITY_MEDIUM: i64 = 1;
pub const RUNTIME_DOCTOR_PLAN_SEVERITY_LOW: i64 = 2;

pub const RUNTIME_DOCTOR_PLAN_SETTING_RESPONSES_ACTIVE: i64 = 1;
pub const RUNTIME_DOCTOR_PLAN_SETTING_COMPACT_ACTIVE: i64 = 2;
pub const RUNTIME_DOCTOR_PLAN_SETTING_WEBSOCKET_ACTIVE: i64 = 3;
pub const RUNTIME_DOCTOR_PLAN_SETTING_STANDARD_ACTIVE: i64 = 4;
pub const RUNTIME_DOCTOR_PLAN_SETTING_ACTIVE_REQUEST: i64 = 5;
pub const RUNTIME_DOCTOR_PLAN_SETTING_PROFILE_SOFT: i64 = 6;
pub const RUNTIME_DOCTOR_PLAN_SETTING_PROFILE_HARD: i64 = 7;
pub const RUNTIME_DOCTOR_PLAN_SETTING_CONNECT_WORKERS: i64 = 8;
pub const RUNTIME_DOCTOR_PLAN_SETTING_CONNECT_QUEUE: i64 = 9;
pub const RUNTIME_DOCTOR_PLAN_SETTING_CONNECT_OVERFLOW: i64 = 10;
pub const RUNTIME_DOCTOR_PLAN_SETTING_DNS_WORKERS: i64 = 11;
pub const RUNTIME_DOCTOR_PLAN_SETTING_DNS_QUEUE: i64 = 12;
pub const RUNTIME_DOCTOR_PLAN_SETTING_DNS_OVERFLOW: i64 = 13;
pub const RUNTIME_DOCTOR_PLAN_SETTING_PRESSURE_WAIT: i64 = 14;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeDoctorPlanMarkerCounts {
    pub lane: i64,
    pub active: i64,
    pub profile_inflight: i64,
    pub profile_health: i64,
    pub websocket_rejected: i64,
    pub websocket_reject: i64,
    pub websocket_enqueue: i64,
    pub websocket_dispatch: i64,
    pub auth_failed: i64,
    pub auth_recovered: i64,
    pub state_backpressure: i64,
    pub journal_backpressure: i64,
    pub sync_probe_skip: i64,
    pub probe_backpressure: i64,
    pub transport_backoff: i64,
    pub profile_transport_failure: i64,
    pub stream_read_error: i64,
    pub upstream_connect_timeout: i64,
    pub upstream_connect_error: i64,
    pub upstream_connect_dns_error: i64,
    pub upstream_tls_handshake_error: i64,
    pub quota_blocked: i64,
    pub responses_pre_send_skip: i64,
    pub websocket_pre_send_skip: i64,
    pub precommit_budget: i64,
    pub compact_precommit_budget: i64,
    pub compact_exit_precommit_budget: i64,
    pub compact_candidate: i64,
    pub compact_exit_candidate: i64,
    pub dns_reject: i64,
    pub dns_enqueue: i64,
    pub dns_dispatch: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeDoctorPlanObservations {
    /// Optional scalar observations use `-1` for absent values.
    pub lane_active: i64,
    pub lane_limit: i64,
    pub active_active: i64,
    pub active_limit: i64,
    pub inflight_hard_limit: i64,
    pub websocket_pending: i64,
    pub websocket_max_pending: i64,
    pub websocket_worker_count: i64,
    pub websocket_queue_capacity: i64,
    pub dns_pending: i64,
    pub dns_max_pending: i64,
    pub dns_worker_count: i64,
    pub dns_queue_capacity: i64,
    pub state_backlog: i64,
    pub journal_backlog: i64,
    pub probe_backlog: i64,
    pub sync_cold_start_jobs: i64,
    pub sync_cold_start_profiles: i64,
}

impl Default for RuntimeDoctorPlanObservations {
    fn default() -> Self {
        Self {
            lane_active: -1,
            lane_limit: -1,
            active_active: -1,
            active_limit: -1,
            inflight_hard_limit: -1,
            websocket_pending: -1,
            websocket_max_pending: -1,
            websocket_worker_count: -1,
            websocket_queue_capacity: -1,
            dns_pending: -1,
            dns_max_pending: -1,
            dns_worker_count: -1,
            dns_queue_capacity: -1,
            state_backlog: -1,
            journal_backlog: -1,
            probe_backlog: -1,
            sync_cold_start_jobs: -1,
            sync_cold_start_profiles: -1,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeDoctorPlanTuning {
    pub active_request_limit: i64,
    pub responses_active_limit: i64,
    pub compact_active_limit: i64,
    pub websocket_active_limit: i64,
    pub standard_active_limit: i64,
    pub admission_wait_budget_ms: i64,
    pub pressure_admission_wait_budget_ms: i64,
    pub websocket_connect_worker_count: i64,
    pub websocket_connect_queue_capacity: i64,
    pub websocket_connect_overflow_capacity: i64,
    pub websocket_dns_worker_count: i64,
    pub websocket_dns_queue_capacity: i64,
    pub websocket_dns_overflow_capacity: i64,
    pub profile_inflight_soft_limit: i64,
    pub profile_inflight_hard_limit: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeDoctorPlanInput {
    pub operation: i64,
    pub lane: i64,
    pub compact_exit_pressure: i64,
    pub compact_reason: i64,
    pub quota_stale_risk: i64,
    pub context_dependent: i64,
    pub counts: RuntimeDoctorPlanMarkerCounts,
    pub observations: RuntimeDoctorPlanObservations,
    pub tuning: RuntimeDoctorPlanTuning,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeDoctorPlanOutput {
    pub abi_version: i64,
    pub next_step: i64,
    pub detail: i64,
    pub selected_marker: i64,
    pub selected_source: i64,
    pub suggestion_count: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeDoctorPlan {
    pub abi_version: i64,
    pub next_step: i64,
    pub detail: i64,
    pub selected_marker: i64,
    pub selected_source: i64,
    pub suggestion_count: i64,
    pub suggestion_ids: [i64; RUNTIME_DOCTOR_PLAN_MAX_SUGGESTIONS],
    pub suggestion_severities: [i64; RUNTIME_DOCTOR_PLAN_MAX_SUGGESTIONS],
    pub suggestion_markers: [i64; RUNTIME_DOCTOR_PLAN_MAX_SUGGESTIONS],
    pub suggestion_counts: [i64; RUNTIME_DOCTOR_PLAN_MAX_SUGGESTIONS],
    pub suggestion_setting_counts: [i64; RUNTIME_DOCTOR_PLAN_MAX_SUGGESTIONS],
    pub setting_keys: [i64; RUNTIME_DOCTOR_PLAN_MAX_SUGGESTIONS * RUNTIME_DOCTOR_PLAN_MAX_SETTINGS],
    pub setting_current_values:
        [i64; RUNTIME_DOCTOR_PLAN_MAX_SUGGESTIONS * RUNTIME_DOCTOR_PLAN_MAX_SETTINGS],
    pub setting_suggested_values:
        [i64; RUNTIME_DOCTOR_PLAN_MAX_SUGGESTIONS * RUNTIME_DOCTOR_PLAN_MAX_SETTINGS],
}

const _: () = {
    assert!(std::mem::size_of::<RuntimeDoctorPlanMarkerCounts>() == 32 * 8);
    assert!(std::mem::size_of::<RuntimeDoctorPlanObservations>() == 18 * 8);
    assert!(std::mem::size_of::<RuntimeDoctorPlanTuning>() == 15 * 8);
    assert!(std::mem::size_of::<RuntimeDoctorPlanInput>() == 71 * 8);
    assert!(std::mem::size_of::<RuntimeDoctorPlanOutput>() == 6 * 8);
};

unsafe extern "C" {
    fn prodex_mojo_rich_runtime_doctor_plan_v1(
        abi_version: i64,
        input: u64,
        output: u64,
        suggestion_ids: u64,
        suggestion_severities: u64,
        suggestion_markers: u64,
        suggestion_counts: u64,
        suggestion_setting_counts: u64,
        setting_keys: u64,
        setting_current_values: u64,
        setting_suggested_values: u64,
    ) -> i64;
}

fn status_error(status: i64) -> MojoError {
    match status {
        1 => MojoError::InvalidInput,
        4 => MojoError::AbiMismatch,
        _ => MojoError::InvalidOutput,
    }
}

fn input_is_valid(input: &RuntimeDoctorPlanInput) -> bool {
    (0..=RUNTIME_DOCTOR_PLAN_OP_POLICY_SUGGESTIONS).contains(&input.operation)
        && (RUNTIME_DOCTOR_PLAN_LANE_MISSING..=RUNTIME_DOCTOR_PLAN_LANE_OTHER).contains(&input.lane)
        && (0..=1).contains(&input.compact_exit_pressure)
        && (RUNTIME_DOCTOR_PLAN_COMPACT_REASON_UNKNOWN
            ..=RUNTIME_DOCTOR_PLAN_COMPACT_REASON_INFLIGHT)
            .contains(&input.compact_reason)
        && (0..=1).contains(&input.quota_stale_risk)
        && (0..=1).contains(&input.context_dependent)
        && [
            input.counts.lane,
            input.counts.active,
            input.counts.profile_inflight,
            input.counts.profile_health,
            input.counts.websocket_rejected,
            input.counts.websocket_reject,
            input.counts.websocket_enqueue,
            input.counts.websocket_dispatch,
            input.counts.auth_failed,
            input.counts.auth_recovered,
            input.counts.state_backpressure,
            input.counts.journal_backpressure,
            input.counts.sync_probe_skip,
            input.counts.probe_backpressure,
            input.counts.transport_backoff,
            input.counts.profile_transport_failure,
            input.counts.stream_read_error,
            input.counts.upstream_connect_timeout,
            input.counts.upstream_connect_error,
            input.counts.upstream_connect_dns_error,
            input.counts.upstream_tls_handshake_error,
            input.counts.quota_blocked,
            input.counts.responses_pre_send_skip,
            input.counts.websocket_pre_send_skip,
            input.counts.precommit_budget,
            input.counts.compact_precommit_budget,
            input.counts.compact_exit_precommit_budget,
            input.counts.compact_candidate,
            input.counts.compact_exit_candidate,
            input.counts.dns_reject,
            input.counts.dns_enqueue,
            input.counts.dns_dispatch,
        ]
        .iter()
        .all(|value| (0..=RUNTIME_DOCTOR_PLAN_MAX_COUNT).contains(value))
        && [
            input.observations.lane_active,
            input.observations.lane_limit,
            input.observations.active_active,
            input.observations.active_limit,
            input.observations.inflight_hard_limit,
            input.observations.websocket_pending,
            input.observations.websocket_max_pending,
            input.observations.websocket_worker_count,
            input.observations.websocket_queue_capacity,
            input.observations.dns_pending,
            input.observations.dns_max_pending,
            input.observations.dns_worker_count,
            input.observations.dns_queue_capacity,
            input.observations.state_backlog,
            input.observations.journal_backlog,
            input.observations.probe_backlog,
            input.observations.sync_cold_start_jobs,
            input.observations.sync_cold_start_profiles,
        ]
        .iter()
        .all(|value| (-1..=RUNTIME_DOCTOR_PLAN_MAX_SCALAR).contains(value))
        && [
            input.tuning.active_request_limit,
            input.tuning.responses_active_limit,
            input.tuning.compact_active_limit,
            input.tuning.websocket_active_limit,
            input.tuning.standard_active_limit,
            input.tuning.admission_wait_budget_ms,
            input.tuning.pressure_admission_wait_budget_ms,
            input.tuning.websocket_connect_worker_count,
            input.tuning.websocket_connect_queue_capacity,
            input.tuning.websocket_connect_overflow_capacity,
            input.tuning.websocket_dns_worker_count,
            input.tuning.websocket_dns_queue_capacity,
            input.tuning.websocket_dns_overflow_capacity,
            input.tuning.profile_inflight_soft_limit,
            input.tuning.profile_inflight_hard_limit,
        ]
        .iter()
        .all(|value| (0..=RUNTIME_DOCTOR_PLAN_MAX_SCALAR).contains(value))
}

fn output_is_valid(output: &RuntimeDoctorPlan) -> bool {
    if output.abi_version != RUNTIME_DOCTOR_PLAN_ABI_VERSION
        || !(RUNTIME_DOCTOR_PLAN_NEXT_NONE..=RUNTIME_DOCTOR_PLAN_NEXT_PROBE_REFRESH)
            .contains(&output.next_step)
        || !(RUNTIME_DOCTOR_PLAN_NEXT_NONE..=RUNTIME_DOCTOR_PLAN_NEXT_PROBE_REFRESH)
            .contains(&output.detail)
        || !(RUNTIME_DOCTOR_PLAN_MARKER_NONE..=RUNTIME_DOCTOR_PLAN_MARKER_TLS_ERROR)
            .contains(&output.selected_marker)
        || !(RUNTIME_DOCTOR_PLAN_SOURCE_NONE..=RUNTIME_DOCTOR_PLAN_SOURCE_WEBSOCKET_SKIP)
            .contains(&output.selected_source)
        || !(0..=RUNTIME_DOCTOR_PLAN_MAX_SUGGESTIONS as i64).contains(&output.suggestion_count)
    {
        return false;
    }
    let count = output.suggestion_count as usize;
    for index in 0..count {
        if !(RUNTIME_DOCTOR_PLAN_SUGGESTION_LANE..=RUNTIME_DOCTOR_PLAN_SUGGESTION_ROUTE_HEALTH)
            .contains(&output.suggestion_ids[index])
            || !(RUNTIME_DOCTOR_PLAN_SEVERITY_MEDIUM..=RUNTIME_DOCTOR_PLAN_SEVERITY_LOW)
                .contains(&output.suggestion_severities[index])
            || !(RUNTIME_DOCTOR_PLAN_MARKER_NONE..=RUNTIME_DOCTOR_PLAN_MARKER_WEBSOCKET_DISPATCH)
                .contains(&output.suggestion_markers[index])
            || !(0..=RUNTIME_DOCTOR_PLAN_MAX_COUNT).contains(&output.suggestion_counts[index])
            || !(0..=RUNTIME_DOCTOR_PLAN_MAX_SETTINGS as i64)
                .contains(&output.suggestion_setting_counts[index])
        {
            return false;
        }
        let settings = output.suggestion_setting_counts[index] as usize;
        for setting in 0..settings {
            let flat = index * RUNTIME_DOCTOR_PLAN_MAX_SETTINGS + setting;
            if !(RUNTIME_DOCTOR_PLAN_SETTING_RESPONSES_ACTIVE
                ..=RUNTIME_DOCTOR_PLAN_SETTING_PRESSURE_WAIT)
                .contains(&output.setting_keys[flat])
                || !(0..=RUNTIME_DOCTOR_PLAN_MAX_SCALAR)
                    .contains(&output.setting_current_values[flat])
                || !(1..=RUNTIME_DOCTOR_PLAN_MAX_SCALAR)
                    .contains(&output.setting_suggested_values[flat])
            {
                return false;
            }
        }
    }
    output.suggestion_count == 0 || output.next_step == RUNTIME_DOCTOR_PLAN_NEXT_NONE
}

/// Run the bounded runtime-doctor diagnosis and policy planning kernel.
pub fn runtime_doctor_plan(input: RuntimeDoctorPlanInput) -> Result<RuntimeDoctorPlan, MojoError> {
    ensure_rich_abi()?;
    if !input_is_valid(&input) {
        return Err(MojoError::InvalidInput);
    }
    let mut output = RuntimeDoctorPlanOutput::default();
    let mut suggestion_ids = [0_i64; RUNTIME_DOCTOR_PLAN_MAX_SUGGESTIONS];
    let mut suggestion_severities = [0_i64; RUNTIME_DOCTOR_PLAN_MAX_SUGGESTIONS];
    let mut suggestion_markers = [0_i64; RUNTIME_DOCTOR_PLAN_MAX_SUGGESTIONS];
    let mut suggestion_counts = [0_i64; RUNTIME_DOCTOR_PLAN_MAX_SUGGESTIONS];
    let mut suggestion_setting_counts = [0_i64; RUNTIME_DOCTOR_PLAN_MAX_SUGGESTIONS];
    let mut setting_keys =
        [0_i64; RUNTIME_DOCTOR_PLAN_MAX_SUGGESTIONS * RUNTIME_DOCTOR_PLAN_MAX_SETTINGS];
    let mut setting_current_values =
        [0_i64; RUNTIME_DOCTOR_PLAN_MAX_SUGGESTIONS * RUNTIME_DOCTOR_PLAN_MAX_SETTINGS];
    let mut setting_suggested_values =
        [0_i64; RUNTIME_DOCTOR_PLAN_MAX_SUGGESTIONS * RUNTIME_DOCTOR_PLAN_MAX_SETTINGS];
    let status = unsafe {
        prodex_mojo_rich_runtime_doctor_plan_v1(
            RUNTIME_DOCTOR_PLAN_ABI_VERSION,
            mojo_pointer_address(&input),
            mojo_mut_pointer_address(&mut output),
            mojo_mut_pointer_address(suggestion_ids.as_mut_ptr()),
            mojo_mut_pointer_address(suggestion_severities.as_mut_ptr()),
            mojo_mut_pointer_address(suggestion_markers.as_mut_ptr()),
            mojo_mut_pointer_address(suggestion_counts.as_mut_ptr()),
            mojo_mut_pointer_address(suggestion_setting_counts.as_mut_ptr()),
            mojo_mut_pointer_address(setting_keys.as_mut_ptr()),
            mojo_mut_pointer_address(setting_current_values.as_mut_ptr()),
            mojo_mut_pointer_address(setting_suggested_values.as_mut_ptr()),
        )
    };
    if status != 0 {
        return Err(status_error(status));
    }
    let output = RuntimeDoctorPlan {
        abi_version: output.abi_version,
        next_step: output.next_step,
        detail: output.detail,
        selected_marker: output.selected_marker,
        selected_source: output.selected_source,
        suggestion_count: output.suggestion_count,
        suggestion_ids,
        suggestion_severities,
        suggestion_markers,
        suggestion_counts,
        suggestion_setting_counts,
        setting_keys,
        setting_current_values,
        setting_suggested_values,
    };
    output_is_valid(&output)
        .then_some(output)
        .ok_or(MojoError::InvalidOutput)
}

pub fn runtime_doctor_plan_self_test() -> bool {
    let mut input = RuntimeDoctorPlanInput {
        operation: RUNTIME_DOCTOR_PLAN_OP_POLICY_SUGGESTIONS,
        lane: RUNTIME_DOCTOR_PLAN_LANE_COMPACT,
        ..RuntimeDoctorPlanInput::default()
    };
    input.counts.lane = 1;
    input.observations.lane_active = 6;
    input.observations.lane_limit = 6;
    input.tuning.active_request_limit = 8;
    input.tuning.compact_active_limit = 1;
    runtime_doctor_plan(input).is_ok_and(|plan| {
        plan.suggestion_count == 1
            && plan.suggestion_ids[0] == RUNTIME_DOCTOR_PLAN_SUGGESTION_LANE
            && plan.suggestion_setting_counts[0] >= 1
            && plan.setting_suggested_values[0] > plan.setting_current_values[0]
    })
}

pub const RUNTIME_DOCTOR_SUMMARY_PLAN_ABI_VERSION: i64 = 1;
pub const RUNTIME_DOCTOR_SUMMARY_MARKER_COUNT: usize = 128;

pub const RUNTIME_DOCTOR_PRESSURE_LOW: i64 = 0;
pub const RUNTIME_DOCTOR_PRESSURE_ELEVATED: i64 = 1;
pub const RUNTIME_DOCTOR_PRESSURE_ACTIVE: i64 = 2;
pub const RUNTIME_DOCTOR_PRESSURE_STALE_RISK: i64 = 3;

pub const RUNTIME_DOCTOR_DIAGNOSIS_NONE: i64 = 0;
pub const RUNTIME_DOCTOR_DIAGNOSIS_NO_POINTER: i64 = 1;
pub const RUNTIME_DOCTOR_DIAGNOSIS_NO_LOG: i64 = 2;
pub const RUNTIME_DOCTOR_DIAGNOSIS_EMPTY_LOG: i64 = 3;
pub const RUNTIME_DOCTOR_DIAGNOSIS_PROXY_OVERLOAD_BACKOFF: i64 = 4;
pub const RUNTIME_DOCTOR_DIAGNOSIS_LANE_PRESSURE: i64 = 5;
pub const RUNTIME_DOCTOR_DIAGNOSIS_ACTIVE_PRESSURE: i64 = 6;
pub const RUNTIME_DOCTOR_DIAGNOSIS_QUEUE_OVERLOAD: i64 = 7;
pub const RUNTIME_DOCTOR_DIAGNOSIS_CIRCUIT_OPEN: i64 = 8;
pub const RUNTIME_DOCTOR_DIAGNOSIS_CIRCUIT_HALF_OPEN: i64 = 9;
pub const RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_FRAME_TIMEOUT: i64 = 10;
pub const RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_HOLD_TIMEOUT: i64 = 11;
pub const RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_DNS_TIMEOUT: i64 = 12;
pub const RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_DNS_REJECT: i64 = 13;
pub const RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_DNS_OVERFLOW: i64 = 14;
pub const RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_LOCAL_PRESSURE: i64 = 15;
pub const RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_CONNECT_REJECT: i64 = 16;
pub const RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_CONNECT_ENQUEUE: i64 = 17;
pub const RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_CONNECT_DISPATCH: i64 = 18;
pub const RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_TUNNEL_FAILURE: i64 = 19;
pub const RUNTIME_DOCTOR_DIAGNOSIS_PROFILE_INFLIGHT: i64 = 20;
pub const RUNTIME_DOCTOR_DIAGNOSIS_PROFILE_HEALTH: i64 = 21;
pub const RUNTIME_DOCTOR_DIAGNOSIS_PROFILE_BAD_PAIRING: i64 = 22;
pub const RUNTIME_DOCTOR_DIAGNOSIS_PROFILE_AUTH_FAILURE: i64 = 23;
pub const RUNTIME_DOCTOR_DIAGNOSIS_PROVIDER_AUTH_FAILURE: i64 = 24;
pub const RUNTIME_DOCTOR_DIAGNOSIS_COMPACT_FALLBACK_BLOCKED: i64 = 25;
pub const RUNTIME_DOCTOR_DIAGNOSIS_COMPACT_PRESSURE_SHED: i64 = 26;
pub const RUNTIME_DOCTOR_DIAGNOSIS_CHAIN_DEAD: i64 = 27;
pub const RUNTIME_DOCTOR_DIAGNOSIS_STALE_CONTINUATION: i64 = 28;
pub const RUNTIME_DOCTOR_DIAGNOSIS_CHAIN_RETRIED: i64 = 29;
pub const RUNTIME_DOCTOR_DIAGNOSIS_PREVIOUS_RESPONSE_BLOCKED: i64 = 30;
pub const RUNTIME_DOCTOR_DIAGNOSIS_PREVIOUS_RESPONSE_FALLBACK: i64 = 31;
pub const RUNTIME_DOCTOR_DIAGNOSIS_PREVIOUS_RESPONSE_NOT_FOUND: i64 = 32;
pub const RUNTIME_DOCTOR_DIAGNOSIS_COMPACT_FINAL_FAILURE: i64 = 33;
pub const RUNTIME_DOCTOR_DIAGNOSIS_COMPACT_EXIT_PATHS: i64 = 34;
pub const RUNTIME_DOCTOR_DIAGNOSIS_COMPAT_WARNING: i64 = 35;
pub const RUNTIME_DOCTOR_DIAGNOSIS_DEAD_CONTINUATIONS: i64 = 36;
pub const RUNTIME_DOCTOR_DIAGNOSIS_SUSPECT_CONTINUATIONS: i64 = 37;
pub const RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_WATCHDOG: i64 = 38;
pub const RUNTIME_DOCTOR_DIAGNOSIS_AUTH_RECOVERED: i64 = 39;
pub const RUNTIME_DOCTOR_DIAGNOSIS_PRECOMMIT_BUDGET: i64 = 40;
pub const RUNTIME_DOCTOR_DIAGNOSIS_QUOTA_HARDENING: i64 = 41;
pub const RUNTIME_DOCTOR_DIAGNOSIS_GEMINI_QUOTA_RETRY: i64 = 42;
pub const RUNTIME_DOCTOR_DIAGNOSIS_PROVIDER_MODEL_FALLBACK: i64 = 43;
pub const RUNTIME_DOCTOR_DIAGNOSIS_GEMINI_STREAM_RETRY: i64 = 44;
pub const RUNTIME_DOCTOR_DIAGNOSIS_GEMINI_COMPACT_FALLBACK: i64 = 45;
pub const RUNTIME_DOCTOR_DIAGNOSIS_GEMINI_LIVE_ERROR: i64 = 46;
pub const RUNTIME_DOCTOR_DIAGNOSIS_STREAM_READ: i64 = 47;
pub const RUNTIME_DOCTOR_DIAGNOSIS_LOCAL_WRITER: i64 = 48;
pub const RUNTIME_DOCTOR_DIAGNOSIS_UPSTREAM_CONNECT: i64 = 49;
pub const RUNTIME_DOCTOR_DIAGNOSIS_STATE_SAVE: i64 = 50;
pub const RUNTIME_DOCTOR_DIAGNOSIS_PERSISTENCE: i64 = 51;
pub const RUNTIME_DOCTOR_DIAGNOSIS_SYNC_PROBE: i64 = 52;
pub const RUNTIME_DOCTOR_DIAGNOSIS_PROBE_BACKPRESSURE: i64 = 53;
pub const RUNTIME_DOCTOR_DIAGNOSIS_DEGRADED_ROUTES: i64 = 54;
pub const RUNTIME_DOCTOR_DIAGNOSIS_ORPHAN_DIRS: i64 = 55;
pub const RUNTIME_DOCTOR_DIAGNOSIS_PROBE_ERROR: i64 = 56;
pub const RUNTIME_DOCTOR_DIAGNOSIS_PROBE_ACTIVITY: i64 = 57;
pub const RUNTIME_DOCTOR_DIAGNOSIS_WRITER_STALL: i64 = 58;
pub const RUNTIME_DOCTOR_DIAGNOSIS_BROKER_MISMATCH: i64 = 59;
pub const RUNTIME_DOCTOR_DIAGNOSIS_BINARY_MISMATCH: i64 = 60;
pub const RUNTIME_DOCTOR_DIAGNOSIS_SELECTION: i64 = 61;
pub const RUNTIME_DOCTOR_DIAGNOSIS_NO_RECENT_FAILURE: i64 = 62;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeDoctorSummaryPlanInput {
    pub marker_counts: [i64; RUNTIME_DOCTOR_SUMMARY_MARKER_COUNT],
    pub line_count: i64,
    pub pointer_exists: i64,
    pub log_exists: i64,
    pub stale_persisted_usage_snapshots: i64,
    pub orphan_managed_dirs: i64,
    pub startup_audit_risk: i64,
    pub persisted_dead_continuations: i64,
    pub suspect_continuations: i64,
    pub degraded_routes: i64,
    pub runtime_broker_mismatch: i64,
    pub prodex_binary_mismatch: i64,
    pub persisted_quota_snapshot_risk: i64,
}

impl Default for RuntimeDoctorSummaryPlanInput {
    fn default() -> Self {
        Self {
            marker_counts: [0; RUNTIME_DOCTOR_SUMMARY_MARKER_COUNT],
            line_count: 0,
            pointer_exists: 0,
            log_exists: 0,
            stale_persisted_usage_snapshots: 0,
            orphan_managed_dirs: 0,
            startup_audit_risk: 0,
            persisted_dead_continuations: 0,
            suspect_continuations: 0,
            degraded_routes: 0,
            runtime_broker_mismatch: 0,
            prodex_binary_mismatch: 0,
            persisted_quota_snapshot_risk: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeDoctorSummaryPlan {
    pub abi_version: i64,
    pub selection_pressure: i64,
    pub transport_pressure: i64,
    pub persistence_pressure: i64,
    pub quota_freshness_pressure: i64,
    pub startup_audit_pressure: i64,
    pub diagnosis_kind: i64,
}

const _: () = {
    assert!(std::mem::size_of::<RuntimeDoctorSummaryPlanInput>() == 140 * 8);
    assert!(std::mem::size_of::<RuntimeDoctorSummaryPlan>() == 7 * 8);
};

pub const RUNTIME_DOCTOR_STATE_PLAN_ABI_VERSION: i64 = 1;
pub const RUNTIME_DOCTOR_STATE_OP_QUOTA: i64 = 0;
pub const RUNTIME_DOCTOR_STATE_OP_SCORE: i64 = 1;
pub const RUNTIME_DOCTOR_STATE_OP_CIRCUIT: i64 = 2;
pub const RUNTIME_DOCTOR_STATE_ROUTE_RESPONSES: i64 = 0;
pub const RUNTIME_DOCTOR_STATE_ROUTE_WEBSOCKET: i64 = 1;
pub const RUNTIME_DOCTOR_STATE_ROUTE_COMPACT: i64 = 2;
pub const RUNTIME_DOCTOR_STATE_ROUTE_STANDARD: i64 = 3;
pub const RUNTIME_DOCTOR_STATE_STATUS_READY: i64 = 0;
pub const RUNTIME_DOCTOR_STATE_STATUS_THIN: i64 = 1;
pub const RUNTIME_DOCTOR_STATE_STATUS_CRITICAL: i64 = 2;
pub const RUNTIME_DOCTOR_STATE_STATUS_EXHAUSTED: i64 = 3;
pub const RUNTIME_DOCTOR_STATE_STATUS_UNKNOWN: i64 = 4;
pub const RUNTIME_DOCTOR_STATE_CIRCUIT_CLOSED: i64 = 0;
pub const RUNTIME_DOCTOR_STATE_CIRCUIT_HALF_OPEN: i64 = 1;
pub const RUNTIME_DOCTOR_STATE_CIRCUIT_OPEN: i64 = 2;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeDoctorStatePlanInput {
    pub operation: i64,
    pub route_kind: i64,
    pub now: i64,
    pub checked_at: i64,
    pub five_hour_status: i64,
    pub five_hour_reset_at: i64,
    pub weekly_status: i64,
    pub weekly_reset_at: i64,
    pub stale_grace_seconds: i64,
    pub score: i64,
    pub updated_at: i64,
    pub decay_seconds: i64,
    pub circuit_until: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeDoctorStatePlan {
    pub abi_version: i64,
    pub freshness: i64,
    pub five_hour_status: i64,
    pub weekly_status: i64,
    pub route_band: i64,
    pub effective_score: i64,
    pub circuit_state: i64,
}

const _: () = {
    assert!(std::mem::size_of::<RuntimeDoctorStatePlanInput>() == 13 * 8);
    assert!(std::mem::size_of::<RuntimeDoctorStatePlan>() == 7 * 8);
};

unsafe extern "C" {
    fn prodex_mojo_rich_runtime_doctor_summary_plan_v1(
        abi_version: i64,
        input: u64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_rich_runtime_doctor_state_plan_v1(
        abi_version: i64,
        input: u64,
        output: u64,
    ) -> i64;
}

fn summary_plan_status_error(status: i64) -> MojoError {
    match status {
        1 => MojoError::InvalidInput,
        4 => MojoError::AbiMismatch,
        _ => MojoError::InvalidOutput,
    }
}

fn summary_plan_input_is_valid(input: &RuntimeDoctorSummaryPlanInput) -> bool {
    input
        .marker_counts
        .iter()
        .all(|value| (0..=RUNTIME_DOCTOR_PLAN_MAX_COUNT).contains(value))
        && (0..=RUNTIME_DOCTOR_PLAN_MAX_COUNT).contains(&input.line_count)
        && [
            input.pointer_exists,
            input.log_exists,
            input.orphan_managed_dirs,
            input.startup_audit_risk,
            input.runtime_broker_mismatch,
            input.prodex_binary_mismatch,
            input.persisted_quota_snapshot_risk,
        ]
        .iter()
        .all(|value| (0..=1).contains(value))
        && [
            input.stale_persisted_usage_snapshots,
            input.persisted_dead_continuations,
            input.suspect_continuations,
            input.degraded_routes,
        ]
        .iter()
        .all(|value| (0..=RUNTIME_DOCTOR_PLAN_MAX_COUNT).contains(value))
}

fn summary_plan_output_is_valid(output: &RuntimeDoctorSummaryPlan) -> bool {
    output.abi_version == RUNTIME_DOCTOR_SUMMARY_PLAN_ABI_VERSION
        && (RUNTIME_DOCTOR_PRESSURE_LOW..=RUNTIME_DOCTOR_PRESSURE_STALE_RISK)
            .contains(&output.selection_pressure)
        && (RUNTIME_DOCTOR_PRESSURE_LOW..=RUNTIME_DOCTOR_PRESSURE_STALE_RISK)
            .contains(&output.transport_pressure)
        && (RUNTIME_DOCTOR_PRESSURE_LOW..=RUNTIME_DOCTOR_PRESSURE_STALE_RISK)
            .contains(&output.persistence_pressure)
        && (RUNTIME_DOCTOR_PRESSURE_LOW..=RUNTIME_DOCTOR_PRESSURE_STALE_RISK)
            .contains(&output.quota_freshness_pressure)
        && (RUNTIME_DOCTOR_PRESSURE_LOW..=RUNTIME_DOCTOR_PRESSURE_STALE_RISK)
            .contains(&output.startup_audit_pressure)
        && (RUNTIME_DOCTOR_DIAGNOSIS_NONE..=RUNTIME_DOCTOR_DIAGNOSIS_NO_RECENT_FAILURE)
            .contains(&output.diagnosis_kind)
}

/// Run the bounded sanitized runtime-doctor summary and diagnosis kernel.
pub fn runtime_doctor_summary_plan(
    input: RuntimeDoctorSummaryPlanInput,
) -> Result<RuntimeDoctorSummaryPlan, MojoError> {
    ensure_rich_abi()?;
    if !summary_plan_input_is_valid(&input) {
        return Err(MojoError::InvalidInput);
    }
    let mut output = RuntimeDoctorSummaryPlan::default();
    let status = unsafe {
        prodex_mojo_rich_runtime_doctor_summary_plan_v1(
            RUNTIME_DOCTOR_SUMMARY_PLAN_ABI_VERSION,
            mojo_pointer_address(&input),
            mojo_mut_pointer_address(&mut output),
        )
    };
    if status != 0 {
        return Err(summary_plan_status_error(status));
    }
    summary_plan_output_is_valid(&output)
        .then_some(output)
        .ok_or(MojoError::InvalidOutput)
}

fn state_plan_input_is_valid(input: &RuntimeDoctorStatePlanInput) -> bool {
    (RUNTIME_DOCTOR_STATE_OP_QUOTA..=RUNTIME_DOCTOR_STATE_OP_CIRCUIT).contains(&input.operation)
        && (RUNTIME_DOCTOR_STATE_ROUTE_RESPONSES..=RUNTIME_DOCTOR_STATE_ROUTE_STANDARD)
            .contains(&input.route_kind)
        && (RUNTIME_DOCTOR_STATE_STATUS_READY..=RUNTIME_DOCTOR_STATE_STATUS_UNKNOWN)
            .contains(&input.five_hour_status)
        && (RUNTIME_DOCTOR_STATE_STATUS_READY..=RUNTIME_DOCTOR_STATE_STATUS_UNKNOWN)
            .contains(&input.weekly_status)
        && input.stale_grace_seconds >= 0
        && input.score >= 0
        && input.circuit_until >= -1
        && (input.operation != RUNTIME_DOCTOR_STATE_OP_SCORE || input.decay_seconds > 0)
}

fn state_plan_output_is_valid(output: &RuntimeDoctorStatePlan) -> bool {
    output.abi_version == RUNTIME_DOCTOR_STATE_PLAN_ABI_VERSION
        && (RUNTIME_DOCTOR_STATE_STATUS_READY..=RUNTIME_DOCTOR_STATE_STATUS_UNKNOWN)
            .contains(&output.freshness)
        && (RUNTIME_DOCTOR_STATE_STATUS_READY..=RUNTIME_DOCTOR_STATE_STATUS_UNKNOWN)
            .contains(&output.five_hour_status)
        && (RUNTIME_DOCTOR_STATE_STATUS_READY..=RUNTIME_DOCTOR_STATE_STATUS_UNKNOWN)
            .contains(&output.weekly_status)
        && (RUNTIME_DOCTOR_STATE_STATUS_READY..=RUNTIME_DOCTOR_STATE_STATUS_UNKNOWN)
            .contains(&output.route_band)
        && output.effective_score >= 0
        && (RUNTIME_DOCTOR_STATE_CIRCUIT_CLOSED..=RUNTIME_DOCTOR_STATE_CIRCUIT_OPEN)
            .contains(&output.circuit_state)
}

/// Run the bounded runtime-doctor quota, decay, and circuit state kernel.
pub fn runtime_doctor_state_plan(
    input: RuntimeDoctorStatePlanInput,
) -> Result<RuntimeDoctorStatePlan, MojoError> {
    ensure_rich_abi()?;
    if !state_plan_input_is_valid(&input) {
        return Err(MojoError::InvalidInput);
    }
    let mut output = RuntimeDoctorStatePlan::default();
    let status = unsafe {
        prodex_mojo_rich_runtime_doctor_state_plan_v1(
            RUNTIME_DOCTOR_STATE_PLAN_ABI_VERSION,
            mojo_pointer_address(&input),
            mojo_mut_pointer_address(&mut output),
        )
    };
    if status != 0 {
        return Err(summary_plan_status_error(status));
    }
    state_plan_output_is_valid(&output)
        .then_some(output)
        .ok_or(MojoError::InvalidOutput)
}

pub fn runtime_doctor_summary_plan_self_test() -> bool {
    let mut input = RuntimeDoctorSummaryPlanInput {
        pointer_exists: 1,
        log_exists: 1,
        line_count: 1,
        ..RuntimeDoctorSummaryPlanInput::default()
    };
    input.marker_counts[1] = 1;
    input.marker_counts[84] = 1;
    runtime_doctor_summary_plan(input).is_ok_and(|plan| {
        plan.selection_pressure == RUNTIME_DOCTOR_PRESSURE_ELEVATED
            && plan.diagnosis_kind == RUNTIME_DOCTOR_DIAGNOSIS_LANE_PRESSURE
    })
}

pub fn runtime_doctor_state_plan_self_test() -> bool {
    let quota = runtime_doctor_state_plan(RuntimeDoctorStatePlanInput {
        operation: RUNTIME_DOCTOR_STATE_OP_QUOTA,
        route_kind: RUNTIME_DOCTOR_STATE_ROUTE_RESPONSES,
        now: 100,
        checked_at: 90,
        five_hour_status: RUNTIME_DOCTOR_STATE_STATUS_READY,
        five_hour_reset_at: i64::MAX,
        weekly_status: RUNTIME_DOCTOR_STATE_STATUS_THIN,
        weekly_reset_at: i64::MAX,
        stale_grace_seconds: 300,
        ..RuntimeDoctorStatePlanInput::default()
    })
    .is_ok_and(|plan| {
        plan.freshness == RUNTIME_DOCTOR_STATE_STATUS_READY
            && plan.route_band == RUNTIME_DOCTOR_STATE_STATUS_THIN
    });
    let score = runtime_doctor_state_plan(RuntimeDoctorStatePlanInput {
        operation: RUNTIME_DOCTOR_STATE_OP_SCORE,
        now: 110,
        updated_at: 100,
        score: 5,
        decay_seconds: 5,
        ..RuntimeDoctorStatePlanInput::default()
    })
    .is_ok_and(|plan| plan.effective_score == 3);
    quota && score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_self_test_passes() {
        assert!(runtime_doctor_plan_self_test());
    }

    #[test]
    fn summary_and_state_plan_self_tests_pass() {
        assert!(runtime_doctor_summary_plan_self_test());
        assert!(runtime_doctor_state_plan_self_test());
    }
}
