from rich_text import rich_view_ptr
from rich_types import ProdexRichStringView

comptime JSON_VIEW_MAX_DEPTH: Int64 = 256

def deepseek_json_byte(view: ProdexRichStringView, index: Int64) -> UInt8:
    return rich_view_ptr(view)[unsafe_offset=index]


def deepseek_json_skip_ws(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Int64:
    var index = start
    while index < end:
        var value = deepseek_json_byte(view, index)
        if value != 9 and value != 10 and value != 13 and value != 32:
            break
        index += 1
    return index


def deepseek_json_string_end(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Int64:
    if start < 0 or start >= end or deepseek_json_byte(view, start) != 34:
        return -1
    var index = start + 1
    while index < end:
        var value = deepseek_json_byte(view, index)
        if value == 92:
            if index + 1 >= end:
                return -1
            index += 2
        elif value == 34:
            return index + 1
        elif value < 32:
            return -1
        else:
            index += 1
    return -1


def deepseek_json_value_end(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    depth: Int64,
) -> Int64:
    if depth > JSON_VIEW_MAX_DEPTH:
        return -1
    var index = deepseek_json_skip_ws(view, start, end)
    if index >= end:
        return -1
    var opening = deepseek_json_byte(view, index)
    if opening == 34:
        return deepseek_json_string_end(view, index, end)
    if opening == 91:
        index += 1
        index = deepseek_json_skip_ws(view, index, end)
        if index < end and deepseek_json_byte(view, index) == 93:
            return index + 1
        while index < end:
            var value_end = deepseek_json_value_end(view, index, end, depth + 1)
            if value_end < 0:
                return -1
            index = deepseek_json_skip_ws(view, value_end, end)
            if index < end and deepseek_json_byte(view, index) == 44:
                index = deepseek_json_skip_ws(view, index + 1, end)
                continue
            if index < end and deepseek_json_byte(view, index) == 93:
                return index + 1
            return -1
        return -1
    if opening == 123:
        index += 1
        index = deepseek_json_skip_ws(view, index, end)
        if index < end and deepseek_json_byte(view, index) == 125:
            return index + 1
        while index < end:
            var key_end = deepseek_json_string_end(view, index, end)
            if key_end < 0:
                return -1
            index = deepseek_json_skip_ws(view, key_end, end)
            if index >= end or deepseek_json_byte(view, index) != 58:
                return -1
            index = deepseek_json_skip_ws(view, index + 1, end)
            var value_end = deepseek_json_value_end(view, index, end, depth + 1)
            if value_end < 0:
                return -1
            index = deepseek_json_skip_ws(view, value_end, end)
            if index < end and deepseek_json_byte(view, index) == 44:
                index = deepseek_json_skip_ws(view, index + 1, end)
                continue
            if index < end and deepseek_json_byte(view, index) == 125:
                return index + 1
            return -1
        return -1
    var primitive_start = index
    while index < end:
        var value = deepseek_json_byte(view, index)
        if value == 9 or value == 10 or value == 13 or value == 32 or value == 44 or value == 93 or value == 125:
            break
        index += 1
    if index == primitive_start:
        return -1
    return index


def deepseek_json_object_member(
    view: ProdexRichStringView,
    object_start: Int64,
    object_end: Int64,
    key: StringSlice,
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=-1)
    if object_start < 0 or object_end > Int64(view.len) or object_end <= object_start + 1:
        return result^
    if deepseek_json_byte(view, object_start) != 123 or deepseek_json_byte(view, object_end - 1) != 125:
        return result^
    var index = deepseek_json_skip_ws(view, object_start + 1, object_end - 1)
    while index < object_end - 1:
        var key_start = index
        var key_end = deepseek_json_string_end(view, key_start, object_end - 1)
        if key_end < 0:
            return InlineArray[Int64, 2](fill=-1)^
        index = deepseek_json_skip_ws(view, key_end, object_end - 1)
        if index >= object_end - 1 or deepseek_json_byte(view, index) != 58:
            return InlineArray[Int64, 2](fill=-1)^
        var value_start = deepseek_json_skip_ws(view, index + 1, object_end - 1)
        var value_end = deepseek_json_value_end(view, value_start, object_end - 1, 0)
        if value_end < 0:
            return InlineArray[Int64, 2](fill=-1)^
        if deepseek_json_raw_equals(view, key_start, key_end, key):
            result[0] = value_start
            result[1] = value_end
        index = deepseek_json_skip_ws(view, value_end, object_end - 1)
        if index < object_end - 1 and deepseek_json_byte(view, index) == 44:
            index = deepseek_json_skip_ws(view, index + 1, object_end - 1)
            continue
        if index == object_end - 1:
            break
        return InlineArray[Int64, 2](fill=-1)^
    return result^


def deepseek_json_raw_equals(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    if start < 0 or end < start + 2:
        return False
    if deepseek_json_byte(view, start) != 34 or deepseek_json_byte(view, end - 1) != 34:
        return False
    var expected_length = Int64(literal.byte_length())
    if end - start - 2 != expected_length:
        return False
    var expected = literal.unsafe_ptr()
    var actual = rich_view_ptr(view)
    for index in range(expected_length):
        if actual[unsafe_offset=start + 1 + index] != expected[unsafe_offset=index]:
            return False
    return True


def deepseek_json_fragment_valid(view: ProdexRichStringView) -> Bool:
    if view.len == 0:
        return False
    var start = deepseek_json_skip_ws(view, 0, Int64(view.len))
    var value_end = deepseek_json_value_end(view, start, Int64(view.len), 0)
    return value_end >= 0 and deepseek_json_skip_ws(view, value_end, Int64(view.len)) == Int64(view.len)

