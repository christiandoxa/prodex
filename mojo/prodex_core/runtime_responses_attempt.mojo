# Pre-commit Responses recovery policy.

comptime RESPONSES_ATTEMPT_RECOVERY_ABI_VERSION: Int64 = 1
comptime RESPONSES_ATTEMPT_RECOVERY_SKIP: Int64 = 0
comptime RESPONSES_ATTEMPT_RECOVERY_FULL_HISTORY: Int64 = 1


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
