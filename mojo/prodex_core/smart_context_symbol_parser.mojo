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
comptime SYMBOL_MAX_NAME_BYTES: Int64 = 517
comptime SYMBOL_ATTRIBUTE_LOOKBACK_LINES: Int64 = 6
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


def symbol_line_value(
    lines: Pointer[mut=False, UInt64, _], index: Int64, field: Int64
) -> UInt64:
    return lines[unsafe_offset=index * 2 + field]


def symbol_line_address(
    lines: Pointer[mut=False, UInt64, _], index: Int64
) -> UInt:
    return UInt(symbol_line_value(lines, index, 0))


def symbol_line_length(
    lines: Pointer[mut=False, UInt64, _], index: Int64
) -> Int64:
    return Int64(symbol_line_value(lines, index, 1))


def symbol_byte(address: UInt, index: Int64) -> UInt8:
    var input = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    return input[unsafe_offset=index]


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
    if cursor == name_start or cursor - name_start > 512:
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
            if close_quote == cursor or close_quote - cursor > 512:
                return (False, 0, 0, SYMBOL_NAME_TEST)
            return (True, cursor, close_quote, SYMBOL_NAME_SOURCE)
    elif symbol_literal_at["'"](address, cursor, end):
        cursor += 1
        var close_quote = symbol_find["'"](address, cursor, end)
        if close_quote >= 0:
            if close_quote == cursor or close_quote - cursor > 512:
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
    lines: Pointer[mut=False, UInt64, _], line_index: Int64
) -> Bool:
    var lower_bound = line_index - SYMBOL_ATTRIBUTE_LOOKBACK_LINES
    if lower_bound < 0:
        lower_bound = 0
    var previous = line_index - 1
    while previous >= lower_bound:
        var address = symbol_line_address(lines, previous)
        var length = symbol_line_length(lines, previous)
        var trimmed_start = symbol_trim_start(address, 0, length)
        var trimmed_end = symbol_trim_end(address, trimmed_start, length)
        if trimmed_start == trimmed_end:
            previous -= 1
            continue
        if symbol_test_attribute(address, trimmed_start, trimmed_end):
            return True
        if not symbol_literal_at["#["](address, trimmed_start, trimmed_end):
            break
        previous -= 1
    return False


def symbol_decision(
    symbol: Tuple[Bool, Int64, Int64],
    label: UInt64,
    name_kind: UInt64,
    style: UInt64,
) -> Tuple[Bool, Int64, Int64, UInt64, UInt64, UInt64]:
    return (True, symbol[1], symbol[2], label, name_kind, style)


def symbol_missing() -> Tuple[Bool, Int64, Int64, UInt64, UInt64, UInt64]:
    return (False, 0, 0, SYMBOL_OTHER, SYMBOL_NAME_SOURCE, SYMBOL_STYLE_BRACE)


def symbol_parse_declaration(
    address: UInt, start: Int64, end: Int64, rust_test: Bool
) -> Tuple[Bool, Int64, Int64, UInt64, UInt64, UInt64]:
    var symbol = symbol_after_keyword["fn "](address, start, end)
    if symbol[0]:
        var label = SYMBOL_FUNCTION
        if rust_test:
            label = SYMBOL_TEST
        return symbol_decision(
            symbol, label, SYMBOL_NAME_SOURCE, SYMBOL_STYLE_BRACE
        )
    symbol = symbol_python_name(address, start, end)
    if symbol[0]:
        var label = SYMBOL_FUNCTION
        if symbol_literal_at["test_"](address, symbol[1], symbol[2]):
            label = SYMBOL_TEST
        return symbol_decision(
            symbol, label, SYMBOL_NAME_SOURCE, SYMBOL_STYLE_PYTHON
        )
    var js_test = symbol_js_test_name(address, start, end)
    if js_test[0]:
        return symbol_decision(
            (True, js_test[1], js_test[2]),
            SYMBOL_TEST,
            js_test[3],
            SYMBOL_STYLE_BRACE,
        )
    symbol = symbol_js_function_name(address, start, end)
    if symbol[0]:
        return symbol_decision(
            symbol, SYMBOL_FUNCTION, SYMBOL_NAME_SOURCE, SYMBOL_STYLE_BRACE
        )
    symbol = symbol_after_keyword["struct "](address, start, end)
    if symbol[0]:
        return symbol_decision(
            symbol, SYMBOL_OTHER, SYMBOL_NAME_SOURCE, SYMBOL_STYLE_BRACE
        )
    symbol = symbol_after_keyword["enum "](address, start, end)
    if symbol[0]:
        return symbol_decision(
            symbol, SYMBOL_OTHER, SYMBOL_NAME_SOURCE, SYMBOL_STYLE_BRACE
        )
    symbol = symbol_after_keyword["trait "](address, start, end)
    if symbol[0]:
        return symbol_decision(
            symbol, SYMBOL_OTHER, SYMBOL_NAME_SOURCE, SYMBOL_STYLE_BRACE
        )
    symbol = symbol_after_keyword["impl "](address, start, end)
    if symbol[0]:
        return symbol_decision(
            symbol, SYMBOL_OTHER, SYMBOL_NAME_IMPL, SYMBOL_STYLE_BRACE
        )
    symbol = symbol_after_keyword["mod "](address, start, end)
    if symbol[0]:
        return symbol_decision(
            symbol, SYMBOL_OTHER, SYMBOL_NAME_SOURCE, SYMBOL_STYLE_BRACE
        )
    symbol = symbol_after_keyword["class "](address, start, end)
    if symbol[0]:
        return symbol_decision(
            symbol, SYMBOL_OTHER, SYMBOL_NAME_SOURCE, SYMBOL_STYLE_BRACE
        )
    return symbol_missing()


def symbol_parse_line(
    lines: Pointer[mut=False, UInt64, _], line_index: Int64
) -> Tuple[Bool, Int64, Int64, UInt64, UInt64, UInt64]:
    var address = symbol_line_address(lines, line_index)
    var end = symbol_line_length(lines, line_index)
    var start = symbol_trim_start(address, 0, end)
    if (
        start >= end
        or symbol_literal_at["//"](address, start, end)
        or symbol_literal_at["/*"](address, start, end)
        or symbol_byte(address, start) == 42
        or symbol_literal_at["#["](address, start, end)
        or symbol_byte(address, start) == 64
    ):
        return symbol_missing()
    return symbol_parse_declaration(
        address, start, end, symbol_has_rust_test_attribute(lines, line_index)
    )


def symbol_name_length(
    name_start: Int64, name_end: Int64, name_kind: UInt64
) -> Int64:
    if name_kind == SYMBOL_NAME_SOURCE:
        return name_end - name_start
    if name_kind == SYMBOL_NAME_IMPL:
        return 5 + name_end - name_start
    if name_kind == SYMBOL_NAME_TEST:
        return 4
    return 2


def symbol_write_name(
    address: UInt,
    name_start: Int64,
    name_end: Int64,
    name_kind: UInt64,
    output: Pointer[mut=True, UInt8, _],
) -> Int64:
    var written: Int64 = 0
    if name_kind == SYMBOL_NAME_IMPL:
        output[unsafe_offset=0] = 105
        output[unsafe_offset=1] = 109
        output[unsafe_offset=2] = 112
        output[unsafe_offset=3] = 108
        output[unsafe_offset=4] = 32
        written = 5
    elif name_kind == SYMBOL_NAME_TEST:
        output[unsafe_offset=0] = 116
        output[unsafe_offset=1] = 101
        output[unsafe_offset=2] = 115
        output[unsafe_offset=3] = 116
        return 4
    elif name_kind == SYMBOL_NAME_IT:
        output[unsafe_offset=0] = 105
        output[unsafe_offset=1] = 116
        return 2
    for index in range(name_end - name_start):
        output[unsafe_offset=written + index] = symbol_byte(
            address, name_start + index
        )
    return written + name_end - name_start
