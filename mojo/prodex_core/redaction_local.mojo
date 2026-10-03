from std.collections import Array

from std.memory import Pointer

from redaction import (
    reaction_ascii_alphanumeric,
    reaction_ascii_alphabetic,
    reaction_key_sensitive_ptr,
    reaction_normalized_key_contains,
    reaction_normalized_key_matches,
    reaction_secret_token_end,
    reaction_skip_ascii_whitespace,
    reaction_starts_with_ignore_ascii_case,
    reaction_writer_literal,
    reaction_writer_range,
)
from rich_text import rich_view_valid
from rich_types import ProdexRichStringView, rich_view_ptr

comptime LOCAL_REDACTION_ABI_VERSION: Int64 = 1
comptime LOCAL_REDACTION_OK: Int64 = 0
comptime LOCAL_REDACTION_INVALID: Int64 = 1
comptime LOCAL_REDACTION_CAPACITY: Int64 = 2
comptime LOCAL_REDACTION_ABI: Int64 = 4
comptime LOCAL_REDACTION_FINDING_COUNT: Int64 = 12
comptime LOCAL_REDACTION_ACCESS_TOKEN: Int64 = 7
comptime LOCAL_REDACTION_API_KEY: Int64 = 8
comptime LOCAL_REDACTION_PRIVATE_KEY: Int64 = 9
comptime LOCAL_REDACTION_PASSWORD: Int64 = 10
comptime LOCAL_REDACTION_STREAM_COUNT: Int64 = 6


def local_find_literal(
    source: Pointer[mut=False, UInt8, _],
    length: Int64,
    start: Int64,
    literal: StringSlice,
) -> Int64:
    var literal_length = Int64(literal.byte_length())
    if start < 0 or literal_length == 0 or literal_length > length - start:
        return -1
    var target = literal.unsafe_ptr()
    for candidate in range(start, length - literal_length + 1):
        var matched = True
        for offset in range(literal_length):
            if (
                source[unsafe_offset=candidate + offset]
                != target[unsafe_offset=offset]
            ):
                matched = False
                break
        if matched:
            return candidate
    return -1


def local_sensitive_key_kind(
    key: Pointer[mut=False, UInt8, _], length: Int64
) -> Int64:
    if reaction_normalized_key_contains(key, length, StringSlice("privatekey")):
        return LOCAL_REDACTION_PRIVATE_KEY
    if reaction_normalized_key_contains(key, length, StringSlice("apikey")):
        return LOCAL_REDACTION_API_KEY
    if reaction_normalized_key_contains(
        key, length, StringSlice("token")
    ) or reaction_normalized_key_matches(
        key, length, StringSlice("authorization")
    ):
        return LOCAL_REDACTION_ACCESS_TOKEN
    return LOCAL_REDACTION_PASSWORD


def local_next_private_key(
    input: ProdexRichStringView,
    length: Int64,
    cursor: Pointer[mut=True, Int64, _],
    start_output: Pointer[mut=True, Int64, _],
    end_output: Pointer[mut=True, Int64, _],
) -> Bool:
    var source = rich_view_ptr(input)
    var offset = cursor[]
    var header_marker = StringSlice("-----BEGIN ")
    var key_marker = StringSlice("PRIVATE KEY-----")
    var key_marker_length = Int64(key_marker.byte_length())
    while offset < length:
        var start = local_find_literal(source, length, offset, header_marker)
        if start < 0:
            break
        var header_end = local_find_literal(source, length, start, key_marker)
        if header_end < 0:
            break
        var body_start = header_end + key_marker_length
        var footer = local_find_literal(source, length, body_start, key_marker)
        var end = length if footer < 0 else footer + key_marker_length
        cursor[] = end
        start_output[] = start
        end_output[] = end
        return True
    cursor[] = length
    return False


def local_delimited_value_range(
    source: Pointer[mut=False, UInt8, _],
    length: Int64,
    start: Int64,
    start_output: Pointer[mut=True, Int64, _],
    end_output: Pointer[mut=True, Int64, _],
):
    start_output[] = start
    end_output[] = start
    if start < 0 or start >= length:
        return
    var first = source[unsafe_offset=start]
    if first == 39 or first == 34:
        var value_start = start + 1
        var end = value_start
        while end < length and source[unsafe_offset=end] != first:
            end += 1
        start_output[] = value_start
        end_output[] = end
        return
    end_output[] = reaction_secret_token_end(source, length, start)


def local_next_labeled_credential(
    input: ProdexRichStringView,
    length: Int64,
    cursor: Pointer[mut=True, Int64, _],
    start_output: Pointer[mut=True, Int64, _],
    end_output: Pointer[mut=True, Int64, _],
    kind_output: Pointer[mut=True, Int64, _],
) -> Bool:
    var source = rich_view_ptr(input)
    var index = cursor[]
    while index < length:
        var first = source[unsafe_offset=index]
        if not reaction_ascii_alphabetic(first) and first != 95 and first != 45:
            index += 1
            continue
        var key_start = index
        index += 1
        while index < length:
            var value = source[unsafe_offset=index]
            if (
                not reaction_ascii_alphanumeric(value)
                and value != 95
                and value != 45
            ):
                break
            index += 1
        var key_end = index
        var separator_index = reaction_skip_ascii_whitespace(
            source, length, index
        )
        if separator_index >= length:
            continue
        var separator = source[unsafe_offset=separator_index]
        if separator != 58 and separator != 61:
            continue
        var key = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
            unsafe_from_address=Int(input.ptr) + Int(key_start)
        )
        if not reaction_key_sensitive_ptr(key, key_end - key_start):
            continue
        var value_start = reaction_skip_ascii_whitespace(
            source, length, separator_index + 1
        )
        var value_start_storage: Int64 = 0
        var value_start_output = Pointer(to=value_start_storage)
        var value_end_storage: Int64 = value_start
        var value_end_output = Pointer(to=value_end_storage)
        local_delimited_value_range(
            source, length, value_start, value_start_output, value_end_output
        )
        index = max(value_end_output[], key_end)
        cursor[] = index
        if value_start_output[] < value_end_output[]:
            start_output[] = value_start_output[]
            end_output[] = value_end_output[]
            kind_output[] = local_sensitive_key_kind(key, key_end - key_start)
            return True
    cursor[] = length
    return False


def local_next_bearer_token(
    input: ProdexRichStringView,
    length: Int64,
    cursor: Pointer[mut=True, Int64, _],
    start_output: Pointer[mut=True, Int64, _],
    end_output: Pointer[mut=True, Int64, _],
) -> Bool:
    var source = rich_view_ptr(input)
    var index = cursor[]
    while index + 6 <= length:
        if reaction_starts_with_ignore_ascii_case(
            source, length, index, StringSlice("bearer")
        ) and (
            index == 0
            or not reaction_ascii_alphanumeric(source[unsafe_offset=index - 1])
        ):
            var start = reaction_skip_ascii_whitespace(
                source, length, index + 6
            )
            if start > index + 6:
                var end = reaction_secret_token_end(source, length, start)
                if end > start:
                    cursor[] = end
                    start_output[] = start
                    end_output[] = end
                    return True
        index += 1
    cursor[] = length
    return False


def local_api_prefix_length(
    source: Pointer[mut=False, UInt8, _], length: Int64, index: Int64
) -> Int64:
    if reaction_starts_with_ignore_ascii_case(
        source, length, index, StringSlice("sk-proj-")
    ):
        return 8
    if reaction_starts_with_ignore_ascii_case(
        source, length, index, StringSlice("sk-ant-")
    ):
        return 7
    if reaction_starts_with_ignore_ascii_case(
        source, length, index, StringSlice("sk-live-")
    ):
        return 8
    if reaction_starts_with_ignore_ascii_case(
        source, length, index, StringSlice("sk_test_")
    ):
        return 8
    if reaction_starts_with_ignore_ascii_case(
        source, length, index, StringSlice("sk_live_")
    ):
        return 8
    if reaction_starts_with_ignore_ascii_case(
        source, length, index, StringSlice("sk-")
    ):
        return 3
    if reaction_starts_with_ignore_ascii_case(
        source, length, index, StringSlice("sk_")
    ):
        return 3
    return 0


def local_next_prefixed_api_key(
    input: ProdexRichStringView,
    length: Int64,
    cursor: Pointer[mut=True, Int64, _],
    start_output: Pointer[mut=True, Int64, _],
    end_output: Pointer[mut=True, Int64, _],
) -> Bool:
    var source = rich_view_ptr(input)
    var index = cursor[]
    while index < length:
        var prefix_length = local_api_prefix_length(source, length, index)
        if prefix_length == 0:
            index += 1
            continue
        var end = reaction_secret_token_end(source, length, index)
        if end >= index + prefix_length + 8:
            cursor[] = end
            start_output[] = index
            end_output[] = end
            return True
        index += 1
    cursor[] = length
    return False


def local_email_byte(value: UInt8) -> Bool:
    return (
        reaction_ascii_alphanumeric(value)
        or value == 46
        or value == 95
        or value == 37
        or value == 43
        or value == 45
        or value == 64
    )


def local_email_token_valid(
    source: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var at: Int64 = -1
    for index in range(start, end):
        if source[unsafe_offset=index] == 64:
            at = index
            break
    if at <= start or at + 1 >= end:
        return False
    var part_start = at + 1
    var has_dot = False
    for index in range(part_start, end):
        if source[unsafe_offset=index] == 46:
            if index == part_start:
                return False
            has_dot = True
            part_start = index + 1
    return has_dot and part_start < end


def local_next_email(
    input: ProdexRichStringView,
    length: Int64,
    cursor: Pointer[mut=True, Int64, _],
    start_output: Pointer[mut=True, Int64, _],
    end_output: Pointer[mut=True, Int64, _],
) -> Bool:
    var source = rich_view_ptr(input)
    var index = cursor[]
    while index < length:
        if not local_email_byte(source[unsafe_offset=index]):
            index += 1
            continue
        var start = index
        while index < length and local_email_byte(source[unsafe_offset=index]):
            index += 1
        if local_email_token_valid(source, start, index):
            cursor[] = index
            start_output[] = start
            end_output[] = index
            return True
    cursor[] = length
    return False


def local_next_financial_identifier(
    input: ProdexRichStringView,
    length: Int64,
    cursor: Pointer[mut=True, Int64, _],
    start_output: Pointer[mut=True, Int64, _],
    end_output: Pointer[mut=True, Int64, _],
) -> Bool:
    var source = rich_view_ptr(input)
    var index = cursor[]
    while index < length:
        var first = source[unsafe_offset=index]
        if first < 48 or first > 57:
            index += 1
            continue
        var start = index
        var digits: Int64 = 0
        while index < length:
            var value = source[unsafe_offset=index]
            if value >= 48 and value <= 57:
                digits += 1
            elif value != 32 and value != 45:
                break
            index += 1
        while index > start:
            var tail = source[unsafe_offset=index - 1]
            if tail != 32 and tail != 45:
                break
            index -= 1
        if digits >= 13 and digits <= 19:
            cursor[] = max(index, start + 1)
            start_output[] = start
            end_output[] = index
            return True
        index = max(index, start + 1)
    cursor[] = length
    return False


def local_next_match(
    input: ProdexRichStringView,
    length: Int64,
    stream: Int64,
    cursors: Pointer[mut=True, Int64, _],
    starts: Pointer[mut=True, Int64, _],
    ends: Pointer[mut=True, Int64, _],
    kinds: Pointer[mut=True, Int64, _],
) -> Bool:
    starts[unsafe_offset=stream] = -1
    ends[unsafe_offset=stream] = -1
    var cursor = Pointer(to=cursors[unsafe_offset=stream])
    var start = Pointer(to=starts[unsafe_offset=stream])
    var end = Pointer(to=ends[unsafe_offset=stream])
    var kind = Pointer(to=kinds[unsafe_offset=stream])
    if stream == 0:
        kind[] = LOCAL_REDACTION_PRIVATE_KEY
        return local_next_private_key(input, length, cursor, start, end)
    if stream == 1:
        return local_next_labeled_credential(
            input, length, cursor, start, end, kind
        )
    if stream == 2:
        kind[] = LOCAL_REDACTION_ACCESS_TOKEN
        return local_next_bearer_token(input, length, cursor, start, end)
    if stream == 3:
        kind[] = LOCAL_REDACTION_API_KEY
        return local_next_prefixed_api_key(input, length, cursor, start, end)
    if stream == 4:
        kind[] = 0
        return local_next_email(input, length, cursor, start, end)
    kind[] = 5
    return local_next_financial_identifier(input, length, cursor, start, end)


def local_stream_match_before(
    starts: Pointer[mut=True, Int64, _],
    ends: Pointer[mut=True, Int64, _],
    kinds: Pointer[mut=True, Int64, _],
    left: Int64,
    right: Int64,
) -> Bool:
    if starts[unsafe_offset=left] != starts[unsafe_offset=right]:
        return starts[unsafe_offset=left] < starts[unsafe_offset=right]
    if ends[unsafe_offset=left] != ends[unsafe_offset=right]:
        return ends[unsafe_offset=left] > ends[unsafe_offset=right]
    return kinds[unsafe_offset=left] < kinds[unsafe_offset=right]


def local_mask_from_streams(
    input: ProdexRichStringView,
    length: Int64,
    cursors: Pointer[mut=True, Int64, _],
    starts: Pointer[mut=True, Int64, _],
    ends: Pointer[mut=True, Int64, _],
    kinds: Pointer[mut=True, Int64, _],
    output: Pointer[mut=True, UInt8, _],
    output_capacity: Int64,
    match_output: Pointer[mut=True, Int64, _],
    match_capacity: Int64,
    match_count_output: Pointer[mut=True, Int64, _],
    written_output: Pointer[mut=True, Int64, _],
) -> Int64:
    var source = rich_view_ptr(input)
    var written_storage: Int64 = 0
    var written = Pointer(to=written_storage)
    var selected: Int64 = 0
    var covered_until: Int64 = 0
    var source_cursor: Int64 = 0
    while True:
        var selected_stream: Int64 = -1
        for stream in range(LOCAL_REDACTION_STREAM_COUNT):
            if starts[unsafe_offset=stream] < 0:
                continue
            if selected_stream < 0 or local_stream_match_before(
                starts, ends, kinds, stream, selected_stream
            ):
                selected_stream = stream
        if selected_stream < 0:
            break
        var start = starts[unsafe_offset=selected_stream]
        var end = ends[unsafe_offset=selected_stream]
        var kind = kinds[unsafe_offset=selected_stream]
        if start >= covered_until and start < end:
            if selected >= match_capacity:
                return LOCAL_REDACTION_CAPACITY
            if not reaction_writer_range(
                source, source_cursor, start, output, output_capacity, written
            ):
                return LOCAL_REDACTION_CAPACITY
            if not reaction_writer_literal(
                output, output_capacity, written, StringSlice("<redacted>")
            ):
                return LOCAL_REDACTION_CAPACITY
            var record_offset = selected * 3
            match_output[unsafe_offset=record_offset] = start
            match_output[unsafe_offset=record_offset + 1] = end
            match_output[unsafe_offset=record_offset + 2] = kind
            selected += 1
            covered_until = end
            source_cursor = end
        _ = local_next_match(
            input, length, selected_stream, cursors, starts, ends, kinds
        )
    if not reaction_writer_range(
        source, source_cursor, length, output, output_capacity, written
    ):
        return LOCAL_REDACTION_CAPACITY
    match_count_output[] = selected
    written_output[] = written[]
    return LOCAL_REDACTION_OK


@export("prodex_redaction_local_inspection_v1")
def prodex_redaction_local_inspection_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    sensitive_kind: Int64,
    output_address: UInt,
    output_capacity: Int64,
    match_output_address: UInt,
    match_capacity: Int64,
    match_count_address: UInt,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != LOCAL_REDACTION_ABI_VERSION:
        return LOCAL_REDACTION_ABI
    if (
        input_length < 0
        or output_capacity <= 0
        or match_capacity < 0
        or match_count_address == 0
        or written_address == 0
        or output_address == 0
        or match_output_address == 0
        or (input_length > 0 and input_address == 0)
        or sensitive_kind < -1
        or sensitive_kind >= LOCAL_REDACTION_FINDING_COUNT
    ):
        return LOCAL_REDACTION_INVALID
    var input = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(input, input_length):
        return LOCAL_REDACTION_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var match_output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(match_output_address)
    )
    var match_count_output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(match_count_address)
    )
    var written_output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    if sensitive_kind >= 0 and input_length > 0:
        if match_capacity == 0:
            return LOCAL_REDACTION_CAPACITY
        var written_storage: Int64 = 0
        var written = Pointer(to=written_storage)
        if not reaction_writer_literal(
            output, output_capacity, written, StringSlice("<redacted>")
        ):
            return LOCAL_REDACTION_CAPACITY
        match_output[unsafe_offset=0] = 0
        match_output[unsafe_offset=1] = input_length
        match_output[unsafe_offset=2] = sensitive_kind
        match_count_output[] = 1
        written_output[] = written[]
        return LOCAL_REDACTION_OK

    var cursors_storage = Array[Int64, 6](fill=0)
    var starts_storage = Array[Int64, 6](fill=-1)
    var ends_storage = Array[Int64, 6](fill=-1)
    var kinds_storage = Array[Int64, 6](fill=0)
    var cursors_pointer = cursors_storage.unsafe_ptr()
    var starts_pointer = starts_storage.unsafe_ptr()
    var ends_pointer = ends_storage.unsafe_ptr()
    var kinds_pointer = kinds_storage.unsafe_ptr()
    if sensitive_kind < 0:
        for stream in range(LOCAL_REDACTION_STREAM_COUNT):
            _ = local_next_match(
                input,
                input_length,
                stream,
                cursors_pointer,
                starts_pointer,
                ends_pointer,
                kinds_pointer,
            )
    return local_mask_from_streams(
        input,
        input_length,
        cursors_pointer,
        starts_pointer,
        ends_pointer,
        kinds_pointer,
        output,
        output_capacity,
        match_output,
        match_capacity,
        match_count_output,
        written_output,
    )
