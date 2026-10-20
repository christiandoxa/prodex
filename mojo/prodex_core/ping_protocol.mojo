from std.collections import Array

from std.memory import Pointer
from json_sink import JsonSink, js_byte, js_literal, js_u64, js_view

from json_view import (
    deepseek_json_byte,
    deepseek_json_object_member,
    deepseek_json_raw_equals,
    deepseek_json_skip_ws,
    deepseek_json_value_end,
)
from rich_text import rich_view_ptr, rich_view_valid, rich_trim_bounds, rich_utf8_continuation
from rich_types import ProdexRichStringView

comptime PING_PROTOCOL_ABI_VERSION: Int64 = 1
comptime PING_PROTOCOL_MAX_BYTES: Int64 = 4_194_304
comptime PING_DETAIL_STATUS_OK: Int64 = 0
comptime PING_DETAIL_STATUS_INVALID: Int64 = 1
comptime PING_DETAIL_STATUS_CAPACITY: Int64 = 3
comptime PING_DETAIL_STATUS_ABI: Int64 = 4
comptime PING_DETAIL_MAX_BYTES: Int64 = 128
comptime PING_STATUS_LABEL_MAX_BYTES: Int64 = 32
comptime PING_SUMMARY_MAX_STATUSES: Int64 = 65_536

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
comptime PING_ERROR_DETAIL_MAX_BYTES: Int64 = 4096

def ping_status_detail_view(status: Int64) -> ProdexRichStringView:
    var detail = StringSlice("")
    if status == PING_STATUS_OK:
        detail = StringSlice("valid model response received")
    elif status == PING_STATUS_AUTH_FAILED:
        detail = StringSlice("OpenAI authentication failed")
    elif status == PING_STATUS_DNS_FAILED:
        detail = StringSlice("OpenAI hostname resolution failed")
    elif status == PING_STATUS_TLS_FAILED:
        detail = StringSlice("OpenAI TLS connection failed")
    elif status == PING_STATUS_TIMEOUT:
        detail = StringSlice("OpenAI diagnostic timed out")
    elif status == PING_STATUS_RATE_LIMITED:
        detail = StringSlice("OpenAI temporarily rate limited the profile")
    elif status == PING_STATUS_QUOTA_EXHAUSTED:
        detail = StringSlice("OpenAI reported quota exhaustion")
    elif status == PING_STATUS_UPSTREAM_OVERLOADED:
        detail = StringSlice("OpenAI upstream is temporarily unavailable")
    elif status == PING_STATUS_MODEL_UNAVAILABLE:
        detail = StringSlice("selected OpenAI model is unavailable")
    elif status == PING_STATUS_PROTOCOL_FAILED:
        detail = StringSlice("Codex did not complete a structured turn")
    elif status == PING_STATUS_TURN_FAILED:
        detail = StringSlice("Codex turn failed")
    elif status == PING_STATUS_PROCESS_FAILED:
        detail = StringSlice("Codex diagnostic process failed")
    elif status == PING_STATUS_SPAWN_FAILED:
        detail = StringSlice("OpenAI application ping could not start")
    elif status == PING_STATUS_CANCELLED:
        detail = StringSlice("OpenAI ping was cancelled")
    elif status == PING_STATUS_UNEXPECTED_RESPONSE:
        detail = StringSlice("completed turn did not return a model response")
    else:
        return ProdexRichStringView(0, 0)
    return ProdexRichStringView(UInt(Int(detail.unsafe_ptr())), UInt(detail.byte_length()))


def ping_status_detail_copy(
    status: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Int64:
    var detail = ping_status_detail_view(status)
    var length = Int64(detail.len)
    if length == 0:
        return PING_DETAIL_STATUS_INVALID
    if length > PING_DETAIL_MAX_BYTES:
        return PING_DETAIL_STATUS_INVALID
    if capacity < length:
        return PING_DETAIL_STATUS_CAPACITY
    var source = rich_view_ptr(detail)
    for index in range(length):
        output[unsafe_offset=index] = source[unsafe_offset=index]
    written[] = length
    return PING_DETAIL_STATUS_OK

def ping_status_label_copy(
    status: Int64,
    json_label: Bool,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Int64:
    if status < PING_STATUS_OK or status > PING_STATUS_CANCELLED or capacity < 0:
        return PING_DETAIL_STATUS_INVALID
    var label = StringSlice("")
    if json_label:
        if status == PING_STATUS_OK:
            label = StringSlice("ok")
        elif status == PING_STATUS_PROTOCOL_FAILED:
            label = StringSlice("protocol_failed")
        elif status == PING_STATUS_UNEXPECTED_RESPONSE:
            label = StringSlice("unexpected_response")
        elif status == PING_STATUS_TURN_FAILED:
            label = StringSlice("turn_failed")
        elif status == PING_STATUS_AUTH_FAILED:
            label = StringSlice("auth_failed")
        elif status == PING_STATUS_DNS_FAILED:
            label = StringSlice("dns_failed")
        elif status == PING_STATUS_TLS_FAILED:
            label = StringSlice("tls_failed")
        elif status == PING_STATUS_TIMEOUT:
            label = StringSlice("timeout")
        elif status == PING_STATUS_RATE_LIMITED:
            label = StringSlice("rate_limited")
        elif status == PING_STATUS_QUOTA_EXHAUSTED:
            label = StringSlice("exhausted")
        elif status == PING_STATUS_UPSTREAM_OVERLOADED:
            label = StringSlice("upstream_overloaded")
        elif status == PING_STATUS_MODEL_UNAVAILABLE:
            label = StringSlice("model_unavailable")
        elif status == PING_STATUS_PROCESS_FAILED:
            label = StringSlice("process_failed")
        elif status == PING_STATUS_SPAWN_FAILED:
            label = StringSlice("spawn_failed")
        elif status == PING_STATUS_CANCELLED:
            label = StringSlice("cancelled")
    elif status == PING_STATUS_OK:
        label = StringSlice("OK")
    elif status == PING_STATUS_PROTOCOL_FAILED:
        label = StringSlice("PROTOCOL_FAILED")
    elif status == PING_STATUS_UNEXPECTED_RESPONSE:
        label = StringSlice("UNEXPECTED_RESPONSE")
    elif status == PING_STATUS_TURN_FAILED:
        label = StringSlice("TURN_FAILED")
    elif status == PING_STATUS_AUTH_FAILED:
        label = StringSlice("AUTH_FAILED")
    elif status == PING_STATUS_DNS_FAILED:
        label = StringSlice("DNS_FAILED")
    elif status == PING_STATUS_TLS_FAILED:
        label = StringSlice("TLS_FAILED")
    elif status == PING_STATUS_TIMEOUT:
        label = StringSlice("TIMEOUT")
    elif status == PING_STATUS_RATE_LIMITED:
        label = StringSlice("RATE_LIMITED")
    elif status == PING_STATUS_QUOTA_EXHAUSTED:
        label = StringSlice("EXHAUSTED")
    elif status == PING_STATUS_UPSTREAM_OVERLOADED:
        label = StringSlice("UPSTREAM_OVERLOADED")
    elif status == PING_STATUS_MODEL_UNAVAILABLE:
        label = StringSlice("MODEL_UNAVAILABLE")
    elif status == PING_STATUS_PROCESS_FAILED:
        label = StringSlice("PROCESS_FAILED")
    elif status == PING_STATUS_SPAWN_FAILED:
        label = StringSlice("SPAWN_FAILED")
    elif status == PING_STATUS_CANCELLED:
        label = StringSlice("CANCELLED")
    if label.byte_length() == 0:
        return PING_DETAIL_STATUS_INVALID
    var length = Int64(label.byte_length())
    if length > PING_STATUS_LABEL_MAX_BYTES:
        return PING_DETAIL_STATUS_INVALID
    if length > capacity:
        return PING_DETAIL_STATUS_CAPACITY
    var source = label.unsafe_ptr()
    for index in range(length):
        output[unsafe_offset=index] = source[unsafe_offset=index]
    written[] = length
    return PING_DETAIL_STATUS_OK

def ping_status_is_temporary(status: Int64) -> Bool:
    return (
        status == PING_STATUS_DNS_FAILED
        or status == PING_STATUS_TLS_FAILED
        or status == PING_STATUS_TIMEOUT
        or status == PING_STATUS_RATE_LIMITED
        or status == PING_STATUS_UPSTREAM_OVERLOADED
    )

def ping_ascii_lower(value: UInt8) -> UInt8:
    return value + 32 if value >= 65 and value <= 90 else value

def ping_contains(view: ProdexRichStringView, literal: StringSlice, *, lowercase: Bool = True) -> Bool:
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
            var byte = left[unsafe_offset=start + index]
            if lowercase:
                byte = ping_ascii_lower(byte)
            if byte != right[unsafe_offset=index]:
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


def ping_bounded_detail_bounds(view: ProdexRichStringView) -> Tuple[Int64, Int64]:
    # CR/LF replacement preserves byte lengths and edge-whitespace membership.
    var bounds = rich_trim_bounds(view)
    var end = bounds[1]
    if end - bounds[0] > PING_ERROR_DETAIL_MAX_BYTES:
        end = bounds[0] + PING_ERROR_DETAIL_MAX_BYTES
        var bytes = rich_view_ptr(view)
        while end > bounds[0] and rich_utf8_continuation(bytes[unsafe_offset=end]):
            end -= 1
    return (bounds[0], end)


def ping_normalized_byte(byte: UInt8) -> UInt8:
    return UInt8(32) if byte == 10 or byte == 13 else byte


def ping_diagnostic_emit(
    sink: Pointer[mut=True, JsonSink, _], operation: Int64,
    label: ProdexRichStringView, normalized: ProdexRichStringView,
    append: Bool, exit_failed: Bool, exit_code: Int64,
):
    if operation != 0:
        js_view(sink, label)
    if operation == 2 and exit_failed:
        js_literal(sink, StringSlice(" (exit code "))
        if exit_code < 0:
            js_byte(sink, 45)
        js_u64(sink, UInt64(-exit_code if exit_code < 0 else exit_code))
        js_byte(sink, 41)
    if append:
        if operation == 1:
            js_literal(sink, StringSlice(": "))
        elif operation == 2:
            js_literal(sink, StringSlice(": stderr: "))
        var bytes = rich_view_ptr(normalized)
        for index in range(Int64(normalized.len)):
            js_byte(sink, ping_normalized_byte(bytes[unsafe_offset=index]))


@export("prodex_mojo_ping_diagnostic_v1")
def prodex_mojo_ping_diagnostic_v1(
    version: Int64, operation: Int64, raw_address: UInt, raw_length: Int64,
    redacted_address: UInt, redacted_length: Int64, exit_failed: Int64, exit_code: Int64,
    output_address: UInt, output_capacity: Int64, metadata_address: UInt,
) abi("C") -> Int64:
    if version != PING_PROTOCOL_ABI_VERSION:
        return 4
    if operation < 0 or operation > 2 or raw_length < 0 or redacted_length < 0 or exit_failed < 0 or exit_failed > 1 or exit_code < -2147483648 or exit_code > 2147483647 or (exit_failed == 0 and exit_code != 0) or (operation != 2 and (exit_failed != 0 or exit_code != 0)) or output_capacity < 0 or metadata_address == 0 or metadata_address % 8 != 0 or (output_capacity > 0 and output_address == 0):
        return 1
    if (operation == 0 and raw_length != 0) or (operation == 1 and raw_length > PING_PROTOCOL_MAX_BYTES):
        return 1
    var raw = ProdexRichStringView(raw_address, UInt(raw_length))
    var redacted = ProdexRichStringView(redacted_address, UInt(redacted_length))
    if not rich_view_valid(raw, raw_length) or not rich_view_valid(redacted, redacted_length):
        return 2
    var bounds = ping_bounded_detail_bounds(redacted)
    var normalized = ProdexRichStringView(redacted_address + UInt(bounds[0]), UInt(bounds[1] - bounds[0]))
    var label = ProdexRichStringView(0, 0)
    var status: Int64 = -1
    if operation == 1:
        status = ping_classify_failure(raw)
        if status == PING_STATUS_PROCESS_FAILED:
            var detail = StringSlice("OpenAI application ping process failed")
            if ping_contains(raw, StringSlice("clean the diagnostic directory"), lowercase=False):
                detail = StringSlice("OpenAI application ping cleanup failed")
            label = ProdexRichStringView(UInt(Int(detail.unsafe_ptr())), UInt(detail.byte_length()))
        else:
            label = ping_status_detail_view(status)
    elif operation == 2:
        label = raw.copy()
    if operation == 1 and label.len > UInt(PING_DETAIL_MAX_BYTES):
        return 1
    var append = normalized.len > 0
    if operation == 1 and normalized.len == label.len:
        var source = rich_view_ptr(normalized)
        var prefix = rich_view_ptr(label)
        var equal = True
        for index in range(Int64(label.len)):
            if ping_normalized_byte(source[unsafe_offset=index]) != prefix[unsafe_offset=index]:
                equal = False
                break
        append = not equal
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var measure = JsonSink(output, output_capacity, 0, True, False)
    ping_diagnostic_emit(Pointer(to=measure), operation, label, normalized, append, exit_failed == 1, exit_code)
    if measure.failed or measure.written > output_capacity:
        return 3
    var sink = JsonSink(output, output_capacity, 0, False, False)
    ping_diagnostic_emit(Pointer(to=sink), operation, label, normalized, append, exit_failed == 1, exit_code)
    if sink.failed:
        return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = status
    metadata[unsafe_offset=1] = sink.written
    return 0

def ping_bounds_present(bounds: Array[Int64, 2]) -> Bool:
    return bounds[0] >= 0 and bounds[1] > bounds[0]

def ping_json_string_nonempty(
    view: ProdexRichStringView, bounds: Array[Int64, 2]
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
) -> Array[Int64, 2]:
    var result = Array[Int64, 2](fill=0)
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

@export("prodex_mojo_ping_status_detail_v1")
def prodex_mojo_ping_status_detail_v1(
    abi_version: Int64,
    status: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PING_PROTOCOL_ABI_VERSION:
        return PING_DETAIL_STATUS_ABI
    if (
        status < PING_STATUS_OK
        or status > PING_STATUS_CANCELLED
        or output_capacity < 0
        or written_address == 0
        or (output_capacity > 0 and output_address == 0)
    ):
        return PING_DETAIL_STATUS_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    return ping_status_detail_copy(status, output, output_capacity, written)

@export("prodex_mojo_ping_status_label_v1")
def prodex_mojo_ping_status_label_v1(
    abi_version: Int64,
    status: Int64,
    json_label: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PING_PROTOCOL_ABI_VERSION:
        return PING_DETAIL_STATUS_ABI
    if (
        status < PING_STATUS_OK
        or status > PING_STATUS_CANCELLED
        or json_label < 0
        or json_label > 1
        or output_capacity < 0
        or written_address == 0
        or (output_capacity > 0 and output_address == 0)
    ):
        return PING_DETAIL_STATUS_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    return ping_status_label_copy(status, json_label == 1, output, output_capacity, written)

@export("prodex_mojo_ping_status_temporary_v1")
def prodex_mojo_ping_status_temporary_v1(
    abi_version: Int64,
    status: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != PING_PROTOCOL_ABI_VERSION:
        return PING_DETAIL_STATUS_ABI
    if status < PING_STATUS_OK or status > PING_STATUS_CANCELLED or output_address == 0:
        return PING_DETAIL_STATUS_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = Int64(ping_status_is_temporary(status))
    return PING_DETAIL_STATUS_OK

@export("prodex_mojo_ping_summary_plan_v1")
def prodex_mojo_ping_summary_plan_v1(
    abi_version: Int64,
    statuses_address: UInt,
    status_count: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != PING_PROTOCOL_ABI_VERSION:
        return PING_DETAIL_STATUS_ABI
    if (
        status_count < 0
        or status_count > PING_SUMMARY_MAX_STATUSES
        or output_address == 0
        or (status_count > 0 and statuses_address == 0)
    ):
        return PING_DETAIL_STATUS_INVALID
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var healthy: UInt64 = 0
    var exhausted: UInt64 = 0
    var auth_failures: UInt64 = 0
    var temporary_failures: UInt64 = 0
    var statuses = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(statuses_address)
    )
    for index in range(status_count):
        var status = statuses[unsafe_offset=index]
        if status < PING_STATUS_OK or status > PING_STATUS_CANCELLED:
            return PING_DETAIL_STATUS_INVALID
        if status == PING_STATUS_OK:
            healthy += 1
        elif status == PING_STATUS_QUOTA_EXHAUSTED:
            exhausted += 1
        elif status == PING_STATUS_AUTH_FAILED:
            auth_failures += 1
        elif ping_status_is_temporary(status):
            temporary_failures += 1
    var total = UInt64(status_count)
    output[unsafe_offset=0] = total
    output[unsafe_offset=1] = healthy
    output[unsafe_offset=2] = exhausted
    output[unsafe_offset=3] = auth_failures
    output[unsafe_offset=4] = temporary_failures
    output[unsafe_offset=5] = total - healthy - exhausted - auth_failures - temporary_failures
    output[unsafe_offset=6] = UInt64(healthy > 0)
    return PING_DETAIL_STATUS_OK


# Stable profile name ordering for completed parallel ping probes. Mojo owns
# bytewise UTF-8 lexical precedence; Rust retains each completed result and
# merely applies this compiled permutation to its original result objects.
comptime PING_PROFILE_ORDER_MAX: Int64 = 65_536


def ping_profile_order_less(
    names: Pointer[mut=False, UInt64, _], left: Int64, right: Int64,
) -> Bool:
    var l_offset = left * 2
    var r_offset = right * 2
    var left_length = Int64(names[unsafe_offset=l_offset + 1])
    var right_length = Int64(names[unsafe_offset=r_offset + 1])
    var a = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(names[unsafe_offset=l_offset])
    )
    var b = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(names[unsafe_offset=r_offset])
    )
    for index in range(min(left_length, right_length)):
        var left_byte = a[unsafe_offset=index]
        var right_byte = b[unsafe_offset=index]
        if left_byte != right_byte:
            return left_byte < right_byte
    if left_length != right_length:
        return left_length < right_length
    return left < right


def ping_profile_order_heap_down(
    names: Pointer[mut=False, UInt64, _],
    indices: Pointer[mut=True, Int64, _],
    first: Int64, count: Int64,
):
    var root = first
    while root * 2 + 1 < count:
        var child = root * 2 + 1
        var swap = root
        if ping_profile_order_less(
            names, indices[unsafe_offset=swap], indices[unsafe_offset=child],
        ):
            swap = child
        if child + 1 < count and ping_profile_order_less(
            names, indices[unsafe_offset=swap], indices[unsafe_offset=child + 1],
        ):
            swap = child + 1
        if swap == root:
            return
        var saved = indices[unsafe_offset=root]
        indices[unsafe_offset=root] = indices[unsafe_offset=swap]
        indices[unsafe_offset=swap] = saved
        root = swap


@export("prodex_mojo_ping_profile_order_v1")
def prodex_mojo_ping_profile_order_v1(
    abi_version: Int64,
    names_address: UInt, count: Int64,
    output_address: UInt, output_capacity: Int64,
) abi("C") -> Int64:
    if abi_version != PING_PROTOCOL_ABI_VERSION:
        return PING_DETAIL_STATUS_ABI
    if (
        count < 0 or count > PING_PROFILE_ORDER_MAX
        or (count > 0 and (names_address == 0 or names_address % 8 != 0))
        or output_address == 0 or output_address % 8 != 0
        or output_capacity < count
    ):
        return PING_DETAIL_STATUS_INVALID
    var names = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(names_address)
    )
    for index in range(count):
        var address = names[unsafe_offset=index * 2]
        var length = names[unsafe_offset=index * 2 + 1]
        if (
            length > UInt64(9223372036854775807)
            or (length > 0 and address == 0)
            or not rich_view_valid(
                ProdexRichStringView(UInt(address), UInt(length)),
                PING_PROTOCOL_MAX_BYTES,
            )
        ):
            return PING_DETAIL_STATUS_INVALID
    var indices = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(count):
        indices[unsafe_offset=index] = index
    var start = count // 2
    while start > 0:
        start -= 1
        ping_profile_order_heap_down(names, indices, start, count)
    var end = count
    while end > 1:
        end -= 1
        var saved = indices[unsafe_offset=0]
        indices[unsafe_offset=0] = indices[unsafe_offset=end]
        indices[unsafe_offset=end] = saved
        ping_profile_order_heap_down(names, indices, 0, end)
    return PING_DETAIL_STATUS_OK
