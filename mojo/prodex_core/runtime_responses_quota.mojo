# Bounded response-quota continuation policy.
comptime RESPONSES_QUOTA_ABI_VERSION: Int64 = 1


@export("prodex_runtime_responses_quota_turn_state_replay_v1")
def prodex_runtime_responses_quota_turn_state_replay_v1(
    abi_version: Int64,
    previous_response_present: Int64,
    turn_state_present: Int64,
    turn_state_owner_matches: Int64,
    compact_followup_present: Int64,
    reconstructable_full_history: Int64,
) abi("C") -> Int64:
    if abi_version != RESPONSES_QUOTA_ABI_VERSION:
        return -4
    if (
        previous_response_present < 0
        or previous_response_present > 1
        or turn_state_present < 0
        or turn_state_present > 1
        or turn_state_owner_matches < 0
        or turn_state_owner_matches > 1
        or compact_followup_present < 0
        or compact_followup_present > 1
        or reconstructable_full_history < 0
        or reconstructable_full_history > 1
    ):
        return -1
    return Int64(
        previous_response_present == 0
        and turn_state_present == 1
        and turn_state_owner_matches == 1
        and compact_followup_present == 0
        and reconstructable_full_history == 1
    )
