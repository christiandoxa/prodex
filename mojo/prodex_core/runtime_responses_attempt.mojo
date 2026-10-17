from std.memory import Pointer

# Pre-commit Responses recovery policy.

comptime RESPONSES_ATTEMPT_RECOVERY_ABI_VERSION: Int64 = 1
comptime RESPONSES_ATTEMPT_RECOVERY_SKIP: Int64 = 0
comptime RESPONSES_ATTEMPT_RECOVERY_FULL_HISTORY: Int64 = 1
comptime RESPONSES_ATTEMPT_RETRY_SIGNAL_ABI_VERSION: Int64 = 1


def responses_attempt_recovery_bool(value: Int64) -> Bool:
    return value == 0 or value == 1


@export("prodex_runtime_responses_attempt_recovery_v1")
def prodex_runtime_responses_attempt_recovery_v1(
    abi_version: Int64,
    exact_invalid_previous_response_id: Int64,
    full_history_fallback_used: Int64,
    previous_response_present: Int64,
    owner_matches_profile: Int64,
    session_present: Int64,
    reconstructable_full_history: Int64,
    stream_committed: Int64,
    hard_affinity: Int64,
) abi("C") -> Int64:
    if abi_version != RESPONSES_ATTEMPT_RECOVERY_ABI_VERSION:
        return -4
    if (
        not responses_attempt_recovery_bool(exact_invalid_previous_response_id)
        or not responses_attempt_recovery_bool(full_history_fallback_used)
        or not responses_attempt_recovery_bool(previous_response_present)
        or not responses_attempt_recovery_bool(owner_matches_profile)
        or not responses_attempt_recovery_bool(session_present)
        or not responses_attempt_recovery_bool(reconstructable_full_history)
        or not responses_attempt_recovery_bool(stream_committed)
        or not responses_attempt_recovery_bool(hard_affinity)
    ):
        return -1

    # A committed stream is already observable upstream and can never be replayed.
    if stream_committed == 1:
        return RESPONSES_ATTEMPT_RECOVERY_SKIP
    # Hard affinity may only recover on the verified owner; it never rotates here.
    if hard_affinity == 1 and owner_matches_profile == 0:
        return RESPONSES_ATTEMPT_RECOVERY_SKIP
    if (
        exact_invalid_previous_response_id == 1
        and full_history_fallback_used == 0
        and previous_response_present == 1
        and owner_matches_profile == 1
        and session_present == 1
        and reconstructable_full_history == 1
    ):
        return RESPONSES_ATTEMPT_RECOVERY_FULL_HISTORY
    return RESPONSES_ATTEMPT_RECOVERY_SKIP


@export("prodex_runtime_responses_attempt_retry_signal_v1")
def prodex_runtime_responses_attempt_retry_signal_v1(
    abi_version: Int64,
    previous_response_present: Int64,
    owner_matches_profile: Int64,
    fallback_available: Int64,
) abi("C") -> Int64:
    if abi_version != RESPONSES_ATTEMPT_RETRY_SIGNAL_ABI_VERSION:
        return -4
    if (
        not responses_attempt_recovery_bool(previous_response_present)
        or not responses_attempt_recovery_bool(owner_matches_profile)
        or not responses_attempt_recovery_bool(fallback_available)
    ):
        return -1
    if (
        previous_response_present == 1
        and owner_matches_profile == 1
        and fallback_available == 1
    ):
        return RESPONSES_ATTEMPT_RECOVERY_FULL_HISTORY
    return RESPONSES_ATTEMPT_RECOVERY_SKIP

# Previous-response recovery combines the already-Mojo retry signal with
# route-level stale-continuation and affinity-release policy. Rust owns the
# prior parsed decision and applies host bindings, not the state transition.
comptime PREVIOUS_RESPONSE_RECOVERY_TRANSITION_ABI_VERSION: Int64 = 1
comptime PREVIOUS_RESPONSE_RECOVERY_RETRY_OWNER: UInt64 = 0
comptime PREVIOUS_RESPONSE_RECOVERY_STALE: UInt64 = 1
comptime PREVIOUS_RESPONSE_RECOVERY_ROTATE: UInt64 = 2
comptime PREVIOUS_RESPONSE_RECOVERY_MAX_COUNT: UInt64 = 18446744073709551615


@export("prodex_runtime_previous_response_recovery_transition_v1")
def prodex_runtime_previous_response_recovery_transition_v1(
    abi_version: Int64,
    retry_delay_present: Int64,
    stale_continuation: Int64,
    fresh_fallback_blocked: Int64,
    policy_fail_stale: Int64,
    policy_log_fresh_fallback_blocked: Int64,
    policy_clear_trusted_affinity: Int64,
    retry_index: UInt64,
    output_address: UInt,
    output_capacity: Int64,
) abi("C") -> Int64:
    if abi_version != PREVIOUS_RESPONSE_RECOVERY_TRANSITION_ABI_VERSION:
        return 4
    if output_address == 0 or output_capacity < 7:
        return 1
    for flag in [
        retry_delay_present,
        stale_continuation,
        fresh_fallback_blocked,
        policy_fail_stale,
        policy_log_fresh_fallback_blocked,
        policy_clear_trusted_affinity,
    ]:
        if flag != 0 and flag != 1:
            return 1
    if retry_delay_present == 1 and retry_index == PREVIOUS_RESPONSE_RECOVERY_MAX_COUNT:
        return 1
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    # Slots: action, next retry index, reset state, log blocked, release
    # affinity, clear response binding, clear trusted affinity.
    if retry_delay_present == 1:
        output[unsafe_offset=0] = PREVIOUS_RESPONSE_RECOVERY_RETRY_OWNER
        output[unsafe_offset=1] = retry_index + 1
        for offset in range(2, 7):
            output[unsafe_offset=offset] = 0
    elif policy_fail_stale == 1 and stale_continuation == 1:
        output[unsafe_offset=0] = PREVIOUS_RESPONSE_RECOVERY_STALE
        output[unsafe_offset=1] = 0
        output[unsafe_offset=2] = 1
        for offset in range(3, 7):
            output[unsafe_offset=offset] = 0
    else:
        output[unsafe_offset=0] = PREVIOUS_RESPONSE_RECOVERY_ROTATE
        output[unsafe_offset=1] = 0
        output[unsafe_offset=2] = 1
        output[unsafe_offset=3] = UInt64(
            policy_log_fresh_fallback_blocked == 1
            and fresh_fallback_blocked == 1
        )
        output[unsafe_offset=4] = 1
        output[unsafe_offset=5] = 1
        output[unsafe_offset=6] = UInt64(policy_clear_trusted_affinity == 1)
    return 0
