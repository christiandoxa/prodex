from std.memory import Pointer

comptime RUNTIME_STATE_QUOTA_ABI_VERSION: Int64 = 1
comptime RUNTIME_STATE_QUOTA_OK: Int64 = 0
comptime RUNTIME_STATE_QUOTA_INVALID: Int64 = 1
comptime RUNTIME_STATE_QUOTA_ABI: Int64 = 4

comptime MODE_TIMESTAMP_PERSIST: Int64 = 0
comptime MODE_FRESHNESS: Int64 = 1
comptime MODE_SNAPSHOT_USABLE: Int64 = 2
comptime MODE_PROBE_APPLY: Int64 = 3

comptime INT64_MAX: Int64 = 9223372036854775807
comptime INT64_MIN: Int64 = -9223372036854775808


def state_quota_valid_bool(value: Int64) -> Bool:
    return value == 0 or value == 1


def state_quota_saturating_sub(left: Int64, right: Int64) -> Int64:
    if right > 0 and left < INT64_MIN + right:
        return INT64_MIN
    if right < 0 and left > INT64_MAX + right:
        return INT64_MAX
    return left - right


def state_quota_saturating_add(left: Int64, right: Int64) -> Int64:
    if right > 0 and left > INT64_MAX - right:
        return INT64_MAX
    if right < 0 and left < INT64_MIN - right:
        return INT64_MIN
    return left + right


@export("prodex_runtime_state_quota_policy_v1")
def prodex_runtime_state_quota_policy_v1(
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
    input8: Int64,
    input9: Int64,
    input10: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != RUNTIME_STATE_QUOTA_ABI_VERSION:
        return RUNTIME_STATE_QUOTA_ABI
    if mode < MODE_TIMESTAMP_PERSIST or mode > MODE_PROBE_APPLY or output_address == 0:
        return RUNTIME_STATE_QUOTA_INVALID

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(8):
        output[unsafe_offset=index] = 0

    if mode == MODE_TIMESTAMP_PERSIST:
        output[unsafe_offset=0] = Int64(
            state_quota_saturating_sub(input1, input0) > input2
        )
        return RUNTIME_STATE_QUOTA_OK

    if mode == MODE_FRESHNESS:
        var age = state_quota_saturating_sub(input1, input0)
        if age <= input2:
            output[unsafe_offset=0] = 0
        elif age <= input3:
            output[unsafe_offset=0] = 1
        else:
            output[unsafe_offset=0] = 2
        return RUNTIME_STATE_QUOTA_OK

    if mode == MODE_SNAPSHOT_USABLE:
        if not state_quota_valid_bool(input0) or not state_quota_valid_bool(input2):
            return RUNTIME_STATE_QUOTA_INVALID
        var five_active = (
            input0 == 1 and input1 != INT64_MAX and input1 > input5
        )
        var weekly_active = (
            input2 == 1 and input3 != INT64_MAX and input3 > input5
        )
        var five_expired = (
            input0 == 1 and input1 != INT64_MAX and input1 <= input5
        )
        var weekly_expired = (
            input2 == 1 and input3 != INT64_MAX and input3 <= input5
        )
        var hold_active = five_active or weekly_active
        var hold_expired = five_expired or weekly_expired
        output[unsafe_offset=1] = Int64(hold_active)
        output[unsafe_offset=2] = Int64(hold_expired)

        if hold_active:
            output[unsafe_offset=0] = 1
        elif hold_expired:
            output[unsafe_offset=0] = 0
        else:
            output[unsafe_offset=0] = Int64(
                state_quota_saturating_sub(input5, input4) <= input6
            )
        return RUNTIME_STATE_QUOTA_OK

    # probe apply:
    # 0 previous snapshot present
    # 1 snapshots materially match
    # 2 previous checked_at
    # 3 previous retry present
    # 4 previous retry until
    # 5 blocking reset present
    # 6 blocking reset
    # 7 quota blocked
    # 8 now
    # 9 quota quarantine fallback seconds
    # 10 touch persist interval seconds
    if (
        not state_quota_valid_bool(input0)
        or not state_quota_valid_bool(input1)
        or not state_quota_valid_bool(input3)
        or not state_quota_valid_bool(input5)
        or not state_quota_valid_bool(input7)
    ):
        return RUNTIME_STATE_QUOTA_INVALID


    var snapshot_should_persist = (
        input0 == 0
        or input1 == 0
        or state_quota_saturating_sub(input8, input2) > input10
    )
    output[unsafe_offset=0] = Int64(snapshot_should_persist)

    var blocking_present = input5 == 1 and input6 > input8
    output[unsafe_offset=1] = Int64(blocking_present)
    if blocking_present:
        output[unsafe_offset=2] = input6

    if input7 == 0:
        return RUNTIME_STATE_QUOTA_OK

    var quarantine_until = (
        input6 if blocking_present
        else state_quota_saturating_add(input8, input9)
    )
    var retry_until = quarantine_until
    if input3 == 1:
        retry_until = max(input4, quarantine_until)
    output[unsafe_offset=3] = 1
    output[unsafe_offset=4] = retry_until
    output[unsafe_offset=5] = Int64(input3 == 0 or retry_until != input4)
    return RUNTIME_STATE_QUOTA_OK
