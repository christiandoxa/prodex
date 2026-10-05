from std.memory import Pointer

from rich_text import (
    rich_codepoint,
    rich_codepoint_width,
    rich_unicode_space,
    rich_view_valid,
)
from rich_types import ProdexRichStringView

comptime SYMBOL_ABI_VERSION: Int64 = 1
comptime SYMBOL_OK: Int64 = 0
comptime SYMBOL_INVALID: Int64 = 1
comptime SYMBOL_CAPACITY: Int64 = 3
comptime SYMBOL_ABI_MISMATCH: Int64 = 4
comptime SYMBOL_MAX_INPUT_BYTES: Int64 = 64 * 1024 * 1024
comptime SYMBOL_MAX_RANGES: Int64 = 256
comptime SYMBOL_MAX_EXCERPT_BYTES: Int64 = 16 * 1024
comptime SYMBOL_MAX_FIELD_BYTES: Int64 = 512
comptime SYMBOL_PREFIX_LINES: Int64 = 6
comptime SYMBOL_RANGE_LINES: Int64 = 24
comptime SYMBOL_SIGNATURE_LINES: Int64 = 6
comptime SYMBOL_RECORD_WIDTH: Int64 = 7
comptime SYMBOL_NONE: UInt64 = 0xFFFFFFFFFFFFFFFF

comptime SYMBOL_FUNCTION: UInt64 = 0
comptime SYMBOL_TEST: UInt64 = 1
comptime SYMBOL_OTHER: UInt64 = 2
comptime SYMBOL_NAME_SOURCE: UInt64 = 0
comptime SYMBOL_NAME_IMPL: UInt64 = 1
comptime SYMBOL_NAME_TEST: UInt64 = 2
comptime SYMBOL_NAME_IT: UInt64 = 3
comptime SYMBOL_STYLE_BRACE: UInt64 = 0
comptime SYMBOL_STYLE_PYTHON: UInt64 = 1


def symbol_byte(address: UInt, index: Int64) -> UInt8:
    var input = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    return input[unsafe_offset=index]


def symbol_line_value(
    lines: Pointer[mut=False, UInt64, _], index: Int64, field: Int64
) -> Int64:
    return Int64(lines[unsafe_offset=index * 2 + field])


def symbol_literal_at[
    literal: StaticString
](address: UInt, start: Int64, end: Int64) -> Bool:
    var n = Int64(literal.byte_length())
    if start < 0 or start + n > end:
        return False
    var input = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    var wanted = literal.unsafe_ptr()
    for index in range(n):
        if input[unsafe_offset=start + index] != wanted[unsafe_offset=index]:
            return False
    return True


def symbol_range_equals[
    literal: StaticString
](address: UInt, start: Int64, end: Int64) -> Bool:
    return end - start == Int64(literal.byte_length()) and symbol_literal_at[
        literal
    ](address, start, end)


def symbol_find[
    literal: StaticString
](address: UInt, start: Int64, end: Int64) -> Int64:
    var n = Int64(literal.byte_length())
    var cursor = start
    while cursor + n <= end:
        if symbol_literal_at[literal](address, cursor, end):
            return cursor
        cursor += 1
    return -1


def symbol_contains[
    literal: StaticString
](address: UInt, start: Int64, end: Int64) -> Bool:
    return symbol_find[literal](address, start, end) >= 0


def symbol_ends_with[
    literal: StaticString
](address: UInt, start: Int64, end: Int64) -> Bool:
    var n = Int64(literal.byte_length())
    return n <= end - start and symbol_range_equals[literal](
        address, end - n, end
    )


def symbol_trim_start(address: UInt, start: Int64, end: Int64) -> Int64:
    var cursor = start
    var input = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    while cursor < end:
        var width = rich_codepoint_width(input[unsafe_offset=cursor])
        if not rich_unicode_space(rich_codepoint(input, cursor, width)):
            break
        cursor += width
    return cursor


def symbol_trim_end(address: UInt, start: Int64, end: Int64) -> Int64:
    var cursor = start
    var kept_end = start
    var input = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    while cursor < end:
        var width = rich_codepoint_width(input[unsafe_offset=cursor])
        if not rich_unicode_space(rich_codepoint(input, cursor, width)):
            kept_end = cursor + width
        cursor += width
    return kept_end


def symbol_whitespace_count(address: UInt, start: Int64, end: Int64) -> Int64:
    var cursor = start
    var count: Int64 = 0
    var input = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    while cursor < end:
        var width = rich_codepoint_width(input[unsafe_offset=cursor])
        if not rich_unicode_space(rich_codepoint(input, cursor, width)):
            break
        cursor += width
        count += 1
    return count


def symbol_identifier(
    address: UInt, start: Int64, end: Int64
) -> Tuple[Bool, Int64, Int64]:
    var cursor = symbol_trim_start(address, start, end)
    var name_start = cursor
    while cursor < end:
        var byte = symbol_byte(address, cursor)
        if not (
            (byte >= 48 and byte <= 57)
            or (byte >= 65 and byte <= 90)
            or (byte >= 97 and byte <= 122)
            or byte == 95
            or byte == 36
            or byte == 35
        ):
            break
        cursor += 1
    if cursor == name_start:
        return (False, 0, 0)
    if cursor - name_start >= 2 and symbol_literal_at["r#"](
        address, name_start, cursor
    ):
        name_start += 2
    if cursor == name_start or cursor - name_start > SYMBOL_MAX_FIELD_BYTES:
        return (False, 0, 0)
    return (True, name_start, cursor)


def symbol_after_keyword[
    literal: StaticString
](address: UInt, start: Int64, end: Int64) -> Tuple[Bool, Int64, Int64]:
    var found = symbol_find[literal](address, start, end)
    if found < 0:
        return (False, 0, 0)
    var cursor = found + Int64(literal.byte_length())
    while cursor < end and symbol_byte(address, cursor) == 42:
        cursor += 1
    cursor = symbol_trim_start(address, cursor, end)
    return symbol_identifier(address, cursor, end)


def symbol_python_name(
    address: UInt, start: Int64, end: Int64
) -> Tuple[Bool, Int64, Int64]:
    var cursor = start
    if symbol_literal_at["async def "](address, cursor, end):
        cursor += 10
    elif symbol_literal_at["def "](address, cursor, end):
        cursor += 4
    else:
        return (False, 0, 0)
    return symbol_identifier(address, cursor, end)


def symbol_js_test_name(
    address: UInt, start: Int64, end: Int64
) -> Tuple[Bool, Int64, Int64, UInt64]:
    var cursor = start
    var fallback = SYMBOL_NAME_TEST
    if symbol_literal_at["test("](address, cursor, end):
        cursor += 5
    elif symbol_literal_at["it("](address, cursor, end):
        cursor += 3
        fallback = SYMBOL_NAME_IT
    else:
        return (False, 0, 0, SYMBOL_NAME_TEST)
    cursor = symbol_trim_start(address, cursor, end)
    if symbol_literal_at['"'](address, cursor, end):
        cursor += 1
        var close_quote = symbol_find['"'](address, cursor, end)
        if close_quote >= 0:
            if (
                close_quote == cursor
                or close_quote - cursor > SYMBOL_MAX_FIELD_BYTES
            ):
                return (False, 0, 0, SYMBOL_NAME_TEST)
            return (True, cursor, close_quote, SYMBOL_NAME_SOURCE)
    elif symbol_literal_at["'"](address, cursor, end):
        cursor += 1
        var close_quote = symbol_find["'"](address, cursor, end)
        if close_quote >= 0:
            if (
                close_quote == cursor
                or close_quote - cursor > SYMBOL_MAX_FIELD_BYTES
            ):
                return (False, 0, 0, SYMBOL_NAME_TEST)
            return (True, cursor, close_quote, SYMBOL_NAME_SOURCE)
    return (True, 0, 0, fallback)


def symbol_last_word(
    address: UInt, start: Int64, end: Int64
) -> Tuple[Bool, Int64, Int64]:
    var cursor = start
    var current_start: Int64 = -1
    var current_end: Int64 = -1
    var word_start: Int64 = -1
    var word_end: Int64 = -1
    var input = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    while cursor < end:
        var width = rich_codepoint_width(input[unsafe_offset=cursor])
        if rich_unicode_space(rich_codepoint(input, cursor, width)):
            if current_start >= 0:
                word_start = current_start
                word_end = cursor
                current_start = -1
        else:
            if current_start < 0:
                current_start = cursor
            current_end = cursor + width
        cursor += width
    if current_start >= 0:
        word_start = current_start
        word_end = current_end
    if word_end <= 0:
        return (False, 0, 0)
    var left = word_end
    while left > 0 and (
        symbol_byte(address, left - 1) == 58
        or symbol_byte(address, left - 1) == 63
    ):
        left -= 1
    while word_start < left and (
        symbol_byte(address, word_start) == 58
        or symbol_byte(address, word_start) == 63
    ):
        word_start += 1
    return symbol_identifier(address, word_start, left)


def symbol_js_function_name(
    address: UInt, start: Int64, end: Int64
) -> Tuple[Bool, Int64, Int64]:
    var found = symbol_after_keyword["function "](address, start, end)
    if found[0]:
        return found
    var equals = symbol_find["="](address, start, end)
    if equals < 0:
        return (False, 0, 0)
    var right_start = equals + 1
    if not symbol_contains["=>"](
        address, right_start, end
    ) and not symbol_literal_at["function"](
        address, symbol_trim_start(address, right_start, end), end
    ):
        return (False, 0, 0)
    return symbol_last_word(address, start, equals)


def symbol_test_attribute(address: UInt, start: Int64, end: Int64) -> Bool:
    return (
        symbol_range_equals["#[test]"](address, start, end)
        or symbol_literal_at["#[tokio::test"](address, start, end)
        or symbol_literal_at["#[async_std::test"](address, start, end)
        or symbol_literal_at["#[rstest"](address, start, end)
    )


def symbol_has_rust_test_attribute(
    address: UInt,
    lines: Pointer[mut=False, UInt64, _],
    line_index: Int64,
) -> Bool:
    var lower_bound = line_index - SYMBOL_PREFIX_LINES
    if lower_bound < 0:
        lower_bound = 0
    var previous = line_index - 1
    while previous >= lower_bound:
        var start = symbol_line_value(lines, previous, 0)
        var end = symbol_line_value(lines, previous, 1)
        var trimmed_start = symbol_trim_start(address, start, end)
        var trimmed_end = symbol_trim_end(address, trimmed_start, end)
        if trimmed_start == trimmed_end:
            previous -= 1
            continue
        if symbol_test_attribute(address, trimmed_start, trimmed_end):
            return True
        if not symbol_literal_at["#["](address, trimmed_start, trimmed_end):
            break
        previous -= 1
    return False


def symbol_parse_declaration(
    address: UInt,
    start: Int64,
    end: Int64,
    rust_test: Bool,
) -> Tuple[Bool, Int64, Int64, UInt64, UInt64, UInt64]:
    var symbol = symbol_after_keyword["fn "](address, start, end)
    if symbol[0]:
        var label = SYMBOL_FUNCTION
        if rust_test:
            label = SYMBOL_TEST
        return (
            True,
            symbol[1],
            symbol[2],
            label,
            SYMBOL_NAME_SOURCE,
            SYMBOL_STYLE_BRACE,
        )

    symbol = symbol_python_name(address, start, end)
    if symbol[0]:
        var label = SYMBOL_FUNCTION
        if symbol_literal_at["test_"](address, symbol[1], symbol[2]):
            label = SYMBOL_TEST
        return (
            True,
            symbol[1],
            symbol[2],
            label,
            SYMBOL_NAME_SOURCE,
            SYMBOL_STYLE_PYTHON,
        )

    var js_test = symbol_js_test_name(address, start, end)
    if js_test[0]:
        return (
            True,
            js_test[1],
            js_test[2],
            SYMBOL_TEST,
            js_test[3],
            SYMBOL_STYLE_BRACE,
        )

    symbol = symbol_js_function_name(address, start, end)
    if symbol[0]:
        return (
            True,
            symbol[1],
            symbol[2],
            SYMBOL_FUNCTION,
            SYMBOL_NAME_SOURCE,
            SYMBOL_STYLE_BRACE,
        )

    symbol = symbol_after_keyword["struct "](address, start, end)
    if symbol[0]:
        return (
            True,
            symbol[1],
            symbol[2],
            SYMBOL_OTHER,
            SYMBOL_NAME_SOURCE,
            SYMBOL_STYLE_BRACE,
        )
    symbol = symbol_after_keyword["enum "](address, start, end)
    if symbol[0]:
        return (
            True,
            symbol[1],
            symbol[2],
            SYMBOL_OTHER,
            SYMBOL_NAME_SOURCE,
            SYMBOL_STYLE_BRACE,
        )
    symbol = symbol_after_keyword["trait "](address, start, end)
    if symbol[0]:
        return (
            True,
            symbol[1],
            symbol[2],
            SYMBOL_OTHER,
            SYMBOL_NAME_SOURCE,
            SYMBOL_STYLE_BRACE,
        )
    symbol = symbol_after_keyword["impl "](address, start, end)
    if symbol[0]:
        return (
            True,
            symbol[1],
            symbol[2],
            SYMBOL_OTHER,
            SYMBOL_NAME_IMPL,
            SYMBOL_STYLE_BRACE,
        )
    symbol = symbol_after_keyword["mod "](address, start, end)
    if symbol[0]:
        return (
            True,
            symbol[1],
            symbol[2],
            SYMBOL_OTHER,
            SYMBOL_NAME_SOURCE,
            SYMBOL_STYLE_BRACE,
        )
    symbol = symbol_after_keyword["class "](address, start, end)
    if symbol[0]:
        return (
            True,
            symbol[1],
            symbol[2],
            SYMBOL_OTHER,
            SYMBOL_NAME_SOURCE,
            SYMBOL_STYLE_BRACE,
        )
    return (False, 0, 0, SYMBOL_OTHER, SYMBOL_NAME_SOURCE, SYMBOL_STYLE_BRACE)


def symbol_parse_line(
    address: UInt,
    lines: Pointer[mut=False, UInt64, _],
    line_index: Int64,
) -> Tuple[Bool, Int64, Int64, UInt64, UInt64, UInt64]:
    var start = symbol_line_value(lines, line_index, 0)
    var end = symbol_line_value(lines, line_index, 1)
    start = symbol_trim_start(address, start, end)
    if (
        start >= end
        or symbol_literal_at["//"](address, start, end)
        or symbol_literal_at["/*"](address, start, end)
        or symbol_byte(address, start) == 42
        or symbol_literal_at["#["](address, start, end)
        or symbol_byte(address, start) == 64
    ):
        return (
            False,
            0,
            0,
            SYMBOL_OTHER,
            SYMBOL_NAME_SOURCE,
            SYMBOL_STYLE_BRACE,
        )
    var rust_test = symbol_has_rust_test_attribute(address, lines, line_index)
    return symbol_parse_declaration(address, start, end, rust_test)


def symbol_prefix_start(
    address: UInt,
    lines: Pointer[mut=False, UInt64, _],
    line_index: Int64,
) -> Int64:
    var start = line_index
    var lower_bound = line_index - SYMBOL_PREFIX_LINES
    if lower_bound < 0:
        lower_bound = 0
    while start > lower_bound:
        var previous = start - 1
        var line_start = symbol_line_value(lines, previous, 0)
        var line_end = symbol_line_value(lines, previous, 1)
        line_start = symbol_trim_start(address, line_start, line_end)
        if (
            line_start == line_end
            or symbol_literal_at["#["](address, line_start, line_end)
            or symbol_byte(address, line_start) == 64
            or symbol_literal_at["//"](address, line_start, line_end)
        ):
            start -= 1
        else:
            break
    return start


def symbol_brace_end(
    address: UInt,
    lines: Pointer[mut=False, UInt64, _],
    line_count: Int64,
    declaration_index: Int64,
) -> Int64:
    var max_end = declaration_index + SYMBOL_RANGE_LINES - 1
    if max_end >= line_count:
        max_end = line_count - 1
    var balance: Int64 = 0
    var saw_open = False
    for index in range(declaration_index, max_end + 1):
        var start = symbol_line_value(lines, index, 0)
        var end = symbol_line_value(lines, index, 1)
        for cursor in range(start, end):
            var byte = symbol_byte(address, cursor)
            if byte == 123:
                saw_open = True
                balance += 1
            elif byte == 125 and saw_open:
                balance -= 1
        if saw_open and balance <= 0:
            return index
        if not saw_open and index > declaration_index:
            var distance = index - declaration_index
            var trimmed_end = symbol_trim_end(address, start, end)
            if distance >= SYMBOL_SIGNATURE_LINES or symbol_ends_with[";"](
                address, start, trimmed_end
            ):
                return index
    return max_end


def symbol_python_end(
    address: UInt,
    lines: Pointer[mut=False, UInt64, _],
    line_count: Int64,
    declaration_index: Int64,
) -> Int64:
    var decl_start = symbol_line_value(lines, declaration_index, 0)
    var decl_end = symbol_line_value(lines, declaration_index, 1)
    var base_indent = symbol_whitespace_count(address, decl_start, decl_end)
    var max_end = declaration_index + SYMBOL_RANGE_LINES - 1
    if max_end >= line_count:
        max_end = line_count - 1
    var end = declaration_index
    for index in range(declaration_index + 1, max_end + 1):
        var start = symbol_line_value(lines, index, 0)
        var line_end = symbol_line_value(lines, index, 1)
        var trimmed_start = symbol_trim_start(address, start, line_end)
        var trimmed_end = symbol_trim_end(address, trimmed_start, line_end)
        if (
            trimmed_start < trimmed_end
            and symbol_byte(address, trimmed_start) != 35
            and symbol_whitespace_count(address, start, line_end) <= base_indent
        ):
            break
        end = index
    return end


def symbol_excerpt_bytes(
    lines: Pointer[mut=False, UInt64, _],
    start_line: Int64,
    end_line: Int64,
) -> Int64:
    var total: Int64 = end_line - start_line
    for index in range(start_line - 1, end_line):
        total += symbol_line_value(lines, index, 1) - symbol_line_value(
            lines, index, 0
        )
    return total


def symbol_name_length(start: Int64, end: Int64, name_kind: UInt64) -> Int64:
    if name_kind == SYMBOL_NAME_TEST:
        return 4
    if name_kind == SYMBOL_NAME_IT:
        return 2
    var result = end - start
    if name_kind == SYMBOL_NAME_IMPL:
        result += 5
    return result


def symbol_name_byte(
    address: UInt, start: Int64, end: Int64, name_kind: UInt64, offset: Int64
) -> UInt8:
    if name_kind == SYMBOL_NAME_TEST:
        if offset == 0:
            return 116
        if offset == 1:
            return 101
        if offset == 2:
            return 115
        return 116
    if name_kind == SYMBOL_NAME_IT:
        if offset == 0:
            return 105
        return 116
    if name_kind == SYMBOL_NAME_IMPL:
        if offset == 0:
            return 105
        if offset == 1:
            return 109
        if offset == 2:
            return 112
        if offset == 3:
            return 108
        if offset == 4:
            return 32
        return symbol_byte(address, start + offset - 5)
    return symbol_byte(address, start + offset)


def symbol_name_equal(
    address: UInt,
    left_start: Int64,
    left_end: Int64,
    left_kind: UInt64,
    right_start: Int64,
    right_end: Int64,
    right_kind: UInt64,
) -> Bool:
    var left_length = symbol_name_length(left_start, left_end, left_kind)
    var right_length = symbol_name_length(right_start, right_end, right_kind)
    if left_length != right_length:
        return False
    for index in range(left_length):
        if symbol_name_byte(
            address, left_start, left_end, left_kind, index
        ) != symbol_name_byte(
            address, right_start, right_end, right_kind, index
        ):
            return False
    return True


def symbol_duplicate(
    address: UInt,
    output: Pointer[mut=False, UInt64, _],
    count: Int64,
    start_line: Int64,
    end_line: Int64,
    name_start: Int64,
    name_end: Int64,
    label: UInt64,
    name_kind: UInt64,
) -> Bool:
    for row in range(count):
        var base = row * SYMBOL_RECORD_WIDTH
        if (
            Int64(output[unsafe_offset=base]) == start_line
            and Int64(output[unsafe_offset=base + 1]) == end_line
            and output[unsafe_offset=base + 5] == label
            and symbol_name_equal(
                address,
                Int64(output[unsafe_offset=base + 3]),
                Int64(output[unsafe_offset=base + 4]),
                output[unsafe_offset=base + 6],
                name_start,
                name_end,
                name_kind,
            )
        ):
            return True
    return False


@export("prodex_smart_context_symbol_index_v1")
def prodex_smart_context_symbol_index_v1(
    abi_version: Int64,
    text_address: UInt,
    text_length: Int64,
    line_spans_address: UInt,
    line_count: Int64,
    max_ranges: Int64,
    max_excerpt_bytes: Int64,
    output_address: UInt,
    output_capacity: Int64,
    metadata_address: UInt,
) abi("C") -> Int64:
    if abi_version != SYMBOL_ABI_VERSION:
        return SYMBOL_ABI_MISMATCH
    if (
        text_length < 0
        or text_length > SYMBOL_MAX_INPUT_BYTES
        or (text_length > 0 and text_address == 0)
        or line_count < 0
        or line_count > text_length + 1
        or (line_count > 0 and line_spans_address == 0)
        or max_ranges < 0
        or max_ranges > SYMBOL_MAX_RANGES
        or max_excerpt_bytes < 0
        or max_excerpt_bytes > SYMBOL_MAX_EXCERPT_BYTES
        or output_capacity < max_ranges * SYMBOL_RECORD_WIDTH
        or (output_capacity > 0 and output_address == 0)
        or metadata_address == 0
    ):
        return SYMBOL_INVALID

    var view = ProdexRichStringView(text_address, UInt(text_length))
    if not rich_view_valid(view, SYMBOL_MAX_INPUT_BYTES):
        return SYMBOL_INVALID
    var lines = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(line_spans_address)
    )
    var previous_end: Int64 = 0
    for index in range(line_count):
        var start = symbol_line_value(lines, index, 0)
        var end = symbol_line_value(lines, index, 1)
        if start < previous_end or end < start or end > text_length:
            return SYMBOL_INVALID
        previous_end = end

    var metadata = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(metadata_address)
    )
    metadata[unsafe_offset=0] = 0
    metadata[unsafe_offset=1] = 1
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var count: Int64 = 0
    var complete = True
    for index in range(line_count):
        var parsed = symbol_parse_line(text_address, lines, index)
        if not parsed[0]:
            continue
        var start_index = symbol_prefix_start(text_address, lines, index)
        var declaration_index = index
        var end_index = declaration_index
        if parsed[5] == SYMBOL_STYLE_PYTHON:
            end_index = symbol_python_end(
                text_address, lines, line_count, declaration_index
            )
        else:
            end_index = symbol_brace_end(
                text_address, lines, line_count, declaration_index
            )
        var start_line = start_index + 1
        var end_line = end_index + 1
        if count >= max_ranges:
            complete = False
            continue
        if (
            symbol_excerpt_bytes(lines, start_line, end_line)
            > max_excerpt_bytes
        ):
            complete = False
            continue
        var output_read = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
            unsafe_from_address=Int(output_address)
        )
        if symbol_duplicate(
            text_address,
            output_read,
            count,
            start_line,
            end_line,
            parsed[1],
            parsed[2],
            parsed[3],
            parsed[4],
        ):
            continue
        var base = count * SYMBOL_RECORD_WIDTH
        output[unsafe_offset=base] = UInt64(start_line)
        output[unsafe_offset=base + 1] = UInt64(end_line)
        output[unsafe_offset=base + 2] = UInt64(index + 1)
        output[unsafe_offset=base + 3] = (
            UInt64(parsed[1]) if parsed[4] <= SYMBOL_NAME_IMPL else SYMBOL_NONE
        )
        output[unsafe_offset=base + 4] = (
            UInt64(parsed[2]) if parsed[4] <= SYMBOL_NAME_IMPL else SYMBOL_NONE
        )
        output[unsafe_offset=base + 5] = parsed[3]
        output[unsafe_offset=base + 6] = parsed[4]
        count += 1
    metadata[unsafe_offset=0] = UInt64(count)
    metadata[unsafe_offset=1] = UInt64(complete)
    return SYMBOL_OK
