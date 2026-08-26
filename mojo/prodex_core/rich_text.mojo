from std.memory import Pointer

from rich_types import (
    ProdexRichSlice,
    ProdexRichStringView,
)


def rich_utf8_continuation(value: UInt8) -> Bool:
    return value >= 0x80 and value <= 0xBF


def rich_utf8_valid(ptr: Pointer[mut=False, UInt8, _], length: Int64) -> Bool:
    var index: Int64 = 0
    while index < length:
        var lead = ptr[unsafe_offset=index]
        if lead <= 0x7F:
            index += 1
        elif lead >= 0xC2 and lead <= 0xDF:
            if index + 1 >= length or not rich_utf8_continuation(ptr[unsafe_offset=index + 1]):
                return False
            index += 2
        elif lead == 0xE0 or lead >= 0xE1 and lead <= 0xEF:
            if index + 2 >= length:
                return False
            var second = ptr[unsafe_offset=index + 1]
            if not rich_utf8_continuation(second) or lead == 0xE0 and second < 0xA0 or lead == 0xED and second > 0x9F or not rich_utf8_continuation(ptr[unsafe_offset=index + 2]):
                return False
            index += 3
        elif lead >= 0xF0 and lead <= 0xF4:
            if index + 3 >= length:
                return False
            var second = ptr[unsafe_offset=index + 1]
            if not rich_utf8_continuation(second) or lead == 0xF0 and second < 0x90 or lead == 0xF4 and second > 0x8F or not rich_utf8_continuation(ptr[unsafe_offset=index + 2]) or not rich_utf8_continuation(ptr[unsafe_offset=index + 3]):
                return False
            index += 4
        else:
            return False
    return True


def rich_view_valid(view: ProdexRichStringView, maximum: Int64) -> Bool:
    if view.len > UInt(maximum):
        return False
    if view.len == 0:
        return True
    if not view.ptr:
        return False
    return rich_utf8_valid(view.ptr.unsafe_value(), Int64(view.len))


def rich_codepoint_width(value: UInt8) -> Int64:
    if value <= 0x7F:
        return 1
    if value <= 0xDF:
        return 2
    if value <= 0xEF:
        return 3
    return 4


def rich_codepoint(
    ptr: Pointer[mut=False, UInt8, _], index: Int64, width: Int64
) -> Int64:
    var lead = Int64(ptr[unsafe_offset=index])
    if width == 1:
        return lead
    if width == 2:
        return ((lead & 0x1F) << 6) | (Int64(ptr[unsafe_offset=index + 1]) & 0x3F)
    if width == 3:
        return ((lead & 0x0F) << 12) | ((Int64(ptr[unsafe_offset=index + 1]) & 0x3F) << 6) | (Int64(ptr[unsafe_offset=index + 2]) & 0x3F)
    return ((lead & 0x07) << 18) | ((Int64(ptr[unsafe_offset=index + 1]) & 0x3F) << 12) | ((Int64(ptr[unsafe_offset=index + 2]) & 0x3F) << 6) | (Int64(ptr[unsafe_offset=index + 3]) & 0x3F)


def rich_unicode_space(codepoint: Int64) -> Bool:
    return codepoint >= 0x1C and codepoint <= 0x1F or codepoint == 9 or codepoint == 10 or codepoint == 11 or codepoint == 12 or codepoint == 13 or codepoint == 32 or codepoint == 0x85 or codepoint == 0xA0 or codepoint == 0x1680 or codepoint >= 0x2000 and codepoint <= 0x200A or codepoint == 0x2028 or codepoint == 0x2029 or codepoint == 0x202F or codepoint == 0x205F or codepoint == 0x3000


def rich_trim_bounds(view: ProdexRichStringView) -> InlineArray[Int64, 2]:
    var bounds = InlineArray[Int64, 2](fill=0)
    var end = Int64(view.len)
    bounds[1] = end
    if end == 0:
        return bounds^
    var ptr = view.ptr.unsafe_value()
    var start: Int64 = 0
    while start < end:
        var width = rich_codepoint_width(ptr[unsafe_offset=start])
        if not rich_unicode_space(rich_codepoint(ptr, start, width)):
            break
        start += width
    while end > start:
        var probe = end - 1
        while probe > start and rich_utf8_continuation(ptr[unsafe_offset=probe]):
            probe -= 1
        var width = end - probe
        if not rich_unicode_space(rich_codepoint(ptr, probe, width)):
            break
        end = probe
    bounds[0] = start
    bounds[1] = end
    return bounds^


def rich_copy_range(
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    lowercase: Bool,
) -> ProdexRichSlice:
    var length = end - start
    var offset = written[]
    if length < 0 or offset < 0 or offset > capacity or length > capacity - offset:
        return ProdexRichSlice(-1, -1)
    for index in range(length):
        var value = ptr[unsafe_offset=start + index]
        if lowercase and value >= 65 and value <= 90:
            value += 32
        output[unsafe_offset=offset + index] = value
    written[] = offset + length
    return ProdexRichSlice(offset, length)


def rich_copy_trimmed(
    view: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    lowercase: Bool,
) -> ProdexRichSlice:
    var bounds = rich_trim_bounds(view)
    return rich_copy_range(view.ptr.unsafe_value(), bounds[0], bounds[1], output, capacity, written, lowercase)


def rich_view_matches_literal[literal: StaticString](
    view: ProdexRichStringView, lowercase: Bool
) -> Bool:
    if view.len != UInt(literal.byte_length()):
        return False
    if literal.byte_length() == 0:
        return True
    if not view.ptr:
        return False
    var right = literal.unsafe_ptr()
    var left = view.ptr.unsafe_value()
    for index in range(Int64(view.len)):
        var value = left[unsafe_offset=index]
        if lowercase and value >= 65 and value <= 90:
            value += 32
        if value != right[unsafe_offset=index]:
            return False
    return True


def rich_view_prefix[literal: StaticString](
    view: ProdexRichStringView, lowercase: Bool
) -> Bool:
    if view.len < UInt(literal.byte_length()) or literal.byte_length() == 0:
        return literal.byte_length() == 0
    if not view.ptr:
        return False
    var right = literal.unsafe_ptr()
    var left = view.ptr.unsafe_value()
    for index in range(Int64(literal.byte_length())):
        var value = left[unsafe_offset=index]
        if lowercase and value >= 65 and value <= 90:
            value += 32
        if value != right[unsafe_offset=index]:
            return False
    return True


def rich_slice_prefix(
    output: Pointer[mut=True, UInt8, _],
    slice: ProdexRichSlice,
    literal: StringSlice,
    lowercase: Bool,
) -> Bool:
    if slice.len < Int64(literal.byte_length()):
        return False
    var right = literal.unsafe_ptr()
    for index in range(Int64(literal.byte_length())):
        var value = output[unsafe_offset=slice.offset + index]
        if lowercase and value >= 65 and value <= 90:
            value += 32
        if value != right[unsafe_offset=index]:
            return False
    return True


def rich_slice_contains(
    output: Pointer[mut=True, UInt8, _],
    slice: ProdexRichSlice,
    literal: StringSlice,
    lowercase: Bool,
) -> Bool:
    var needle = Int64(literal.byte_length())
    if needle == 0:
        return True
    if needle > slice.len:
        return False
    var right = literal.unsafe_ptr()
    for start in range(slice.len - needle + 1):
        var matched = True
        for index in range(needle):
            var value = output[unsafe_offset=slice.offset + start + index]
            if lowercase and value >= 65 and value <= 90:
                value += 32
            if value != right[unsafe_offset=index]:
                matched = False
                break
        if matched:
            return True
    return False


def rich_views_equal(left: ProdexRichStringView, right: ProdexRichStringView) -> Bool:
    if left.len != right.len:
        return False
    if left.len == 0:
        return True
    if not left.ptr or not right.ptr:
        return False
    var left_ptr = left.ptr.unsafe_value()
    var right_ptr = right.ptr.unsafe_value()
    for index in range(Int64(left.len)):
        if left_ptr[unsafe_offset=index] != right_ptr[unsafe_offset=index]:
            return False
    return True


def rich_slice_equal_folded(
    output: Pointer[mut=True, UInt8, _], left: ProdexRichSlice, right: ProdexRichSlice
) -> Bool:
    if left.len != right.len:
        return False
    for index in range(left.len):
        var a = output[unsafe_offset=left.offset + index]
        var b = output[unsafe_offset=right.offset + index]
        if a >= 65 and a <= 90:
            a += 32
        if b >= 65 and b <= 90:
            b += 32
        if a != b:
            return False
    return True


def rich_slice_matches_literal[literal: StaticString](
    output: Pointer[mut=True, UInt8, _],
    slice: ProdexRichSlice,
    lowercase: Bool,
) -> Bool:
    if slice.len != Int64(literal.byte_length()):
        return False
    var right = literal.unsafe_ptr()
    for index in range(slice.len):
        var value = output[unsafe_offset=slice.offset + index]
        var expected = right[unsafe_offset=index]
        if lowercase and value >= 65 and value <= 90:
            value += 32
        if value != expected:
            return False
    return True


def rich_hash_slice(
    output: Pointer[mut=True, UInt8, _], slice: ProdexRichSlice
) -> UInt64:
    var hash: UInt64 = 1469598103934665603 ^ UInt64(slice.len)
    for index in range(slice.len):
        var byte = output[unsafe_offset=slice.offset + index]
        if byte >= 65 and byte <= 90:
            byte += 32
        hash = (hash ^ UInt64(byte)) * 1099511628211
    return hash


def rich_hash_pair(
    output: Pointer[mut=True, UInt8, _],
    provider: ProdexRichSlice,
    model: ProdexRichSlice,
) -> UInt64:
    var value = rich_hash_slice(output, provider) ^ (rich_hash_slice(output, model) << 1)
    return value ^ UInt64(provider.len * 17 + model.len)


def rich_valid_identifier(view: ProdexRichStringView) -> Bool:
    if view.len == 0 or not view.ptr:
        return False
    var ptr = view.ptr.unsafe_value()
    var index: Int64 = 0
    while index < Int64(view.len):
        var width = rich_codepoint_width(ptr[unsafe_offset=index])
        if rich_unicode_space(rich_codepoint(ptr, index, width)):
            return False
        index += width
    return True


def rich_capability_token[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    if end - start != Int64(literal.byte_length()):
        return False
    var right = literal.unsafe_ptr()
    for index in range(end - start):
        var value = ptr[unsafe_offset=start + index]
        if value >= 65 and value <= 90:
            value += 32
        if value != right[unsafe_offset=index]:
            return False
    return True


def rich_capability_mask(view: ProdexRichStringView) -> Int64:
    if view.len == 0 or not view.ptr:
        return 0
    var ptr = view.ptr.unsafe_value()
    var length = Int64(view.len)
    var mask: Int64 = 0
    var start: Int64 = 0
    while start < length:
        while start < length and (ptr[unsafe_offset=start] == 32 or ptr[unsafe_offset=start] == 9 or ptr[unsafe_offset=start] == 44 or ptr[unsafe_offset=start] == 59 or ptr[unsafe_offset=start] == 124):
            start += 1
        var end = start
        while end < length and not (ptr[unsafe_offset=end] == 32 or ptr[unsafe_offset=end] == 9 or ptr[unsafe_offset=end] == 44 or ptr[unsafe_offset=end] == 59 or ptr[unsafe_offset=end] == 124):
            end += 1
        if rich_capability_token["responses_api"](ptr, start, end) or rich_capability_token["responses"](ptr, start, end):
            mask |= 1
        elif rich_capability_token["streaming"](ptr, start, end):
            mask |= 2
        elif rich_capability_token["tools"](ptr, start, end):
            mask |= 4
        elif rich_capability_token["vision"](ptr, start, end):
            mask |= 8
        elif rich_capability_token["json_mode"](ptr, start, end) or rich_capability_token["json"](ptr, start, end):
            mask |= 16
        elif rich_capability_token["remote_compact"](ptr, start, end) or rich_capability_token["compact"](ptr, start, end):
            mask |= 32
        elif rich_capability_token["websocket"](ptr, start, end):
            mask |= 64
        start = end + 1
    return mask


def rich_required_hash_capacity(count: Int64) -> Int64:
    var capacity: Int64 = 1
    var target = count * 2
    while capacity < target:
        capacity *= 2
    return capacity
