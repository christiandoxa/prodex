from std.memory import Pointer

from json_view import (
    deepseek_json_byte,
    deepseek_json_object_member,
    deepseek_json_raw_equals,
    deepseek_json_skip_ws,
    deepseek_json_value_end,
)
from rich_text import rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime PING_PROTOCOL_ABI_VERSION: Int64 = 1
comptime PING_PROTOCOL_MAX_BYTES: Int64 = 4_194_304

comptime PING_STATUS_OK: Int64 = 0
comptime PING_STATUS_PROTOCOL_FAILED: Int64 = 1
comptime PING_STATUS_UNEXPECTED_RESPONSE: Int64 = 2
comptime PING_STATUS_TURN_FAILED: Int64 = 3
comptime PING_STATUS_AUTH_FAILED: Int64 = 4
comptime PING_STATUS_DNS_FAILED: Int64 = 5
comptime PING_STATUS_TLS_FAILED: Int64 = 6
comptime PING_STATUS_TIMEOUT: Int64 = 7
comptime PING_STATUS_RATE_LIMITED: Int64 = 8
comptime PING_STATUS_QUOTA_EXHAUSTED: Int64 = 9
comptime PING_STATUS_UPSTREAM_OVERLOADED: Int64 = 10
comptime PING_STATUS_MODEL_UNAVAILABLE: Int64 = 11
comptime PING_STATUS_PROCESS_FAILED: Int64 = 12
comptime PING_STATUS_SPAWN_FAILED: Int64 = 13
comptime PING_STATUS_CANCELLED: Int64 = 14

def ping_ascii_lower(value: UInt8) -> UInt8:
    return value + 32 if value >= 65 and value <= 90 else value

def ping_contains(view: ProdexRichStringView, literal: StringSlice) -> Bool:
    var needle = Int64(literal.byte_length())
    var length = Int64(view.len)
    if needle == 0:
        return True
    if needle > length or view.ptr == 0:
        return False
    var left = rich_view_ptr(view)
    var right = literal.unsafe_ptr()
    for start in range(length - needle + 1):
        var matched = True
        for index in range(needle):
            if ping_ascii_lower(left[unsafe_offset=start + index]) != right[unsafe_offset=index]:
                matched = False
                break
        if matched:
            return True
    return False

def ping_quota_failure(view: ProdexRichStringView) -> Bool:
    var quota = (
        ping_contains(view, StringSlice("insufficient_quota"))
        or ping_contains(view, StringSlice("usage_limit_reached"))
        or ping_contains(view, StringSlice("quota exceeded"))
        or ping_contains(view, StringSlice("quota_exceeded"))
        or ping_contains(view, StringSlice("quota exhausted"))
        or ping_contains(view, StringSlice("insufficient quota"))
    )
    if not quota:
        return False
    return (
        not ping_contains(view, StringSlice("429"))
        or ping_contains(view, StringSlice("insufficient_quota"))
        or ping_contains(view, StringSlice("usage_limit_reached"))
        or ping_contains(view, StringSlice("quota_exceeded"))
    )

def ping_classify_failure(view: ProdexRichStringView) -> Int64:
    if (
        ping_contains(view, StringSlice("failed to start"))
        or ping_contains(view, StringSlice("could not start"))
        or ping_contains(view, StringSlice("failed to execute"))
        or ping_contains(view, StringSlice("failed to resolve prodex executable"))
    ):
        return PING_STATUS_SPAWN_FAILED
    if (
        ping_contains(view, StringSlice("structured turn"))
        or ping_contains(view, StringSlice("malformed jsonl"))
        or ping_contains(view, StringSlice("unsupported event"))
        or ping_contains(view, StringSlice("protocol failure"))
    ):
        return PING_STATUS_PROTOCOL_FAILED
    if (
        ping_contains(view, StringSlice("503"))
        or ping_contains(view, StringSlice("502"))
        or ping_contains(view, StringSlice("504"))
        or ping_contains(view, StringSlice("overloaded"))
        or ping_contains(view, StringSlice("temporarily unavailable"))
    ):
        return PING_STATUS_UPSTREAM_OVERLOADED
    if ping_quota_failure(view):
        return PING_STATUS_QUOTA_EXHAUSTED
    if (
        ping_contains(view, StringSlice("401"))
        or ping_contains(view, StringSlice("unauthorized"))
        or ping_contains(view, StringSlice("invalid api key"))
        or ping_contains(view, StringSlice("authentication"))
    ):
        return PING_STATUS_AUTH_FAILED
    if (
        ping_contains(view, StringSlice("429"))
        or ping_contains(view, StringSlice("rate_limit"))
        or ping_contains(view, StringSlice("rate limit"))
    ):
        return PING_STATUS_RATE_LIMITED
    if (
        ping_contains(view, StringSlice("dns"))
        or ping_contains(view, StringSlice("resolve"))
        or ping_contains(view, StringSlice("name or service not known"))
        or ping_contains(view, StringSlice("getaddrinfo"))
    ):
        return PING_STATUS_DNS_FAILED
    if (
        ping_contains(view, StringSlice("tls"))
        or ping_contains(view, StringSlice("certificate"))
        or ping_contains(view, StringSlice("handshake"))
    ):
        return PING_STATUS_TLS_FAILED
    if (
        ping_contains(view, StringSlice("unsupported model"))
        or ping_contains(view, StringSlice("model_not_found"))
    ):
        return PING_STATUS_MODEL_UNAVAILABLE
    if ping_contains(view, StringSlice("cancel")):
        return PING_STATUS_CANCELLED
    if (
        ping_contains(view, StringSlice("timeout"))
        or ping_contains(view, StringSlice("timed out"))
    ):
        return PING_STATUS_TIMEOUT
    return PING_STATUS_PROCESS_FAILED

def ping_bounds_present(bounds: InlineArray[Int64, 2]) -> Bool:
    return bounds[0] >= 0 and bounds[1] > bounds[0]

def ping_json_string_nonempty(
    view: ProdexRichStringView, bounds: InlineArray[Int64, 2]
) -> Bool:
    if not ping_bounds_present(bounds):
        return False
    var start = bounds[0]
    var end = bounds[1]
    if deepseek_json_byte(view, start) != 34 or deepseek_json_byte(view, end - 1) != 34:
        return False
    var index = start + 1
    while index < end - 1:
        var value = deepseek_json_byte(view, index)
        if value == 92:
            if index + 1 >= end - 1:
                return False
            var escaped = deepseek_json_byte(view, index + 1)
            if escaped != 110 and escaped != 114 and escaped != 116:
                return True
            index += 2
            continue
        if value != 9 and value != 10 and value != 13 and value != 32:
            return True
        index += 1
    return False

def ping_event_is(
    view: ProdexRichStringView,
    object_start: Int64,
    object_end: Int64,
    literal: StringSlice,
) -> Bool:
    var bounds = deepseek_json_object_member(
        view, object_start, object_end, StringSlice("type")
    )
    return ping_bounds_present(bounds) and deepseek_json_raw_equals(
        view, bounds[0], bounds[1], literal
    )

def ping_validate_item(
    view: ProdexRichStringView,
    object_start: Int64,
    object_end: Int64,
    event_completed: Bool,
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=0)
    var item = deepseek_json_object_member(
        view, object_start, object_end, StringSlice("item")
    )
    if not ping_bounds_present(item) or deepseek_json_byte(view, item[0]) != 123:
        result[0] = PING_STATUS_PROTOCOL_FAILED
        return result^
    var item_type = deepseek_json_object_member(
        view, item[0], item[1], StringSlice("type")
    )
    if not ping_bounds_present(item_type):
        result[0] = PING_STATUS_PROTOCOL_FAILED
        return result^
    if deepseek_json_raw_equals(
        view, item_type[0], item_type[1], StringSlice("reasoning")
    ):
        return result^
    if not deepseek_json_raw_equals(
        view, item_type[0], item_type[1], StringSlice("agent_message")
    ):
        result[0] = PING_STATUS_PROTOCOL_FAILED
        return result^
    if event_completed:
        var text = deepseek_json_object_member(
            view, item[0], item[1], StringSlice("text")
        )
        if ping_json_string_nonempty(view, text):
            result[1] = 1
    return result^

@fieldwise_init
struct PingProtocolState(Copyable):
    var thread_started: Int64
    var turn_started: Int64
    var turn_completed: Int64
    var agent_message_completed: Int64
    var final_message_nonempty: Int64
    var saw_line: Int64

def ping_validate_jsonl(view: ProdexRichStringView) -> Int64:
    var state = PingProtocolState(0, 0, 0, 0, 0, 0)
    var state_ptr = Pointer(to=state)
    var length = Int64(view.len)
    var line_start: Int64 = 0
    var cursor: Int64 = 0

    while cursor <= length:
        if cursor == length or deepseek_json_byte(view, cursor) == 10:
            var start = deepseek_json_skip_ws(view, line_start, cursor)
            var end = cursor
            while end > start:
                var value = deepseek_json_byte(view, end - 1)
                if value == 9 or value == 13 or value == 32:
                    end -= 1
                else:
                    break
            if start >= end:
                if cursor != length or line_start < length:
                    return PING_STATUS_PROTOCOL_FAILED
                break
            state_ptr[].saw_line = 1
            var value_end = deepseek_json_value_end(view, start, end, 0)
            if value_end < 0 or deepseek_json_skip_ws(view, value_end, end) != end:
                return PING_STATUS_PROTOCOL_FAILED
            if deepseek_json_byte(view, start) != 123:
                return PING_STATUS_PROTOCOL_FAILED

            if ping_event_is(view, start, end, StringSlice("thread.started")):
                var thread_id = deepseek_json_object_member(
                    view, start, end, StringSlice("thread_id")
                )
                if (
                    state_ptr[].thread_started == 1
                    or state_ptr[].turn_started == 1
                    or state_ptr[].turn_completed == 1
                    or not ping_json_string_nonempty(view, thread_id)
                ):
                    return PING_STATUS_PROTOCOL_FAILED
                state_ptr[].thread_started = 1
            elif ping_event_is(view, start, end, StringSlice("turn.started")):
                if state_ptr[].thread_started == 0 or state_ptr[].turn_started == 1 or state_ptr[].turn_completed == 1:
                    return PING_STATUS_PROTOCOL_FAILED
                state_ptr[].turn_started = 1
            elif ping_event_is(view, start, end, StringSlice("turn.completed")):
                if state_ptr[].thread_started == 0 or state_ptr[].turn_started == 0:
                    return PING_STATUS_PROTOCOL_FAILED
                # A completed turn is terminal. One final newline is allowed,
                # but any subsequent byte would have been another JSONL line
                # and is therefore a protocol violation.
                if cursor < length and cursor + 1 < length:
                    return PING_STATUS_PROTOCOL_FAILED
                if (
                    state_ptr[].agent_message_completed == 0
                    or state_ptr[].final_message_nonempty == 0
                ):
                    return PING_STATUS_UNEXPECTED_RESPONSE
                return PING_STATUS_OK
            elif ping_event_is(view, start, end, StringSlice("turn.failed")):
                var line = ProdexRichStringView(
                    view.ptr + UInt(start), UInt(end - start)
                )
                var status = ping_classify_failure(line)
                return PING_STATUS_TURN_FAILED if status == PING_STATUS_PROCESS_FAILED else status
            elif ping_event_is(view, start, end, StringSlice("error")):
                var line = ProdexRichStringView(
                    view.ptr + UInt(start), UInt(end - start)
                )
                return ping_classify_failure(line)
            else:
                var started = ping_event_is(
                    view, start, end, StringSlice("item.started")
                )
                var updated = ping_event_is(
                    view, start, end, StringSlice("item.updated")
                )
                var completed = ping_event_is(
                    view, start, end, StringSlice("item.completed")
                )
                if not started and not updated and not completed:
                    return PING_STATUS_PROTOCOL_FAILED
                if state_ptr[].thread_started == 0 or state_ptr[].turn_started == 0 or state_ptr[].turn_completed == 1:
                    return PING_STATUS_PROTOCOL_FAILED
                var item_result = ping_validate_item(view, start, end, completed)
                if item_result[0] != PING_STATUS_OK:
                    return item_result[0]
                if completed:
                    var item = deepseek_json_object_member(
                        view, start, end, StringSlice("item")
                    )
                    var item_type = deepseek_json_object_member(
                        view, item[0], item[1], StringSlice("type")
                    )
                    if deepseek_json_raw_equals(
                        view,
                        item_type[0],
                        item_type[1],
                        StringSlice("agent_message"),
                    ):
                        state_ptr[].agent_message_completed = 1
                        if item_result[1] == 1:
                            state_ptr[].final_message_nonempty = 1
            line_start = cursor + 1
        cursor += 1

    return PING_STATUS_PROTOCOL_FAILED

@export("prodex_mojo_ping_validate_jsonl_v1")
def prodex_mojo_ping_validate_jsonl_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    status_address: UInt,
) abi("C") -> Int64:
    if abi_version != PING_PROTOCOL_ABI_VERSION:
        return 4
    if (
        input_length < 0
        or input_length > PING_PROTOCOL_MAX_BYTES
        or status_address == 0
        or (input_length > 0 and input_address == 0)
    ):
        return 1
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, PING_PROTOCOL_MAX_BYTES):
        return 2
    var status = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(status_address)
    )
    status[] = ping_validate_jsonl(view)
    return 0

@export("prodex_mojo_ping_classify_failure_v1")
def prodex_mojo_ping_classify_failure_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    status_address: UInt,
) abi("C") -> Int64:
    if abi_version != PING_PROTOCOL_ABI_VERSION:
        return 4
    if (
        input_length < 0
        or input_length > PING_PROTOCOL_MAX_BYTES
        or status_address == 0
        or (input_length > 0 and input_address == 0)
    ):
        return 1
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, PING_PROTOCOL_MAX_BYTES):
        return 2
    var status = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(status_address)
    )
    status[] = ping_classify_failure(view)
    return 0
