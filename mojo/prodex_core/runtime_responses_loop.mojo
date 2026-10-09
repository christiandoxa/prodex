comptime RESPONSES_LOOP_ABI_VERSION: Int64 = 1

comptime RESPONSES_LOOP_PHASE_BUDGET: Int64 = 0
comptime RESPONSES_LOOP_PHASE_CANDIDATE: Int64 = 1

comptime RESPONSES_LOOP_ACTION_ATTEMPT: Int64 = 0
comptime RESPONSES_LOOP_ACTION_WAIT_TRANSIENT: Int64 = 1
comptime RESPONSES_LOOP_ACTION_WAIT_INFLIGHT: Int64 = 2
comptime RESPONSES_LOOP_ACTION_RETURN_COMPACT_FAILURE: Int64 = 3
comptime RESPONSES_LOOP_ACTION_WAIT_COLD_START: Int64 = 4
comptime RESPONSES_LOOP_ACTION_DIRECT_FALLBACK: Int64 = 5
comptime RESPONSES_LOOP_ACTION_RETURN_FINAL_FAILURE: Int64 = 6
comptime RESPONSES_LOOP_ACTION_RETURN_WITHOUT_ROTATION: Int64 = 7

def responses_loop_bool_is_valid(value: Int64) -> Bool:
    return value == 0 or value == 1


# Owns only pre-commit response-loop precedence. The Rust caller supplies
# facts acquired from state and performs the selected wait/fallback effects.
@export("prodex_runtime_responses_loop_action_v1")
def prodex_runtime_responses_loop_action_v1(
    abi_version: Int64,
    phase: Int64,
    budget_exhausted: Int64,
    hard_affinity: Int64,
    compact_followup: Int64,
    transient_recovery_pending: Int64,
    inflight_relief_pending: Int64,
    cold_start_pending: Int64,
    direct_fallback_allowed: Int64,
    stream_committed: Int64,
) abi("C") -> Int64:
    if abi_version != RESPONSES_LOOP_ABI_VERSION:
        return -4
    if (
        phase < RESPONSES_LOOP_PHASE_BUDGET
        or phase > RESPONSES_LOOP_PHASE_CANDIDATE
        or not responses_loop_bool_is_valid(budget_exhausted)
        or not responses_loop_bool_is_valid(hard_affinity)
        or not responses_loop_bool_is_valid(compact_followup)
        or not responses_loop_bool_is_valid(transient_recovery_pending)
        or not responses_loop_bool_is_valid(inflight_relief_pending)
        or not responses_loop_bool_is_valid(cold_start_pending)
        or not responses_loop_bool_is_valid(direct_fallback_allowed)
        or not responses_loop_bool_is_valid(stream_committed)
    ):
        return -1

    if stream_committed == 1:
        return RESPONSES_LOOP_ACTION_RETURN_WITHOUT_ROTATION

    if phase == RESPONSES_LOOP_PHASE_BUDGET:
        if budget_exhausted == 0:
            return RESPONSES_LOOP_ACTION_ATTEMPT
        if compact_followup == 1:
            return RESPONSES_LOOP_ACTION_RETURN_COMPACT_FAILURE
        if transient_recovery_pending == 1 and hard_affinity == 0:
            return RESPONSES_LOOP_ACTION_WAIT_TRANSIENT
        if direct_fallback_allowed == 1 and hard_affinity == 0:
            return RESPONSES_LOOP_ACTION_DIRECT_FALLBACK
        return RESPONSES_LOOP_ACTION_RETURN_FINAL_FAILURE

    if transient_recovery_pending == 1 and hard_affinity == 0:
        return RESPONSES_LOOP_ACTION_WAIT_TRANSIENT
    if inflight_relief_pending == 1:
        return RESPONSES_LOOP_ACTION_WAIT_INFLIGHT
    if compact_followup == 1:
        return RESPONSES_LOOP_ACTION_RETURN_COMPACT_FAILURE
    if cold_start_pending == 1:
        return RESPONSES_LOOP_ACTION_WAIT_COLD_START
    if direct_fallback_allowed == 1 and hard_affinity == 0:
        return RESPONSES_LOOP_ACTION_DIRECT_FALLBACK
    return RESPONSES_LOOP_ACTION_RETURN_FINAL_FAILURE
