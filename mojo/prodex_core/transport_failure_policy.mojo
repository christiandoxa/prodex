from std.memory import Pointer

from rich_text import rich_view_valid
from rich_types import ProdexRichStringView, rich_view_ptr

comptime TRANSPORT_FAILURE_ABI_VERSION: Int64 = 1
comptime TRANSPORT_FAILURE_OK: Int64 = 0
comptime TRANSPORT_FAILURE_INVALID: Int64 = 1
comptime TRANSPORT_FAILURE_CAPACITY: Int64 = 2
comptime TRANSPORT_FAILURE_ABI: Int64 = 4

comptime TRANSPORT_FAILURE_DNS: Int64 = 0
comptime TRANSPORT_FAILURE_CONNECT_TIMEOUT: Int64 = 1
comptime TRANSPORT_FAILURE_CONNECT_REFUSED: Int64 = 2
comptime TRANSPORT_FAILURE_CONNECT_RESET: Int64 = 3
comptime TRANSPORT_FAILURE_TLS_HANDSHAKE: Int64 = 4
comptime TRANSPORT_FAILURE_CONNECTION_ABORTED: Int64 = 5
comptime TRANSPORT_FAILURE_BROKEN_PIPE: Int64 = 6
comptime TRANSPORT_FAILURE_UNEXPECTED_EOF: Int64 = 7
comptime TRANSPORT_FAILURE_READ_TIMEOUT: Int64 = 8
comptime TRANSPORT_FAILURE_UPSTREAM_CLOSED_BEFORE_COMMIT: Int64 = 9
comptime TRANSPORT_FAILURE_OTHER: Int64 = 10

comptime TRANSPORT_TEXT_LABEL: Int64 = 0
comptime TRANSPORT_TEXT_MARKER: Int64 = 1


def transport_failure_view(address: UInt, length: Int64) -> ProdexRichStringView:
    return ProdexRichStringView(address, UInt(length))


def transport_failure_copy_literal(
    literal: StringSlice,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) -> Int64:
    if output_address == 0 or output_capacity < 0 or written_address == 0:
        return TRANSPORT_FAILURE_INVALID
    var length = Int64(literal.byte_length())
    if length > output_capacity:
        return TRANSPORT_FAILURE_CAPACITY
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var source = literal.unsafe_ptr()
    for index in range(length):
        output[unsafe_offset=index] = source[unsafe_offset=index]
    written[] = length
    return TRANSPORT_FAILURE_OK


def transport_failure_contains(
    view: ProdexRichStringView,
    literal: StringSlice,
) -> Bool:
    var needle = Int64(literal.byte_length())
    var length = Int64(view.len)
    if needle == 0:
        return True
    if needle > length:
        return False
    var source = rich_view_ptr(view)
    var right = literal.unsafe_ptr()
    for start in range(length - needle + 1):
        var matched = True
        for index in range(needle):
            var value = source[unsafe_offset=start + index]
            if value >= 65 and value <= 90:
                value += 32
            if value != right[unsafe_offset=index]:
                matched = False
                break
        if matched:
            return True
    return False


@export("prodex_transport_failure_text_v1")
def prodex_transport_failure_text_v1(
    abi_version: Int64,
    operation: Int64,
    present: Int64,
    kind: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != TRANSPORT_FAILURE_ABI_VERSION:
        return TRANSPORT_FAILURE_ABI
    if operation < 0 or operation > 1 or (present != 0 and present != 1):
        return TRANSPORT_FAILURE_INVALID
    if present == 1 and (kind < 0 or kind > TRANSPORT_FAILURE_OTHER):
        return TRANSPORT_FAILURE_INVALID

    var label = StringSlice("")
    if operation == TRANSPORT_TEXT_LABEL:
        if present != 1:
            return TRANSPORT_FAILURE_INVALID
        if kind == TRANSPORT_FAILURE_DNS:
            label = StringSlice("dns")
        elif kind == TRANSPORT_FAILURE_CONNECT_TIMEOUT:
            label = StringSlice("connect_timeout")
        elif kind == TRANSPORT_FAILURE_CONNECT_REFUSED:
            label = StringSlice("connection_refused")
        elif kind == TRANSPORT_FAILURE_CONNECT_RESET:
            label = StringSlice("connection_reset")
        elif kind == TRANSPORT_FAILURE_TLS_HANDSHAKE:
            label = StringSlice("tls_handshake")
        elif kind == TRANSPORT_FAILURE_CONNECTION_ABORTED:
            label = StringSlice("connection_aborted")
        elif kind == TRANSPORT_FAILURE_BROKEN_PIPE:
            label = StringSlice("broken_pipe")
        elif kind == TRANSPORT_FAILURE_UNEXPECTED_EOF:
            label = StringSlice("unexpected_eof")
        elif kind == TRANSPORT_FAILURE_READ_TIMEOUT:
            label = StringSlice("read_timeout")
        elif kind == TRANSPORT_FAILURE_UPSTREAM_CLOSED_BEFORE_COMMIT:
            label = StringSlice("upstream_closed_before_commit")
        else:
            label = StringSlice("other")
    else:
        if present == 1 and (
            kind == TRANSPORT_FAILURE_CONNECT_TIMEOUT
            or kind == TRANSPORT_FAILURE_READ_TIMEOUT
        ):
            label = StringSlice("upstream_connect_timeout")
        elif present == 1 and kind == TRANSPORT_FAILURE_DNS:
            label = StringSlice("upstream_connect_dns_error")
        elif present == 1 and kind == TRANSPORT_FAILURE_TLS_HANDSHAKE:
            label = StringSlice("upstream_tls_handshake_error")
        else:
            label = StringSlice("upstream_connect_error")

    return transport_failure_copy_literal(
        label, output_address, output_capacity, written_address
    )


@export("prodex_transport_failure_classify_message_v1")
def prodex_transport_failure_classify_message_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
) abi("C") -> Int64:
    if abi_version != TRANSPORT_FAILURE_ABI_VERSION:
        return -3
    if length < 0:
        return -2
    var view = transport_failure_view(address, length)
    if not rich_view_valid(view, 262144):
        return -2

    if (
        transport_failure_contains(view, StringSlice("dns"))
        or transport_failure_contains(
            view, StringSlice("failed to lookup address information")
        )
        or transport_failure_contains(view, StringSlice("no such host"))
        or transport_failure_contains(
            view, StringSlice("name or service not known")
        )
    ):
        return TRANSPORT_FAILURE_DNS
    if transport_failure_contains(view, StringSlice("connection refused")):
        return TRANSPORT_FAILURE_CONNECT_REFUSED
    if (
        transport_failure_contains(view, StringSlice("timed out"))
        or transport_failure_contains(view, StringSlice("timeout"))
    ):
        return TRANSPORT_FAILURE_CONNECT_TIMEOUT
    if (
        transport_failure_contains(view, StringSlice("tls"))
        or transport_failure_contains(view, StringSlice("handshake"))
        or transport_failure_contains(view, StringSlice("certificate"))
    ):
        return TRANSPORT_FAILURE_TLS_HANDSHAKE
    if transport_failure_contains(view, StringSlice("connection reset")):
        return TRANSPORT_FAILURE_CONNECT_RESET
    if transport_failure_contains(view, StringSlice("broken pipe")):
        return TRANSPORT_FAILURE_BROKEN_PIPE
    if transport_failure_contains(view, StringSlice("unexpected eof")):
        return TRANSPORT_FAILURE_UNEXPECTED_EOF
    if transport_failure_contains(view, StringSlice("connection aborted")):
        return TRANSPORT_FAILURE_CONNECTION_ABORTED
    if (
        transport_failure_contains(
            view, StringSlice("connection closed before message completed")
        )
        or transport_failure_contains(
            view, StringSlice("stream closed before response.completed")
        )
        or transport_failure_contains(
            view, StringSlice("closed before response.completed")
        )
    ):
        return TRANSPORT_FAILURE_UPSTREAM_CLOSED_BEFORE_COMMIT
    if transport_failure_contains(view, StringSlice("unable to connect")):
        return TRANSPORT_FAILURE_OTHER
    return -1


@export("prodex_transport_failure_health_penalty_v1")
def prodex_transport_failure_health_penalty_v1(
    abi_version: Int64,
    kind: Int64,
) abi("C") -> Int64:
    if abi_version != TRANSPORT_FAILURE_ABI_VERSION:
        return -3
    if kind < 0 or kind > TRANSPORT_FAILURE_OTHER:
        return -2
    if (
        kind == TRANSPORT_FAILURE_DNS
        or kind == TRANSPORT_FAILURE_CONNECT_TIMEOUT
        or kind == TRANSPORT_FAILURE_CONNECT_REFUSED
        or kind == TRANSPORT_FAILURE_CONNECT_RESET
        or kind == TRANSPORT_FAILURE_TLS_HANDSHAKE
    ):
        return 5
    return 4
