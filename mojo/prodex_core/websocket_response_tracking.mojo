from std.memory import Pointer

from response_forwarding import response_equals, response_generation_start
from rich_text import rich_utf8_valid
from rich_types import ProdexRichStringView

comptime WEBSOCKET_RESPONSE_TRACKING_ABI_VERSION: Int64 = 1
comptime WEBSOCKET_RESPONSE_TRACKING_FIELD_COUNT: Int64 = 10
comptime WEBSOCKET_RESPONSE_TRACKING_OUTPUT_COUNT: Int64 = 8

comptime FRAME_TEXT: Int64 = 0
comptime FRAME_BINARY: Int64 = 1
comptime FRAME_KEEPALIVE: Int64 = 2

comptime RETRY_NONE: UInt64 = 0
comptime RETRY_CONNECTION_LIMIT: UInt64 = 1
comptime RETRY_QUOTA: UInt64 = 2
comptime RETRY_RATE_LIMITED: UInt64 = 3
comptime RETRY_OVERLOADED: UInt64 = 4
comptime RETRY_PREVIOUS_RESPONSE_NOT_FOUND: UInt64 = 5

comptime ACTION_FORWARD: Int64 = 0
comptime ACTION_FORWARD_UNCOMMITTED: Int64 = 1
comptime ACTION_BUFFER: Int64 = 2
comptime ACTION_COMMIT_BUFFERED: Int64 = 3
comptime ACTION_RETRY_CONNECTION_LIMIT: Int64 = 4
comptime ACTION_RETRY_QUOTA: Int64 = 5
comptime ACTION_RETRY_RATE_LIMITED: Int64 = 6
comptime ACTION_RETRY_OVERLOADED: Int64 = 7
comptime ACTION_RETRY_PREVIOUS_RESPONSE_NOT_FOUND: Int64 = 8
comptime ACTION_KEEPALIVE: Int64 = 9


def websocket_response_tracking_event_terminal(
    address: UInt,
    length: Int64,
    event_present: Int64,
) -> Bool:
    if event_present == 0:
        return False
    return (
        response_equals(address, length, StringSlice("response.completed"))
        or response_equals(address, length, StringSlice("response.failed"))
        or response_equals(address, length, StringSlice("response.incomplete"))
    )


def websocket_response_tracking_event_resets_socket(
    address: UInt,
    length: Int64,
    event_present: Int64,
) -> Bool:
    if event_present == 0:
        return False
    return (
        response_equals(address, length, StringSlice("error"))
        or response_equals(address, length, StringSlice("response.failed"))
        or response_equals(address, length, StringSlice("response.incomplete"))
    )


@export("prodex_websocket_response_frame_plan_v1")
def prodex_websocket_response_frame_plan_v1(
    abi_version: Int64,
    fields_address: UInt,
    field_count: Int64,
    event_address: UInt,
    event_length: Int64,
    event_present: Int64,
    output_address: UInt,
    output_count: Int64,
) abi("C") -> Int64:
    if abi_version != WEBSOCKET_RESPONSE_TRACKING_ABI_VERSION:
        return 4
    if (
        field_count != WEBSOCKET_RESPONSE_TRACKING_FIELD_COUNT
        or output_count != WEBSOCKET_RESPONSE_TRACKING_OUTPUT_COUNT
        or fields_address == 0
        or output_address == 0
        or event_length < 0
        or event_present < 0
        or event_present > 1
        or (event_present == 1 and event_length > 0 and event_address == 0)
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
        or fields[unsafe_offset=2] > 1
        or fields[unsafe_offset=4] > 1
        or fields[unsafe_offset=5] > 1
        or fields[unsafe_offset=6] > 1
        or fields[unsafe_offset=7] > 1
        or fields[unsafe_offset=8] > 1
        or fields[unsafe_offset=9] > UInt64(FRAME_KEEPALIVE)
        or fields[unsafe_offset=3] > RETRY_PREVIOUS_RESPONSE_NOT_FOUND
    ):
        return 1
    if event_present == 1 and not rich_utf8_valid(
        Pointer[mut=False, UInt8, ImmUntrackedOrigin](unsafe_from_address=Int(event_address)),
        event_length,
    ):
        return 1

    var committed = fields[unsafe_offset=0] == 1
    var precommit_hold = fields[unsafe_offset=1] == 1
    var promoted_precommit_hold = fields[unsafe_offset=2] == 1
    var retry_kind = fields[unsafe_offset=3]
    var terminal_hint = fields[unsafe_offset=4] == 1
    var realtime_websocket = fields[unsafe_offset=5] == 1
    var generation_started = fields[unsafe_offset=6] == 1
    var text_nonempty = fields[unsafe_offset=7] == 1
    var frame_kind = fields[unsafe_offset=9]

    output[unsafe_offset=0] = ACTION_FORWARD
    output[unsafe_offset=1] = 0
    output[unsafe_offset=2] = 0
    output[unsafe_offset=3] = 0
    output[unsafe_offset=4] = 0
    output[unsafe_offset=5] = 0
    output[unsafe_offset=6] = 0
    output[unsafe_offset=7] = 0

    if frame_kind == UInt64(FRAME_KEEPALIVE):
        output[unsafe_offset=0] = ACTION_KEEPALIVE
        return 0

    if frame_kind == UInt64(FRAME_BINARY):
        output[unsafe_offset=1] = Int64(not committed)
        output[unsafe_offset=2] = 1
        return 0

    var generation_start = (
        not generation_started
        and fields[unsafe_offset=8] == 1
        and response_generation_start(event_address, event_length)
    )
    output[unsafe_offset=6] = Int64(generation_start)

    if not committed and retry_kind != RETRY_NONE:
        if retry_kind == RETRY_CONNECTION_LIMIT:
            output[unsafe_offset=0] = ACTION_RETRY_CONNECTION_LIMIT
        elif retry_kind == RETRY_QUOTA:
            output[unsafe_offset=0] = ACTION_RETRY_QUOTA
        elif retry_kind == RETRY_RATE_LIMITED:
            output[unsafe_offset=0] = ACTION_RETRY_RATE_LIMITED
        elif retry_kind == RETRY_OVERLOADED:
            output[unsafe_offset=0] = ACTION_RETRY_OVERLOADED
        else:
            output[unsafe_offset=0] = ACTION_RETRY_PREVIOUS_RESPONSE_NOT_FOUND
        return 0

    if not committed and precommit_hold:
        if promoted_precommit_hold:
            output[unsafe_offset=0] = ACTION_COMMIT_BUFFERED
            output[unsafe_offset=1] = 1
        else:
            output[unsafe_offset=0] = ACTION_BUFFER
        return 0

    if not committed and (not text_nonempty or fields[unsafe_offset=8] == 0):
        output[unsafe_offset=0] = ACTION_FORWARD_UNCOMMITTED
        output[unsafe_offset=2] = 1
        return 0

    var terminal = terminal_hint or websocket_response_tracking_event_terminal(
        event_address, event_length, Int64(fields[unsafe_offset=8])
    )
    output[unsafe_offset=1] = Int64(not committed)
    output[unsafe_offset=2] = 1
    output[unsafe_offset=3] = Int64(terminal)
    output[unsafe_offset=4] = Int64(
        terminal
        and not realtime_websocket
        and websocket_response_tracking_event_resets_socket(
            event_address, event_length, Int64(fields[unsafe_offset=8])
        )
    )
    output[unsafe_offset=5] = Int64(not precommit_hold)
    output[unsafe_offset=7] = Int64(
        committed and retry_kind == RETRY_PREVIOUS_RESPONSE_NOT_FOUND
    )
    return 0
