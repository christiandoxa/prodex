from std.memory import Pointer

comptime PRODEX_STATE_POLICY_ABI_VERSION: Int64 = 1
comptime PRODEX_STATE_POLICY_OK: Int64 = 0
comptime PRODEX_STATE_POLICY_INVALID: Int64 = 1
comptime PRODEX_STATE_POLICY_ABI: Int64 = 4

comptime STATE_POLICY_PROVIDER_CAPABILITIES: Int64 = 0
comptime STATE_POLICY_BINDING_MERGE: Int64 = 1
comptime STATE_POLICY_ACTIVE_PROFILE: Int64 = 2
comptime STATE_POLICY_LAST_RUN_KEEP: Int64 = 3
comptime STATE_POLICY_BINDING_KEEP: Int64 = 4

comptime INT64_MAX: Int64 = 9223372036854775807
comptime INT64_MIN: Int64 = -9223372036854775808


def state_bool(value: Int64) -> Bool:
    return value == 1


def state_valid_bool(value: Int64) -> Bool:
    return value == 0 or value == 1


def state_saturating_sub(left: Int64, right: Int64) -> Int64:
    if right > 0 and left < INT64_MIN + right:
        return INT64_MIN
    if right < 0 and left > INT64_MAX + right:
        return INT64_MAX
    return left - right


def state_provider_capabilities(
    provider: Int64, output: Pointer[mut=True, Int64, _]
) -> Int64:
    if provider < 0 or provider > 5:
        return PRODEX_STATE_POLICY_INVALID

    # route policy: native=0, responses-adapter=1, external-cli=2
    # quota shape: OpenAI=0, Gemini=1, Copilot=2, external=3
    if provider == 0:
        output[0] = 0
        output[1] = 0
        output[2] = 1
        output[3] = 1
        output[4] = 0
    elif provider == 1:
        output[0] = 1
        output[1] = 1
        output[2] = 1
        output[3] = 0
        output[4] = 1
    elif provider == 3:
        output[0] = 1
        output[1] = 2
        output[2] = 1
        output[3] = 0
        output[4] = 1
    elif provider == 5:
        output[0] = 2
        output[1] = 3
        output[2] = 0
        output[3] = 0
        output[4] = 1
    else:
        output[0] = 1
        output[1] = 3
        output[2] = 1
        output[3] = 0
        output[4] = 1

    output[5] = output[3]
    output[6] = output[3]
    return PRODEX_STATE_POLICY_OK


def state_binding_merge(
    left_conflict: Int64,
    right_conflict: Int64,
    profile_names_equal: Int64,
    left_identity_present: Int64,
    right_identity_present: Int64,
    identities_conflict: Int64,
    left_bound_at: Int64,
    right_bound_at: Int64,
    output: Pointer[mut=True, Int64, _],
) -> Int64:
    if (
        not state_valid_bool(left_conflict)
        or not state_valid_bool(right_conflict)
        or not state_valid_bool(profile_names_equal)
        or not state_valid_bool(left_identity_present)
        or not state_valid_bool(right_identity_present)
        or not state_valid_bool(identities_conflict)
    ):
        return PRODEX_STATE_POLICY_INVALID

    var bound_at = max(left_bound_at, right_bound_at)
    output[1] = bound_at

    if (
        state_bool(left_conflict)
        or state_bool(right_conflict)
        or not state_bool(profile_names_equal)
        or state_bool(identities_conflict)
    ):
        output[0] = 0
        return PRODEX_STATE_POLICY_OK

    if state_bool(left_identity_present) and not state_bool(right_identity_present):
        output[0] = 1
    elif not state_bool(left_identity_present) and state_bool(right_identity_present):
        output[0] = 2
    elif right_bound_at > left_bound_at:
        output[0] = 2
    else:
        output[0] = 1
    return PRODEX_STATE_POLICY_OK


def state_active_profile(
    existing_present: Int64,
    incoming_present: Int64,
    names_equal: Int64,
    existing_selected_present: Int64,
    incoming_selected_present: Int64,
    existing_selected_at: Int64,
    incoming_selected_at: Int64,
    output: Pointer[mut=True, Int64, _],
) -> Int64:
    if (
        not state_valid_bool(existing_present)
        or not state_valid_bool(incoming_present)
        or not state_valid_bool(names_equal)
        or not state_valid_bool(existing_selected_present)
        or not state_valid_bool(incoming_selected_present)
    ):
        return PRODEX_STATE_POLICY_INVALID

    if state_bool(incoming_present):
        if (
            state_bool(existing_present)
            and not state_bool(names_equal)
        ):
            var existing_greater = False
            if state_bool(existing_selected_present):
                existing_greater = (
                    not state_bool(incoming_selected_present)
                    or existing_selected_at > incoming_selected_at
                )
            output[0] = 1 if existing_greater else 2
        else:
            output[0] = 2
    elif state_bool(existing_present):
        output[0] = 1
    else:
        output[0] = 0
    return PRODEX_STATE_POLICY_OK


@export("prodex_state_policy_v1")
def prodex_state_policy_v1(
    abi_version: Int64,
    mode: Int64,
    input0: Int64,
    input1: Int64,
    input2: Int64,
    input3: Int64,
    input4: Int64,
    input5: Int64,
    input6: Int64,
    input7: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_STATE_POLICY_ABI_VERSION:
        return PRODEX_STATE_POLICY_ABI
    if (
        mode < STATE_POLICY_PROVIDER_CAPABILITIES
        or mode > STATE_POLICY_BINDING_KEEP
        or output_address == 0
    ):
        return PRODEX_STATE_POLICY_INVALID

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(8):
        output[unsafe_offset=index] = 0

    if mode == STATE_POLICY_PROVIDER_CAPABILITIES:
        return state_provider_capabilities(input0, output)

    if mode == STATE_POLICY_BINDING_MERGE:
        return state_binding_merge(
            input0,
            input1,
            input2,
            input3,
            input4,
            input5,
            input6,
            input7,
            output,
        )

    if mode == STATE_POLICY_ACTIVE_PROFILE:
        return state_active_profile(
            input0, input1, input2, input3, input4, input5, input6, output
        )

    if mode == STATE_POLICY_LAST_RUN_KEEP:
        if not state_valid_bool(input0):
            return PRODEX_STATE_POLICY_INVALID
        var oldest_allowed = state_saturating_sub(input2, input3)
        output[0] = Int64(state_bool(input0) and input1 >= oldest_allowed)
        return PRODEX_STATE_POLICY_OK

    if (
        not state_valid_bool(input0)
        or not state_valid_bool(input1)
        or not state_valid_bool(input2)
        or not state_valid_bool(input6)
    ):
        return PRODEX_STATE_POLICY_INVALID
    if state_bool(input0):
        output[0] = 1
        return PRODEX_STATE_POLICY_OK
    if not state_bool(input6):
        output[0] = Int64(state_bool(input1) or state_bool(input2))
        return PRODEX_STATE_POLICY_OK

    var oldest_allowed = state_saturating_sub(input4, input5)
    output[0] = Int64(
        (state_bool(input1) or state_bool(input2)) and input3 >= oldest_allowed
    )
    return PRODEX_STATE_POLICY_OK
