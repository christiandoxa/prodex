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
