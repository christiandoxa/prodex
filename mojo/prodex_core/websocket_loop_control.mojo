from std.memory import Pointer

comptime WEBSOCKET_LOOP_CONTROL_ABI_VERSION: Int64 = 1
comptime WEBSOCKET_LOOP_CONTROL_FIELD_COUNT: Int64 = 10


@export("prodex_runtime_websocket_precommit_exhausted_v1")
def prodex_runtime_websocket_precommit_exhausted_v1(
    abi_version: Int64,
    fields_address: UInt,
    field_count: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != WEBSOCKET_LOOP_CONTROL_ABI_VERSION:
        return 4
    if (
        field_count != WEBSOCKET_LOOP_CONTROL_FIELD_COUNT
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
    if (
        fields[unsafe_offset=0] > 1
        or fields[unsafe_offset=1] > 1
        or fields[unsafe_offset=3] > 1
        or fields[unsafe_offset=4] > 1
        or fields[unsafe_offset=5] > 1
        or fields[unsafe_offset=6] > 1
        or fields[unsafe_offset=7] > 1
    ):
        return 1

    if fields[unsafe_offset=0] == 1:
        output[] = 0
        return 0

    if (
        fields[unsafe_offset=3] == 0
        and fields[unsafe_offset=7] == 1
        and (
            fields[unsafe_offset=4] == 1
            or fields[unsafe_offset=5] == 1
            or fields[unsafe_offset=6] == 1
        )
    ):
        output[] = 0
        return 0

    if fields[unsafe_offset=8] == 0:
        if (
            fields[unsafe_offset=6] == 1
            and fields[unsafe_offset=3] == 0
            and fields[unsafe_offset=1] == 0
            and fields[unsafe_offset=2] < fields[unsafe_offset=9]
        ):
            output[] = 0
        elif fields[unsafe_offset=1] == 1:
            output[] = 1
        else:
            output[] = 0
        return 0

    if fields[unsafe_offset=9] == 0:
        return 1
    if fields[unsafe_offset=1] >= fields[unsafe_offset=9]:
        output[] = 1
    else:
        output[] = 0
    return 0
