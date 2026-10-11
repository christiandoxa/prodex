from std.memory import Pointer


comptime RUNTIME_LOCAL_REWRITE_TRANSPORT_ABI_VERSION: Int64 = 1
comptime RUNTIME_LOCAL_REWRITE_TRANSPORT_OK: Int64 = 0
comptime RUNTIME_LOCAL_REWRITE_TRANSPORT_INVALID: Int64 = 1
comptime RUNTIME_LOCAL_REWRITE_TRANSPORT_ABI: Int64 = 4
# The ABI accepts header names only; Rust retains values and credentials.
comptime RUNTIME_LOCAL_REWRITE_TRANSPORT_MAX_HEADER_BYTES: Int64 = 4_096

comptime RUNTIME_LOCAL_REWRITE_HEADER_FORWARD: Int64 = 0
comptime RUNTIME_LOCAL_REWRITE_HEADER_TRANSPORT_LOCAL: Int64 = 1
comptime RUNTIME_LOCAL_REWRITE_HEADER_INTERNAL: Int64 = 2
comptime RUNTIME_LOCAL_REWRITE_HEADER_AUTH_REPLACED: Int64 = 3


def runtime_local_rewrite_header_byte_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def runtime_local_rewrite_header_equals(
    source: Pointer[mut=False, UInt8, ImmUntrackedOrigin],
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    var expected = Int64(literal.byte_length())
    if end - start != expected:
        return False
    var target = literal.unsafe_ptr()
    for index in range(expected):
        if runtime_local_rewrite_header_byte_lower(
            source[unsafe_offset=start + index]
        ) != runtime_local_rewrite_header_byte_lower(target[unsafe_offset=index]):
            return False
    return True


def runtime_local_rewrite_header_starts_with(
    source: Pointer[mut=False, UInt8, ImmUntrackedOrigin],
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    var expected = Int64(literal.byte_length())
    if end - start < expected:
        return False
    var target = literal.unsafe_ptr()
    for index in range(expected):
        if runtime_local_rewrite_header_byte_lower(
            source[unsafe_offset=start + index]
        ) != runtime_local_rewrite_header_byte_lower(target[unsafe_offset=index]):
            return False
    return True


def runtime_local_rewrite_header_is_transport_local(
    source: Pointer[mut=False, UInt8, ImmUntrackedOrigin],
    start: Int64,
    end: Int64,
) -> Bool:
    for literal in [
        StringSlice("connection"),
        StringSlice("content-length"),
        StringSlice("host"),
        StringSlice("keep-alive"),
        StringSlice("proxy-authenticate"),
        StringSlice("proxy-authorization"),
        StringSlice("te"),
        StringSlice("trailer"),
        StringSlice("transfer-encoding"),
        StringSlice("upgrade"),
    ]:
        if runtime_local_rewrite_header_equals(source, start, end, literal):
            return True
    return runtime_local_rewrite_header_starts_with(
        source, start, end, StringSlice("sec-websocket-")
    )


@export("prodex_runtime_local_rewrite_header_policy_v1")
def prodex_runtime_local_rewrite_header_policy_v1(
    abi_version: Int64,
    replacing_auth: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != RUNTIME_LOCAL_REWRITE_TRANSPORT_ABI_VERSION:
        return RUNTIME_LOCAL_REWRITE_TRANSPORT_ABI
    if (
        replacing_auth < 0
        or replacing_auth > 1
        or length < 0
        or length > RUNTIME_LOCAL_REWRITE_TRANSPORT_MAX_HEADER_BYTES
        or (length > 0 and address == 0)
        or output_address == 0
    ):
        return RUNTIME_LOCAL_REWRITE_TRANSPORT_INVALID

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    if length == 0:
        output[] = RUNTIME_LOCAL_REWRITE_HEADER_FORWARD
        return RUNTIME_LOCAL_REWRITE_TRANSPORT_OK

    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    var start: Int64 = 0
    var end = length
    while start < end and source[unsafe_offset=start] <= 32:
        start += 1
    while end > start and source[unsafe_offset=end - 1] <= 32:
        end -= 1

    if runtime_local_rewrite_header_is_transport_local(source, start, end):
        output[] = RUNTIME_LOCAL_REWRITE_HEADER_TRANSPORT_LOCAL
    elif runtime_local_rewrite_header_starts_with(
        source, start, end, StringSlice("x-prodex-internal-")
    ):
        output[] = RUNTIME_LOCAL_REWRITE_HEADER_INTERNAL
    elif replacing_auth == 1 and (
        runtime_local_rewrite_header_equals(source, start, end, StringSlice("authorization"))
        or runtime_local_rewrite_header_equals(
            source, start, end, StringSlice("chatgpt-account-id")
        )
    ):
        output[] = RUNTIME_LOCAL_REWRITE_HEADER_AUTH_REPLACED
    else:
        output[] = RUNTIME_LOCAL_REWRITE_HEADER_FORWARD
    return RUNTIME_LOCAL_REWRITE_TRANSPORT_OK
