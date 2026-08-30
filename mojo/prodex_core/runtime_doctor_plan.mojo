from std.memory import Pointer


comptime RUNTIME_DOCTOR_PLAN_ABI_VERSION: Int64 = 1
comptime RUNTIME_DOCTOR_PLAN_MAX_COUNT: Int64 = 1_000_000
comptime RUNTIME_DOCTOR_PLAN_MAX_SCALAR: Int64 = 4_000_000_000
comptime RUNTIME_DOCTOR_PLAN_MAX_SUGGESTIONS: Int64 = 7
comptime RUNTIME_DOCTOR_PLAN_MAX_SETTINGS: Int64 = 3

comptime PLAN_OP_PREVIOUS_RESPONSE: Int64 = 0
comptime PLAN_OP_COMPACT_FINAL_FAILURE: Int64 = 1
comptime PLAN_OP_LANE_PRESSURE: Int64 = 2
comptime PLAN_OP_ACTIVE_PRESSURE: Int64 = 3
comptime PLAN_OP_PROFILE_INFLIGHT: Int64 = 4
comptime PLAN_OP_ROUTE_HEALTH: Int64 = 5
comptime PLAN_OP_WEBSOCKET_CONNECT: Int64 = 6
comptime PLAN_OP_PROFILE_AUTH: Int64 = 7
comptime PLAN_OP_PERSISTENCE: Int64 = 8
comptime PLAN_OP_SYNC_PROBE: Int64 = 9
comptime PLAN_OP_PROBE_REFRESH: Int64 = 10
comptime PLAN_OP_TRANSPORT: Int64 = 11
comptime PLAN_OP_QUOTA: Int64 = 12
comptime PLAN_OP_PRECOMMIT: Int64 = 13
comptime PLAN_OP_POLICY_SUGGESTIONS: Int64 = 14

comptime PLAN_LANE_MISSING: Int64 = 0
comptime PLAN_LANE_RESPONSES: Int64 = 1
comptime PLAN_LANE_COMPACT: Int64 = 2
comptime PLAN_LANE_WEBSOCKET: Int64 = 3
comptime PLAN_LANE_STANDARD: Int64 = 4
comptime PLAN_LANE_OTHER: Int64 = 5

comptime PLAN_COMPACT_REASON_UNKNOWN: Int64 = 0
comptime PLAN_COMPACT_REASON_QUOTA: Int64 = 1
comptime PLAN_COMPACT_REASON_OVERLOAD: Int64 = 2
comptime PLAN_COMPACT_REASON_TRANSPORT: Int64 = 3
comptime PLAN_COMPACT_REASON_INFLIGHT: Int64 = 4

comptime PLAN_NEXT_NONE: Int64 = 0
comptime PLAN_NEXT_CONTEXT_DEPENDENT: Int64 = 1
comptime PLAN_NEXT_COMPACT_PRESSURE: Int64 = 2
comptime PLAN_NEXT_COMPACT_QUOTA: Int64 = 3
comptime PLAN_NEXT_COMPACT_OVERLOAD: Int64 = 4
comptime PLAN_NEXT_COMPACT_TRANSPORT: Int64 = 5
comptime PLAN_NEXT_COMPACT_INFLIGHT: Int64 = 6
comptime PLAN_NEXT_LANE_RESPONSES: Int64 = 7
comptime PLAN_NEXT_PROFILE_HARD_LIMIT: Int64 = 8
comptime PLAN_NEXT_WEBSOCKET_REJECTED: Int64 = 9
comptime PLAN_NEXT_WEBSOCKET_DISPATCH: Int64 = 10
comptime PLAN_NEXT_WEBSOCKET_ENQUEUE: Int64 = 11
comptime PLAN_NEXT_AUTH_FAILED: Int64 = 12
comptime PLAN_NEXT_AUTH_RECOVERED: Int64 = 13
comptime PLAN_NEXT_SYNC_JOBS: Int64 = 14
comptime PLAN_NEXT_SYNC_PROFILES: Int64 = 15
comptime PLAN_NEXT_QUOTA_SYNC: Int64 = 16
comptime PLAN_NEXT_QUOTA_PROBE: Int64 = 17
comptime PLAN_NEXT_QUOTA_STALE: Int64 = 18
comptime PLAN_NEXT_PRECOMMIT_COMPACT: Int64 = 19
comptime PLAN_NEXT_PRECOMMIT_GENERAL: Int64 = 20
comptime PLAN_NEXT_ACTIVE_PRESSURE: Int64 = 21
comptime PLAN_NEXT_ROUTE_HEALTH: Int64 = 22
comptime PLAN_NEXT_PROBE_REFRESH: Int64 = 23

comptime PLAN_MARKER_NONE: Int64 = 0
comptime PLAN_MARKER_WEBSOCKET_REJECTED: Int64 = 1
comptime PLAN_MARKER_WEBSOCKET_REJECT: Int64 = 2
comptime PLAN_MARKER_WEBSOCKET_ENQUEUE: Int64 = 3
comptime PLAN_MARKER_WEBSOCKET_DISPATCH: Int64 = 4
comptime PLAN_MARKER_AUTH_FAILED: Int64 = 5
comptime PLAN_MARKER_AUTH_RECOVERED: Int64 = 6
comptime PLAN_MARKER_TRANSPORT_BACKOFF: Int64 = 7
comptime PLAN_MARKER_PROFILE_TRANSPORT_FAILURE: Int64 = 8
comptime PLAN_MARKER_STREAM_READ_ERROR: Int64 = 9
comptime PLAN_MARKER_CONNECT_TIMEOUT: Int64 = 10
comptime PLAN_MARKER_CONNECT_ERROR: Int64 = 11
comptime PLAN_MARKER_DNS_ERROR: Int64 = 12
comptime PLAN_MARKER_TLS_ERROR: Int64 = 13

comptime PLAN_SOURCE_NONE: Int64 = 0
comptime PLAN_SOURCE_STATE: Int64 = 1
comptime PLAN_SOURCE_JOURNAL: Int64 = 2
comptime PLAN_SOURCE_JOBS: Int64 = 3
comptime PLAN_SOURCE_PROFILES: Int64 = 4
comptime PLAN_SOURCE_QUOTA: Int64 = 5
comptime PLAN_SOURCE_RESPONSES_SKIP: Int64 = 6
comptime PLAN_SOURCE_WEBSOCKET_SKIP: Int64 = 7

comptime PLAN_SUGGESTION_LANE: Int64 = 1
comptime PLAN_SUGGESTION_ACTIVE: Int64 = 2
comptime PLAN_SUGGESTION_PROFILE_INFLIGHT: Int64 = 3
comptime PLAN_SUGGESTION_WEBSOCKET_CONNECT: Int64 = 4
comptime PLAN_SUGGESTION_WEBSOCKET_DNS: Int64 = 5
comptime PLAN_SUGGESTION_PERSISTENCE: Int64 = 6
comptime PLAN_SUGGESTION_ROUTE_HEALTH: Int64 = 7

comptime PLAN_SEVERITY_MEDIUM: Int64 = 1
comptime PLAN_SEVERITY_LOW: Int64 = 2

comptime PLAN_SETTING_RESPONSES_ACTIVE: Int64 = 1
comptime PLAN_SETTING_COMPACT_ACTIVE: Int64 = 2
comptime PLAN_SETTING_WEBSOCKET_ACTIVE: Int64 = 3
comptime PLAN_SETTING_STANDARD_ACTIVE: Int64 = 4
comptime PLAN_SETTING_ACTIVE_REQUEST: Int64 = 5
comptime PLAN_SETTING_PROFILE_SOFT: Int64 = 6
comptime PLAN_SETTING_PROFILE_HARD: Int64 = 7
comptime PLAN_SETTING_CONNECT_WORKERS: Int64 = 8
comptime PLAN_SETTING_CONNECT_QUEUE: Int64 = 9
comptime PLAN_SETTING_CONNECT_OVERFLOW: Int64 = 10
comptime PLAN_SETTING_DNS_WORKERS: Int64 = 11
comptime PLAN_SETTING_DNS_QUEUE: Int64 = 12
comptime PLAN_SETTING_DNS_OVERFLOW: Int64 = 13
comptime PLAN_SETTING_PRESSURE_WAIT: Int64 = 14


@fieldwise_init
struct ProdexRuntimeDoctorPlanMarkerCounts(Copyable):
    var lane: Int64
    var active: Int64
    var profile_inflight: Int64
    var profile_health: Int64
    var websocket_rejected: Int64
    var websocket_reject: Int64
    var websocket_enqueue: Int64
    var websocket_dispatch: Int64
    var auth_failed: Int64
    var auth_recovered: Int64
    var state_backpressure: Int64
    var journal_backpressure: Int64
    var sync_probe_skip: Int64
    var probe_backpressure: Int64
    var transport_backoff: Int64
    var profile_transport_failure: Int64
    var stream_read_error: Int64
    var upstream_connect_timeout: Int64
    var upstream_connect_error: Int64
    var upstream_connect_dns_error: Int64
    var upstream_tls_handshake_error: Int64
    var quota_blocked: Int64
    var responses_pre_send_skip: Int64
    var websocket_pre_send_skip: Int64
    var precommit_budget: Int64
    var compact_precommit_budget: Int64
    var compact_exit_precommit_budget: Int64
    var compact_candidate: Int64
    var compact_exit_candidate: Int64
    var dns_reject: Int64
    var dns_enqueue: Int64
    var dns_dispatch: Int64


@fieldwise_init
struct ProdexRuntimeDoctorPlanObservations(Copyable):
    # -1 means absent. Remaining values are bounded non-sensitive scalars.
    var lane_active: Int64
    var lane_limit: Int64
    var active_active: Int64
    var active_limit: Int64
    var inflight_hard_limit: Int64
    var websocket_pending: Int64
    var websocket_max_pending: Int64
    var websocket_worker_count: Int64
    var websocket_queue_capacity: Int64
    var dns_pending: Int64
    var dns_max_pending: Int64
    var dns_worker_count: Int64
    var dns_queue_capacity: Int64
    var state_backlog: Int64
    var journal_backlog: Int64
    var probe_backlog: Int64
    var sync_cold_start_jobs: Int64
    var sync_cold_start_profiles: Int64


@fieldwise_init
struct ProdexRuntimeDoctorPlanTuning(Copyable):
    var active_request_limit: Int64
    var responses_active_limit: Int64
    var compact_active_limit: Int64
    var websocket_active_limit: Int64
    var standard_active_limit: Int64
    var admission_wait_budget_ms: Int64
    var pressure_admission_wait_budget_ms: Int64
    var websocket_connect_worker_count: Int64
    var websocket_connect_queue_capacity: Int64
    var websocket_connect_overflow_capacity: Int64
    var websocket_dns_worker_count: Int64
    var websocket_dns_queue_capacity: Int64
    var websocket_dns_overflow_capacity: Int64
    var profile_inflight_soft_limit: Int64
    var profile_inflight_hard_limit: Int64


@fieldwise_init
struct ProdexRuntimeDoctorPlanInput(Copyable):
    var operation: Int64
    var lane: Int64
    var compact_exit_pressure: Int64
    var compact_reason: Int64
    var quota_stale_risk: Int64
    var context_dependent: Int64
    var counts: ProdexRuntimeDoctorPlanMarkerCounts
    var observations: ProdexRuntimeDoctorPlanObservations
    var tuning: ProdexRuntimeDoctorPlanTuning


@fieldwise_init
struct ProdexRuntimeDoctorPlan(Copyable):
    var abi_version: Int64
    var next_step: Int64
    var detail: Int64
    var selected_marker: Int64
    var selected_source: Int64
    var suggestion_count: Int64


@fieldwise_init
struct ProdexRuntimeDoctorPlanBuffers(Copyable):
    var suggestion_ids: UInt
    var suggestion_severities: UInt
    var suggestion_markers: UInt
    var suggestion_counts: UInt
    var suggestion_setting_counts: UInt
    var setting_keys: UInt
    var setting_current_values: UInt
    var setting_suggested_values: UInt


def runtime_doctor_count_valid(value: Int64) -> Bool:
    return value >= 0 and value <= RUNTIME_DOCTOR_PLAN_MAX_COUNT


def runtime_doctor_scalar_valid(value: Int64) -> Bool:
    return value >= 0 and value <= RUNTIME_DOCTOR_PLAN_MAX_SCALAR


def runtime_doctor_optional_scalar_valid(value: Int64) -> Bool:
    return value >= -1 and value <= RUNTIME_DOCTOR_PLAN_MAX_SCALAR


def runtime_doctor_counts_valid(counts: ProdexRuntimeDoctorPlanMarkerCounts) -> Bool:
    return (
        runtime_doctor_count_valid(counts.lane)
        and runtime_doctor_count_valid(counts.active)
        and runtime_doctor_count_valid(counts.profile_inflight)
        and runtime_doctor_count_valid(counts.profile_health)
        and runtime_doctor_count_valid(counts.websocket_rejected)
        and runtime_doctor_count_valid(counts.websocket_reject)
        and runtime_doctor_count_valid(counts.websocket_enqueue)
        and runtime_doctor_count_valid(counts.websocket_dispatch)
        and runtime_doctor_count_valid(counts.auth_failed)
        and runtime_doctor_count_valid(counts.auth_recovered)
        and runtime_doctor_count_valid(counts.state_backpressure)
        and runtime_doctor_count_valid(counts.journal_backpressure)
        and runtime_doctor_count_valid(counts.sync_probe_skip)
        and runtime_doctor_count_valid(counts.probe_backpressure)
        and runtime_doctor_count_valid(counts.transport_backoff)
        and runtime_doctor_count_valid(counts.profile_transport_failure)
        and runtime_doctor_count_valid(counts.stream_read_error)
        and runtime_doctor_count_valid(counts.upstream_connect_timeout)
        and runtime_doctor_count_valid(counts.upstream_connect_error)
        and runtime_doctor_count_valid(counts.upstream_connect_dns_error)
        and runtime_doctor_count_valid(counts.upstream_tls_handshake_error)
        and runtime_doctor_count_valid(counts.quota_blocked)
        and runtime_doctor_count_valid(counts.responses_pre_send_skip)
        and runtime_doctor_count_valid(counts.websocket_pre_send_skip)
        and runtime_doctor_count_valid(counts.precommit_budget)
        and runtime_doctor_count_valid(counts.compact_precommit_budget)
        and runtime_doctor_count_valid(counts.compact_exit_precommit_budget)
        and runtime_doctor_count_valid(counts.compact_candidate)
        and runtime_doctor_count_valid(counts.compact_exit_candidate)
        and runtime_doctor_count_valid(counts.dns_reject)
        and runtime_doctor_count_valid(counts.dns_enqueue)
        and runtime_doctor_count_valid(counts.dns_dispatch)
    )


def runtime_doctor_observations_valid(
    observations: ProdexRuntimeDoctorPlanObservations,
) -> Bool:
    return (
        runtime_doctor_optional_scalar_valid(observations.lane_active)
        and runtime_doctor_optional_scalar_valid(observations.lane_limit)
        and runtime_doctor_optional_scalar_valid(observations.active_active)
        and runtime_doctor_optional_scalar_valid(observations.active_limit)
        and runtime_doctor_optional_scalar_valid(observations.inflight_hard_limit)
        and runtime_doctor_optional_scalar_valid(observations.websocket_pending)
        and runtime_doctor_optional_scalar_valid(observations.websocket_max_pending)
        and runtime_doctor_optional_scalar_valid(observations.websocket_worker_count)
        and runtime_doctor_optional_scalar_valid(observations.websocket_queue_capacity)
        and runtime_doctor_optional_scalar_valid(observations.dns_pending)
        and runtime_doctor_optional_scalar_valid(observations.dns_max_pending)
        and runtime_doctor_optional_scalar_valid(observations.dns_worker_count)
        and runtime_doctor_optional_scalar_valid(observations.dns_queue_capacity)
        and runtime_doctor_optional_scalar_valid(observations.state_backlog)
        and runtime_doctor_optional_scalar_valid(observations.journal_backlog)
        and runtime_doctor_optional_scalar_valid(observations.probe_backlog)
        and runtime_doctor_optional_scalar_valid(observations.sync_cold_start_jobs)
        and runtime_doctor_optional_scalar_valid(observations.sync_cold_start_profiles)
    )


def runtime_doctor_tuning_valid(tuning: ProdexRuntimeDoctorPlanTuning) -> Bool:
    return (
        runtime_doctor_scalar_valid(tuning.active_request_limit)
        and runtime_doctor_scalar_valid(tuning.responses_active_limit)
        and runtime_doctor_scalar_valid(tuning.compact_active_limit)
        and runtime_doctor_scalar_valid(tuning.websocket_active_limit)
        and runtime_doctor_scalar_valid(tuning.standard_active_limit)
        and runtime_doctor_scalar_valid(tuning.admission_wait_budget_ms)
        and runtime_doctor_scalar_valid(tuning.pressure_admission_wait_budget_ms)
        and runtime_doctor_scalar_valid(tuning.websocket_connect_worker_count)
        and runtime_doctor_scalar_valid(tuning.websocket_connect_queue_capacity)
        and runtime_doctor_scalar_valid(tuning.websocket_connect_overflow_capacity)
        and runtime_doctor_scalar_valid(tuning.websocket_dns_worker_count)
        and runtime_doctor_scalar_valid(tuning.websocket_dns_queue_capacity)
        and runtime_doctor_scalar_valid(tuning.websocket_dns_overflow_capacity)
        and runtime_doctor_scalar_valid(tuning.profile_inflight_soft_limit)
        and runtime_doctor_scalar_valid(tuning.profile_inflight_hard_limit)
    )


def runtime_doctor_input_valid(input: ProdexRuntimeDoctorPlanInput) -> Bool:
    return (
        input.operation >= PLAN_OP_PREVIOUS_RESPONSE
        and input.operation <= PLAN_OP_POLICY_SUGGESTIONS
        and input.lane >= PLAN_LANE_MISSING
        and input.lane <= PLAN_LANE_OTHER
        and input.compact_exit_pressure >= 0
        and input.compact_exit_pressure <= 1
        and input.compact_reason >= PLAN_COMPACT_REASON_UNKNOWN
        and input.compact_reason <= PLAN_COMPACT_REASON_INFLIGHT
        and input.quota_stale_risk >= 0
        and input.quota_stale_risk <= 1
        and input.context_dependent >= 0
        and input.context_dependent <= 1
        and runtime_doctor_counts_valid(input.counts)
        and runtime_doctor_observations_valid(input.observations)
        and runtime_doctor_tuning_valid(input.tuning)
    )


def runtime_doctor_optional(value: Int64, fallback: Int64) -> Int64:
    if value >= 0:
        return value
    return fallback


def runtime_doctor_max(left: Int64, right: Int64) -> Int64:
    if left >= right:
        return left
    return right


def runtime_doctor_scale_up(value: Int64) -> Int64:
    var base = value
    if base < 1:
        base = 1
    var increment = base / 2
    if base % 2 == 1:
        increment += 1
    if base > RUNTIME_DOCTOR_PLAN_MAX_SCALAR - increment:
        return RUNTIME_DOCTOR_PLAN_MAX_SCALAR
    var target = base + increment
    if target <= base:
        target = base + 1
    return target


def runtime_doctor_scale_down(value: Int64) -> Int64:
    var target = (value * 3 + 3) / 4
    if target < 1:
        target = 1
    return target


def runtime_doctor_selected_websocket_marker(
    counts: ProdexRuntimeDoctorPlanMarkerCounts,
) -> Int64:
    if counts.websocket_rejected > 0:
        return PLAN_MARKER_WEBSOCKET_REJECTED
    if counts.websocket_reject > 0:
        return PLAN_MARKER_WEBSOCKET_REJECT
    if counts.websocket_enqueue > 0:
        return PLAN_MARKER_WEBSOCKET_ENQUEUE
    return PLAN_MARKER_WEBSOCKET_DISPATCH


def runtime_doctor_selected_transport_marker(
    counts: ProdexRuntimeDoctorPlanMarkerCounts,
) -> Int64:
    if counts.transport_backoff > 0:
        return PLAN_MARKER_TRANSPORT_BACKOFF
    if counts.profile_transport_failure > 0:
        return PLAN_MARKER_PROFILE_TRANSPORT_FAILURE
    if counts.stream_read_error > 0:
        return PLAN_MARKER_STREAM_READ_ERROR
    if counts.upstream_connect_timeout > 0:
        return PLAN_MARKER_CONNECT_TIMEOUT
    if counts.upstream_connect_error > 0:
        return PLAN_MARKER_CONNECT_ERROR
    if counts.upstream_connect_dns_error > 0:
        return PLAN_MARKER_DNS_ERROR
    return PLAN_MARKER_TLS_ERROR


def runtime_doctor_reset_output(
    output: Pointer[mut=True, ProdexRuntimeDoctorPlan, _],
) -> None:
    output[].abi_version = RUNTIME_DOCTOR_PLAN_ABI_VERSION
    output[].next_step = PLAN_NEXT_NONE
    output[].detail = PLAN_NEXT_NONE
    output[].selected_marker = PLAN_MARKER_NONE
    output[].selected_source = PLAN_SOURCE_NONE
    output[].suggestion_count = 0


def runtime_doctor_add_suggestion(
    output: Pointer[mut=True, ProdexRuntimeDoctorPlan, _],
    buffers: ProdexRuntimeDoctorPlanBuffers,
    suggestion_id: Int64,
    severity: Int64,
    marker: Int64,
    count: Int64,
) -> Int64:
    var suggestion_ids = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(buffers.suggestion_ids))
    var suggestion_severities = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(buffers.suggestion_severities))
    var suggestion_markers = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(buffers.suggestion_markers))
    var suggestion_counts = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(buffers.suggestion_counts))
    var suggestion_setting_counts = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(buffers.suggestion_setting_counts))
    var slot = output[].suggestion_count
    if slot >= RUNTIME_DOCTOR_PLAN_MAX_SUGGESTIONS:
        return -1
    suggestion_ids[unsafe_offset=slot] = suggestion_id
    suggestion_severities[unsafe_offset=slot] = severity
    suggestion_markers[unsafe_offset=slot] = marker
    suggestion_counts[unsafe_offset=slot] = count
    suggestion_setting_counts[unsafe_offset=slot] = 0
    output[].suggestion_count = slot + 1
    return slot


def runtime_doctor_add_setting(
    output: Pointer[mut=True, ProdexRuntimeDoctorPlan, _],
    buffers: ProdexRuntimeDoctorPlanBuffers,
    slot: Int64,
    key: Int64,
    current: Int64,
    suggested: Int64,
) -> None:
    var suggestion_setting_counts = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(buffers.suggestion_setting_counts))
    var setting_keys = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(buffers.setting_keys))
    var setting_current_values = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(buffers.setting_current_values))
    var setting_suggested_values = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(buffers.setting_suggested_values))
    var setting = suggestion_setting_counts[unsafe_offset=slot]
    if setting >= RUNTIME_DOCTOR_PLAN_MAX_SETTINGS:
        return
    var flat = slot * RUNTIME_DOCTOR_PLAN_MAX_SETTINGS + setting
    setting_keys[unsafe_offset=flat] = key
    setting_current_values[unsafe_offset=flat] = current
    setting_suggested_values[unsafe_offset=flat] = suggested
    suggestion_setting_counts[unsafe_offset=slot] = setting + 1


def runtime_doctor_fill_next(
    input: ProdexRuntimeDoctorPlanInput,
    output: Pointer[mut=True, ProdexRuntimeDoctorPlan, _],
) -> None:
    if input.operation == PLAN_OP_PREVIOUS_RESPONSE:
        if input.context_dependent == 1:
            output[].detail = PLAN_NEXT_CONTEXT_DEPENDENT
    elif input.operation == PLAN_OP_COMPACT_FINAL_FAILURE:
        if input.compact_exit_pressure == 1:
            output[].detail = PLAN_NEXT_COMPACT_PRESSURE
        elif input.compact_reason == PLAN_COMPACT_REASON_QUOTA:
            output[].detail = PLAN_NEXT_COMPACT_QUOTA
        elif input.compact_reason == PLAN_COMPACT_REASON_OVERLOAD:
            output[].detail = PLAN_NEXT_COMPACT_OVERLOAD
        elif input.compact_reason == PLAN_COMPACT_REASON_TRANSPORT:
            output[].detail = PLAN_NEXT_COMPACT_TRANSPORT
        elif input.compact_reason == PLAN_COMPACT_REASON_INFLIGHT:
            output[].detail = PLAN_NEXT_COMPACT_INFLIGHT
    elif input.operation == PLAN_OP_LANE_PRESSURE:
        if input.lane == PLAN_LANE_RESPONSES:
            output[].detail = PLAN_NEXT_LANE_RESPONSES
    elif input.operation == PLAN_OP_ACTIVE_PRESSURE:
        output[].detail = PLAN_NEXT_ACTIVE_PRESSURE
    elif input.operation == PLAN_OP_PROFILE_INFLIGHT:
        if input.observations.inflight_hard_limit >= 0:
            output[].detail = PLAN_NEXT_PROFILE_HARD_LIMIT
    elif input.operation == PLAN_OP_ROUTE_HEALTH:
        output[].detail = PLAN_NEXT_ROUTE_HEALTH
    elif input.operation == PLAN_OP_WEBSOCKET_CONNECT:
        output[].selected_marker = runtime_doctor_selected_websocket_marker(input.counts)
        if output[].selected_marker == PLAN_MARKER_WEBSOCKET_REJECTED or output[].selected_marker == PLAN_MARKER_WEBSOCKET_REJECT:
            output[].detail = PLAN_NEXT_WEBSOCKET_REJECTED
        elif output[].selected_marker == PLAN_MARKER_WEBSOCKET_DISPATCH:
            output[].detail = PLAN_NEXT_WEBSOCKET_DISPATCH
        else:
            output[].detail = PLAN_NEXT_WEBSOCKET_ENQUEUE
    elif input.operation == PLAN_OP_PROFILE_AUTH:
        if input.counts.auth_failed > 0:
            output[].selected_marker = PLAN_MARKER_AUTH_FAILED
            output[].detail = PLAN_NEXT_AUTH_FAILED
        else:
            output[].selected_marker = PLAN_MARKER_AUTH_RECOVERED
            output[].detail = PLAN_NEXT_AUTH_RECOVERED
    elif input.operation == PLAN_OP_PERSISTENCE:
        if input.counts.state_backpressure > 0:
            output[].selected_source = PLAN_SOURCE_STATE
        elif input.counts.journal_backpressure > 0:
            output[].selected_source = PLAN_SOURCE_JOURNAL
    elif input.operation == PLAN_OP_SYNC_PROBE:
        if input.observations.sync_cold_start_jobs >= 0:
            output[].detail = PLAN_NEXT_SYNC_JOBS
            output[].selected_source = PLAN_SOURCE_JOBS
        elif input.observations.sync_cold_start_profiles >= 0:
            output[].detail = PLAN_NEXT_SYNC_PROFILES
            output[].selected_source = PLAN_SOURCE_PROFILES
    elif input.operation == PLAN_OP_PROBE_REFRESH:
        output[].detail = PLAN_NEXT_PROBE_REFRESH
    elif input.operation == PLAN_OP_TRANSPORT:
        output[].selected_marker = runtime_doctor_selected_transport_marker(input.counts)
    elif input.operation == PLAN_OP_QUOTA:
        if input.counts.sync_probe_skip > 0:
            output[].detail = PLAN_NEXT_QUOTA_SYNC
            if input.observations.sync_cold_start_jobs >= 0:
                output[].selected_source = PLAN_SOURCE_JOBS
            elif input.observations.sync_cold_start_profiles >= 0:
                output[].selected_source = PLAN_SOURCE_PROFILES
        elif input.counts.probe_backpressure > 0:
            output[].detail = PLAN_NEXT_QUOTA_PROBE
        elif input.quota_stale_risk == 1:
            output[].detail = PLAN_NEXT_QUOTA_STALE
        if output[].detail != PLAN_NEXT_QUOTA_SYNC and output[].detail != PLAN_NEXT_QUOTA_PROBE:
            if input.counts.quota_blocked > 0:
                output[].selected_source = PLAN_SOURCE_QUOTA
            elif input.counts.responses_pre_send_skip > 0:
                output[].selected_source = PLAN_SOURCE_RESPONSES_SKIP
            elif input.counts.websocket_pre_send_skip > 0:
                output[].selected_source = PLAN_SOURCE_WEBSOCKET_SKIP
    elif input.operation == PLAN_OP_PRECOMMIT:
        if input.counts.compact_precommit_budget > 0 or input.counts.compact_exit_precommit_budget > 0 or input.counts.compact_candidate > 0 or input.counts.compact_exit_candidate > 0:
            output[].detail = PLAN_NEXT_PRECOMMIT_COMPACT
        else:
            output[].detail = PLAN_NEXT_PRECOMMIT_GENERAL


def runtime_doctor_fill_suggestions(
    input: ProdexRuntimeDoctorPlanInput,
    output: Pointer[mut=True, ProdexRuntimeDoctorPlan, _],
    buffers: ProdexRuntimeDoctorPlanBuffers,
) -> None:
    var lane = input.lane
    if input.counts.lane > 0 and lane == PLAN_LANE_MISSING:
        lane = PLAN_LANE_RESPONSES
    var lane_key: Int64 = 0
    var current_lane: Int64 = 0
    if lane == PLAN_LANE_RESPONSES:
        lane_key = PLAN_SETTING_RESPONSES_ACTIVE
        current_lane = input.tuning.responses_active_limit
    elif lane == PLAN_LANE_COMPACT:
        lane_key = PLAN_SETTING_COMPACT_ACTIVE
        current_lane = input.tuning.compact_active_limit
    elif lane == PLAN_LANE_WEBSOCKET:
        lane_key = PLAN_SETTING_WEBSOCKET_ACTIVE
        current_lane = input.tuning.websocket_active_limit
    elif lane == PLAN_LANE_STANDARD:
        lane_key = PLAN_SETTING_STANDARD_ACTIVE
        current_lane = input.tuning.standard_active_limit
    if input.counts.lane > 0 and lane_key != 0:
        var observed_active = runtime_doctor_optional(input.observations.lane_active, current_lane)
        var observed_limit = runtime_doctor_optional(input.observations.lane_limit, current_lane)
        var lane_base = runtime_doctor_max(current_lane, observed_limit)
        lane_base = runtime_doctor_max(lane_base, observed_active + 1)
        var target_lane = runtime_doctor_scale_up(lane_base)
        var slot = runtime_doctor_add_suggestion(
            output,
            buffers,
            PLAN_SUGGESTION_LANE,
            PLAN_SEVERITY_MEDIUM,
            PLAN_MARKER_NONE,
            input.counts.lane,
        )
        if slot >= 0:
            runtime_doctor_add_setting(
                output, buffers, slot, lane_key, current_lane, target_lane
            )
            if target_lane >= input.tuning.active_request_limit:
                var global_target = target_lane + 2
                if global_target > RUNTIME_DOCTOR_PLAN_MAX_SCALAR:
                    global_target = RUNTIME_DOCTOR_PLAN_MAX_SCALAR
                runtime_doctor_add_setting(
                    output,
                    buffers,
                    slot,
                    PLAN_SETTING_ACTIVE_REQUEST,
                    input.tuning.active_request_limit,
                    global_target,
                )

    if input.counts.active > 0:
        var observed_active = runtime_doctor_optional(
            input.observations.active_active, input.tuning.active_request_limit
        )
        var observed_limit = runtime_doctor_optional(
            input.observations.active_limit, input.tuning.active_request_limit
        )
        var active_base = runtime_doctor_max(input.tuning.active_request_limit, observed_limit)
        active_base = runtime_doctor_max(active_base, observed_active + 1)
        var slot = runtime_doctor_add_suggestion(
            output,
            buffers,
            PLAN_SUGGESTION_ACTIVE,
            PLAN_SEVERITY_MEDIUM,
            PLAN_MARKER_NONE,
            input.counts.active,
        )
        if slot >= 0:
            runtime_doctor_add_setting(
                output,
                buffers,
                slot,
                PLAN_SETTING_ACTIVE_REQUEST,
                input.tuning.active_request_limit,
                runtime_doctor_scale_up(active_base),
            )

    if input.counts.profile_inflight > 0:
        var observed_hard = runtime_doctor_optional(
            input.observations.inflight_hard_limit,
            input.tuning.profile_inflight_hard_limit,
        )
        var hard_base = runtime_doctor_max(input.tuning.profile_inflight_hard_limit, observed_hard)
        var target_hard = runtime_doctor_scale_up(hard_base)
        var target_soft = runtime_doctor_scale_up(input.tuning.profile_inflight_soft_limit)
        var soft_cap = runtime_doctor_max(target_hard - 1, 1)
        if target_soft > soft_cap:
            target_soft = soft_cap
        var slot = runtime_doctor_add_suggestion(
            output,
            buffers,
            PLAN_SUGGESTION_PROFILE_INFLIGHT,
            PLAN_SEVERITY_MEDIUM,
            PLAN_MARKER_NONE,
            input.counts.profile_inflight,
        )
        if slot >= 0:
            runtime_doctor_add_setting(
                output,
                buffers,
                slot,
                PLAN_SETTING_PROFILE_SOFT,
                input.tuning.profile_inflight_soft_limit,
                target_soft,
            )
            runtime_doctor_add_setting(
                output,
                buffers,
                slot,
                PLAN_SETTING_PROFILE_HARD,
                input.tuning.profile_inflight_hard_limit,
                target_hard,
            )

    var websocket_marker = runtime_doctor_selected_websocket_marker(input.counts)
    var websocket_count = input.counts.websocket_rejected + input.counts.websocket_reject + input.counts.websocket_enqueue + input.counts.websocket_dispatch
    if websocket_count > 0:
        var worker = runtime_doctor_optional(
            input.observations.websocket_worker_count,
            input.tuning.websocket_connect_worker_count,
        )
        var queue = runtime_doctor_optional(
            input.observations.websocket_queue_capacity,
            input.tuning.websocket_connect_queue_capacity,
        )
        var pending = runtime_doctor_optional(input.observations.websocket_pending, 0)
        var max_pending = runtime_doctor_optional(input.observations.websocket_max_pending, 0)
        var target_worker = runtime_doctor_scale_up(
            runtime_doctor_max(input.tuning.websocket_connect_worker_count, worker)
        )
        var queue_base = runtime_doctor_max(input.tuning.websocket_connect_queue_capacity, queue)
        queue_base = runtime_doctor_max(queue_base, target_worker)
        var target_queue = runtime_doctor_scale_up(queue_base)
        var overflow_base = runtime_doctor_max(input.tuning.websocket_connect_overflow_capacity, pending)
        overflow_base = runtime_doctor_max(overflow_base, max_pending)
        overflow_base = runtime_doctor_max(overflow_base, target_queue)
        var target_overflow = runtime_doctor_scale_up(overflow_base)
        var slot = runtime_doctor_add_suggestion(
            output,
            buffers,
            PLAN_SUGGESTION_WEBSOCKET_CONNECT,
            PLAN_SEVERITY_MEDIUM,
            websocket_marker,
            websocket_count,
        )
        if slot >= 0:
            runtime_doctor_add_setting(
                output,
                buffers,
                slot,
                PLAN_SETTING_CONNECT_WORKERS,
                input.tuning.websocket_connect_worker_count,
                target_worker,
            )
            runtime_doctor_add_setting(
                output,
                buffers,
                slot,
                PLAN_SETTING_CONNECT_QUEUE,
                input.tuning.websocket_connect_queue_capacity,
                target_queue,
            )
            runtime_doctor_add_setting(
                output,
                buffers,
                slot,
                PLAN_SETTING_CONNECT_OVERFLOW,
                input.tuning.websocket_connect_overflow_capacity,
                target_overflow,
            )

    var dns_marker: Int64 = PLAN_MARKER_NONE
    if input.counts.dns_reject > 0:
        dns_marker = PLAN_MARKER_WEBSOCKET_REJECT
    elif input.counts.dns_enqueue > 0:
        dns_marker = PLAN_MARKER_WEBSOCKET_ENQUEUE
    elif input.counts.dns_dispatch > 0:
        dns_marker = PLAN_MARKER_WEBSOCKET_DISPATCH
    var dns_count = input.counts.dns_reject + input.counts.dns_enqueue + input.counts.dns_dispatch
    if dns_count > 0:
        var worker = runtime_doctor_optional(
            input.observations.dns_worker_count,
            input.tuning.websocket_dns_worker_count,
        )
        var queue = runtime_doctor_optional(
            input.observations.dns_queue_capacity,
            input.tuning.websocket_dns_queue_capacity,
        )
        var pending = runtime_doctor_optional(input.observations.dns_pending, 0)
        var max_pending = runtime_doctor_optional(input.observations.dns_max_pending, 0)
        var target_worker = runtime_doctor_scale_up(
            runtime_doctor_max(input.tuning.websocket_dns_worker_count, worker)
        )
        var queue_base = runtime_doctor_max(input.tuning.websocket_dns_queue_capacity, queue)
        queue_base = runtime_doctor_max(queue_base, target_worker)
        var target_queue = runtime_doctor_scale_up(queue_base)
        var overflow_base = runtime_doctor_max(input.tuning.websocket_dns_overflow_capacity, pending)
        overflow_base = runtime_doctor_max(overflow_base, max_pending)
        overflow_base = runtime_doctor_max(overflow_base, target_queue)
        var target_overflow = runtime_doctor_scale_up(overflow_base)
        var slot = runtime_doctor_add_suggestion(
            output,
            buffers,
            PLAN_SUGGESTION_WEBSOCKET_DNS,
            PLAN_SEVERITY_MEDIUM,
            dns_marker,
            dns_count,
        )
        if slot >= 0:
            runtime_doctor_add_setting(
                output,
                buffers,
                slot,
                PLAN_SETTING_DNS_WORKERS,
                input.tuning.websocket_dns_worker_count,
                target_worker,
            )
            runtime_doctor_add_setting(
                output,
                buffers,
                slot,
                PLAN_SETTING_DNS_QUEUE,
                input.tuning.websocket_dns_queue_capacity,
                target_queue,
            )
            runtime_doctor_add_setting(
                output,
                buffers,
                slot,
                PLAN_SETTING_DNS_OVERFLOW,
                input.tuning.websocket_dns_overflow_capacity,
                target_overflow,
            )

    var persistence_count = input.counts.state_backpressure + input.counts.journal_backpressure
    if persistence_count > 0:
        var target_compact = runtime_doctor_scale_down(input.tuning.compact_active_limit)
        var target_standard = runtime_doctor_scale_down(input.tuning.standard_active_limit)
        var target_wait = runtime_doctor_max(
            input.tuning.pressure_admission_wait_budget_ms,
            input.tuning.admission_wait_budget_ms,
        )
        if target_wait > RUNTIME_DOCTOR_PLAN_MAX_SCALAR - 500:
            target_wait = RUNTIME_DOCTOR_PLAN_MAX_SCALAR
        else:
            target_wait += 500
        var slot = runtime_doctor_add_suggestion(
            output,
            buffers,
            PLAN_SUGGESTION_PERSISTENCE,
            PLAN_SEVERITY_MEDIUM,
            PLAN_MARKER_NONE,
            persistence_count,
        )
        if slot >= 0:
            runtime_doctor_add_setting(
                output,
                buffers,
                slot,
                PLAN_SETTING_COMPACT_ACTIVE,
                input.tuning.compact_active_limit,
                target_compact,
            )
            runtime_doctor_add_setting(
                output,
                buffers,
                slot,
                PLAN_SETTING_STANDARD_ACTIVE,
                input.tuning.standard_active_limit,
                target_standard,
            )
            runtime_doctor_add_setting(
                output,
                buffers,
                slot,
                PLAN_SETTING_PRESSURE_WAIT,
                input.tuning.pressure_admission_wait_budget_ms,
                target_wait,
            )

    if input.counts.profile_health > 0:
        var target_soft = runtime_doctor_scale_down(input.tuning.profile_inflight_soft_limit)
        var target_hard = runtime_doctor_scale_down(input.tuning.profile_inflight_hard_limit)
        if target_hard < target_soft + 1:
            target_hard = target_soft + 1
        var slot = runtime_doctor_add_suggestion(
            output,
            buffers,
            PLAN_SUGGESTION_ROUTE_HEALTH,
            PLAN_SEVERITY_LOW,
            PLAN_MARKER_NONE,
            input.counts.profile_health,
        )
        if slot >= 0:
            runtime_doctor_add_setting(
                output,
                buffers,
                slot,
                PLAN_SETTING_PROFILE_SOFT,
                input.tuning.profile_inflight_soft_limit,
                target_soft,
            )
            runtime_doctor_add_setting(
                output,
                buffers,
                slot,
                PLAN_SETTING_PROFILE_HARD,
                input.tuning.profile_inflight_hard_limit,
                target_hard,
            )


@export("prodex_mojo_rich_runtime_doctor_plan_v1")
def prodex_mojo_rich_runtime_doctor_plan_v1(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
    suggestion_ids_address: UInt,
    suggestion_severities_address: UInt,
    suggestion_markers_address: UInt,
    suggestion_counts_address: UInt,
    suggestion_setting_counts_address: UInt,
    setting_keys_address: UInt,
    setting_current_values_address: UInt,
    setting_suggested_values_address: UInt,
) abi("C") -> Int64:
    if output_address == 0:
        return 1
    var output = Pointer[
        mut=True, ProdexRuntimeDoctorPlan, MutUntrackedOrigin
    ](unsafe_from_address=Int(output_address))
    runtime_doctor_reset_output(output)
    if abi_version != RUNTIME_DOCTOR_PLAN_ABI_VERSION:
        return 4
    if input_address == 0:
        return 1
    var input_pointer = Pointer[
        mut=False, ProdexRuntimeDoctorPlanInput, ImmUntrackedOrigin
    ](unsafe_from_address=Int(input_address))
    var input = input_pointer[].copy()
    if not runtime_doctor_input_valid(input):
        return 1
    if (
        suggestion_ids_address == 0
        or suggestion_severities_address == 0
        or suggestion_markers_address == 0
        or suggestion_counts_address == 0
        or suggestion_setting_counts_address == 0
        or setting_keys_address == 0
        or setting_current_values_address == 0
        or setting_suggested_values_address == 0
    ):
        return 1
    var buffers = ProdexRuntimeDoctorPlanBuffers(
        suggestion_ids_address,
        suggestion_severities_address,
        suggestion_markers_address,
        suggestion_counts_address,
        suggestion_setting_counts_address,
        setting_keys_address,
        setting_current_values_address,
        setting_suggested_values_address,
    )
    if input.operation == PLAN_OP_POLICY_SUGGESTIONS:
        runtime_doctor_fill_suggestions(input, output, buffers)
    else:
        runtime_doctor_fill_next(input, output)
    return 0
