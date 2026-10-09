from std.memory import Pointer


comptime NONCOMPACT_LOOP_ABI_VERSION: Int64 = 1
comptime NONCOMPACT_BUDGET_FIELD_COUNT: Int64 = 8
comptime NONCOMPACT_ACTION_FIELD_COUNT: Int64 = 12

comptime NONCOMPACT_BUDGET_PROCEED: Int64 = 0
comptime NONCOMPACT_BUDGET_WAIT_TRANSIENT: Int64 = 1
comptime NONCOMPACT_BUDGET_RETURN: Int64 = 2

comptime NONCOMPACT_ACTION_ATTEMPT_PREFERRED: Int64 = 0
comptime NONCOMPACT_ACTION_SELECT_FRESH: Int64 = 1
comptime NONCOMPACT_ACTION_ATTEMPT_CANDIDATE: Int64 = 2
comptime NONCOMPACT_ACTION_WAIT_INFLIGHT: Int64 = 3
comptime NONCOMPACT_ACTION_CONTINUE: Int64 = 4
comptime NONCOMPACT_ACTION_WAIT_COLD_START: Int64 = 5
comptime NONCOMPACT_ACTION_WAIT_TRANSIENT: Int64 = 6
comptime NONCOMPACT_ACTION_RETURN: Int64 = 7


def noncompact_bool_fields_valid(
    fields: Pointer[mut=False, UInt64, _], count: Int64
) -> Bool:
    for index in range(count):
        if fields[unsafe_offset=index] > 1:
            return False
    return True


@export("prodex_runtime_noncompact_precommit_decision_v1")
def prodex_runtime_noncompact_precommit_decision_v1(
    abi_version: Int64,
    fields_address: UInt,
    field_count: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != NONCOMPACT_LOOP_ABI_VERSION:
        return 4
    if (
        field_count != NONCOMPACT_BUDGET_FIELD_COUNT
        or fields_address == 0
        or output_address == 0
    ):
        return 1

    var fields = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(fields_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    if not noncompact_bool_fields_valid(fields, 4):
        return 1

    # The generic elapsed/attempt budget is already computed by Mojo. This
    # policy owns only the standard route's transient and sweep precedence.
    output[] = NONCOMPACT_BUDGET_PROCEED
    if (
        fields[unsafe_offset=1] == 0
        and fields[unsafe_offset=2] == 1
        and fields[unsafe_offset=3] == 1
    ):
        return 0

    var profile_count = fields[unsafe_offset=6]
    if profile_count == 0:
        profile_count = 1
    var exhausted: Bool
    if fields[unsafe_offset=4] == 0:
        if fields[unsafe_offset=5] < profile_count:
            return 0
        exhausted = fields[unsafe_offset=0] == 1
    else:
        exhausted = fields[unsafe_offset=5] >= fields[unsafe_offset=7]

    if not exhausted:
        return 0
    output[] = (
        NONCOMPACT_BUDGET_RETURN
        if fields[unsafe_offset=1] == 1
        else NONCOMPACT_BUDGET_WAIT_TRANSIENT
    )
    return 0


@export("prodex_runtime_noncompact_next_action_v1")
def prodex_runtime_noncompact_next_action_v1(
    abi_version: Int64,
    fields_address: UInt,
    field_count: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != NONCOMPACT_LOOP_ABI_VERSION:
        return 4
    if (
        field_count != NONCOMPACT_ACTION_FIELD_COUNT
        or fields_address == 0
        or output_address == 0
    ):
        return 1

    var fields = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(fields_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    # Stage is field zero; all remaining fields are booleans.
    if fields[unsafe_offset=0] > 4:
        return 1
    for index in range(11):
        if fields[unsafe_offset=index + 1] > 1:
            return 1

    var stage = fields[unsafe_offset=0]
    if stage == 0:
        if (
            fields[unsafe_offset=1] == 1
            and (
                fields[unsafe_offset=2] == 1
                or fields[unsafe_offset=3] == 0
            )
        ):
            output[] = NONCOMPACT_ACTION_ATTEMPT_PREFERRED
        else:
            output[] = NONCOMPACT_ACTION_SELECT_FRESH
        return 0

    if stage == 1:
        output[] = (
            NONCOMPACT_ACTION_ATTEMPT_CANDIDATE
            if fields[unsafe_offset=4] == 1
            else NONCOMPACT_ACTION_WAIT_INFLIGHT
        )
        return 0

    if stage == 2:
        output[] = (
            NONCOMPACT_ACTION_ATTEMPT_CANDIDATE
            if fields[unsafe_offset=5] == 1
            or fields[unsafe_offset=6] == 0
            else NONCOMPACT_ACTION_WAIT_INFLIGHT
        )
        return 0

    if stage == 3:
        if fields[unsafe_offset=7] == 1:
            output[] = NONCOMPACT_ACTION_CONTINUE
        elif (
            fields[unsafe_offset=8] == 0
            and fields[unsafe_offset=9] == 1
            and fields[unsafe_offset=10] == 0
        ):
            output[] = NONCOMPACT_ACTION_WAIT_COLD_START
        elif fields[unsafe_offset=8] == 0:
            output[] = NONCOMPACT_ACTION_WAIT_TRANSIENT
        else:
            output[] = NONCOMPACT_ACTION_RETURN
        return 0

    output[] = (
        NONCOMPACT_ACTION_CONTINUE
        if fields[unsafe_offset=11] == 1
        else NONCOMPACT_ACTION_RETURN
    )
    return 0
