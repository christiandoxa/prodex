from std.memory import Pointer

from rich_text import rich_codepoint, rich_codepoint_width, rich_unicode_space, rich_view_valid
from rich_types import ProdexRichStringView

comptime MARKER_ABI_VERSION: Int64 = 1
comptime MARKER_OK: Int64 = 0
comptime MARKER_INVALID: Int64 = 1
comptime MARKER_CAPACITY: Int64 = 3

comptime OP_PATH_LIKE_FILE: Int64 = 0
comptime OP_FILE_LOCATION_TOKEN: Int64 = 1
comptime OP_DIFF_FILE_PATH: Int64 = 2
comptime OP_DIFF_SPAN: Int64 = 3
comptime OP_TEST_FAILURE: Int64 = 4
comptime OP_TEST_SYMBOL: Int64 = 5
comptime OP_ERROR_CODE: Int64 = 6
comptime OP_COMMAND_LINE_KIND: Int64 = 7

comptime MAX_INPUT_BYTES: Int64 = 4 * 1024 * 1024
comptime MAX_FIELD_BYTES: Int64 = 512
comptime U64_NONE: UInt64 = 0xFFFFFFFFFFFFFFFF

def marker_ptr(address: UInt) -> Pointer[mut=False, UInt8, ImmUntrackedOrigin]:
    return Pointer[mut=False, UInt8, ImmUntrackedOrigin](unsafe_from_address=Int(address))

def marker_byte(address: UInt, index: Int64) -> UInt8:
    return marker_ptr(address)[unsafe_offset=index]

def marker_literal_at[literal: StaticString](
    address: UInt, start: Int64, end: Int64
) -> Bool:
    var n = Int64(literal.byte_length())
    if start < 0 or start + n > end:
        return False
    var source = marker_ptr(address)
    var wanted = literal.unsafe_ptr()
    for index in range(n):
        if source[unsafe_offset=start + index] != wanted[unsafe_offset=index]:
            return False
    return True

def marker_range_equals[literal: StaticString](
    address: UInt, start: Int64, end: Int64
) -> Bool:
    return end - start == Int64(literal.byte_length()) and marker_literal_at[literal](
        address, start, end
    )

def marker_contains[literal: StaticString](
    address: UInt, start: Int64, end: Int64
) -> Bool:
    var n = Int64(literal.byte_length())
    if n == 0:
        return True
    var cursor = start
    while cursor + n <= end:
        if marker_literal_at[literal](address, cursor, end):
            return True
        cursor += 1
    return False

def marker_find[literal: StaticString](
    address: UInt, start: Int64, end: Int64
) -> Int64:
    var n = Int64(literal.byte_length())
    var cursor = start
    while cursor + n <= end:
        if marker_literal_at[literal](address, cursor, end):
            return cursor
        cursor += 1
    return -1

def marker_ends_with[literal: StaticString](
    address: UInt, start: Int64, end: Int64
) -> Bool:
    var n = Int64(literal.byte_length())
    if n > end - start:
        return False
    return marker_range_equals[literal](address, end - n, end)

def marker_ascii_ws(value: UInt8) -> Bool:
    return value == 9 or value == 10 or value == 11 or value == 12 or value == 13 or value == 32

def marker_trim_punctuation(value: UInt8) -> Bool:
    return (
        value == 34 or value == 39 or value == 96
        or value == 40 or value == 41
        or value == 91 or value == 93
        or value == 123 or value == 125
        or value == 44 or value == 59
    )

def marker_trim_bounds(address: UInt, length: Int64) -> Tuple[Int64, Int64]:
    var start: Int64 = 0
    var end = length
    while start < end and marker_trim_punctuation(marker_byte(address, start)):
        start += 1
    while end > start and marker_trim_punctuation(marker_byte(address, end - 1)):
        end -= 1
    while end > start and marker_byte(address, end - 1) == 58:
        end -= 1
    return (start, end)

def marker_parse_u64(address: UInt, start: Int64, end: Int64) -> UInt64:
    if start >= end:
        return U64_NONE
    var value: UInt64 = 0
    for index in range(start, end):
        var byte = marker_byte(address, index)
        if byte < 48 or byte > 57:
            return U64_NONE
        var digit = UInt64(byte - 48)
        if value > (U64_NONE - digit) // 10:
            return U64_NONE
        value = value * 10 + digit
    return value

def marker_field_span_valid(start: Int64, end: Int64) -> Bool:
    return start >= 0 and end > start and end - start <= MAX_FIELD_BYTES

def marker_extension_known(address: UInt, start: Int64, end: Int64) -> Bool:
    return (
        marker_range_equals["rs"](address, start, end)
        or marker_range_equals["toml"](address, start, end)
        or marker_range_equals["json"](address, start, end)
        or marker_range_equals["md"](address, start, end)
        or marker_range_equals["ts"](address, start, end)
        or marker_range_equals["tsx"](address, start, end)
        or marker_range_equals["js"](address, start, end)
        or marker_range_equals["jsx"](address, start, end)
        or marker_range_equals["py"](address, start, end)
        or marker_range_equals["go"](address, start, end)
        or marker_range_equals["java"](address, start, end)
        or marker_range_equals["kt"](address, start, end)
        or marker_range_equals["swift"](address, start, end)
        or marker_range_equals["c"](address, start, end)
        or marker_range_equals["cc"](address, start, end)
        or marker_range_equals["cpp"](address, start, end)
        or marker_range_equals["h"](address, start, end)
        or marker_range_equals["hpp"](address, start, end)
        or marker_range_equals["css"](address, start, end)
        or marker_range_equals["scss"](address, start, end)
        or marker_range_equals["html"](address, start, end)
        or marker_range_equals["yml"](address, start, end)
        or marker_range_equals["yaml"](address, start, end)
        or marker_range_equals["sh"](address, start, end)
        or marker_range_equals["bash"](address, start, end)
        or marker_range_equals["zsh"](address, start, end)
        or marker_range_equals["sql"](address, start, end)
        or marker_range_equals["lock"](address, start, end)
    )

def marker_path_like_file(address: UInt, start: Int64, end: Int64) -> Bool:
    if start < 0 or end <= start or end - start > MAX_FIELD_BYTES:
        return False
    var last_dot: Int64 = -1
    for index in range(start, end):
        var byte = marker_byte(address, index)
        if byte == 47 or byte == 92:
            return True
        if byte == 46:
            last_dot = index
    return last_dot >= start and last_dot + 1 < end and marker_extension_known(
        address, last_dot + 1, end
    )

def marker_strip_location_prefixes(
    address: UInt, start: Int64, end: Int64
) -> Int64:
    var cursor = start
    while marker_literal_at["file://"](address, cursor, end):
        cursor += 7
    while marker_literal_at["a/"](address, cursor, end):
        cursor += 2
    while marker_literal_at["b/"](address, cursor, end):
        cursor += 2
    return cursor

def marker_file_location(
    address: UInt,
    length: Int64,
    meta: Pointer[mut=True, UInt64, _],
) -> Bool:
    var bounds = marker_trim_bounds(address, length)
    var start = bounds[0]
    var end = bounds[1]
    if start >= end:
        return False

    var last_colon: Int64 = -1
    var cursor = end
    while cursor > start:
        cursor -= 1
        if marker_byte(address, cursor) == 58:
            last_colon = cursor
            break
    if last_colon < 0:
        return False
    var last_number = marker_parse_u64(address, last_colon + 1, end)
    if last_number == U64_NONE:
        return False

    var prefix_end = last_colon
    var second_colon: Int64 = -1
    cursor = prefix_end
    while cursor > start:
        cursor -= 1
        if marker_byte(address, cursor) == 58:
            second_colon = cursor
            break

    var path_end = prefix_end
    var line = last_number
    var column = U64_NONE
    if second_colon >= 0:
        var maybe_line = marker_parse_u64(address, second_colon + 1, prefix_end)
        if maybe_line != U64_NONE:
            path_end = second_colon
            line = maybe_line
            column = last_number

    var path_start = marker_strip_location_prefixes(address, start, path_end)
    if not marker_path_like_file(address, path_start, path_end):
        return False
    meta[unsafe_offset=0] = UInt64(path_start)
    meta[unsafe_offset=1] = UInt64(path_end)
    meta[unsafe_offset=2] = line
    meta[unsafe_offset=3] = column
    return True

def marker_diff_file_path(
    address: UInt,
    length: Int64,
    meta: Pointer[mut=True, UInt64, _],
) -> Bool:
    if marker_range_equals["/dev/null"](address, 0, length):
        return False
    var start: Int64 = 0
    var end = length
    if marker_literal_at["a/"](address, start, end):
        start += 2
    if marker_literal_at["b/"](address, start, end):
        start += 2
    while start < end and marker_byte(address, start) == 34:
        start += 1
    while end > start and marker_byte(address, end - 1) == 34:
        end -= 1
    if not marker_field_span_valid(start, end):
        return False
    meta[unsafe_offset=0] = UInt64(start)
    meta[unsafe_offset=1] = UInt64(end)
    return True

def marker_diff_span(
    address: UInt,
    length: Int64,
    prefix: UInt64,
    meta: Pointer[mut=True, UInt64, _],
) -> Bool:
    if length < 2 or UInt64(marker_byte(address, 0)) != prefix:
        return False
    var comma: Int64 = -1
    for index in range(Int64(1), length):
        if marker_byte(address, index) == 44:
            comma = index
            break
    var start_end = comma if comma >= 0 else length
    var first = marker_parse_u64(address, 1, start_end)
    if first == U64_NONE:
        return False
    var count: UInt64 = 1
    if comma >= 0:
        count = marker_parse_u64(address, comma + 1, length)
        if count == U64_NONE:
            return False
    meta[unsafe_offset=0] = first
    meta[unsafe_offset=1] = count
    return True

def marker_test_failure(address: UInt, length: Int64) -> Bool:
    return (
        marker_contains["test result: FAILED"](address, 0, length)
        or marker_range_equals["failures:"](address, 0, length)
        or marker_literal_at["failures:"](address, 0, length)
        or marker_literal_at["FAIL "](address, 0, length)
        or marker_literal_at["FAILED "](address, 0, length)
        or marker_contains[" panicked at "](address, 0, length)
        or (
            marker_literal_at["---- "](address, 0, length)
            and marker_ends_with[" stdout ----"](address, 0, length)
        )
    )

def marker_test_symbol(
    address: UInt,
    length: Int64,
    meta: Pointer[mut=True, UInt64, _],
) -> Bool:
    if (
        marker_literal_at["---- "](address, 0, length)
        and marker_ends_with[" stdout ----"](address, 0, length)
    ):
        var start: Int64 = 5
        var end = length - Int64(StringSlice(" stdout ----").byte_length())
        if marker_field_span_valid(start, end):
            meta[unsafe_offset=0] = UInt64(start)
            meta[unsafe_offset=1] = UInt64(end)
            return True

    if marker_literal_at["thread '"](address, 0, length):
        var marker = marker_find["' panicked at "](address, 8, length)
        if marker >= 0 and marker_field_span_valid(8, marker):
            meta[unsafe_offset=0] = 8
            meta[unsafe_offset=1] = UInt64(marker)
            return True
    return False

def marker_write_literal[literal: StaticString](
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    var n = Int64(literal.byte_length())
    if written[] < 0 or written[] + n > capacity:
        return False
    var source = literal.unsafe_ptr()
    for index in range(n):
        output[unsafe_offset=written[] + index] = source[unsafe_offset=index]
    written[] += n
    return True

def marker_write_range(
    address: UInt,
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    if not marker_field_span_valid(start, end) or written[] + end - start > capacity:
        return False
    var source = marker_ptr(address)
    for index in range(end - start):
        output[unsafe_offset=written[] + index] = source[unsafe_offset=start + index]
    written[] += end - start
    return True

def marker_skip_unicode_ws(address: UInt, start: Int64, end: Int64) -> Int64:
    var cursor = start
    var source = marker_ptr(address)
    while cursor < end:
        var width = rich_codepoint_width(source[unsafe_offset=cursor])
        if not rich_unicode_space(rich_codepoint(source, cursor, width)):
            break
        cursor += width
    return cursor

def marker_token_end(address: UInt, start: Int64, end: Int64) -> Int64:
    var cursor = start
    var source = marker_ptr(address)
    while cursor < end:
        var width = rich_codepoint_width(source[unsafe_offset=cursor])
        if rich_unicode_space(rich_codepoint(source, cursor, width)):
            break
        cursor += width
    return cursor

def marker_error_code(
    address: UInt,
    length: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    var bracket = marker_find["error["](address, 0, length)
    if bracket >= 0:
        var start = bracket + 6
        var end = start
        while end < length and marker_byte(address, end) != 93:
            end += 1
        if end < length and marker_write_range(
            address, start, end, output, capacity, written
        ):
            return True

    if (
        marker_contains["error:"](address, 0, length)
        or marker_contains["Error:"](address, 0, length)
        or marker_contains["ERROR"](address, 0, length)
    ):
        return marker_write_literal["error"](output, capacity, written)

    var exit_marker = marker_find["exit code "](address, 0, length)
    if exit_marker >= 0:
        var start = marker_skip_unicode_ws(address, exit_marker + 10, length)
        var end = marker_token_end(address, start, length)
        if marker_field_span_valid(start, end):
            return (
                marker_write_literal["exit_code_"](output, capacity, written)
                and marker_write_range(address, start, end, output, capacity, written)
            )

    var status_marker = marker_find["status code "](address, 0, length)
    if status_marker >= 0:
        var start = marker_skip_unicode_ws(address, status_marker + 12, length)
        var end = marker_token_end(address, start, length)
        if marker_field_span_valid(start, end):
            return (
                marker_write_literal["status_code_"](output, capacity, written)
                and marker_write_range(address, start, end, output, capacity, written)
            )
    return False

def marker_command_kind(address: UInt, length: Int64) -> UInt64:
    if marker_range_equals["Traceback (most recent call last):"](address, 0, length):
        return 1
    if marker_literal_at["diff --git "](address, 0, length) or marker_literal_at["@@ "](address, 0, length):
        return 2
    if (
        marker_contains["test result:"](address, 0, length)
        or (
            marker_literal_at["running "](address, 0, length)
            and marker_ends_with[" tests"](address, 0, length)
        )
    ):
        return 3
    if marker_contains["error: could not compile"](address, 0, length):
        return 4
    if marker_literal_at["npm ERR!"](address, 0, length) or marker_literal_at["FAIL "](address, 0, length):
        return 5
    return 0

@export("prodex_smart_context_markers_v1")
def prodex_smart_context_markers_v1(
    abi_version: Int64,
    operation: Int64,
    address: UInt,
    length: Int64,
    aux: UInt64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
    meta_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != MARKER_ABI_VERSION
        or operation < OP_PATH_LIKE_FILE
        or operation > OP_COMMAND_LINE_KIND
        or length < 0
        or length > MAX_INPUT_BYTES
        or (length > 0 and address == 0)
        or meta_address == 0
    ):
        return MARKER_INVALID

    var input = ProdexRichStringView(address, UInt(length))
    if not rich_view_valid(input, MAX_INPUT_BYTES):
        return MARKER_INVALID

    var meta = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(meta_address)
    )
    for index in range(4):
        meta[unsafe_offset=index] = U64_NONE

    if operation == OP_PATH_LIKE_FILE:
        meta[unsafe_offset=0] = UInt64(marker_path_like_file(address, 0, length))
        return MARKER_OK

    if operation == OP_FILE_LOCATION_TOKEN:
        meta[unsafe_offset=0] = 0
        if marker_file_location(address, length, meta):
            meta[unsafe_offset=0] = meta[unsafe_offset=0]
        else:
            meta[unsafe_offset=0] = U64_NONE
        return MARKER_OK

    if operation == OP_DIFF_FILE_PATH:
        if not marker_diff_file_path(address, length, meta):
            meta[unsafe_offset=0] = U64_NONE
        return MARKER_OK

    if operation == OP_DIFF_SPAN:
        if aux > 255:
            return MARKER_INVALID
        if not marker_diff_span(address, length, aux, meta):
            meta[unsafe_offset=0] = U64_NONE
        return MARKER_OK

    if operation == OP_TEST_FAILURE:
        meta[unsafe_offset=0] = UInt64(marker_test_failure(address, length))
        return MARKER_OK

    if operation == OP_TEST_SYMBOL:
        if not marker_test_symbol(address, length, meta):
            meta[unsafe_offset=0] = U64_NONE
        return MARKER_OK

    if operation == OP_ERROR_CODE:
        if output_address == 0 or output_capacity <= 0 or written_address == 0:
            return MARKER_INVALID
        var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
            unsafe_from_address=Int(output_address)
        )
        var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
            unsafe_from_address=Int(written_address)
        )
        written[] = 0
        if not marker_error_code(
            address, length, output, output_capacity, written
        ):
            written[] = 0
        return MARKER_OK

    meta[unsafe_offset=0] = marker_command_kind(address, length)
    return MARKER_OK

comptime SEMANTIC_ABI_VERSION: Int64 = 1
comptime SEMANTIC_OK: Int64 = 0
comptime SEMANTIC_INVALID: Int64 = 1
comptime SEMANTIC_CAPACITY: Int64 = 3
comptime SEMANTIC_ABI_MISMATCH: Int64 = 4
comptime SEMANTIC_MAX_RANGES: Int64 = 256
comptime SEMANTIC_MAX_EXCERPT_BYTES: Int64 = 16 * 1024
comptime SEMANTIC_MAX_FIELD_BYTES: Int64 = 512
comptime SEMANTIC_RECORD_WIDTH: Int64 = 15
comptime SEMANTIC_NONE: UInt64 = 0xFFFFFFFFFFFFFFFF

def semantic_line_value(
    lines: Pointer[mut=False, UInt64, _], index: Int64, field: Int64
) -> Int64:
    return Int64(lines[unsafe_offset=index * 2 + field])

def semantic_next_token(
    address: UInt, start: Int64, end: Int64
) -> Tuple[Bool, Int64, Int64]:
    var cursor = marker_skip_unicode_ws(address, start, end)
    if cursor >= end:
        return (False, 0, 0)
    var token_start = cursor
    cursor = marker_token_end(address, cursor, end)
    return (True, token_start, cursor)

def semantic_excerpt_bytes(
    lines: Pointer[mut=False, UInt64, _], start_line: Int64, end_line: Int64
) -> Int64:
    var total = end_line - start_line
    for index in range(start_line - 1, end_line):
        total += semantic_line_value(lines, index, 1) - semantic_line_value(lines, index, 0)
    return total

def semantic_line_starts_with[literal: StaticString](
    address: UInt, length: Int64
) -> Bool:
    return marker_literal_at[literal](address, 0, length)

def semantic_diff_path(
    address: UInt, length: Int64, span: Pointer[mut=True, UInt64, _]
) -> Bool:
    if not (
        semantic_line_starts_with["+++ "](address, length)
        or semantic_line_starts_with["--- "](address, length)
    ):
        return False
    var token = semantic_next_token(address, 4, length)
    if not token[0]:
        return False
    if not marker_diff_file_path(address + UInt(token[1]), token[2] - token[1], span):
        return False
    span[unsafe_offset=0] += UInt64(token[1])
    span[unsafe_offset=1] += UInt64(token[1])
    return True

def semantic_diff_hunk(
    address: UInt,
    length: Int64,
    old_span: Pointer[mut=True, UInt64, _],
    new_span: Pointer[mut=True, UInt64, _],
) -> Bool:
    var first = semantic_next_token(address, 0, length)
    if not first[0] or not marker_range_equals["@@"](
        address, first[1], first[2]
    ):
        return False
    var second = semantic_next_token(address, first[2], length)
    if not second[0] or not marker_diff_span(
        address + UInt(second[1]), second[2] - second[1], 45, old_span
    ):
        return False
    var third = semantic_next_token(address, second[2], length)
    if not third[0] or not marker_diff_span(
        address + UInt(third[1]), third[2] - third[1], 43, new_span
    ):
        return False
    return True

def semantic_diff_hunk_end(
    text_address: UInt,
    lines: Pointer[mut=False, UInt64, _],
    line_count: Int64,
    start_index: Int64,
) -> Int64:
    var max_end = start_index + 24
    if max_end >= line_count:
        max_end = line_count - 1
    for index in range(start_index + 1, max_end + 1):
        var address = text_address + UInt(semantic_line_value(lines, index, 0))
        var length = semantic_line_value(lines, index, 1) - semantic_line_value(lines, index, 0)
        if semantic_line_starts_with["@@ "](address, length) or semantic_line_starts_with[
            "diff --git "
        ](address, length):
            return index
        if length <= 0 or not (
            marker_byte(address, 0) == 32
            or marker_byte(address, 0) == 43
            or marker_byte(address, 0) == 45
            or semantic_line_starts_with["\\ No newline"](address, length)
        ):
            return index
    return max_end + 1

def semantic_copy_input(
    address: UInt,
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Tuple[Bool, UInt64, UInt64]:
    if (
        start < 0
        or end <= start
        or end - start > SEMANTIC_MAX_FIELD_BYTES
        or written[] < 0
        or written[] + end - start > capacity
    ):
        return (False, SEMANTIC_NONE, 0)
    var source = marker_ptr(address)
    var offset = written[]
    for index in range(end - start):
        output[unsafe_offset=offset + index] = source[unsafe_offset=start + index]
    written[] += end - start
    return (True, UInt64(offset), UInt64(end - start))

def semantic_copy_buffer(
    source: Pointer[mut=False, UInt8, _],
    length: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Tuple[Bool, UInt64, UInt64]:
    if (
        length <= 0
        or length > SEMANTIC_MAX_FIELD_BYTES
        or written[] < 0
        or written[] + length > capacity
    ):
        return (False, SEMANTIC_NONE, 0)
    var offset = written[]
    for index in range(length):
        output[unsafe_offset=offset + index] = source[unsafe_offset=index]
    written[] += length
    return (True, UInt64(offset), UInt64(length))

def semantic_fields_equal(
    fields: Pointer[mut=False, UInt8, _],
    left_start: UInt64,
    left_length: UInt64,
    right_start: UInt64,
    right_length: UInt64,
) -> Bool:
    if left_length != right_length:
        return False
    if left_length == 0:
        return True
    if left_start == SEMANTIC_NONE or right_start == SEMANTIC_NONE:
        return False
    for index in range(Int64(left_length)):
        if fields[unsafe_offset=Int64(left_start) + index] != fields[
            unsafe_offset=Int64(right_start) + index
        ]:
            return False
    return True

def semantic_record_duplicate(
    output: Pointer[mut=False, UInt64, _],
    fields: Pointer[mut=False, UInt8, _],
    count: Int64,
    kind: UInt64,
    start_line: UInt64,
    end_line: UInt64,
    path_start: UInt64,
    path_length: UInt64,
    code_start: UInt64,
    code_length: UInt64,
    symbol_start: UInt64,
    symbol_length: UInt64,
) -> Bool:
    for row in range(count):
        var base = row * SEMANTIC_RECORD_WIDTH
        if (
            output[unsafe_offset=base] == kind
            and output[unsafe_offset=base + 1] == start_line
            and output[unsafe_offset=base + 2] == end_line
            and semantic_fields_equal(
                fields,
                output[unsafe_offset=base + 3],
                output[unsafe_offset=base + 4],
                path_start,
                path_length,
            )
            and semantic_fields_equal(
                fields,
                output[unsafe_offset=base + 11],
                output[unsafe_offset=base + 12],
                code_start,
                code_length,
            )
            and semantic_fields_equal(
                fields,
                output[unsafe_offset=base + 13],
                output[unsafe_offset=base + 14],
                symbol_start,
                symbol_length,
            )
        ):
            return True
    return False

def semantic_emit_range(
    output: Pointer[mut=True, UInt64, _],
    output_read: Pointer[mut=False, UInt64, _],
    fields: Pointer[mut=False, UInt8, _],
    lines: Pointer[mut=False, UInt64, _],
    max_excerpt_bytes: Int64,
    max_ranges: Int64,
    count: Int64,
    complete: Pointer[mut=True, Int64, _],
    field_written: Pointer[mut=True, Int64, _],
    kind: UInt64,
    start_line: Int64,
    end_line: Int64,
    path_start: UInt64,
    path_length: UInt64,
    line: UInt64,
    column: UInt64,
    old_start: UInt64,
    old_count: UInt64,
    new_start: UInt64,
    new_count: UInt64,
    code_start: UInt64,
    code_length: UInt64,
    symbol_start: UInt64,
    symbol_length: UInt64,
) -> Int64:
    var before = field_written[]
    if count >= max_ranges:
        complete[] = 0
        field_written[] = before
        return count
    if (
        start_line < 1
        or end_line < start_line
        or semantic_excerpt_bytes(lines, start_line, end_line) > max_excerpt_bytes
    ):
        complete[] = 0
        field_written[] = before
        return count
    if semantic_record_duplicate(
        output_read,
        fields,
        count,
        kind,
        UInt64(start_line),
        UInt64(end_line),
        path_start,
        path_length,
        code_start,
        code_length,
        symbol_start,
        symbol_length,
    ):
        field_written[] = before
        return count
    var base = count * SEMANTIC_RECORD_WIDTH
    output[unsafe_offset=base] = kind
    output[unsafe_offset=base + 1] = UInt64(start_line)
    output[unsafe_offset=base + 2] = UInt64(end_line)
    output[unsafe_offset=base + 3] = path_start
    output[unsafe_offset=base + 4] = path_length
    output[unsafe_offset=base + 5] = line
    output[unsafe_offset=base + 6] = column
    output[unsafe_offset=base + 7] = old_start
    output[unsafe_offset=base + 8] = old_count
    output[unsafe_offset=base + 9] = new_start
    output[unsafe_offset=base + 10] = new_count
    output[unsafe_offset=base + 11] = code_start
    output[unsafe_offset=base + 12] = code_length
    output[unsafe_offset=base + 13] = symbol_start
    output[unsafe_offset=base + 14] = symbol_length
    return count + 1

@export("prodex_smart_context_semantic_index_v1")
def prodex_smart_context_semantic_index_v1(
    abi_version: Int64,
    text_address: UInt,
    text_length: Int64,
    line_spans_address: UInt,
    line_count: Int64,
    max_ranges: Int64,
    max_excerpt_bytes: Int64,
    output_address: UInt,
    output_capacity: Int64,
    field_address: UInt,
    field_capacity: Int64,
    metadata_address: UInt,
) abi("C") -> Int64:
    if abi_version != SEMANTIC_ABI_VERSION:
        return SEMANTIC_ABI_MISMATCH
    if (
        text_length < 0
        or text_length > MAX_INPUT_BYTES
        or (text_length > 0 and text_address == 0)
        or line_count < 0
        or line_count > text_length + 1
        or (line_count > 0 and line_spans_address == 0)
        or max_ranges < 0
        or max_ranges > SEMANTIC_MAX_RANGES
        or max_excerpt_bytes < 0
        or max_excerpt_bytes > SEMANTIC_MAX_EXCERPT_BYTES
        or output_capacity < max_ranges * SEMANTIC_RECORD_WIDTH
        or output_address == 0
        or field_capacity <= 0
        or field_address == 0
        or metadata_address == 0
    ):
        return SEMANTIC_INVALID

    var view = ProdexRichStringView(text_address, UInt(text_length))
    if not rich_view_valid(view, MAX_INPUT_BYTES):
        return SEMANTIC_INVALID
    var lines = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(line_spans_address)
    )
    var previous_end: Int64 = 0
    for index in range(line_count):
        var start = semantic_line_value(lines, index, 0)
        var end = semantic_line_value(lines, index, 1)
        if start < previous_end or end < start or end > text_length:
            return SEMANTIC_INVALID
        previous_end = end

    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var output_read = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var fields = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(field_address)
    )
    var fields_read = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(field_address)
    )
    var metadata = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(metadata_address)
    )
    metadata[unsafe_offset=0] = 0
    metadata[unsafe_offset=1] = 1
    metadata[unsafe_offset=2] = 0
    metadata[unsafe_offset=3] = 0
    var command_kind: UInt64 = 0
    var saw_diff = False
    var saw_cargo_test = False
    var saw_cargo_build = False
    var saw_npm_test = False
    for index in range(line_count):
        var start = semantic_line_value(lines, index, 0)
        var end = semantic_line_value(lines, index, 1)
        var kind = marker_command_kind(UInt(text_address + UInt(start)), end - start)
        if kind == 1:
            command_kind = 1
            break
        if kind == 2:
            saw_diff = True
        elif kind == 3:
            saw_cargo_test = True
        elif kind == 4:
            saw_cargo_build = True
        elif kind == 5:
            saw_npm_test = True
    if command_kind == 0:
        if saw_cargo_test:
            command_kind = 3
        elif saw_npm_test:
            command_kind = 5
        elif saw_cargo_build:
            command_kind = 4
        elif saw_diff:
            command_kind = 2
    metadata[unsafe_offset=2] = command_kind

    var field_written: Int64 = 0
    var count: Int64 = 0
    var complete: Int64 = 1
    var current_path_start: UInt64 = SEMANTIC_NONE
    var current_path_length: UInt64 = 0
    for index in range(line_count):
        var line_start = semantic_line_value(lines, index, 0)
        var line_end = semantic_line_value(lines, index, 1)
        var line_address = UInt(text_address + UInt(line_start))
        var line_length = line_end - line_start

        var path_meta = Array[UInt64, 4](fill=SEMANTIC_NONE)
        if semantic_diff_path(line_address, line_length, Pointer(to=path_meta[0])):
            var copied_path = semantic_copy_input(
                line_address,
                Int64(path_meta[0]),
                Int64(path_meta[1]),
                fields,
                field_capacity,
                Pointer(to=field_written),
            )
            if copied_path[0]:
                current_path_start = copied_path[1]
                current_path_length = copied_path[2]
            else:
                current_path_start = SEMANTIC_NONE
                current_path_length = 0
                complete = 0

        var old_span = Array[UInt64, 4](fill=SEMANTIC_NONE)
        var new_span = Array[UInt64, 4](fill=SEMANTIC_NONE)
        if semantic_diff_hunk(
            line_address, line_length, Pointer(to=old_span[0]), Pointer(to=new_span[0])
        ):
            var hunk_end = semantic_diff_hunk_end(
                text_address, lines, line_count, index
            )
            count = semantic_emit_range(
                output,
                output_read,
                fields_read,
                lines,
                max_excerpt_bytes,
                max_ranges,
                count,
                Pointer(to=complete),
                Pointer(to=field_written),
                2,
                index + 1,
                hunk_end,
                current_path_start,
                current_path_length,
                SEMANTIC_NONE,
                SEMANTIC_NONE,
                old_span[0],
                old_span[1],
                new_span[0],
                new_span[1],
                SEMANTIC_NONE,
                0,
                SEMANTIC_NONE,
                0,
            )

        var cursor: Int64 = 0
        while cursor < line_length:
            var token = semantic_next_token(line_address, cursor, line_length)
            if not token[0]:
                break
            var location_meta = Array[UInt64, 4](fill=SEMANTIC_NONE)
            if marker_file_location(
                line_address + UInt(token[1]),
                token[2] - token[1],
                Pointer(to=location_meta[0]),
            ):
                var copied_path = semantic_copy_input(
                    line_address + UInt(token[1]),
                    Int64(location_meta[0]),
                    Int64(location_meta[1]),
                    fields,
                    field_capacity,
                    Pointer(to=field_written),
                )
                if not copied_path[0]:
                    complete = 0
                else:
                    count = semantic_emit_range(
                        output,
                        output_read,
                        fields_read,
                        lines,
                        max_excerpt_bytes,
                        max_ranges,
                        count,
                        Pointer(to=complete),
                        Pointer(to=field_written),
                        1,
                        index + 1,
                        index + 1,
                        copied_path[1],
                        copied_path[2],
                        location_meta[2],
                        location_meta[3],
                        SEMANTIC_NONE,
                        0,
                        SEMANTIC_NONE,
                        0,
                        SEMANTIC_NONE,
                        0,
                        SEMANTIC_NONE,
                        0,
                    )
                break
            cursor = token[2]

        if marker_test_failure(line_address, line_length):
            var symbol_start: UInt64 = SEMANTIC_NONE
            var symbol_length: UInt64 = 0
            var symbol_meta = Array[UInt64, 4](fill=SEMANTIC_NONE)
            if marker_test_symbol(
                line_address, line_length, Pointer(to=symbol_meta[0])
            ):
                var copied_symbol = semantic_copy_input(
                    line_address,
                    Int64(symbol_meta[0]),
                    Int64(symbol_meta[1]),
                    fields,
                    field_capacity,
                    Pointer(to=field_written),
                )
                if copied_symbol[0]:
                    symbol_start = copied_symbol[1]
                    symbol_length = copied_symbol[2]
                else:
                    complete = 0
            var failure_start = index if index > 0 else 1
            var failure_end = min(index + 2, line_count)
            count = semantic_emit_range(
                output,
                output_read,
                fields_read,
                lines,
                max_excerpt_bytes,
                max_ranges,
                count,
                Pointer(to=complete),
                Pointer(to=field_written),
                3,
                failure_start,
                failure_end,
                SEMANTIC_NONE,
                0,
                SEMANTIC_NONE,
                SEMANTIC_NONE,
                SEMANTIC_NONE,
                0,
                SEMANTIC_NONE,
                0,
                SEMANTIC_NONE,
                0,
                symbol_start,
                symbol_length,
            )

        var error_buffer = Array[UInt8, 512](fill=0)
        var error_code_written: Int64 = 0
        if marker_error_code(
            line_address,
            line_length,
            Pointer(to=error_buffer[0]),
            SEMANTIC_MAX_FIELD_BYTES,
            Pointer(to=error_code_written),
        ) and error_code_written > 0:
            var copied_code = semantic_copy_buffer(
                Pointer(to=error_buffer[0]),
                error_code_written,
                fields,
                field_capacity,
                Pointer(to=field_written),
            )
            if not copied_code[0]:
                complete = 0
            else:
                count = semantic_emit_range(
                output,
                output_read,
                fields_read,
                lines,
                max_excerpt_bytes,
                max_ranges,
                count,
                Pointer(to=complete),
                Pointer(to=field_written),
                4,
                index + 1,
                index + 1,
                SEMANTIC_NONE,
                0,
                SEMANTIC_NONE,
                SEMANTIC_NONE,
                SEMANTIC_NONE,
                0,
                SEMANTIC_NONE,
                0,
                copied_code[1],
                copied_code[2],
                SEMANTIC_NONE,
                0,
                )
    metadata[unsafe_offset=0] = UInt64(count)
    metadata[unsafe_offset=1] = UInt64(complete)
    metadata[unsafe_offset=3] = UInt64(field_written)
    return SEMANTIC_OK
