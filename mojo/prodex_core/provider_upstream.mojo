from std.memory import Pointer



comptime PROVIDER_UPSTREAM_ABI_VERSION: Int64 = 1
comptime PROVIDER_UPSTREAM_STATUS_OK: Int64 = 0
comptime PROVIDER_UPSTREAM_STATUS_INVALID: Int64 = 1
comptime PROVIDER_UPSTREAM_STATUS_ABI: Int64 = 4
comptime PROVIDER_UPSTREAM_MAX_ATTEMPTS: Int64 = 256

comptime PROVIDER_UPSTREAM_OP_DISPATCH: Int64 = 0
comptime PROVIDER_UPSTREAM_OP_ROUTE_KIND: Int64 = 1
comptime PROVIDER_UPSTREAM_OP_STANDARD_PATH: Int64 = 2
comptime PROVIDER_UPSTREAM_OP_AUTH_SHAPE: Int64 = 3
comptime PROVIDER_UPSTREAM_OP_ATTEMPT_INDEX: Int64 = 4

comptime PROVIDER_UPSTREAM_ROUTE_RESPONSES: Int64 = 0
comptime PROVIDER_UPSTREAM_ROUTE_COMPACT: Int64 = 1
comptime PROVIDER_UPSTREAM_ROUTE_STANDARD: Int64 = 3

comptime PROVIDER_UPSTREAM_AUTH_PRESERVE: Int64 = 0
comptime PROVIDER_UPSTREAM_AUTH_BEARER: Int64 = 1
comptime PROVIDER_UPSTREAM_AUTH_ANTHROPIC_BETA: Int64 = 2
comptime PROVIDER_UPSTREAM_AUTH_X_API_KEY: Int64 = 3
comptime PROVIDER_UPSTREAM_AUTH_GOOGLE_API_KEY: Int64 = 4


def provider_upstream_ends_with(
    address: UInt, length: Int64, literal: StringSlice
) -> Bool:
    var expected = Int64(literal.byte_length())
    if address == 0 or length < expected:
        return False
    var input = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    var target = literal.unsafe_ptr()
    var start = length - expected
    for index in range(expected):
        if input[unsafe_offset=start + index] != target[unsafe_offset=index]:
            return False
    return True


@export("prodex_provider_upstream_policy_v1")
def prodex_provider_upstream_policy_v1(
    abi_version: Int64,
    operation: Int64,
    a: Int64,
    b: Int64,
    c: Int64,
    d: Int64,
    e: Int64,
    f: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != PROVIDER_UPSTREAM_ABI_VERSION:
        return PROVIDER_UPSTREAM_STATUS_ABI
    if (
        operation < PROVIDER_UPSTREAM_OP_DISPATCH
        or operation > PROVIDER_UPSTREAM_OP_ATTEMPT_INDEX
        or output_address == 0
    ):
        return PROVIDER_UPSTREAM_STATUS_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )

    if operation == PROVIDER_UPSTREAM_OP_DISPATCH:
        if a < 0 or a > 5 or b < 0 or b > 5 or c < 0 or c > 10 or (d != 0 and d != 1):
            return PROVIDER_UPSTREAM_STATUS_INVALID
        output[] = a if a == b else -1
        return PROVIDER_UPSTREAM_STATUS_OK

    if operation == PROVIDER_UPSTREAM_OP_ROUTE_KIND:
        if a < 0 or a > 10:
            return PROVIDER_UPSTREAM_STATUS_INVALID
        if a == 0 or a == 2:
            output[] = PROVIDER_UPSTREAM_ROUTE_RESPONSES
        elif a == 1:
            output[] = PROVIDER_UPSTREAM_ROUTE_COMPACT
        else:
            output[] = PROVIDER_UPSTREAM_ROUTE_STANDARD
        return PROVIDER_UPSTREAM_STATUS_OK

    if operation == PROVIDER_UPSTREAM_OP_STANDARD_PATH:
        if a < 0 or a > 4 or (b != 0 and b != 1):
            return PROVIDER_UPSTREAM_STATUS_INVALID
        output[] = Int64(a == 1 and b == 1)
        return PROVIDER_UPSTREAM_STATUS_OK

    if operation == PROVIDER_UPSTREAM_OP_AUTH_SHAPE:
        if (
            a < 0
            or a > 5
            or (b != 0 and b != 1)
            or (c != 0 and c != 1)
            or (d != 0 and d != 1)
        ):
            return PROVIDER_UPSTREAM_STATUS_INVALID
        if a == 0:
            output[] = PROVIDER_UPSTREAM_AUTH_PRESERVE if b == 0 else PROVIDER_UPSTREAM_AUTH_BEARER
        elif a == 1:
            if b == 1:
                output[] = PROVIDER_UPSTREAM_AUTH_ANTHROPIC_BETA
            else:
                output[] = PROVIDER_UPSTREAM_AUTH_X_API_KEY if c == 1 else PROVIDER_UPSTREAM_AUTH_BEARER
        elif a == 2:
            output[] = PROVIDER_UPSTREAM_AUTH_BEARER
        elif a == 3:
            output[] = PROVIDER_UPSTREAM_AUTH_X_API_KEY if c == 1 else PROVIDER_UPSTREAM_AUTH_BEARER
        elif a == 4:
            output[] = PROVIDER_UPSTREAM_AUTH_BEARER if d == 1 or b == 1 else PROVIDER_UPSTREAM_AUTH_GOOGLE_API_KEY
        else:
            return PROVIDER_UPSTREAM_STATUS_INVALID
        return PROVIDER_UPSTREAM_STATUS_OK

    if a <= 0 or a > PROVIDER_UPSTREAM_MAX_ATTEMPTS or b < 0 or b >= a or c < 0 or c >= a:
        return PROVIDER_UPSTREAM_STATUS_INVALID
    output[] = (b + c) % a
    return PROVIDER_UPSTREAM_STATUS_OK


@export("prodex_provider_upstream_suffix_plan_v1")
def prodex_provider_upstream_suffix_plan_v1(
    abi_version: Int64,
    mode: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != PROVIDER_UPSTREAM_ABI_VERSION:
        return PROVIDER_UPSTREAM_STATUS_ABI
    if (
        mode < 0
        or mode > 1
        or length < 0
        or length > 4_096
        or (length > 0 and address == 0)
        or output_address == 0
    ):
        return PROVIDER_UPSTREAM_STATUS_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    if mode == 1:
        output[] = 0 if provider_upstream_ends_with(address, length, StringSlice("/openai")) else 1
        return PROVIDER_UPSTREAM_STATUS_OK
    if provider_upstream_ends_with(address, length, StringSlice("/anthropic/v1")):
        output[] = 0
    elif provider_upstream_ends_with(address, length, StringSlice("/anthropic")):
        output[] = 1
    elif provider_upstream_ends_with(address, length, StringSlice("/v1")) or provider_upstream_ends_with(
        address, length, StringSlice("/beta")
    ):
        output[] = 2
    else:
        output[] = 3
    return PROVIDER_UPSTREAM_STATUS_OK


@export("prodex_provider_upstream_attempt_label_v1")
def prodex_provider_upstream_attempt_label_v1(
    abi_version: Int64,
    candidate_count: Int64,
    candidate_index: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PROVIDER_UPSTREAM_ABI_VERSION:
        return PROVIDER_UPSTREAM_STATUS_ABI
    if (
        candidate_count <= 0
        or candidate_count > PROVIDER_UPSTREAM_MAX_ATTEMPTS
        or candidate_index < 0
        or candidate_index >= candidate_count
        or output_address == 0
        or output_capacity < 0
        or written_address == 0
    ):
        return PROVIDER_UPSTREAM_STATUS_INVALID
    var label = StringSlice("api-key")
    var numbered = candidate_count != 1
    if numbered:
        var index = candidate_index + 1
        var required: Int64 = 9
        if index >= 100:
            required = 11
        elif index >= 10:
            required = 10
        if required > output_capacity:
            return 2
        var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
            unsafe_from_address=Int(output_address)
        )
        var prefix = StringSlice("api-key-").unsafe_ptr()
        for offset in range(8):
            output[unsafe_offset=offset] = prefix[unsafe_offset=offset]
        if index >= 100:
            output[unsafe_offset=8] = UInt8(48 + index / 100)
            output[unsafe_offset=9] = UInt8(48 + (index / 10) % 10)
            output[unsafe_offset=10] = UInt8(48 + index % 10)
        elif index >= 10:
            output[unsafe_offset=8] = UInt8(48 + index / 10)
            output[unsafe_offset=9] = UInt8(48 + index % 10)
        else:
            output[unsafe_offset=8] = UInt8(48 + index)
        var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
            unsafe_from_address=Int(written_address)
        )
        written[] = required
        return PROVIDER_UPSTREAM_STATUS_OK
    if Int64(label.byte_length()) > output_capacity:
        return 2
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var source = label.unsafe_ptr()
    for index in range(Int64(label.byte_length())):
        output[unsafe_offset=index] = source[unsafe_offset=index]
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = Int64(label.byte_length())
    return PROVIDER_UPSTREAM_STATUS_OK
