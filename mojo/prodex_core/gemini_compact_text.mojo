from std.memory import Pointer

from json_view import (
    deepseek_json_byte,
    deepseek_json_fragment_valid,
    deepseek_json_skip_ws,
    deepseek_json_string_end,
    deepseek_json_value_end,
    deepseek_json_object_member,
)
from rich_text import (
    rich_codepoint,
    rich_codepoint_width,
    rich_trim_bounds,
    rich_view_ptr,
    rich_view_valid,
)
from rich_types import ProdexRichStringView

comptime GEMINI_COMPACT_TEXT_ABI_VERSION: Int64 = 1
comptime GEMINI_COMPACT_TEXT_STATUS_OK: Int64 = 0
comptime GEMINI_COMPACT_TEXT_STATUS_INVALID: Int64 = 1
comptime GEMINI_COMPACT_TEXT_STATUS_UTF8: Int64 = 2
comptime GEMINI_COMPACT_TEXT_STATUS_CAPACITY: Int64 = 3
comptime GEMINI_COMPACT_TEXT_STATUS_ABI: Int64 = 4
comptime GEMINI_COMPACT_TEXT_MAX_DEPTH: Int64 = 256


def gemini_compact_text_hex(value: UInt8) -> Int64:
    if value >= 48 and value <= 57:
        return Int64(value - 48)
    if value >= 65 and value <= 70:
        return Int64(value - 55)
    if value >= 97 and value <= 102:
        return Int64(value - 87)
    return -1


def gemini_compact_text_write_byte(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    value: UInt8,
) -> Bool:
    if written[] < 0 or written[] >= capacity:
        return False
    output[unsafe_offset=written[]] = value
    written[] += 1
    return True


def gemini_compact_text_write_codepoint(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    codepoint: Int64,
) -> Bool:
    if (
        codepoint < 0
        or codepoint > 1_114_111
        or codepoint >= 55_296
        and codepoint <= 57_343
    ):
        return False
    if codepoint <= 127:
        return gemini_compact_text_write_byte(
            output, capacity, written, UInt8(codepoint)
        )
    if codepoint <= 2_047:
        return gemini_compact_text_write_byte(
            output, capacity, written, UInt8(192 | (codepoint >> 6))
        ) and gemini_compact_text_write_byte(
            output, capacity, written, UInt8(128 | (codepoint & 63))
        )
    if codepoint <= 65_535:
        return gemini_compact_text_write_byte(
            output, capacity, written, UInt8(224 | (codepoint >> 12))
        ) and gemini_compact_text_write_byte(
            output,
            capacity,
            written,
            UInt8(128 | ((codepoint >> 6) & 63)),
        ) and gemini_compact_text_write_byte(
            output, capacity, written, UInt8(128 | (codepoint & 63))
        )
    return gemini_compact_text_write_byte(
        output, capacity, written, UInt8(240 | (codepoint >> 18))
    ) and gemini_compact_text_write_byte(
        output,
        capacity,
        written,
        UInt8(128 | ((codepoint >> 12) & 63)),
    ) and gemini_compact_text_write_byte(
        output,
        capacity,
        written,
        UInt8(128 | ((codepoint >> 6) & 63)),
    ) and gemini_compact_text_write_byte(
        output, capacity, written, UInt8(128 | (codepoint & 63))
    )


def gemini_compact_text_key_matches(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    if start < 0 or end - start != Int64(literal.byte_length()) + 2:
        return False
    if deepseek_json_byte(view, start) != 34 or deepseek_json_byte(view, end - 1) != 34:
        return False
    var actual = rich_view_ptr(view)
    var expected = literal.unsafe_ptr()
    for index in range(Int64(literal.byte_length())):
        if actual[unsafe_offset=start + 1 + index] != expected[unsafe_offset=index]:
            return False
    return True


def gemini_compact_text_append_range(
    source: ProdexRichStringView,
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    if start < 0 or end < start or end > Int64(source.len):
        return False
    var source_ptr = rich_view_ptr(source)
    for index in range(start, end):
        if not gemini_compact_text_write_byte(
            output, capacity, written, source_ptr[unsafe_offset=index]
        ):
            return False
    return True


def gemini_compact_text_append_string(
    source: ProdexRichStringView,
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Int64:
    if (
        start < 0
        or end <= start + 1
        or deepseek_json_byte(source, start) != 34
        or deepseek_json_byte(source, end - 1) != 34
    ):
        return -1
    var before = written[]
    var source_ptr = rich_view_ptr(source)
    var index = start + 1
    while index < end - 1:
        var value = source_ptr[unsafe_offset=index]
        var codepoint: Int64
        if value == 92:
            if index + 1 >= end - 1:
                return -1
            var escaped = source_ptr[unsafe_offset=index + 1]
            if escaped == 117:
                if index + 5 >= end - 1:
                    return -1
                codepoint = 0
                for offset in range(2, 6):
                    var digit = gemini_compact_text_hex(
                        source_ptr[unsafe_offset=index + Int64(offset)]
                    )
                    if digit < 0:
                        return -1
                    codepoint = codepoint * 16 + digit
                index += 6
                if codepoint >= 55_296 and codepoint <= 56_319:
                    if (
                        index + 5 >= end - 1
                        or source_ptr[unsafe_offset=index] != 92
                        or source_ptr[unsafe_offset=index + 1] != 117
                    ):
                        return -1
                    var low: Int64 = 0
                    for offset in range(2, 6):
                        var digit = gemini_compact_text_hex(
                            source_ptr[unsafe_offset=index + Int64(offset)]
                        )
                        if digit < 0:
                            return -1
                        low = low * 16 + digit
                    if low < 56_320 or low > 57_343:
                        return -1
                    codepoint = 65_536 + ((codepoint - 55_296) << 10) + low - 56_320
                    index += 6
                elif codepoint >= 56_320 and codepoint <= 57_343:
                    return -1
            else:
                if escaped == 34 or escaped == 92 or escaped == 47:
                    codepoint = Int64(escaped)
                elif escaped == 98:
                    codepoint = 8
                elif escaped == 102:
                    codepoint = 12
                elif escaped == 110:
                    codepoint = 10
                elif escaped == 114:
                    codepoint = 13
                elif escaped == 116:
                    codepoint = 9
                else:
                    return -1
                index += 2
        else:
            var width = rich_codepoint_width(value)
            if index + width > end - 1:
                return -1
            codepoint = rich_codepoint(source_ptr, index, width)
            index += width
        if not gemini_compact_text_write_codepoint(
            output, capacity, written, codepoint
        ):
            return -1

    var decoded = ProdexRichStringView(
        UInt(Int(output) + Int(before)), UInt(written[] - before)
    )
    var bounds = rich_trim_bounds(decoded)
    if bounds[0] >= bounds[1]:
        return 2
    return 1


def gemini_compact_text_append_child(
    source: ProdexRichStringView,
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    found: Pointer[mut=True, Bool, _],
    depth: Int64,
) -> Int64:
    var separator = written[]
    if found[]:
        if separator >= capacity:
            return -1
        written[] += 1
    var result = gemini_compact_text_extract_value(
        source, start, end, output, capacity, written, depth
    )
    if result < 0:
        written[] = separator
        return -1
    if result != 1:
        written[] = separator
        return 0
    if found[]:
        var output_ptr = output
        output_ptr[unsafe_offset=separator] = 10
    found[] = True
    return 1


def gemini_compact_text_extract_object(
    source: ProdexRichStringView,
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    depth: Int64,
) -> Int64:
    if deepseek_json_byte(source, start + 1) == 125:
        return 0
    for key in [
        StringSlice("text"),
        StringSlice("output"),
        StringSlice("input"),
        StringSlice("query"),
        StringSlice("command"),
        StringSlice("commands"),
    ]:
        var member = deepseek_json_object_member(source, start, end, key)
        if member[0] < 0:
            continue
        var before = written[]
        var result = gemini_compact_text_extract_value(
            source, member[0], member[1], output, capacity, written, depth + 1
        )
        if result < 0:
            written[] = before
            return -1
        if result == 1:
            return 1
        written[] = before

    var found = False
    var index = deepseek_json_skip_ws(source, start + 1, end)
    while index < end - 1:
        var key_end = deepseek_json_string_end(source, index, end)
        if key_end < 0:
            return -1
        index = deepseek_json_skip_ws(source, key_end, end)
        if index >= end or deepseek_json_byte(source, index) != 58:
            return -1
        var value_start = deepseek_json_skip_ws(source, index + 1, end)
        var value_end = deepseek_json_value_end(
            source, value_start, end, depth + 1
        )
        if value_end < 0:
            return -1
        var result = gemini_compact_text_append_child(
            source,
            value_start,
            value_end,
            output,
            capacity,
            written,
            Pointer(to=found),
            depth + 1,
        )
        if result < 0:
            return -1
        index = deepseek_json_skip_ws(source, value_end, end)
        if index < end and deepseek_json_byte(source, index) == 125:
            return 1 if found else 0
        if index >= end or deepseek_json_byte(source, index) != 44:
            return -1
        index = deepseek_json_skip_ws(source, index + 1, end)
    return -1


def gemini_compact_text_extract_value(
    source: ProdexRichStringView,
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    depth: Int64,
) -> Int64:
    if depth > GEMINI_COMPACT_TEXT_MAX_DEPTH:
        return -1
    var index = deepseek_json_skip_ws(source, start, end)
    if index >= end:
        return -1
    var opening = deepseek_json_byte(source, index)
    if opening == 34:
        var string_end = deepseek_json_string_end(source, index, end)
        if string_end < 0 or string_end > end:
            return -1
        return gemini_compact_text_append_string(
            source, index, string_end, output, capacity, written
        )
    if opening == 91:
        var found = False
        index = deepseek_json_skip_ws(source, index + 1, end)
        if index < end and deepseek_json_byte(source, index) == 93:
            return 0
        while index < end:
            var value_end = deepseek_json_value_end(
                source, index, end, depth + 1
            )
            if value_end < 0:
                return -1
            var result = gemini_compact_text_append_child(
                source,
                index,
                value_end,
                output,
                capacity,
                written,
                Pointer(to=found),
                depth + 1,
            )
            if result < 0:
                return -1
            index = deepseek_json_skip_ws(source, value_end, end)
            if index < end and deepseek_json_byte(source, index) == 93:
                return 1 if found else 0
            if index >= end or deepseek_json_byte(source, index) != 44:
                return -1
            index = deepseek_json_skip_ws(source, index + 1, end)
        return -1
    if opening == 123:
        var object_end = deepseek_json_value_end(source, index, end, depth)
        if object_end < 0 or object_end > end:
            return -1
        return gemini_compact_text_extract_object(
            source, index, object_end, output, capacity, written, depth
        )
    var value_end = deepseek_json_value_end(source, index, end, depth)
    if value_end < 0:
        return -1
    if (
        value_end - index == 4
        and deepseek_json_byte(source, index) == 110
        and deepseek_json_byte(source, index + 1) == 117
        and deepseek_json_byte(source, index + 2) == 108
        and deepseek_json_byte(source, index + 3) == 108
    ):
        return 0
    var before = written[]
    if not gemini_compact_text_append_range(
        source, index, value_end, output, capacity, written
    ):
        written[] = before
        return -1
    return 1


@export("prodex_mojo_gemini_compact_text_v1")
def prodex_mojo_gemini_compact_text_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
    emitted_address: UInt,
) abi("C") -> Int64:
    if abi_version != GEMINI_COMPACT_TEXT_ABI_VERSION:
        return GEMINI_COMPACT_TEXT_STATUS_ABI
    if (
        input_length < 0
        or (input_length > 0 and input_address == 0)
        or output_address == 0
        or output_capacity < 0
        or written_address == 0
        or emitted_address == 0
    ):
        return GEMINI_COMPACT_TEXT_STATUS_INVALID
    var source = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(source, 0x7FFFFFFFFFFFFFFF):
        return GEMINI_COMPACT_TEXT_STATUS_UTF8
    if not deepseek_json_fragment_valid(source):
        return GEMINI_COMPACT_TEXT_STATUS_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var emitted = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(emitted_address)
    )
    written[] = 0
    var result = gemini_compact_text_extract_value(
        source, 0, input_length, output, output_capacity, written, 0
    )
    if result < 0:
        return GEMINI_COMPACT_TEXT_STATUS_CAPACITY
    if result == 0:
        written[] = 0
        emitted[] = 0
    else:
        emitted[] = 1
    return GEMINI_COMPACT_TEXT_STATUS_OK
