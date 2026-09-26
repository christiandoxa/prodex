
from std.memory import Pointer


comptime LOG_PARSE_STATUS_OK: Int64 = 0
comptime LOG_PARSE_STATUS_INVALID: Int64 = 1
comptime LOG_PARSE_STATUS_CAPACITY: Int64 = 2
comptime LOG_FIELD_WIDTH: Int64 = 4


def log_ascii_whitespace(value: UInt8) -> Bool:
    return value == 32 or value == 9 or value == 10 or value == 12 or value == 13


def log_skip_whitespace(
    ptr: Pointer[mut=False, UInt8, _], index: Int64, end: Int64
) -> Int64:
    var cursor = index
    while cursor < end and log_ascii_whitespace(ptr[unsafe_offset=cursor]):
        cursor += 1
    return cursor


def log_skip_key_or_token(
    ptr: Pointer[mut=False, UInt8, _], index: Int64, end: Int64
) -> Int64:
    var cursor = index
    while (
        cursor < end
        and not log_ascii_whitespace(ptr[unsafe_offset=cursor])
        and ptr[unsafe_offset=cursor] != 61
    ):
        cursor += 1
    return cursor


def log_skip_token(
    ptr: Pointer[mut=False, UInt8, _], index: Int64, end: Int64
) -> Int64:
    var cursor = index
    while cursor < end and not log_ascii_whitespace(ptr[unsafe_offset=cursor]):
        cursor += 1
    return cursor


def log_skip_field_value(
    ptr: Pointer[mut=False, UInt8, _], index: Int64, end: Int64
) -> Int64:
    var cursor = index
    if cursor >= end:
        return cursor
    if ptr[unsafe_offset=cursor] == 34:
        cursor += 1
        var escaped = False
        while cursor < end:
            var byte = ptr[unsafe_offset=cursor]
            if escaped:
                escaped = False
                cursor += 1
            elif byte == 92:
                escaped = True
                cursor += 1
            elif byte == 34:
                cursor += 1
                break
            else:
                cursor += 1
        return cursor
    while cursor < end and not log_ascii_whitespace(ptr[unsafe_offset=cursor]):
        cursor += 1
    return cursor


@export("prodex_mojo_log_parse_v1")
def prodex_mojo_log_parse_v1(
    message_address: UInt,
    message_length: Int64,
    records_address: UInt,
    record_capacity: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if (
        message_length < 0
        or record_capacity < 0
        or result_address == 0
        or (message_length > 0 and message_address == 0)
        or (record_capacity > 0 and records_address == 0)
    ):
        return LOG_PARSE_STATUS_INVALID

    var ptr = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(message_address)
    )
    var records = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(records_address)
    )
    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )

    var event_start: Int64 = -1
    var event_length: Int64 = 0
    var fields: Int64 = 0
    var index: Int64 = 0
    while index < message_length:
        index = log_skip_whitespace(ptr, index, message_length)
        if index >= message_length:
            break

        var token_start = index
        index = log_skip_key_or_token(ptr, index, message_length)
        if index < message_length and ptr[unsafe_offset=index] == 61:
            var key_end = index
            var value_start = index + 1
            var next_index = log_skip_field_value(ptr, value_start, message_length)
            if token_start < key_end and value_start < next_index:
                if fields >= record_capacity:
                    result[unsafe_offset=0] = event_start
                    result[unsafe_offset=1] = event_length
                    result[unsafe_offset=2] = fields + 1
                    return LOG_PARSE_STATUS_CAPACITY
                var base = fields * LOG_FIELD_WIDTH
                records[unsafe_offset=base] = token_start
                records[unsafe_offset=base + 1] = key_end
                records[unsafe_offset=base + 2] = value_start
                records[unsafe_offset=base + 3] = next_index
                fields += 1
            index = next_index
            continue

        if token_start < index and event_start < 0:
            event_start = token_start
            event_length = index - token_start
        index = log_skip_token(ptr, index, message_length)

    result[unsafe_offset=0] = event_start
    result[unsafe_offset=1] = event_length
    result[unsafe_offset=2] = fields
    return LOG_PARSE_STATUS_OK
