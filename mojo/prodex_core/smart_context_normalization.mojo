from std.collections import Array

from std.memory import Pointer

from rich_text import (
    rich_codepoint,
    rich_trim_bounds,
    rich_utf8_continuation,
    rich_unicode_space,
    rich_view_ptr,
    rich_view_valid,
)
from rich_types import ProdexRichStringView


comptime PRODEX_RICH_ABI_VERSION: Int64 = 6
comptime SMART_CONTEXT_NORMALIZATION_MAX_BYTES: Int64 = 4 * 1024 * 1024
comptime SMART_CONTEXT_NORMALIZATION_MAX_OUTPUT_BYTES: Int64 = 5 * SMART_CONTEXT_NORMALIZATION_MAX_BYTES + 16
comptime SMART_CONTEXT_CAPSULE_MAX_COUNT: Int64 = 65_536
comptime SMART_CONTEXT_STATUS_OK: Int64 = 0
comptime SMART_CONTEXT_STATUS_INVALID: Int64 = 1
comptime SMART_CONTEXT_STATUS_UTF8: Int64 = 2
comptime SMART_CONTEXT_STATUS_CAPACITY: Int64 = 3
comptime SMART_CONTEXT_STATUS_ABI: Int64 = 4
comptime SMART_CONTEXT_MODE_COMMAND_OUTPUT: Int64 = 0
comptime SMART_CONTEXT_MODE_STATIC_CONTEXT: Int64 = 1
comptime SMART_CONTEXT_MODE_STATIC_NOISE: Int64 = 2
comptime SMART_CONTEXT_BUDGET_MODE_EXACT: Int64 = 0
comptime SMART_CONTEXT_BUDGET_MODE_LARGE: Int64 = 1
comptime SMART_CONTEXT_BUDGET_MODE_CONDENSED: Int64 = 2
comptime SMART_CONTEXT_BUDGET_MODE_MINIMAL: Int64 = 3
comptime SMART_CONTEXT_BUDGET_TIER_EXACT: Int64 = 0
comptime SMART_CONTEXT_BUDGET_TIER_LARGE: Int64 = 1
comptime SMART_CONTEXT_BUDGET_TIER_CONDENSED: Int64 = 2
comptime SMART_CONTEXT_BUDGET_TIER_MINIMAL: Int64 = 3
comptime SMART_CONTEXT_POLICY_REASON_EXACTNESS: UInt64 = 1 << 0
comptime SMART_CONTEXT_POLICY_REASON_STATIC_CHANGED: UInt64 = 1 << 1
comptime SMART_CONTEXT_POLICY_REASON_MISSING_REFS: UInt64 = 1 << 2
comptime SMART_CONTEXT_POLICY_REASON_UNKNOWN_WINDOW: UInt64 = 1 << 3
comptime SMART_CONTEXT_POLICY_REASON_UNSAFE_ACCOUNTING: UInt64 = 1 << 4
comptime SMART_CONTEXT_POLICY_REASON_PLENTY: UInt64 = 1 << 6
comptime SMART_CONTEXT_POLICY_REASON_ALL: UInt64 = (1 << 10) - 1


def smart_context_is_ascii_digit(value: UInt8) -> Bool:
    return value >= 48 and value <= 57


def smart_context_is_ascii_alpha(value: UInt8) -> Bool:
    return value >= 65 and value <= 90 or value >= 97 and value <= 122


def smart_context_is_ascii_alnum(value: UInt8) -> Bool:
    return smart_context_is_ascii_alpha(value) or smart_context_is_ascii_digit(value)


def smart_context_is_ascii_whitespace(value: UInt8) -> Bool:
    return value == 9 or value >= 10 and value <= 13 or value == 32


def smart_context_unicode_whitespace_len(
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Int64:
    if start < 0 or start >= length:
        return 0
    if (
        start + 1 < length
        and source[unsafe_offset=start] == 194
        and (
            source[unsafe_offset=start + 1] == 133
            or source[unsafe_offset=start + 1] == 160
        )
    ):
        return 2
    if start + 2 >= length:
        return 0
    var first = source[unsafe_offset=start]
    var second = source[unsafe_offset=start + 1]
    var third = source[unsafe_offset=start + 2]
    if first == 225 and second == 154 and third == 128:
        return 3
    if (
        first == 226
        and second == 128
        and (
            third >= 128
            and third <= 138
            or third == 168
            or third == 169
            or third == 175
        )
    ):
        return 3
    if first == 226 and second == 129 and third == 159:
        return 3
    if first == 227 and second == 128 and third == 128:
        return 3
    return 0


def smart_context_unicode_control_len(
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Int64:
    if (
        start + 1 < length
        and source[unsafe_offset=start] == 194
        and source[unsafe_offset=start + 1] >= 128
        and source[unsafe_offset=start + 1] <= 159
    ):
        return 2
    return 0


def smart_context_is_token_byte(value: UInt8) -> Bool:
    return smart_context_is_ascii_alnum(value) or value == 95 or value == 45 or value == 46 or value == 58 or value == 47


def smart_context_before_token(
    source: Pointer[mut=False, UInt8, _], index: Int64
) -> Bool:
    if index <= 0:
        return True
    return not smart_context_is_token_byte(source[unsafe_offset=index - 1])


def smart_context_after_token(
    source: Pointer[mut=False, UInt8, _], length: Int64, index: Int64
) -> Bool:
    if index >= length:
        return True
    return not smart_context_is_token_byte(source[unsafe_offset=index])


def smart_context_path_delimiter(value: UInt8) -> Bool:
    return (
        smart_context_is_ascii_whitespace(value)
        or value <= 31
        or value == 127
        or value == 34
        or value == 39
        or value == 96
        or value == 60
        or value == 62
        or value == 124
        or value == 40
        or value == 41
        or value == 91
        or value == 93
        or value == 123
        or value == 125
    )


def smart_context_path_token_end(
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Int64:
    var index = start + 1
    while (
        index < length
        and not smart_context_path_delimiter(source[unsafe_offset=index])
        and smart_context_unicode_whitespace_len(source, length, index) == 0
        and smart_context_unicode_control_len(source, length, index) == 0
    ):
        index += 1
    return index


def smart_context_literal_at[literal: StaticString](
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Bool:
    var literal_length = Int64(literal.byte_length())
    if start < 0 or start + literal_length > length:
        return False
    var right = literal.unsafe_ptr()
    for index in range(literal_length):
        if source[unsafe_offset=start + index] != right[unsafe_offset=index]:
            return False
    return True


def smart_context_ascii_case_literal_at[literal: StaticString](
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Bool:
    var literal_length = Int64(literal.byte_length())
    if start < 0 or start + literal_length > length:
        return False
    var right = literal.unsafe_ptr()
    for index in range(literal_length):
        var value = source[unsafe_offset=start + index]
        if value >= 65 and value <= 90:
            value += 32
        var wanted = right[unsafe_offset=index]
        if wanted >= 65 and wanted <= 90:
            wanted += 32
        if value != wanted:
            return False
    return True


def smart_context_ascii_case_range[literal: StaticString](
    source: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var literal_length = Int64(literal.byte_length())
    if end - start != literal_length:
        return False
    return smart_context_ascii_case_literal_at[literal](source, end, start)


def smart_context_emit_range(
    source: Pointer[mut=False, UInt8, _], start: Int64, end: Int64,
    output: Pointer[mut=True, UInt8, _], capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    var length = end - start
    if start < 0 or end < start or written[] < 0 or written[] > capacity or length > capacity - written[]:
        return False
    for index in range(length):
        output[unsafe_offset=written[] + index] = source[unsafe_offset=start + index]
    written[] += length
    return True


def smart_context_emit_literal[literal: StaticString](
    output: Pointer[mut=True, UInt8, _], capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    var length = Int64(literal.byte_length())
    if written[] < 0 or written[] > capacity or length > capacity - written[]:
        return False
    var literal_ptr = literal.unsafe_ptr()
    for index in range(length):
        output[unsafe_offset=written[] + index] = literal_ptr[unsafe_offset=index]
    written[] += length
    return True


def smart_context_parse_unsigned(
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Array[UInt64, 3]:
    var result = Array[UInt64, 3](fill=0)
    if start < 0 or start >= length:
        return result^
    var index = start
    var value: UInt64 = 0
    while index < length and smart_context_is_ascii_digit(source[unsafe_offset=index]):
        var digit = UInt64(source[unsafe_offset=index] - 48)
        if value > 1844674407370955161 or value == 1844674407370955161 and digit > 5:
            value = 18446744073709551615
        else:
            value = value * 10 + digit
        index += 1
    if index > start:
        result[0] = 1
        result[1] = UInt64(index)
        result[2] = value
    return result^


def smart_context_skip_ascii_spaces(
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Int64:
    var index = start
    while index < length and smart_context_is_ascii_whitespace(source[unsafe_offset=index]):
        index += 1
    return index


def smart_context_ascii_digits(
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64, count: Int64
) -> Bool:
    if start < 0 or count < 0 or start + count > length:
        return False
    for index in range(count):
        if not smart_context_is_ascii_digit(source[unsafe_offset=start + index]):
            return False
    return True


def smart_context_ansi_escape_len(
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Int64:
    if start >= length or source[unsafe_offset=start] != 27:
        return -1
    if start + 1 >= length:
        return 1
    var kind = source[unsafe_offset=start + 1]
    if kind == 91:
        var index = start + 2
        while index < length:
            var value = source[unsafe_offset=index]
            if value >= 64 and value <= 126:
                return index - start + 1
            index += 1
        return length - start
    if kind == 93:
        var index = start + 2
        while index < length:
            if source[unsafe_offset=index] == 7:
                return index - start + 1
            if source[unsafe_offset=index] == 27 and index + 1 < length and source[unsafe_offset=index + 1] == 92:
                return index - start + 2
            index += 1
        return length - start
    if kind >= 128:
        return 1
    if length - start >= 2:
        return 2
    return 1


def smart_context_temp_path_len(
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Int64:
    if smart_context_literal_at["/tmp/"](source, length, start) or smart_context_literal_at["/var/tmp/"](source, length, start) or smart_context_literal_at["/private/tmp/"](source, length, start) or smart_context_literal_at["/var/folders/"](source, length, start) or smart_context_literal_at["$TMPDIR/"](source, length, start) or smart_context_literal_at["%TEMP%\\"](source, length, start) or smart_context_literal_at["%TMP%\\"](source, length, start):
        return smart_context_path_token_end(source, length, start) - start

    if start + 2 >= length or not smart_context_is_ascii_alpha(source[unsafe_offset=start]) or source[unsafe_offset=start + 1] != 58 or not (source[unsafe_offset=start + 2] == 92 or source[unsafe_offset=start + 2] == 47):
        return -1
    var end = smart_context_path_token_end(source, length, start)
    if smart_context_windows_temp_path(source, start, end):
        return end - start
    return -1


def smart_context_windows_literal_at[literal: StaticString](
    source: Pointer[mut=False, UInt8, _], start: Int64, end: Int64, wanted_start: Int64
) -> Bool:
    var literal_length = Int64(literal.byte_length())
    if wanted_start < start or wanted_start + literal_length > end:
        return False
    var right = literal.unsafe_ptr()
    for index in range(literal_length):
        var value = source[unsafe_offset=wanted_start + index]
        if value == 47:
            value = 92
        if value >= 65 and value <= 90:
            value += 32
        var wanted = right[unsafe_offset=index]
        if wanted >= 65 and wanted <= 90:
            wanted += 32
        if value != wanted:
            return False
    return True


def smart_context_windows_temp_path(
    source: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var index = start
    while index < end:
        if smart_context_windows_literal_at["\\appdata\\local\\temp\\"](source, start, end, index) or smart_context_windows_literal_at["c:\\temp\\"](source, start, end, index):
            return True
        index += 1
    return False


def smart_context_timestamp_len(
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Int64:
    if not smart_context_before_token(source, start) or length - start < 16:
        return -1
    if not smart_context_ascii_digits(source, length, start, 4) or source[unsafe_offset=start + 4] != 45 or not smart_context_ascii_digits(source, length, start + 5, 2) or source[unsafe_offset=start + 7] != 45 or not smart_context_ascii_digits(source, length, start + 8, 2) or not (source[unsafe_offset=start + 10] == 84 or source[unsafe_offset=start + 10] == 32) or not smart_context_ascii_digits(source, length, start + 11, 2) or source[unsafe_offset=start + 13] != 58 or not smart_context_ascii_digits(source, length, start + 14, 2):
        return -1

    var index = start + 16
    if index < length and source[unsafe_offset=index] == 58:
        if not smart_context_ascii_digits(source, length, index + 1, 2):
            return -1
        index += 3
        if index < length and source[unsafe_offset=index] == 46:
            var fraction_start = index + 1
            index = fraction_start
            while index < length and smart_context_is_ascii_digit(source[unsafe_offset=index]):
                index += 1
            if index == fraction_start:
                return -1
    if index < length and source[unsafe_offset=index] == 90:
        index += 1
    elif index < length and (source[unsafe_offset=index] == 43 or source[unsafe_offset=index] == 45):
        if not smart_context_ascii_digits(source, length, index + 1, 2):
            return -1
        index += 3
        if index < length and source[unsafe_offset=index] == 58:
            if not smart_context_ascii_digits(source, length, index + 1, 2):
                return -1
            index += 3
        elif smart_context_ascii_digits(source, length, index, 2):
            index += 2
    if not smart_context_after_token(source, length, index):
        return -1
    return index - start


def smart_context_progress_percent_len(
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Int64:
    var parsed = smart_context_parse_unsigned(source, length, start)
    if parsed[0] == 0:
        return -1
    var index = Int64(parsed[1])
    if index < length and source[unsafe_offset=index] == 46:
        var fraction_start = index + 1
        index = fraction_start
        while index < length and smart_context_is_ascii_digit(source[unsafe_offset=index]):
            index += 1
        if index == fraction_start:
            return -1
    if index >= length or source[unsafe_offset=index] != 37:
        return -1
    index += 1
    if not smart_context_after_token(source, length, index):
        return -1
    return index - start


def smart_context_progress_slash_len(
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Int64:
    var left = smart_context_parse_unsigned(source, length, start)
    if left[0] == 0:
        return -1
    var left_end = Int64(left[1])
    if left_end >= length or source[unsafe_offset=left_end] != 47:
        return -1
    var right = smart_context_parse_unsigned(source, length, left_end + 1)
    if right[0] == 0 or left[2] > right[2] or right[2] == 0:
        return -1
    var right_end = Int64(right[1])
    if not smart_context_after_token(source, length, right_end):
        return -1
    return right_end - start


def smart_context_progress_of_len(
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Int64:
    var left = smart_context_parse_unsigned(source, length, start)
    if left[0] == 0:
        return -1
    var index = smart_context_skip_ascii_spaces(source, length, Int64(left[1]))
    if not smart_context_literal_at["of"](source, length, index):
        return -1
    index += 2
    if index >= length or not smart_context_is_ascii_whitespace(source[unsafe_offset=index]):
        return -1
    index = smart_context_skip_ascii_spaces(source, length, index)
    var right = smart_context_parse_unsigned(source, length, index)
    if right[0] == 0 or left[2] > right[2] or right[2] == 0:
        return -1
    var right_end = Int64(right[1])
    if not smart_context_after_token(source, length, right_end):
        return -1
    return right_end - start


def smart_context_progress_len(
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Int64:
    if not smart_context_before_token(source, start):
        return -1
    var result = smart_context_progress_percent_len(source, length, start)
    if result >= 0:
        return result
    result = smart_context_progress_slash_len(source, length, start)
    if result >= 0:
        return result
    return smart_context_progress_of_len(source, length, start)


def smart_context_uuid_exact(
    source: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    if end - start != 36:
        return False
    for index in range(Int64(36)):
        var value = source[unsafe_offset=start + index]
        if index == 8 or index == 13 or index == 18 or index == 23:
            if value != 45:
                return False
        elif not (smart_context_is_ascii_digit(value) or value >= 65 and value <= 70 or value >= 97 and value <= 102):
            return False
    return True


def smart_context_uuid_len(
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Int64:
    if not smart_context_before_token(source, start) or length - start < 36:
        return -1
    if not smart_context_uuid_exact(source, start, start + 36) or not smart_context_after_token(source, length, start + 36):
        return -1
    return 36


def smart_context_duration_unit_len(
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Int64:
    if smart_context_ascii_case_literal_at["milliseconds"](source, length, start):
        return 12
    if smart_context_ascii_case_literal_at["millisecond"](source, length, start):
        return 11
    if smart_context_ascii_case_literal_at["microseconds"](source, length, start):
        return 12
    if smart_context_ascii_case_literal_at["microsecond"](source, length, start):
        return 11
    if smart_context_ascii_case_literal_at["nanoseconds"](source, length, start):
        return 11
    if smart_context_ascii_case_literal_at["nanosecond"](source, length, start):
        return 10
    if smart_context_ascii_case_literal_at["seconds"](source, length, start):
        return 7
    if smart_context_ascii_case_literal_at["second"](source, length, start):
        return 6
    if smart_context_ascii_case_literal_at["minutes"](source, length, start):
        return 7
    if smart_context_ascii_case_literal_at["minute"](source, length, start):
        return 6
    if smart_context_ascii_case_literal_at["hours"](source, length, start):
        return 5
    if smart_context_ascii_case_literal_at["hour"](source, length, start):
        return 4
    if smart_context_ascii_case_literal_at["msecs"](source, length, start):
        return 5
    if smart_context_ascii_case_literal_at["msec"](source, length, start):
        return 4
    if smart_context_ascii_case_literal_at["usecs"](source, length, start):
        return 5
    if smart_context_ascii_case_literal_at["usec"](source, length, start):
        return 4
    if smart_context_ascii_case_literal_at["nsecs"](source, length, start):
        return 5
    if smart_context_ascii_case_literal_at["nsec"](source, length, start):
        return 4
    if smart_context_ascii_case_literal_at["secs"](source, length, start):
        return 4
    if smart_context_ascii_case_literal_at["sec"](source, length, start):
        return 3
    if smart_context_ascii_case_literal_at["mins"](source, length, start):
        return 4
    if smart_context_ascii_case_literal_at["min"](source, length, start):
        return 3
    if smart_context_ascii_case_literal_at["hrs"](source, length, start):
        return 3
    if smart_context_ascii_case_literal_at["hr"](source, length, start):
        return 2
    if smart_context_ascii_case_literal_at["ms"](source, length, start):
        return 2
    if smart_context_ascii_case_literal_at["us"](source, length, start):
        return 2
    if smart_context_ascii_case_literal_at["ns"](source, length, start):
        return 2
    if smart_context_ascii_case_literal_at["s"](source, length, start):
        return 1
    if smart_context_ascii_case_literal_at["m"](source, length, start):
        return 1
    if smart_context_ascii_case_literal_at["h"](source, length, start):
        return 1
    return -1


def smart_context_duration_len(
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Int64:
    if not smart_context_before_token(source, start):
        return -1
    var parsed = smart_context_parse_unsigned(source, length, start)
    if parsed[0] == 0:
        return -1
    var index = Int64(parsed[1])
    if index < length and source[unsafe_offset=index] == 46:
        var fraction_start = index + 1
        index = fraction_start
        while index < length and smart_context_is_ascii_digit(source[unsafe_offset=index]):
            index += 1
        if index == fraction_start:
            return -1
    index = smart_context_skip_ascii_spaces(source, length, index)
    var unit_len = smart_context_duration_unit_len(source, length, index)
    if unit_len < 0:
        return -1
    var end = index + unit_len
    if not smart_context_after_token(source, length, end):
        return -1
    return end - start


def smart_context_key_bounds(
    source: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Array[Int64, 2]:
    var bounds = Array[Int64, 2](fill=0)
    var lower = start
    var upper: Int64 = end
    while lower < upper:
        if smart_context_is_ascii_whitespace(source[unsafe_offset=lower]):
            lower += 1
            continue
        var whitespace_len = smart_context_unicode_whitespace_len(source, upper, lower)
        if whitespace_len == 0:
            break
        lower += whitespace_len
    while upper > lower:
        if smart_context_is_ascii_whitespace(source[unsafe_offset=upper - 1]):
            upper -= 1
            continue
        if (
            upper - lower >= 3
            and smart_context_unicode_whitespace_len(source, upper, upper - 3) == 3
        ):
            upper -= 3
            continue
        if (
            upper - lower >= 2
            and smart_context_unicode_whitespace_len(source, upper, upper - 2) == 2
        ):
            upper -= 2
            continue
        break
    bounds[0] = lower
    bounds[1] = upper
    return bounds^


def smart_context_key_match_byte[literal: StaticString](
    value: UInt8,
    index: Int64,
    plain_matches: Bool,
    prefixed_matches: Bool,
) -> Tuple[Bool, Bool]:
    var literal_length = Int64(literal.byte_length())
    var right = literal.unsafe_ptr()
    var plain = plain_matches
    var prefixed = prefixed_matches
    if index >= literal_length or value != right[unsafe_offset=index]:
        plain = False

    if index < 7:
        var prefix = "prodex ".ptr()
        if value != prefix[unsafe_offset=index]:
            prefixed = False
    else:
        var literal_index = index - 7
        if literal_index >= literal_length or value != right[unsafe_offset=literal_index]:
            prefixed = False
    return (plain, prefixed)


def smart_context_key_matches[literal: StaticString](
    source: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var literal_length = Int64(literal.byte_length())
    var bounds = smart_context_key_bounds(source, start, end)
    var plain_matches = True
    var prefixed_matches = True
    var output_index: Int64 = 0
    var pending_space = False
    var index = bounds[0]
    while index < bounds[1]:
        var value = source[unsafe_offset=index]
        var whitespace_len = smart_context_unicode_whitespace_len(source, bounds[1], index)
        if (
            value == 45
            or value == 95
            or smart_context_is_ascii_whitespace(value)
            or whitespace_len > 0
        ):
            if output_index > 0:
                pending_space = True
            if whitespace_len > 0:
                index += whitespace_len
            else:
                index += 1
            continue
        if pending_space:
            var space_matches = smart_context_key_match_byte[literal](
                32, output_index, plain_matches, prefixed_matches
            )
            plain_matches = space_matches[0]
            prefixed_matches = space_matches[1]
            if not plain_matches and not prefixed_matches:
                return False
            output_index += 1
            pending_space = False
        if value >= 65 and value <= 90:
            value += 32
        var value_matches = smart_context_key_match_byte[literal](
            value, output_index, plain_matches, prefixed_matches
        )
        plain_matches = value_matches[0]
        prefixed_matches = value_matches[1]
        if not plain_matches and not prefixed_matches:
            return False
        output_index += 1
        index += 1
    return (
        plain_matches and output_index == literal_length
        or prefixed_matches and output_index == literal_length + 7
    )


def smart_context_random_id_key_is_volatile(
    source: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return smart_context_key_matches["request id"](source, start, end) or smart_context_key_matches["x request id"](source, start, end) or smart_context_key_matches["trace id"](source, start, end) or smart_context_key_matches["run id"](source, start, end) or smart_context_key_matches["session id"](source, start, end) or smart_context_key_matches["conversation id"](source, start, end) or smart_context_key_matches["turn id"](source, start, end) or smart_context_key_matches["span id"](source, start, end) or smart_context_key_matches["correlation id"](source, start, end) or smart_context_key_matches["invocation id"](source, start, end) or smart_context_key_matches["execution id"](source, start, end) or smart_context_key_matches["operation id"](source, start, end) or smart_context_key_matches["job id"](source, start, end) or smart_context_key_matches["build id"](source, start, end) or smart_context_key_matches["uuid"](source, start, end) or smart_context_key_matches["id"](source, start, end)


def smart_context_random_id_value_len(
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Int64:
    var index = start
    while index < length:
        var value = source[unsafe_offset=index]
        if not (smart_context_is_ascii_alnum(value) or value == 95 or value == 45 or value == 46 or value == 58):
            break
        index += 1
    if index == start:
        return -1
    return index - start


def smart_context_random_id_value_volatile(
    source: Pointer[mut=False, UInt8, _], start: Int64, end: Int64,
    key_start: Int64, key_end: Int64,
) -> Bool:
    if smart_context_uuid_exact(source, start, end):
        return True
    var value_length = end - start
    if smart_context_key_matches["id"](source, key_start, key_end) and value_length < 20:
        return False
    if value_length < 12:
        return False
    var alpha = False
    var digit = False
    var hex_like = True
    var entropy_marks: Int64 = 0
    for index in range(start, end):
        var value = source[unsafe_offset=index]
        if smart_context_is_ascii_alpha(value):
            alpha = True
            if not (smart_context_is_ascii_digit(value) or value >= 65 and value <= 70 or value >= 97 and value <= 102):
                hex_like = False
        elif smart_context_is_ascii_digit(value):
            digit = True
        elif value == 95 or value == 45 or value == 46 or value == 58:
            entropy_marks += 1
        else:
            return False
    return hex_like and value_length >= 16 or alpha and digit and (value_length >= 16 or entropy_marks > 0)


def smart_context_emit_labeled_id(
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64,
    output: Pointer[mut=True, UInt8, _], capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Int64:
    if not smart_context_before_token(source, start):
        return -1
    var separator: Int64 = -1
    var index = start
    while index < length and index - start < 64:
        var value = source[unsafe_offset=index]
        if value == 58 or value == 61:
            separator = index
            break
        if not (smart_context_is_ascii_alnum(value) or value == 95 or value == 45 or value == 32):
            return -1
        index += 1
    if separator < 0 or not smart_context_random_id_key_is_volatile(source, start, separator):
        return -1
    var value_start = smart_context_skip_ascii_spaces(source, length, separator + 1)
    if value_start < length and (source[unsafe_offset=value_start] == 34 or source[unsafe_offset=value_start] == 39):
        value_start += 1
    var value_length = smart_context_random_id_value_len(source, length, value_start)
    if value_length < 0:
        return -1
    var value_end = value_start + value_length
    if not smart_context_random_id_value_volatile(source, value_start, value_end, start, separator):
        return -1
    if value_start - start + 4 > capacity - written[]:
        return -2
    if not smart_context_emit_range(source, start, value_start, output, capacity, written) or not smart_context_emit_literal["<id>"](output, capacity, written):
        return -2
    return value_end - start


def smart_context_static_noise_key(
    source: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return smart_context_key_matches["generated"](source, start, end) or smart_context_key_matches["generated at"](source, start, end) or smart_context_key_matches["generated on"](source, start, end) or smart_context_key_matches["last generated"](source, start, end) or smart_context_key_matches["last generated at"](source, start, end) or smart_context_key_matches["timestamp"](source, start, end) or smart_context_key_matches["current date"](source, start, end) or smart_context_key_matches["current time"](source, start, end) or smart_context_key_matches["current datetime"](source, start, end) or smart_context_key_matches["as of"](source, start, end) or smart_context_key_matches["last updated"](source, start, end) or smart_context_key_matches["updated at"](source, start, end) or smart_context_key_matches["run id"](source, start, end) or smart_context_key_matches["request id"](source, start, end) or smart_context_key_matches["trace id"](source, start, end) or smart_context_key_matches["session id"](source, start, end)


def smart_context_static_noise_value(
    source: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var bounds = smart_context_key_bounds(source, start, end)
    if bounds[0] >= bounds[1]:
        return True
    for index in range(bounds[0], bounds[1]):
        if smart_context_is_ascii_digit(source[unsafe_offset=index]):
            return True
    return smart_context_ascii_case_range["now"](source, bounds[0], bounds[1]) or smart_context_ascii_case_range["today"](source, bounds[0], bounds[1]) or smart_context_ascii_case_range["yesterday"](source, bounds[0], bounds[1]) or smart_context_ascii_case_range["tomorrow"](source, bounds[0], bounds[1])


def smart_context_static_noise(
    view: ProdexRichStringView, source: Pointer[mut=False, UInt8, _], length: Int64
) -> Bool:
    var bounds = rich_trim_bounds(view)
    var start: Int64 = bounds[0]
    var end: Int64 = bounds[1]
    if smart_context_literal_at["<!--"](source, length, start) and end - start >= 7 and smart_context_literal_at["-->"](source, length, end - 3):
        start += 4
        end -= 3
        var inner = smart_context_key_bounds(source, start, end)
        start = inner[0]
        end = inner[1]
    var comment_prefix_len: Int64 = 0
    if smart_context_literal_at["//"](source, length, start):
        comment_prefix_len = 2
    elif smart_context_literal_at["#"](source, length, start) or smart_context_literal_at[";"](source, length, start):
        comment_prefix_len = 1
    if comment_prefix_len > 0:
        var prefix_bounds = smart_context_key_bounds(source, start + comment_prefix_len, end)
        start = prefix_bounds[0]

    var separator: Int64 = -1
    var index = start
    while index < end:
        if source[unsafe_offset=index] == 58:
            separator = index
            break
        index += 1
    if separator < 0:
        index = start
        while index < end:
            if source[unsafe_offset=index] == 61:
                separator = index
                break
            index += 1
    if separator < 0 or not smart_context_static_noise_key(source, start, separator):
        return False
    if smart_context_key_matches["run id"](source, start, separator) or smart_context_key_matches["request id"](source, start, separator) or smart_context_key_matches["trace id"](source, start, separator) or smart_context_key_matches["session id"](source, start, separator):
        return True
    return smart_context_static_noise_value(source, separator + 1, end)


def smart_context_normalization_mode_valid(mode: Int64) -> Bool:
    return mode == SMART_CONTEXT_MODE_COMMAND_OUTPUT or mode == SMART_CONTEXT_MODE_STATIC_CONTEXT or mode == SMART_CONTEXT_MODE_STATIC_NOISE


def smart_context_normalize_segment(
    source: Pointer[mut=False, UInt8, _],
    length: Int64,
    operation: Int64,
    output: Pointer[mut=True, UInt8, _],
    output_capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Int64:
    var index: Int64 = 0
    while index < length:
        var consumed = smart_context_ansi_escape_len(source, length, index)
        if consumed >= 0:
            index += consumed
            continue
        consumed = smart_context_temp_path_len(source, length, index)
        if consumed >= 0:
            if not smart_context_emit_literal["<tmp-path>"](output, output_capacity, written):
                return SMART_CONTEXT_STATUS_CAPACITY
            index += consumed
            continue
        consumed = smart_context_timestamp_len(source, length, index)
        if consumed >= 0:
            if not smart_context_emit_literal["<timestamp>"](output, output_capacity, written):
                return SMART_CONTEXT_STATUS_CAPACITY
            index += consumed
            continue
        if operation == SMART_CONTEXT_MODE_COMMAND_OUTPUT:
            consumed = smart_context_progress_len(source, length, index)
            if consumed >= 0:
                if not smart_context_emit_literal["<progress>"](output, output_capacity, written):
                    return SMART_CONTEXT_STATUS_CAPACITY
                index += consumed
                continue
        var label_result = smart_context_emit_labeled_id(source, length, index, output, output_capacity, written)
        if label_result == -2:
            return SMART_CONTEXT_STATUS_CAPACITY
        if label_result >= 0:
            index += label_result
            continue
        consumed = smart_context_uuid_len(source, length, index)
        if consumed >= 0:
            if not smart_context_emit_literal["<id>"](output, output_capacity, written):
                return SMART_CONTEXT_STATUS_CAPACITY
            index += consumed
            continue
        if operation == SMART_CONTEXT_MODE_COMMAND_OUTPUT:
            consumed = smart_context_duration_len(source, length, index)
            if consumed >= 0:
                if not smart_context_emit_literal["<duration>"](output, output_capacity, written):
                    return SMART_CONTEXT_STATUS_CAPACITY
                index += consumed
                continue
        if written[] >= output_capacity:
            return SMART_CONTEXT_STATUS_CAPACITY
        output[unsafe_offset=written[]] = source[unsafe_offset=index]
        written[] += 1
        index += 1
    return SMART_CONTEXT_STATUS_OK


@export("prodex_mojo_smart_context_normalization_v1")
def prodex_mojo_smart_context_normalization_v1(
    abi_version: Int64,
    input_address: UInt,
    operation: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
    decision_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return SMART_CONTEXT_STATUS_ABI
    if input_address == 0 or not smart_context_normalization_mode_valid(operation):
        return SMART_CONTEXT_STATUS_INVALID
    var input_ptr = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(input_address))
    var input = input_ptr[].copy()
    if input.len > UInt(SMART_CONTEXT_NORMALIZATION_MAX_BYTES):
        return SMART_CONTEXT_STATUS_INVALID
    if not rich_view_valid(input, SMART_CONTEXT_NORMALIZATION_MAX_BYTES):
        return SMART_CONTEXT_STATUS_UTF8
    var source = rich_view_ptr(input)
    var length = Int64(input.len)
    if operation == SMART_CONTEXT_MODE_STATIC_NOISE:
        if decision_address == 0:
            return SMART_CONTEXT_STATUS_INVALID
        var decision = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(decision_address))
        decision[] = Int64(smart_context_static_noise(input, source, length))
        return SMART_CONTEXT_STATUS_OK
    if output_address == 0 or written_address == 0 or output_capacity < 1 or output_capacity > SMART_CONTEXT_NORMALIZATION_MAX_OUTPUT_BYTES:
        return SMART_CONTEXT_STATUS_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))
    written[] = 0
    return smart_context_normalize_segment(
        source, length, operation, output, output_capacity, written
    )



# The complete static-context pipeline is a single compiled Mojo owner.
# Reuses the same volatile-value kernel as standalone normalization.
def smart_context_static_canonicalize_inner(
    source_view: ProdexRichStringView,
    output_address: UInt,
    output: Pointer[mut=True, UInt8, _],
    output_capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Int64:
    var source = rich_view_ptr(source_view)
    written[] = 0
    var length = Int64(source_view.len)
    var cursor: Int64 = 0
    var last_nonblank: Int64 = 0
    var emitted = False
    while cursor < length:
        var begin = cursor
        while cursor < length:
            var byte = source[unsafe_offset=cursor]
            if byte == 10 or byte == 13:
                break
            cursor += 1
        var end = cursor
        if cursor < length:
            var delimiter = source[unsafe_offset=cursor]
            cursor += 1
            if delimiter == 13 and cursor < length:
                if source[unsafe_offset=cursor] == 10:
                    cursor += 1

        # Rust's trim_end removes Unicode whitespace before both tests.
        while end > begin:
            var codepoint_start = end - 1
            while codepoint_start > begin and rich_utf8_continuation(
                source[unsafe_offset=codepoint_start]
            ):
                codepoint_start -= 1
            var codepoint = rich_codepoint(
                source, codepoint_start, end - codepoint_start
            )
            if not rich_unicode_space(codepoint):
                break
            end = codepoint_start
        var line = ProdexRichStringView(
            source_view.ptr + UInt(begin), UInt(end - begin)
        )
        if smart_context_static_noise(line, rich_view_ptr(line), end - begin):
            continue

        var previous = written[]
        if emitted:
            if written[] >= output_capacity:
                return SMART_CONTEXT_STATUS_CAPACITY
            output[unsafe_offset=written[]] = 10
            written[] += 1
        var segment_start = written[]
        var status = smart_context_normalize_segment(
            rich_view_ptr(line),
            end - begin,
            SMART_CONTEXT_MODE_STATIC_CONTEXT,
            output,
            output_capacity,
            written,
        )
        if status != SMART_CONTEXT_STATUS_OK:
            return status
        var produced = ProdexRichStringView(
            output_address + UInt(segment_start), UInt(written[] - segment_start)
        )
        var content = rich_trim_bounds(produced)
        if content[0] != content[1]:
            emitted = True
            last_nonblank = written[]
        elif not emitted:
            # Skip a blank prefix, but retain interior empty lines until
            # the final tail trim.
            written[] = previous
    written[] = last_nonblank
    return SMART_CONTEXT_STATUS_OK


@export("prodex_mojo_smart_context_static_canonicalize_v1")
def prodex_mojo_smart_context_static_canonicalize_v1(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return SMART_CONTEXT_STATUS_ABI
    if (
        input_address == 0
        or output_address == 0
        or written_address == 0
        or output_capacity < 1
        or output_capacity > SMART_CONTEXT_NORMALIZATION_MAX_OUTPUT_BYTES
    ):
        return SMART_CONTEXT_STATUS_INVALID
    var source_view = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_address)
    )[].copy()
    if source_view.len > UInt(SMART_CONTEXT_NORMALIZATION_MAX_BYTES):
        return SMART_CONTEXT_STATUS_INVALID
    if not rich_view_valid(source_view, SMART_CONTEXT_NORMALIZATION_MAX_BYTES):
        return SMART_CONTEXT_STATUS_UTF8
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    return smart_context_static_canonicalize_inner(
        source_view, output_address, output, output_capacity, written
    )


def smart_context_static_payload_put_u64(
    value: UInt64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    var divisor = UInt64(1)
    while value // divisor >= 10:
        divisor *= 10
    var remaining = value
    while True:
        if written[] >= capacity:
            return False
        output[unsafe_offset=written[]] = UInt8(48) + UInt8(remaining // divisor)
        written[] += 1
        remaining = remaining % divisor
        if divisor == 1:
            break
        divisor = divisor // UInt64(10)
    return True


@export("prodex_mojo_smart_context_static_prompt_payload_v1")
def prodex_mojo_smart_context_static_prompt_payload_v1(
    abi_version: Int64,
    id_views_address: UInt,
    text_views_address: UInt,
    byte_lengths_address: UInt,
    item_count: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return SMART_CONTEXT_STATUS_ABI
    if (
        item_count < 0
        or item_count > 128
        or (item_count > 0 and (
            id_views_address == 0
            or text_views_address == 0
            or byte_lengths_address == 0
        ))
        or output_address == 0
        or written_address == 0
        or output_capacity <= 0
    ):
        return SMART_CONTEXT_STATUS_INVALID
    var ids = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(id_views_address)
    )
    var texts = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(text_views_address)
    )
    var sizes = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(byte_lengths_address)
    )
    for index in range(item_count):
        if (
            not rich_view_valid(ids[unsafe_offset=index].copy(), SMART_CONTEXT_NORMALIZATION_MAX_BYTES)
            or not rich_view_valid(texts[unsafe_offset=index].copy(), SMART_CONTEXT_NORMALIZATION_MAX_BYTES)
        ):
            return SMART_CONTEXT_STATUS_UTF8
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if not smart_context_emit_literal["prodex-smart-context-static-prompt-cache-v1\n"](
        output, output_capacity, written
    ):
        return SMART_CONTEXT_STATUS_CAPACITY
    for index in range(item_count):
        var id = ids[unsafe_offset=index].copy()
        var text = texts[unsafe_offset=index].copy()
        if (
            not smart_context_emit_literal["id-bytes:"](output, output_capacity, written)
            or not smart_context_static_payload_put_u64(UInt64(id.len), output, output_capacity, written)
            or not smart_context_emit_literal["\n"](output, output_capacity, written)
            or not smart_context_emit_range(rich_view_ptr(id), 0, Int64(id.len), output, output_capacity, written)
            or not smart_context_emit_literal["\ntext-bytes:"](output, output_capacity, written)
            or not smart_context_static_payload_put_u64(sizes[unsafe_offset=index], output, output_capacity, written)
            or not smart_context_emit_literal["\n"](output, output_capacity, written)
            or not smart_context_emit_range(rich_view_ptr(text), 0, Int64(text.len), output, output_capacity, written)
            or not smart_context_emit_literal["\n"](output, output_capacity, written)
        ):
            return SMART_CONTEXT_STATUS_CAPACITY
    return SMART_CONTEXT_STATUS_OK


@export("prodex_mojo_smart_context_budget_tier_v1")
def prodex_mojo_smart_context_budget_tier_v1(
    abi_version: Int64, available_tokens: UInt64, tier_address: UInt
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION or tier_address == 0:
        return SMART_CONTEXT_STATUS_INVALID
    var tier = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(tier_address))
    if available_tokens >= 16_000:
        tier[] = 0
    elif available_tokens >= 8_000:
        tier[] = 1
    elif available_tokens >= 2_000:
        tier[] = 2
    else:
        tier[] = 3
    return SMART_CONTEXT_STATUS_OK


@export("prodex_mojo_smart_context_memory_capsule_budget_v1")
def prodex_mojo_smart_context_memory_capsule_budget_v1(
    abi_version: Int64,
    available_context_tokens: UInt64,
    available_present: Int64,
    mode: Int64,
    tier: Int64,
    max_rehydrate_tokens: UInt64,
    reason_bits: UInt64,
    accounting_safe: Int64,
    budget_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION or budget_address == 0 or available_present < 0 or available_present > 1 or mode < 0 or mode > 3 or tier < 0 or tier > 3 or accounting_safe < 0 or accounting_safe > 1 or reason_bits & ~SMART_CONTEXT_POLICY_REASON_ALL != 0:
        return SMART_CONTEXT_STATUS_INVALID
    var budget = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(budget_address))
    budget[] = 0
    if accounting_safe == 1 and mode == SMART_CONTEXT_BUDGET_MODE_EXACT and tier == SMART_CONTEXT_BUDGET_TIER_EXACT and reason_bits == SMART_CONTEXT_POLICY_REASON_PLENTY:
        budget[] = 18446744073709551615
        return SMART_CONTEXT_STATUS_OK
    if accounting_safe == 0 or reason_bits & (SMART_CONTEXT_POLICY_REASON_EXACTNESS | SMART_CONTEXT_POLICY_REASON_STATIC_CHANGED | SMART_CONTEXT_POLICY_REASON_MISSING_REFS | SMART_CONTEXT_POLICY_REASON_UNKNOWN_WINDOW | SMART_CONTEXT_POLICY_REASON_UNSAFE_ACCOUNTING) != 0 or available_present == 0:
        return SMART_CONTEXT_STATUS_OK

    var mode_budget: UInt64
    if mode == SMART_CONTEXT_BUDGET_MODE_MINIMAL:
        mode_budget = 256
    elif mode == SMART_CONTEXT_BUDGET_MODE_CONDENSED:
        mode_budget = 1_024
    elif mode == SMART_CONTEXT_BUDGET_MODE_LARGE:
        mode_budget = 4_096
    elif tier == SMART_CONTEXT_BUDGET_TIER_EXACT or tier == SMART_CONTEXT_BUDGET_TIER_LARGE:
        mode_budget = 4_096
    elif tier == SMART_CONTEXT_BUDGET_TIER_CONDENSED:
        mode_budget = 1_024
    else:
        mode_budget = 256
    if mode_budget > max_rehydrate_tokens:
        mode_budget = max_rehydrate_tokens
    if mode_budget > available_context_tokens:
        mode_budget = available_context_tokens
    budget[] = mode_budget
    return SMART_CONTEXT_STATUS_OK


@export("prodex_mojo_smart_context_capsule_plan_v1")
def prodex_mojo_smart_context_capsule_plan_v1(
    abi_version: Int64,
    token_costs_address: UInt,
    required_address: UInt,
    selected_address: UInt,
    used_tokens_address: UInt,
    count: Int64,
    token_budget: UInt64,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION or count < 0 or count > SMART_CONTEXT_CAPSULE_MAX_COUNT or selected_address == 0 or used_tokens_address == 0:
        return SMART_CONTEXT_STATUS_INVALID
    var selected = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(selected_address))
    var used_tokens = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(used_tokens_address))
    used_tokens[] = 0
    if count > 0 and (token_costs_address == 0 or required_address == 0):
        return SMART_CONTEXT_STATUS_INVALID
    var token_costs = Pointer[mut=False, UInt64, ImmUntrackedOrigin](unsafe_from_address=Int(token_costs_address))
    var required = Pointer[mut=False, Int64, ImmUntrackedOrigin](unsafe_from_address=Int(required_address))
    var used: UInt64 = 0
    for index in range(count):
        var required_value = required[unsafe_offset=index]
        if required_value != 0 and required_value != 1:
            return SMART_CONTEXT_STATUS_INVALID
        var cost = token_costs[unsafe_offset=index]
        if cost <= token_budget - used:
            selected[unsafe_offset=index] = 1
            used += cost
        else:
            selected[unsafe_offset=index] = 0
    used_tokens[] = used
    return SMART_CONTEXT_STATUS_OK


# Smart Context identifiers are normalized and length-limited exclusively by
# Mojo. Return 5 to request a host-computed SHA-256 digest only for long IDs.
# The hash itself remains a host-side cryptographic primitive.
@export("prodex_mojo_smart_context_static_bounded_id_v1")
def prodex_mojo_smart_context_static_bounded_id_v1(
    abi_version: Int64,
    input_view_address: UInt,
    hash_view_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return SMART_CONTEXT_STATUS_ABI
    if (
        input_view_address == 0
        or output_address == 0
        or output_capacity <= 0
        or written_address == 0
    ):
        return SMART_CONTEXT_STATUS_INVALID
    var input = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_view_address)
    )[].copy()
    if not rich_view_valid(input, 9_223_372_036_854_775_807):
        return SMART_CONTEXT_STATUS_UTF8
    var bounds = rich_trim_bounds(input)
    var start = bounds[0]
    var end = bounds[1]
    var size = end - start
    var prefix_end = end
    var truncated = False
    var source = rich_view_ptr(input)
    if size > 256:
        truncated = True
        prefix_end = start + 256
        while prefix_end > start and rich_utf8_continuation(
            source[unsafe_offset=prefix_end]
        ):
            prefix_end -= 1
        if hash_view_address == 0:
            return 5
    var digest = ProdexRichStringView(0, 0)
    if truncated:
        digest = Pointer[
            mut=False, ProdexRichStringView, ImmUntrackedOrigin
        ](unsafe_from_address=Int(hash_view_address))[].copy()
        if not rich_view_valid(digest, 256):
            return SMART_CONTEXT_STATUS_UTF8
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    for index in range(start, prefix_end):
        if written[] >= output_capacity:
            return SMART_CONTEXT_STATUS_CAPACITY
        var byte = source[unsafe_offset=index]
        output[unsafe_offset=written[]] = 47 if byte == 92 else byte
        written[] += 1
    if truncated:
        if (
            not smart_context_emit_literal["<id-hash="](
                output, output_capacity, written
            )
            or not smart_context_emit_range(
                rich_view_ptr(digest), 0, Int64(digest.len),
                output, output_capacity, written
            )
            or not smart_context_emit_literal[">"](
                output, output_capacity, written
            )
        ):
            return SMART_CONTEXT_STATUS_CAPACITY
    return SMART_CONTEXT_STATUS_OK


# The 256 KiB truncation boundary, Unicode-safe prefix, canonicalization,
# and diagnostic suffix are an indivisible Mojo semantic operation. Return
# status 5 only when the host must supply the original-text SHA-256.
@export("prodex_mojo_smart_context_static_bounded_text_v1")
def prodex_mojo_smart_context_static_bounded_text_v1(
    abi_version: Int64,
    input_view_address: UInt,
    hash_view_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return SMART_CONTEXT_STATUS_ABI
    if (
        input_view_address == 0
        or output_address == 0
        or output_capacity <= 0
        or written_address == 0
    ):
        return SMART_CONTEXT_STATUS_INVALID
    var input = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_view_address)
    )[].copy()
    if input.len > UInt(9_223_372_036_854_775_807):
        return SMART_CONTEXT_STATUS_INVALID
    var truncated = input.len > UInt(256 * 1024)
    var prefix_length = Int64(input.len)
    var source = rich_view_ptr(input)
    if truncated:
        prefix_length = 256 * 1024
        while prefix_length > 0 and rich_utf8_continuation(
            source[unsafe_offset=prefix_length]
        ):
            prefix_length -= 1
        if hash_view_address == 0:
            return 5
    var prefix = ProdexRichStringView(input.ptr, UInt(prefix_length))
    if not rich_view_valid(prefix, SMART_CONTEXT_NORMALIZATION_MAX_BYTES):
        return SMART_CONTEXT_STATUS_UTF8
    var digest = ProdexRichStringView(0, 0)
    if truncated:
        digest = Pointer[
            mut=False, ProdexRichStringView, ImmUntrackedOrigin
        ](unsafe_from_address=Int(hash_view_address))[].copy()
        if not rich_view_valid(digest, 256):
            return SMART_CONTEXT_STATUS_UTF8
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var status = smart_context_static_canonicalize_inner(
        prefix, output_address, output, output_capacity, written
    )
    if status != SMART_CONTEXT_STATUS_OK:
        return status
    if not truncated:
        return SMART_CONTEXT_STATUS_OK
    if (
        not smart_context_emit_literal["\npsc static fingerprint item truncated bytes="](
            output, output_capacity, written
        )
        or not smart_context_static_payload_put_u64(
            UInt64(input.len), output, output_capacity, written
        )
        or not smart_context_emit_literal[" hash="](
            output, output_capacity, written
        )
        or not smart_context_emit_range(
            rich_view_ptr(digest), 0, Int64(digest.len),
            output, output_capacity, written
        )
    ):
        return SMART_CONTEXT_STATUS_CAPACITY
    return SMART_CONTEXT_STATUS_OK


# Legacy Smart Context fingerprints use FNV-1a over bytes. Mojo owns the
# exact 64-bit wrapping hash and lowercase 16-hex serialization.
# Mode 0 emits hex only; mode 1 emits the historical "scv:" prefix.
@export("prodex_mojo_smart_context_fnv_hash_text_v1")
def prodex_mojo_smart_context_fnv_hash_text_v1(
    abi_version: Int64,
    mode: Int64,
    source_address: UInt,
    source_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return SMART_CONTEXT_STATUS_ABI
    if (
        mode < 0 or mode > 1
        or source_length < 0
        or (source_length > 0 and source_address == 0)
        or output_address == 0
        or written_address == 0
    ):
        return SMART_CONTEXT_STATUS_INVALID
    var required: Int64 = 16
    if mode == 1:
        required = 20
    if output_capacity < required:
        return SMART_CONTEXT_STATUS_CAPACITY
    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(source_address)
    )
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var hash = UInt64(14695981039346656037)
    for index in range(source_length):
        hash = hash ^ UInt64(source[unsafe_offset=index])
        hash = hash * UInt64(1099511628211)
    var cursor: Int64 = 0
    if mode == 1:
        output[unsafe_offset=0] = 115
        output[unsafe_offset=1] = 99
        output[unsafe_offset=2] = 118
        output[unsafe_offset=3] = 58
        cursor = 4
    var shift: Int64 = 60
    for index in range(16):
        var digit = UInt8((hash >> UInt64(shift)) & UInt64(15))
        if digit < 10:
            output[unsafe_offset=cursor + Int64(index)] = 48 + digit
        else:
            output[unsafe_offset=cursor + Int64(index)] = 87 + digit
        shift -= 4
    written[] = required
    return SMART_CONTEXT_STATUS_OK


# ContextScopeId wire contract: SHA-256 inputs are five length-delimited
# UTF-8 fields after the versioned domain separator and a NUL byte.
# Rust retains only the cryptographic SHA-256 primitive and DTO ownership.
@export("prodex_mojo_smart_context_scope_preimage_v1")
def prodex_mojo_smart_context_scope_preimage_v1(
    abi_version: Int64,
    field_views_address: UInt,
    field_count: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return SMART_CONTEXT_STATUS_ABI
    if (
        field_count != 5
        or field_views_address == 0
        or output_address == 0
        or written_address == 0
        or output_capacity < 1
    ):
        return SMART_CONTEXT_STATUS_INVALID
    var fields = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(field_views_address)
    )
    for index in range(5):
        if not rich_view_valid(
            fields[unsafe_offset=index].copy(), 9_223_372_036_854_775_807
        ):
            return SMART_CONTEXT_STATUS_UTF8
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if not smart_context_emit_literal["prodex-smart-context-scope-v1"](
        output, output_capacity, written
    ):
        return SMART_CONTEXT_STATUS_CAPACITY
    if written[] >= output_capacity:
        return SMART_CONTEXT_STATUS_CAPACITY
    output[unsafe_offset=written[]] = 0
    written[] += 1
    for index in range(5):
        var field = fields[unsafe_offset=index].copy()
        var byte_length = UInt64(field.len)
        if output_capacity - written[] < 8:
            return SMART_CONTEXT_STATUS_CAPACITY
        for position in range(8):
            var shift = UInt64((7 - position) * 8)
            output[unsafe_offset=written[]] = UInt8(
                (byte_length >> shift) & UInt64(255)
            )
            written[] += 1
        if not smart_context_emit_range(
            rich_view_ptr(field),
            0,
            Int64(field.len),
            output,
            output_capacity,
            written,
        ):
            return SMART_CONTEXT_STATUS_CAPACITY
    return SMART_CONTEXT_STATUS_OK


# 32-byte SHA-256 digests become the exact stable scscope1: prefixed,
# lowercase 64-hex scope value without a host formatting fallback.
def smart_context_static_sha256_hex_append(
    digest: Pointer[mut=False, UInt8, _],
    output: Pointer[mut=True, UInt8, _],
    written: Pointer[mut=True, Int64, _],
):
    # Both persistent scope and prompt-cache digest codecs share one
    # compiled hexadecimal encoder; output capacity is prevalidated.
    for index in range(32):
        var byte = digest[unsafe_offset=index]
        var high = byte >> UInt8(4)
        var low = byte & UInt8(15)
        if high < 10:
            output[unsafe_offset=written[]] = UInt8(48) + high
        else:
            output[unsafe_offset=written[]] = UInt8(87) + high
        written[] += 1
        if low < 10:
            output[unsafe_offset=written[]] = UInt8(48) + low
        else:
            output[unsafe_offset=written[]] = UInt8(87) + low
        written[] += 1


@export("prodex_mojo_smart_context_scope_digest_text_v1")
def prodex_mojo_smart_context_scope_digest_text_v1(
    abi_version: Int64,
    digest_address: UInt,
    digest_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return SMART_CONTEXT_STATUS_ABI
    if (
        digest_address == 0
        or digest_length != 32
        or output_address == 0
        or written_address == 0
    ):
        return SMART_CONTEXT_STATUS_INVALID
    if output_capacity < 73:
        return SMART_CONTEXT_STATUS_CAPACITY
    var digest = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(digest_address)
    )
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if not smart_context_emit_literal["scscope1:"](
        output, output_capacity, written
    ):
        return SMART_CONTEXT_STATUS_CAPACITY
    smart_context_static_sha256_hex_append(digest, output, written)
    return SMART_CONTEXT_STATUS_OK


# Parse persisted scscope1: identifiers with the exact legacy security
# contract: case-sensitive prefix, 64 ASCII hex digits, normalized lowercase.
@export("prodex_mojo_smart_context_scope_parse_v1")
def prodex_mojo_smart_context_scope_parse_v1(
    abi_version: Int64,
    input_view_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
    valid_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return SMART_CONTEXT_STATUS_ABI
    if (
        input_view_address == 0
        or output_address == 0
        or written_address == 0
        or valid_address == 0
    ):
        return SMART_CONTEXT_STATUS_INVALID
    var input = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_view_address)
    )[].copy()
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var valid = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(valid_address)
    )
    valid[] = 0
    written[] = 0
    if input.len != 73:
        return SMART_CONTEXT_STATUS_OK
    if not rich_view_valid(input, 73):
        return SMART_CONTEXT_STATUS_UTF8
    var source = rich_view_ptr(input)
    var expected = StringSlice("scscope1:").unsafe_ptr()
    for index in range(9):
        if source[unsafe_offset=index] != expected[unsafe_offset=index]:
            return SMART_CONTEXT_STATUS_OK
    for index in range(9, 73):
        var byte = source[unsafe_offset=index]
        if not (
            byte >= 48 and byte <= 57
            or byte >= 65 and byte <= 70
            or byte >= 97 and byte <= 102
        ):
            return SMART_CONTEXT_STATUS_OK
    if output_capacity < 73:
        return SMART_CONTEXT_STATUS_CAPACITY
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(73):
        var byte = source[unsafe_offset=index]
        if byte >= 65 and byte <= 70:
            byte += 32
        output[unsafe_offset=index] = byte
    written[] = 73
    valid[] = 1
    return SMART_CONTEXT_STATUS_OK

# Static fingerprint overflow records are hashed by the host SHA-256
# primitive, but their binary framing and 32-byte wrapping aggregate
# semantics belong to compiled Mojo.
@export("prodex_mojo_smart_context_static_overflow_preimage_v1")
def prodex_mojo_smart_context_static_overflow_preimage_v1(
    abi_version: Int64,
    id_view_address: UInt,
    content_hash_view_address: UInt,
    item_byte_length: UInt64,
    host_usize_bytes: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return SMART_CONTEXT_STATUS_ABI
    if (
        id_view_address == 0
        or content_hash_view_address == 0
        or output_address == 0
        or written_address == 0
        or (host_usize_bytes != 4 and host_usize_bytes != 8)
        or output_capacity < 1
    ):
        return SMART_CONTEXT_STATUS_INVALID
    var id = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(id_view_address)
    )[].copy()
    var content_hash = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(content_hash_view_address)
    )[].copy()
    if (
        not rich_view_valid(id, SMART_CONTEXT_NORMALIZATION_MAX_BYTES)
        or not rich_view_valid(content_hash, SMART_CONTEXT_NORMALIZATION_MAX_BYTES)
    ):
        return SMART_CONTEXT_STATUS_UTF8
    var required = Int64(id.len) + 1 + Int64(content_hash.len) + 1 + host_usize_bytes
    if output_capacity < required:
        return SMART_CONTEXT_STATUS_CAPACITY
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    var id_bytes = rich_view_ptr(id)
    for index in range(Int64(id.len)):
        output[unsafe_offset=written[]] = id_bytes[unsafe_offset=index]
        written[] += 1
    output[unsafe_offset=written[]] = 0
    written[] += 1
    var hash_bytes = rich_view_ptr(content_hash)
    for index in range(Int64(content_hash.len)):
        output[unsafe_offset=written[]] = hash_bytes[unsafe_offset=index]
        written[] += 1
    output[unsafe_offset=written[]] = 0
    written[] += 1
    for index in range(host_usize_bytes):
        output[unsafe_offset=written[]] = UInt8(
            (item_byte_length >> UInt64(index * 8)) & UInt64(255)
        )
        written[] += 1
    return SMART_CONTEXT_STATUS_OK


@export("prodex_mojo_smart_context_static_overflow_accumulate_v1")
def prodex_mojo_smart_context_static_overflow_accumulate_v1(
    abi_version: Int64,
    aggregate_address: UInt,
    sha256_digest_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return SMART_CONTEXT_STATUS_ABI
    if (
        aggregate_address == 0
        or sha256_digest_address == 0
        or output_address == 0
        or output_capacity < 32
    ):
        return SMART_CONTEXT_STATUS_INVALID
    var aggregate = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(aggregate_address)
    )
    var digest = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(sha256_digest_address)
    )
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(32):
        var sum = UInt16(aggregate[unsafe_offset=index]) + UInt16(digest[unsafe_offset=index])
        output[unsafe_offset=index] = UInt8(sum & UInt16(255))
    return SMART_CONTEXT_STATUS_OK

# Mode 0: exactly 64 hex digits for the static overflow checksum.
# Mode 1: the stable scpc2: prefixed 64-digit SHA-256 fingerprint.
# Both share the same Mojo SHA-256 hex codec as ContextScopeId.
@export("prodex_mojo_smart_context_static_digest_text_v1")
def prodex_mojo_smart_context_static_digest_text_v1(
    abi_version: Int64,
    mode: Int64,
    digest_address: UInt,
    digest_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return SMART_CONTEXT_STATUS_ABI
    if (
        mode < 0
        or mode > 1
        or digest_address == 0
        or digest_length != 32
        or output_address == 0
        or written_address == 0
    ):
        return SMART_CONTEXT_STATUS_INVALID
    var required: Int64 = 64
    if mode == 1:
        required = 70
    if output_capacity < required:
        return SMART_CONTEXT_STATUS_CAPACITY
    var digest = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(digest_address)
    )
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if mode == 1:
        if not smart_context_emit_literal["scpc2:"](
            output, output_capacity, written
        ):
            return SMART_CONTEXT_STATUS_CAPACITY
    smart_context_static_sha256_hex_append(digest, output, written)
    return SMART_CONTEXT_STATUS_OK


# Stable truncated-payload metadata remains canonical only if the count,
# byte-size and overflow digest are rendered by the same Mojo owner.
@export("prodex_mojo_smart_context_static_truncation_line_v1")
def prodex_mojo_smart_context_static_truncation_line_v1(
    abi_version: Int64,
    item_count: UInt64,
    total_bytes: UInt64,
    hash_view_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return SMART_CONTEXT_STATUS_ABI
    if (
        hash_view_address == 0
        or output_address == 0
        or written_address == 0
        or output_capacity <= 0
    ):
        return SMART_CONTEXT_STATUS_INVALID
    var digest = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(hash_view_address)
    )[].copy()
    if not rich_view_valid(digest, 64) or digest.len != 64:
        return SMART_CONTEXT_STATUS_UTF8
    var ptr = rich_view_ptr(digest)
    for index in range(64):
        var byte = ptr[unsafe_offset=index]
        if not (
            byte >= 48 and byte <= 57
            or byte >= 97 and byte <= 102
        ):
            return SMART_CONTEXT_STATUS_UTF8
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if (
        not smart_context_emit_literal["psc static fingerprint truncated "](
            output, output_capacity, written
        )
        or not smart_context_static_payload_put_u64(
            item_count, output, output_capacity, written
        )
        or not smart_context_emit_literal[" "](
            output, output_capacity, written
        )
        or not smart_context_static_payload_put_u64(
            total_bytes, output, output_capacity, written
        )
        or not smart_context_emit_literal[" "](
            output, output_capacity, written
        )
        or not smart_context_emit_range(
            ptr, 0, 64, output, output_capacity, written
        )
        or not smart_context_emit_literal["\n"](
            output, output_capacity, written
        )
    ):
        return SMART_CONTEXT_STATUS_CAPACITY
    return SMART_CONTEXT_STATUS_OK
