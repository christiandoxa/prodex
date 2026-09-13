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


# The summary ABI accepts sanitized marker counts only. Rust keeps log parsing,
# redaction, filesystem access, and process evidence outside this kernel.
comptime RUNTIME_DOCTOR_SUMMARY_PLAN_ABI_VERSION: Int64 = 1
comptime RUNTIME_DOCTOR_SUMMARY_MARKER_COUNT: Int64 = 128

comptime SUMMARY_MARKER_RUNTIME_PROXY_OVERLOAD_BACKOFF: Int64 = 0
comptime SUMMARY_MARKER_RUNTIME_PROXY_LANE_LIMIT: Int64 = 1
comptime SUMMARY_MARKER_RUNTIME_PROXY_ACTIVE_LIMIT: Int64 = 2
comptime SUMMARY_MARKER_RUNTIME_PROXY_QUEUE_OVERLOADED: Int64 = 3
comptime SUMMARY_MARKER_PROFILE_CIRCUIT_OPEN: Int64 = 4
comptime SUMMARY_MARKER_PROFILE_CIRCUIT_HALF_OPEN: Int64 = 5
comptime SUMMARY_MARKER_WEBSOCKET_FRAME_TIMEOUT: Int64 = 6
comptime SUMMARY_MARKER_WEBSOCKET_HOLD_TIMEOUT: Int64 = 7
comptime SUMMARY_MARKER_WEBSOCKET_DNS_TIMEOUT: Int64 = 8
comptime SUMMARY_MARKER_WEBSOCKET_DNS_REJECT: Int64 = 9
comptime SUMMARY_MARKER_WEBSOCKET_DNS_ENQUEUE: Int64 = 10
comptime SUMMARY_MARKER_WEBSOCKET_DNS_DISPATCH: Int64 = 11
comptime SUMMARY_MARKER_WEBSOCKET_LOCAL_PRESSURE: Int64 = 12
comptime SUMMARY_MARKER_WEBSOCKET_CONNECT_REJECT: Int64 = 13
comptime SUMMARY_MARKER_WEBSOCKET_CONNECT_REJECTED: Int64 = 14
comptime SUMMARY_MARKER_WEBSOCKET_CONNECT_ENQUEUE: Int64 = 15
comptime SUMMARY_MARKER_WEBSOCKET_CONNECT_DISPATCH: Int64 = 16
comptime SUMMARY_MARKER_WEBSOCKET_TUNNEL_FAILURE: Int64 = 17
comptime SUMMARY_MARKER_PROFILE_INFLIGHT: Int64 = 18
comptime SUMMARY_MARKER_PROFILE_HEALTH: Int64 = 19
comptime SUMMARY_MARKER_PROFILE_BAD_PAIRING: Int64 = 20
comptime SUMMARY_MARKER_PROFILE_AUTH_FAILURE: Int64 = 21
comptime SUMMARY_MARKER_PROVIDER_AUTH_FAILURE: Int64 = 22
comptime SUMMARY_MARKER_COMPACT_FALLBACK_BLOCKED: Int64 = 23
comptime SUMMARY_MARKER_COMPACT_PRESSURE_SHED: Int64 = 24
comptime SUMMARY_MARKER_CHAIN_DEAD: Int64 = 25
comptime SUMMARY_MARKER_STALE_CONTINUATION: Int64 = 26
comptime SUMMARY_MARKER_CHAIN_RETRIED: Int64 = 27
comptime SUMMARY_MARKER_PREVIOUS_RESPONSE_BLOCKED: Int64 = 28
comptime SUMMARY_MARKER_PREVIOUS_RESPONSE_FALLBACK: Int64 = 29
comptime SUMMARY_MARKER_PREVIOUS_RESPONSE_NOT_FOUND: Int64 = 30
comptime SUMMARY_MARKER_COMPACT_FINAL_FAILURE: Int64 = 31
comptime SUMMARY_MARKER_COMPAT_WARNING: Int64 = 32
comptime SUMMARY_MARKER_WEBSOCKET_WATCHDOG: Int64 = 33
comptime SUMMARY_MARKER_AUTH_RECOVERED: Int64 = 34
comptime SUMMARY_MARKER_PRECOMMIT_BUDGET: Int64 = 35
comptime SUMMARY_MARKER_UPSTREAM_USAGE_LIMIT: Int64 = 36
comptime SUMMARY_MARKER_RESPONSES_PRE_SEND: Int64 = 37
comptime SUMMARY_MARKER_WEBSOCKET_PRE_SEND: Int64 = 38
comptime SUMMARY_MARKER_QUOTA_CRITICAL: Int64 = 39
comptime SUMMARY_MARKER_GEMINI_QUOTA_ROTATE: Int64 = 40
comptime SUMMARY_MARKER_GEMINI_RATE_RETRY: Int64 = 41
comptime SUMMARY_MARKER_PROVIDER_MODEL_FALLBACK: Int64 = 42
comptime SUMMARY_MARKER_GEMINI_STREAM_RETRY: Int64 = 43
comptime SUMMARY_MARKER_GEMINI_STREAM_FALLBACK: Int64 = 44
comptime SUMMARY_MARKER_GEMINI_COMPACT_FALLBACK: Int64 = 45
comptime SUMMARY_MARKER_GEMINI_LIVE_ERROR: Int64 = 46
comptime SUMMARY_MARKER_GEMINI_LIVE_SIDECAR_ERROR: Int64 = 47
comptime SUMMARY_MARKER_GEMINI_LIVE_SESSION_ERROR: Int64 = 48
comptime SUMMARY_MARKER_STREAM_READ: Int64 = 49
comptime SUMMARY_MARKER_LOCAL_WRITER: Int64 = 50
comptime SUMMARY_MARKER_CONNECT_TIMEOUT: Int64 = 51
comptime SUMMARY_MARKER_TLS_ERROR: Int64 = 52
comptime SUMMARY_MARKER_CONNECT_ERROR: Int64 = 53
comptime SUMMARY_MARKER_STATE_SAVE_ERROR: Int64 = 54
comptime SUMMARY_MARKER_STATE_SAVE_BACKPRESSURE: Int64 = 55
comptime SUMMARY_MARKER_JOURNAL_SAVE_BACKPRESSURE: Int64 = 56
comptime SUMMARY_MARKER_SYNC_PROBE_SKIP: Int64 = 57
comptime SUMMARY_MARKER_PROBE_BACKPRESSURE: Int64 = 58
comptime SUMMARY_MARKER_PROBE_ERROR: Int64 = 59
comptime SUMMARY_MARKER_PROBE_START: Int64 = 60
comptime SUMMARY_MARKER_FIRST_UPSTREAM_CHUNK: Int64 = 61
comptime SUMMARY_MARKER_FIRST_LOCAL_CHUNK: Int64 = 62
comptime SUMMARY_MARKER_STARTUP_AUDIT: Int64 = 63
comptime SUMMARY_MARKER_COMPACT_EXIT_CANDIDATE: Int64 = 64
comptime SUMMARY_MARKER_COMPACT_EXIT_COMMITTED: Int64 = 65
comptime SUMMARY_MARKER_COMPACT_EXIT_COMMITTED_OWNER: Int64 = 66
comptime SUMMARY_MARKER_COMPACT_EXIT_FOLLOWUP_OWNER: Int64 = 67
comptime SUMMARY_MARKER_COMPACT_EXIT_LINEAGE: Int64 = 68
comptime SUMMARY_MARKER_COMPACT_EXIT_OVERLOAD_RETRY: Int64 = 69
comptime SUMMARY_MARKER_COMPACT_EXIT_PRECOMMIT: Int64 = 70
comptime SUMMARY_MARKER_COMPACT_EXIT_PRESSURE: Int64 = 71
comptime SUMMARY_MARKER_COMPACT_EXIT_QUOTA: Int64 = 72
comptime SUMMARY_MARKER_COMPACT_EXIT_RETRYABLE: Int64 = 73
comptime SUMMARY_MARKER_COMPACT_TRANSPORT: Int64 = 74
comptime SUMMARY_MARKER_COMMITTED: Int64 = 75
comptime SUMMARY_MARKER_COMMITTED_OWNER: Int64 = 76
comptime SUMMARY_MARKER_FOLLOWUP_OWNER: Int64 = 77
comptime SUMMARY_MARKER_LINEAGE: Int64 = 78
comptime SUMMARY_MARKER_OVERLOAD_RETRY: Int64 = 79
comptime SUMMARY_MARKER_COMPACT_PRECOMMIT: Int64 = 80
comptime SUMMARY_MARKER_COMPACT_PRESSURE: Int64 = 81
comptime SUMMARY_MARKER_COMPACT_QUOTA: Int64 = 82
comptime SUMMARY_MARKER_COMPACT_RETRYABLE: Int64 = 83
comptime SUMMARY_MARKER_KEEP_AFFINITY: Int64 = 84
comptime SUMMARY_MARKER_KEEP_CURRENT: Int64 = 85
comptime SUMMARY_MARKER_SELECTION_PICK: Int64 = 86
comptime SUMMARY_MARKER_SELECTION_SKIP_CURRENT: Int64 = 87
comptime SUMMARY_MARKER_SELECTION_SKIP_AFFINITY: Int64 = 88
comptime SUMMARY_MARKER_STATE_SAVE_SKIPPED: Int64 = 89
comptime SUMMARY_MARKER_PROBE_OK: Int64 = 90
comptime SUMMARY_MARKER_DNS_ERROR: Int64 = 91
comptime SUMMARY_MARKER_GEMINI_SIDECAR_ACCEPT_ERROR: Int64 = 92
comptime SUMMARY_MARKER_LOCAL_SELECTION_BLOCKED: Int64 = 93
comptime SUMMARY_MARKER_UPSTREAM_OVERLOAD_PASSTHROUGH: Int64 = 94
comptime SUMMARY_MARKER_UPSTREAM_OVERLOADED: Int64 = 95
comptime SUMMARY_MARKER_UPSTREAM_READ_ERROR: Int64 = 96
comptime SUMMARY_MARKER_UPSTREAM_SEND_ERROR: Int64 = 97
comptime SUMMARY_MARKER_UPSTREAM_STREAM_ERROR: Int64 = 98
comptime SUMMARY_MARKER_PROFILE_TRANSPORT_BACKOFF: Int64 = 99
comptime SUMMARY_MARKER_PROFILE_TRANSPORT_FAILURE: Int64 = 100
comptime SUMMARY_MARKER_JOURNAL_SAVE_ERROR: Int64 = 101
comptime SUMMARY_MARKER_UPSTREAM_CONNECT_HTTP: Int64 = 102
comptime SUMMARY_MARKER_UPSTREAM_CLOSE_BEFORE_COMPLETED: Int64 = 103
comptime SUMMARY_MARKER_UPSTREAM_CONNECTION_CLOSED: Int64 = 104
comptime SUMMARY_MARKER_COMPACT_CANDIDATE: Int64 = 105
comptime SUMMARY_MARKER_LOCAL_SELECTION_PLAN: Int64 = 106
comptime SUMMARY_MARKER_PROFILE_AUTH_PROACTIVE_SYNC_FAILED: Int64 = 107
comptime SUMMARY_MARKER_PROFILE_QUOTA_QUARANTINE: Int64 = 108

comptime SUMMARY_PRESSURE_LOW: Int64 = 0
comptime SUMMARY_PRESSURE_ELEVATED: Int64 = 1
comptime SUMMARY_PRESSURE_ACTIVE: Int64 = 2
comptime SUMMARY_PRESSURE_STALE_RISK: Int64 = 3

comptime SUMMARY_DIAGNOSIS_NONE: Int64 = 0
comptime SUMMARY_DIAGNOSIS_NO_POINTER: Int64 = 1
comptime SUMMARY_DIAGNOSIS_NO_LOG: Int64 = 2
comptime SUMMARY_DIAGNOSIS_EMPTY_LOG: Int64 = 3
comptime SUMMARY_DIAGNOSIS_PROXY_OVERLOAD_BACKOFF: Int64 = 4
comptime SUMMARY_DIAGNOSIS_LANE_PRESSURE: Int64 = 5
comptime SUMMARY_DIAGNOSIS_ACTIVE_PRESSURE: Int64 = 6
comptime SUMMARY_DIAGNOSIS_QUEUE_OVERLOAD: Int64 = 7
comptime SUMMARY_DIAGNOSIS_CIRCUIT_OPEN: Int64 = 8
comptime SUMMARY_DIAGNOSIS_CIRCUIT_HALF_OPEN: Int64 = 9
comptime SUMMARY_DIAGNOSIS_WEBSOCKET_FRAME_TIMEOUT: Int64 = 10
comptime SUMMARY_DIAGNOSIS_WEBSOCKET_HOLD_TIMEOUT: Int64 = 11
comptime SUMMARY_DIAGNOSIS_WEBSOCKET_DNS_TIMEOUT: Int64 = 12
comptime SUMMARY_DIAGNOSIS_WEBSOCKET_DNS_REJECT: Int64 = 13
comptime SUMMARY_DIAGNOSIS_WEBSOCKET_DNS_OVERFLOW: Int64 = 14
comptime SUMMARY_DIAGNOSIS_WEBSOCKET_LOCAL_PRESSURE: Int64 = 15
comptime SUMMARY_DIAGNOSIS_WEBSOCKET_CONNECT_REJECT: Int64 = 16
comptime SUMMARY_DIAGNOSIS_WEBSOCKET_CONNECT_ENQUEUE: Int64 = 17
comptime SUMMARY_DIAGNOSIS_WEBSOCKET_CONNECT_DISPATCH: Int64 = 18
comptime SUMMARY_DIAGNOSIS_WEBSOCKET_TUNNEL_FAILURE: Int64 = 19
comptime SUMMARY_DIAGNOSIS_PROFILE_INFLIGHT: Int64 = 20
comptime SUMMARY_DIAGNOSIS_PROFILE_HEALTH: Int64 = 21
comptime SUMMARY_DIAGNOSIS_PROFILE_BAD_PAIRING: Int64 = 22
comptime SUMMARY_DIAGNOSIS_PROFILE_AUTH_FAILURE: Int64 = 23
comptime SUMMARY_DIAGNOSIS_PROVIDER_AUTH_FAILURE: Int64 = 24
comptime SUMMARY_DIAGNOSIS_COMPACT_FALLBACK_BLOCKED: Int64 = 25
comptime SUMMARY_DIAGNOSIS_COMPACT_PRESSURE_SHED: Int64 = 26
comptime SUMMARY_DIAGNOSIS_CHAIN_DEAD: Int64 = 27
comptime SUMMARY_DIAGNOSIS_STALE_CONTINUATION: Int64 = 28
comptime SUMMARY_DIAGNOSIS_CHAIN_RETRIED: Int64 = 29
comptime SUMMARY_DIAGNOSIS_PREVIOUS_RESPONSE_BLOCKED: Int64 = 30
comptime SUMMARY_DIAGNOSIS_PREVIOUS_RESPONSE_FALLBACK: Int64 = 31
comptime SUMMARY_DIAGNOSIS_PREVIOUS_RESPONSE_NOT_FOUND: Int64 = 32
comptime SUMMARY_DIAGNOSIS_COMPACT_FINAL_FAILURE: Int64 = 33
comptime SUMMARY_DIAGNOSIS_COMPACT_EXIT_PATHS: Int64 = 34
comptime SUMMARY_DIAGNOSIS_COMPAT_WARNING: Int64 = 35
comptime SUMMARY_DIAGNOSIS_DEAD_CONTINUATIONS: Int64 = 36
comptime SUMMARY_DIAGNOSIS_SUSPECT_CONTINUATIONS: Int64 = 37
comptime SUMMARY_DIAGNOSIS_WEBSOCKET_WATCHDOG: Int64 = 38
comptime SUMMARY_DIAGNOSIS_AUTH_RECOVERED: Int64 = 39
comptime SUMMARY_DIAGNOSIS_PRECOMMIT_BUDGET: Int64 = 40
comptime SUMMARY_DIAGNOSIS_QUOTA_HARDENING: Int64 = 41
comptime SUMMARY_DIAGNOSIS_GEMINI_QUOTA_RETRY: Int64 = 42
comptime SUMMARY_DIAGNOSIS_PROVIDER_MODEL_FALLBACK: Int64 = 43
comptime SUMMARY_DIAGNOSIS_GEMINI_STREAM_RETRY: Int64 = 44
comptime SUMMARY_DIAGNOSIS_GEMINI_COMPACT_FALLBACK: Int64 = 45
comptime SUMMARY_DIAGNOSIS_GEMINI_LIVE_ERROR: Int64 = 46
comptime SUMMARY_DIAGNOSIS_STREAM_READ: Int64 = 47
comptime SUMMARY_DIAGNOSIS_LOCAL_WRITER: Int64 = 48
comptime SUMMARY_DIAGNOSIS_UPSTREAM_CONNECT: Int64 = 49
comptime SUMMARY_DIAGNOSIS_STATE_SAVE: Int64 = 50
comptime SUMMARY_DIAGNOSIS_PERSISTENCE: Int64 = 51
comptime SUMMARY_DIAGNOSIS_SYNC_PROBE: Int64 = 52
comptime SUMMARY_DIAGNOSIS_PROBE_BACKPRESSURE: Int64 = 53
comptime SUMMARY_DIAGNOSIS_DEGRADED_ROUTES: Int64 = 54
comptime SUMMARY_DIAGNOSIS_ORPHAN_DIRS: Int64 = 55
comptime SUMMARY_DIAGNOSIS_PROBE_ERROR: Int64 = 56
comptime SUMMARY_DIAGNOSIS_PROBE_ACTIVITY: Int64 = 57
comptime SUMMARY_DIAGNOSIS_WRITER_STALL: Int64 = 58
comptime SUMMARY_DIAGNOSIS_BROKER_MISMATCH: Int64 = 59
comptime SUMMARY_DIAGNOSIS_BINARY_MISMATCH: Int64 = 60
comptime SUMMARY_DIAGNOSIS_SELECTION: Int64 = 61
comptime SUMMARY_DIAGNOSIS_NO_RECENT_FAILURE: Int64 = 62

@fieldwise_init
struct ProdexRuntimeDoctorSummaryPlanInput(Copyable):
    var marker_counts: InlineArray[Int64, 128]
    var line_count: Int64
    var pointer_exists: Int64
    var log_exists: Int64
    var stale_persisted_usage_snapshots: Int64
    var orphan_managed_dirs: Int64
    var startup_audit_risk: Int64
    var persisted_dead_continuations: Int64
    var suspect_continuations: Int64
    var degraded_routes: Int64
    var runtime_broker_mismatch: Int64
    var prodex_binary_mismatch: Int64
    var persisted_quota_snapshot_risk: Int64


@fieldwise_init
struct ProdexRuntimeDoctorSummaryPlan(Copyable):
    var abi_version: Int64
    var selection_pressure: Int64
    var transport_pressure: Int64
    var persistence_pressure: Int64
    var quota_freshness_pressure: Int64
    var startup_audit_pressure: Int64
    var diagnosis_kind: Int64


def runtime_doctor_summary_count(
    input: ProdexRuntimeDoctorSummaryPlanInput,
    index: Int64,
) -> Int64:
    return input.marker_counts[Int(index)]


def runtime_doctor_summary_any_selection(input: ProdexRuntimeDoctorSummaryPlanInput) -> Bool:
    return (
        runtime_doctor_summary_count(input, SUMMARY_MARKER_KEEP_AFFINITY) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_KEEP_CURRENT) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_SELECTION_PICK) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_SELECTION_SKIP_CURRENT) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_SELECTION_SKIP_AFFINITY) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_SYNC_PROBE_SKIP) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_LOCAL_SELECTION_BLOCKED) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_PRECOMMIT_BUDGET) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_COMPACT_PRECOMMIT) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_COMPACT_CANDIDATE) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_COMPACT_TRANSPORT) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_GEMINI_QUOTA_ROTATE) > 0
    )


def runtime_doctor_summary_any_transport(input: ProdexRuntimeDoctorSummaryPlanInput) -> Bool:
    return (
        runtime_doctor_summary_count(input, SUMMARY_MARKER_STREAM_READ) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_CONNECT_TIMEOUT) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_DNS_ERROR) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_TLS_ERROR) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_CONNECT_ERROR) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_UPSTREAM_CONNECT_HTTP) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_UPSTREAM_CLOSE_BEFORE_COMPLETED) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_UPSTREAM_CONNECTION_CLOSED) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_UPSTREAM_READ_ERROR) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_UPSTREAM_SEND_ERROR) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_UPSTREAM_STREAM_ERROR) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_COMPACT_TRANSPORT) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_PROFILE_TRANSPORT_FAILURE) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_PROFILE_TRANSPORT_BACKOFF) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_PROFILE_CIRCUIT_OPEN) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_PROFILE_CIRCUIT_HALF_OPEN) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_FRAME_TIMEOUT) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_HOLD_TIMEOUT) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_DNS_TIMEOUT) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_DNS_ENQUEUE) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_DNS_DISPATCH) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_DNS_REJECT) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_LOCAL_PRESSURE) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_CONNECT_ENQUEUE) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_CONNECT_DISPATCH) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_CONNECT_REJECT) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_CONNECT_REJECTED) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_TUNNEL_FAILURE) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_LOCAL_WRITER) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_GEMINI_STREAM_RETRY) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_GEMINI_STREAM_FALLBACK) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_GEMINI_LIVE_ERROR) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_GEMINI_LIVE_SIDECAR_ERROR) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_GEMINI_LIVE_SESSION_ERROR) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_GEMINI_SIDECAR_ACCEPT_ERROR) > 0
    )


def runtime_doctor_summary_any_compact_exit(input: ProdexRuntimeDoctorSummaryPlanInput) -> Bool:
    return (
        runtime_doctor_summary_count(input, SUMMARY_MARKER_COMPACT_EXIT_CANDIDATE) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_COMPACT_EXIT_COMMITTED) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_COMPACT_EXIT_COMMITTED_OWNER) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_COMPACT_EXIT_FOLLOWUP_OWNER) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_COMPACT_EXIT_LINEAGE) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_COMPACT_EXIT_OVERLOAD_RETRY) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_COMPACT_EXIT_PRECOMMIT) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_COMPACT_EXIT_PRESSURE) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_COMPACT_EXIT_QUOTA) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_COMPACT_EXIT_RETRYABLE) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_COMMITTED) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_COMMITTED_OWNER) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_FOLLOWUP_OWNER) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_LINEAGE) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_OVERLOAD_RETRY) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_COMPACT_PRESSURE) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_COMPACT_QUOTA) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_COMPACT_RETRYABLE) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_COMPACT_TRANSPORT) > 0
    )


def runtime_doctor_summary_select_diagnosis(
    input: ProdexRuntimeDoctorSummaryPlanInput,
) -> Int64:
    if input.pointer_exists == 0:
        return SUMMARY_DIAGNOSIS_NO_POINTER
    if input.log_exists == 0:
        return SUMMARY_DIAGNOSIS_NO_LOG
    if input.line_count == 0:
        return SUMMARY_DIAGNOSIS_EMPTY_LOG

    if runtime_doctor_summary_count(input, SUMMARY_MARKER_RUNTIME_PROXY_OVERLOAD_BACKOFF) > 0:
        return SUMMARY_DIAGNOSIS_PROXY_OVERLOAD_BACKOFF
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_RUNTIME_PROXY_LANE_LIMIT) > 0:
        return SUMMARY_DIAGNOSIS_LANE_PRESSURE
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_RUNTIME_PROXY_ACTIVE_LIMIT) > 0:
        return SUMMARY_DIAGNOSIS_ACTIVE_PRESSURE
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_RUNTIME_PROXY_QUEUE_OVERLOADED) > 0:
        return SUMMARY_DIAGNOSIS_QUEUE_OVERLOAD
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_PROFILE_CIRCUIT_OPEN) > 0:
        return SUMMARY_DIAGNOSIS_CIRCUIT_OPEN
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_PROFILE_CIRCUIT_HALF_OPEN) > 0:
        return SUMMARY_DIAGNOSIS_CIRCUIT_HALF_OPEN
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_FRAME_TIMEOUT) > 0:
        return SUMMARY_DIAGNOSIS_WEBSOCKET_FRAME_TIMEOUT
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_HOLD_TIMEOUT) > 0:
        return SUMMARY_DIAGNOSIS_WEBSOCKET_HOLD_TIMEOUT
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_DNS_TIMEOUT) > 0:
        return SUMMARY_DIAGNOSIS_WEBSOCKET_DNS_TIMEOUT
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_DNS_REJECT) > 0:
        return SUMMARY_DIAGNOSIS_WEBSOCKET_DNS_REJECT
    if (
        runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_DNS_ENQUEUE) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_DNS_DISPATCH) > 0
    ):
        return SUMMARY_DIAGNOSIS_WEBSOCKET_DNS_OVERFLOW

    if runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_LOCAL_PRESSURE) > 0:
        return SUMMARY_DIAGNOSIS_WEBSOCKET_LOCAL_PRESSURE
    if (
        runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_CONNECT_REJECTED) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_CONNECT_REJECT) > 0
    ):
        return SUMMARY_DIAGNOSIS_WEBSOCKET_CONNECT_REJECT
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_CONNECT_ENQUEUE) > 0:
        return SUMMARY_DIAGNOSIS_WEBSOCKET_CONNECT_ENQUEUE
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_CONNECT_DISPATCH) > 0:
        return SUMMARY_DIAGNOSIS_WEBSOCKET_CONNECT_DISPATCH
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_TUNNEL_FAILURE) > 0:
        return SUMMARY_DIAGNOSIS_WEBSOCKET_TUNNEL_FAILURE

    if runtime_doctor_summary_count(input, SUMMARY_MARKER_PROFILE_INFLIGHT) > 0:
        return SUMMARY_DIAGNOSIS_PROFILE_INFLIGHT
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_PROFILE_HEALTH) > 0:
        return SUMMARY_DIAGNOSIS_PROFILE_HEALTH
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_PROFILE_BAD_PAIRING) > 0:
        return SUMMARY_DIAGNOSIS_PROFILE_BAD_PAIRING
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_PROFILE_AUTH_FAILURE) > 0:
        return SUMMARY_DIAGNOSIS_PROFILE_AUTH_FAILURE
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_PROVIDER_AUTH_FAILURE) > 0:
        return SUMMARY_DIAGNOSIS_PROVIDER_AUTH_FAILURE

    if runtime_doctor_summary_count(input, SUMMARY_MARKER_COMPACT_FALLBACK_BLOCKED) > 0:
        return SUMMARY_DIAGNOSIS_COMPACT_FALLBACK_BLOCKED
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_COMPACT_PRESSURE_SHED) > 0:
        return SUMMARY_DIAGNOSIS_COMPACT_PRESSURE_SHED
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_CHAIN_DEAD) > 0:
        return SUMMARY_DIAGNOSIS_CHAIN_DEAD
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_STALE_CONTINUATION) > 0:
        return SUMMARY_DIAGNOSIS_STALE_CONTINUATION
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_CHAIN_RETRIED) > 0:
        return SUMMARY_DIAGNOSIS_CHAIN_RETRIED
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_PREVIOUS_RESPONSE_BLOCKED) > 0:
        return SUMMARY_DIAGNOSIS_PREVIOUS_RESPONSE_BLOCKED
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_PREVIOUS_RESPONSE_FALLBACK) > 0:
        return SUMMARY_DIAGNOSIS_PREVIOUS_RESPONSE_FALLBACK
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_PREVIOUS_RESPONSE_NOT_FOUND) > 0:
        return SUMMARY_DIAGNOSIS_PREVIOUS_RESPONSE_NOT_FOUND
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_COMPACT_FINAL_FAILURE) > 0:
        return SUMMARY_DIAGNOSIS_COMPACT_FINAL_FAILURE
    if runtime_doctor_summary_any_compact_exit(input):
        return SUMMARY_DIAGNOSIS_COMPACT_EXIT_PATHS

    if runtime_doctor_summary_count(input, SUMMARY_MARKER_COMPAT_WARNING) > 0:
        return SUMMARY_DIAGNOSIS_COMPAT_WARNING
    if input.persisted_dead_continuations > 0:
        return SUMMARY_DIAGNOSIS_DEAD_CONTINUATIONS
    if input.suspect_continuations > 0:
        return SUMMARY_DIAGNOSIS_SUSPECT_CONTINUATIONS
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_WATCHDOG) > 0:
        return SUMMARY_DIAGNOSIS_WEBSOCKET_WATCHDOG
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_AUTH_RECOVERED) > 0:
        return SUMMARY_DIAGNOSIS_AUTH_RECOVERED
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_PRECOMMIT_BUDGET) > 0:
        return SUMMARY_DIAGNOSIS_PRECOMMIT_BUDGET
    if (
        runtime_doctor_summary_count(input, SUMMARY_MARKER_UPSTREAM_USAGE_LIMIT) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_RESPONSES_PRE_SEND) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_WEBSOCKET_PRE_SEND) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_QUOTA_CRITICAL) > 0
    ):
        return SUMMARY_DIAGNOSIS_QUOTA_HARDENING
    if (
        runtime_doctor_summary_count(input, SUMMARY_MARKER_GEMINI_QUOTA_ROTATE) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_GEMINI_RATE_RETRY) > 0
    ):
        return SUMMARY_DIAGNOSIS_GEMINI_QUOTA_RETRY

    if runtime_doctor_summary_count(input, SUMMARY_MARKER_PROVIDER_MODEL_FALLBACK) > 0:
        return SUMMARY_DIAGNOSIS_PROVIDER_MODEL_FALLBACK
    if (
        runtime_doctor_summary_count(input, SUMMARY_MARKER_GEMINI_STREAM_RETRY) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_GEMINI_STREAM_FALLBACK) > 0
    ):
        return SUMMARY_DIAGNOSIS_GEMINI_STREAM_RETRY
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_GEMINI_COMPACT_FALLBACK) > 0:
        return SUMMARY_DIAGNOSIS_GEMINI_COMPACT_FALLBACK
    if (
        runtime_doctor_summary_count(input, SUMMARY_MARKER_GEMINI_LIVE_ERROR) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_GEMINI_LIVE_SIDECAR_ERROR) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_GEMINI_LIVE_SESSION_ERROR) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_GEMINI_SIDECAR_ACCEPT_ERROR) > 0
    ):
        return SUMMARY_DIAGNOSIS_GEMINI_LIVE_ERROR

    if runtime_doctor_summary_count(input, SUMMARY_MARKER_STREAM_READ) > 0:
        return SUMMARY_DIAGNOSIS_STREAM_READ
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_LOCAL_WRITER) > 0:
        return SUMMARY_DIAGNOSIS_LOCAL_WRITER
    if (
        runtime_doctor_summary_count(input, SUMMARY_MARKER_CONNECT_TIMEOUT) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_DNS_ERROR) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_TLS_ERROR) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_CONNECT_ERROR) > 0
    ):
        return SUMMARY_DIAGNOSIS_UPSTREAM_CONNECT
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_STATE_SAVE_ERROR) > 0:
        return SUMMARY_DIAGNOSIS_STATE_SAVE
    if (
        runtime_doctor_summary_count(input, SUMMARY_MARKER_STATE_SAVE_BACKPRESSURE) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_JOURNAL_SAVE_BACKPRESSURE) > 0
    ):
        return SUMMARY_DIAGNOSIS_PERSISTENCE
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_SYNC_PROBE_SKIP) > 0:
        return SUMMARY_DIAGNOSIS_SYNC_PROBE
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_PROBE_BACKPRESSURE) > 0:
        return SUMMARY_DIAGNOSIS_PROBE_BACKPRESSURE

    if input.degraded_routes > 0:
        return SUMMARY_DIAGNOSIS_DEGRADED_ROUTES
    if input.orphan_managed_dirs > 0:
        return SUMMARY_DIAGNOSIS_ORPHAN_DIRS
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_PROBE_ERROR) > 0:
        return SUMMARY_DIAGNOSIS_PROBE_ERROR
    if runtime_doctor_summary_count(input, SUMMARY_MARKER_PROBE_START) > 0:
        return SUMMARY_DIAGNOSIS_PROBE_ACTIVITY
    if (
        runtime_doctor_summary_count(input, SUMMARY_MARKER_FIRST_UPSTREAM_CHUNK) > 0
        and runtime_doctor_summary_count(input, SUMMARY_MARKER_FIRST_LOCAL_CHUNK) == 0
    ):
        return SUMMARY_DIAGNOSIS_WRITER_STALL
    if input.runtime_broker_mismatch > 0:
        return SUMMARY_DIAGNOSIS_BROKER_MISMATCH
    if input.prodex_binary_mismatch > 0:
        return SUMMARY_DIAGNOSIS_BINARY_MISMATCH
    if (
        runtime_doctor_summary_count(input, SUMMARY_MARKER_SELECTION_PICK) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_SELECTION_SKIP_CURRENT) > 0
    ):
        return SUMMARY_DIAGNOSIS_SELECTION
    return SUMMARY_DIAGNOSIS_NO_RECENT_FAILURE


def runtime_doctor_summary_validate_input(
    input: ProdexRuntimeDoctorSummaryPlanInput,
) -> Bool:
    # Rust validates the complete fixed marker arena before this call. The
    # fields used below are only read as zero/non-zero decisions, so no
    # unbounded traversal is needed on the Mojo side.
    if input.line_count < 0 or input.line_count > RUNTIME_DOCTOR_PLAN_MAX_COUNT:
        return False
    if (
        input.pointer_exists < 0
        or input.pointer_exists > 1
        or input.log_exists < 0
        or input.log_exists > 1
        or input.startup_audit_risk < 0
        or input.startup_audit_risk > 1
        or input.orphan_managed_dirs < 0
        or input.orphan_managed_dirs > 1
        or input.runtime_broker_mismatch < 0
        or input.runtime_broker_mismatch > 1
        or input.prodex_binary_mismatch < 0
        or input.prodex_binary_mismatch > 1
        or input.persisted_quota_snapshot_risk < 0
        or input.persisted_quota_snapshot_risk > 1
    ):
        return False
    return (
        input.stale_persisted_usage_snapshots >= 0
        and input.stale_persisted_usage_snapshots <= RUNTIME_DOCTOR_PLAN_MAX_COUNT
        and input.persisted_dead_continuations >= 0
        and input.persisted_dead_continuations <= RUNTIME_DOCTOR_PLAN_MAX_COUNT
        and input.suspect_continuations >= 0
        and input.suspect_continuations <= RUNTIME_DOCTOR_PLAN_MAX_COUNT
        and input.degraded_routes >= 0
        and input.degraded_routes <= RUNTIME_DOCTOR_PLAN_MAX_COUNT
    )


def runtime_doctor_summary_reset(
    output: Pointer[mut=True, ProdexRuntimeDoctorSummaryPlan, _],
) -> None:
    output[].abi_version = RUNTIME_DOCTOR_SUMMARY_PLAN_ABI_VERSION
    output[].selection_pressure = SUMMARY_PRESSURE_LOW
    output[].transport_pressure = SUMMARY_PRESSURE_LOW
    output[].persistence_pressure = SUMMARY_PRESSURE_LOW
    output[].quota_freshness_pressure = SUMMARY_PRESSURE_LOW
    output[].startup_audit_pressure = SUMMARY_PRESSURE_LOW
    output[].diagnosis_kind = SUMMARY_DIAGNOSIS_NONE


@export("prodex_mojo_rich_runtime_doctor_summary_plan_v1")
def prodex_mojo_rich_runtime_doctor_summary_plan_v1(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
) abi("C") -> Int64:
    if output_address == 0:
        return 1
    var output = Pointer[
        mut=True, ProdexRuntimeDoctorSummaryPlan, MutUntrackedOrigin
    ](unsafe_from_address=Int(output_address))
    runtime_doctor_summary_reset(output)
    if abi_version != RUNTIME_DOCTOR_SUMMARY_PLAN_ABI_VERSION or input_address == 0:
        return 1
    var input_pointer = Pointer[
        mut=False, ProdexRuntimeDoctorSummaryPlanInput, ImmUntrackedOrigin
    ](unsafe_from_address=Int(input_address))
    var input = input_pointer[].copy()
    if not runtime_doctor_summary_validate_input(input):
        return 1

    var selection = SUMMARY_PRESSURE_LOW
    if runtime_doctor_summary_any_selection(input):
        selection = SUMMARY_PRESSURE_ELEVATED
    var transport = SUMMARY_PRESSURE_LOW
    if runtime_doctor_summary_any_transport(input):
        transport = SUMMARY_PRESSURE_ELEVATED
    var persistence = SUMMARY_PRESSURE_LOW
    if (
        runtime_doctor_summary_count(input, SUMMARY_MARKER_STATE_SAVE_ERROR) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_STATE_SAVE_BACKPRESSURE) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_JOURNAL_SAVE_ERROR) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_JOURNAL_SAVE_BACKPRESSURE) > 0
    ):
        persistence = SUMMARY_PRESSURE_ELEVATED
    elif runtime_doctor_summary_count(input, SUMMARY_MARKER_STATE_SAVE_SKIPPED) > 0:
        persistence = SUMMARY_PRESSURE_ACTIVE
    var quota = SUMMARY_PRESSURE_LOW
    if (
        input.stale_persisted_usage_snapshots > 0
        or input.persisted_quota_snapshot_risk > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_PROBE_ERROR) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_PROBE_BACKPRESSURE) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_SYNC_PROBE_SKIP) > 0
    ):
        quota = SUMMARY_PRESSURE_STALE_RISK
    elif (
        runtime_doctor_summary_count(input, SUMMARY_MARKER_PROBE_START) > 0
        or runtime_doctor_summary_count(input, SUMMARY_MARKER_PROBE_OK) > 0
    ):
        quota = SUMMARY_PRESSURE_ACTIVE
    var startup = SUMMARY_PRESSURE_LOW
    if input.orphan_managed_dirs > 0 or input.startup_audit_risk > 0:
        startup = SUMMARY_PRESSURE_ELEVATED

    output[].selection_pressure = selection
    output[].transport_pressure = transport
    output[].persistence_pressure = persistence
    output[].quota_freshness_pressure = quota
    output[].startup_audit_pressure = startup
    output[].diagnosis_kind = runtime_doctor_summary_select_diagnosis(input)
    return 0


comptime RUNTIME_DOCTOR_STATE_PLAN_ABI_VERSION: Int64 = 1
comptime STATE_OP_QUOTA: Int64 = 0
comptime STATE_OP_SCORE: Int64 = 1
comptime STATE_OP_CIRCUIT: Int64 = 2
comptime STATE_ROUTE_RESPONSES: Int64 = 0
comptime STATE_ROUTE_WEBSOCKET: Int64 = 1
comptime STATE_ROUTE_COMPACT: Int64 = 2
comptime STATE_ROUTE_STANDARD: Int64 = 3
comptime STATE_STATUS_READY: Int64 = 0
comptime STATE_STATUS_THIN: Int64 = 1
comptime STATE_STATUS_CRITICAL: Int64 = 2
comptime STATE_STATUS_EXHAUSTED: Int64 = 3
comptime STATE_STATUS_UNKNOWN: Int64 = 4
comptime STATE_CIRCUIT_CLOSED: Int64 = 0
comptime STATE_CIRCUIT_HALF_OPEN: Int64 = 1
comptime STATE_CIRCUIT_OPEN: Int64 = 2
comptime STATE_INT64_MAX: Int64 = 9223372036854775807

@fieldwise_init
struct ProdexRuntimeDoctorStatePlanInput(Copyable):
    var operation: Int64
    var route_kind: Int64
    var now: Int64
    var checked_at: Int64
    var five_hour_status: Int64
    var five_hour_reset_at: Int64
    var weekly_status: Int64
    var weekly_reset_at: Int64
    var stale_grace_seconds: Int64
    var score: Int64
    var updated_at: Int64
    var decay_seconds: Int64
    var circuit_until: Int64


@fieldwise_init
struct ProdexRuntimeDoctorStatePlan(Copyable):
    var abi_version: Int64
    var freshness: Int64
    var five_hour_status: Int64
    var weekly_status: Int64
    var route_band: Int64
    var effective_score: Int64
    var circuit_state: Int64


def runtime_doctor_state_status_valid(value: Int64) -> Bool:
    return value >= STATE_STATUS_READY and value <= STATE_STATUS_UNKNOWN


def runtime_doctor_state_reset_status(
    status: Int64,
    reset_at: Int64,
    now: Int64,
) -> Int64:
    if reset_at != STATE_INT64_MAX and reset_at <= now:
        return STATE_STATUS_READY
    return status


def runtime_doctor_state_hold_active(
    status: Int64,
    reset_at: Int64,
    now: Int64,
) -> Bool:
    return status == STATE_STATUS_EXHAUSTED and reset_at != STATE_INT64_MAX and reset_at > now


def runtime_doctor_state_hold_expired(
    status: Int64,
    reset_at: Int64,
    now: Int64,
) -> Bool:
    return status == STATE_STATUS_EXHAUSTED and reset_at != STATE_INT64_MAX and reset_at <= now


def runtime_doctor_state_freshness(input: ProdexRuntimeDoctorStatePlanInput) -> Int64:
    if runtime_doctor_state_hold_active(input.five_hour_status, input.five_hour_reset_at, input.now):
        return STATE_STATUS_READY
    if runtime_doctor_state_hold_active(input.weekly_status, input.weekly_reset_at, input.now):
        return STATE_STATUS_READY
    if runtime_doctor_state_hold_expired(input.five_hour_status, input.five_hour_reset_at, input.now):
        return STATE_STATUS_THIN
    if runtime_doctor_state_hold_expired(input.weekly_status, input.weekly_reset_at, input.now):
        return STATE_STATUS_THIN
    if input.now < input.checked_at:
        return STATE_STATUS_READY
    var age: Int64 = 0
    if input.checked_at < 0 and input.now > STATE_INT64_MAX + input.checked_at:
        age = STATE_INT64_MAX
    else:
        age = input.now - input.checked_at
    if age <= input.stale_grace_seconds:
        return STATE_STATUS_READY
    return STATE_STATUS_THIN


def runtime_doctor_state_route_band(
    input: ProdexRuntimeDoctorStatePlanInput,
    five_hour: Int64,
    weekly: Int64,
) -> Int64:
    var band = five_hour
    if weekly > band:
        band = weekly
    var route_status = weekly
    if input.route_kind == STATE_ROUTE_COMPACT or input.route_kind == STATE_ROUTE_STANDARD:
        route_status = five_hour
    if route_status > band:
        band = route_status
    return band


def runtime_doctor_state_effective_score(input: ProdexRuntimeDoctorStatePlanInput) -> Int64:
    if input.now <= input.updated_at:
        return input.score
    var elapsed: Int64 = 0
    if input.updated_at < 0 and input.now > STATE_INT64_MAX + input.updated_at:
        elapsed = STATE_INT64_MAX
    else:
        elapsed = input.now - input.updated_at
    var decay = elapsed / input.decay_seconds
    if decay >= input.score:
        return 0
    return input.score - decay


def runtime_doctor_state_validate_input(input: ProdexRuntimeDoctorStatePlanInput) -> Bool:
    return (
        input.operation >= STATE_OP_QUOTA
        and input.operation <= STATE_OP_CIRCUIT
        and input.route_kind >= STATE_ROUTE_RESPONSES
        and input.route_kind <= STATE_ROUTE_STANDARD
        and runtime_doctor_state_status_valid(input.five_hour_status)
        and runtime_doctor_state_status_valid(input.weekly_status)
        and input.stale_grace_seconds >= 0
        and input.score >= 0
        and input.circuit_until >= -1
        and (input.operation != STATE_OP_SCORE or input.decay_seconds > 0)
    )


def runtime_doctor_state_reset(
    output: Pointer[mut=True, ProdexRuntimeDoctorStatePlan, _],
) -> None:
    output[].abi_version = RUNTIME_DOCTOR_STATE_PLAN_ABI_VERSION
    output[].freshness = STATE_STATUS_READY
    output[].five_hour_status = STATE_STATUS_READY
    output[].weekly_status = STATE_STATUS_READY
    output[].route_band = STATE_STATUS_READY
    output[].effective_score = 0
    output[].circuit_state = STATE_CIRCUIT_CLOSED


@export("prodex_mojo_rich_runtime_doctor_state_plan_v1")
def prodex_mojo_rich_runtime_doctor_state_plan_v1(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
) abi("C") -> Int64:
    if output_address == 0:
        return 1
    var output = Pointer[
        mut=True, ProdexRuntimeDoctorStatePlan, MutUntrackedOrigin
    ](unsafe_from_address=Int(output_address))
    runtime_doctor_state_reset(output)
    if abi_version != RUNTIME_DOCTOR_STATE_PLAN_ABI_VERSION or input_address == 0:
        return 1
    var input_pointer = Pointer[
        mut=False, ProdexRuntimeDoctorStatePlanInput, ImmUntrackedOrigin
    ](unsafe_from_address=Int(input_address))
    var input = input_pointer[].copy()
    if not runtime_doctor_state_validate_input(input):
        return 1
    if input.operation == STATE_OP_QUOTA:
        var five_hour = runtime_doctor_state_reset_status(
            input.five_hour_status, input.five_hour_reset_at, input.now
        )
        var weekly = runtime_doctor_state_reset_status(
            input.weekly_status, input.weekly_reset_at, input.now
        )
        output[].freshness = runtime_doctor_state_freshness(input)
        output[].five_hour_status = five_hour
        output[].weekly_status = weekly
        output[].route_band = runtime_doctor_state_route_band(input, five_hour, weekly)
    elif input.operation == STATE_OP_SCORE:
        output[].effective_score = runtime_doctor_state_effective_score(input)
    else:
        if input.circuit_until < 0:
            output[].circuit_state = STATE_CIRCUIT_CLOSED
        elif input.circuit_until > input.now:
            output[].circuit_state = STATE_CIRCUIT_OPEN
        else:
            output[].circuit_state = STATE_CIRCUIT_HALF_OPEN
    return 0
