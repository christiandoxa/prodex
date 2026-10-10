from std.collections import Array
from std.memory import Pointer

from json_view import (
    deepseek_json_byte,
    deepseek_json_fragment_valid,
    deepseek_json_object_member,
    deepseek_json_raw_equals,
    deepseek_json_skip_ws,
    deepseek_json_value_end,
)
from launch_args_common import launch_rust_space
from parsed_json import pj_equal, pj_less
from rich_types import ProdexRichStringView
from rich_text import (
    rich_codepoint,
    rich_codepoint_width,
    rich_view_ptr,
    rich_view_valid,
)

comptime SESSION_REPORT_ABI_VERSION: Int64 = 1
comptime SESSION_REPORT_OK: Int64 = 0
comptime SESSION_REPORT_INVALID: Int64 = 1
comptime SESSION_REPORT_CAPACITY: Int64 = 3

def session_raw_present(bounds: Array[Int64, 2]) -> Bool:
    return bounds[0] >= 0 and bounds[1] > bounds[0]


def session_raw_root(view: ProdexRichStringView) -> Array[Int64, 2]:
    var result = Array[Int64, 2](fill=-1)
    var start = deepseek_json_skip_ws(view, 0, Int64(view.len))
    var end = deepseek_json_value_end(view, start, Int64(view.len), 0)
    if (
        end < 0
        or deepseek_json_skip_ws(view, end, Int64(view.len)) != Int64(view.len)
        or start >= Int64(view.len)
        or deepseek_json_byte(view, start) != 123
    ):
        return result^
    result[0] = start
    result[1] = end
    return result^


def session_raw_object_field(
    view: ProdexRichStringView,
    object: Array[Int64, 2],
    name: StringSlice,
) -> Array[Int64, 2]:
    if not session_raw_present(object):
        return Array[Int64, 2](fill=-1)^
    var field = deepseek_json_object_member(
        view, object[0], object[1], name
    )
    if (
        not session_raw_present(field)
        or deepseek_json_byte(view, field[0]) != 123
    ):
        return Array[Int64, 2](fill=-1)^
    return field^


def session_json_hex(value: UInt8) -> Int64:
    if value >= 48 and value <= 57:
        return Int64(value - 48)
    if value >= 65 and value <= 70:
        return Int64(value - 65 + 10)
    if value >= 97 and value <= 102:
        return Int64(value - 97 + 10)
    return -1


def session_raw_string_nonblank(
    view: ProdexRichStringView,
    bounds: Array[Int64, 2],
) -> Bool:
    if (
        not session_raw_present(bounds)
        or deepseek_json_byte(view, bounds[0]) != 34
        or deepseek_json_byte(view, bounds[1] - 1) != 34
    ):
        return False
    var ptr = rich_view_ptr(view)
    var index = bounds[0] + 1
    var end = bounds[1] - 1
    while index < end:
        var value = ptr[unsafe_offset=index]
        var codepoint: Int64
        if value == 92:
            if index + 1 >= end:
                return False
            var escaped = ptr[unsafe_offset=index + 1]
            if escaped == 116:
                codepoint = 9
                index += 2
            elif escaped == 110:
                codepoint = 10
                index += 2
            elif escaped == 114:
                codepoint = 13
                index += 2
            elif escaped == 102:
                codepoint = 12
                index += 2
            elif escaped == 98:
                codepoint = 8
                index += 2
            elif escaped == 117:
                if index + 5 >= end:
                    return False
                codepoint = 0
                for offset in range(2, 6):
                    var digit = session_json_hex(
                        ptr[unsafe_offset=index + Int64(offset)]
                    )
                    if digit < 0:
                        return False
                    codepoint = codepoint * 16 + digit
                index += 6
            elif escaped == 34 or escaped == 92 or escaped == 47:
                codepoint = Int64(escaped)
                index += 2
            else:
                return False
        else:
            var width = rich_codepoint_width(value)
            if index + width > end:
                return False
            codepoint = rich_codepoint(ptr, index, width)
            index += width
        if not launch_rust_space(codepoint):
            return True
    return False


def session_raw_string_token(
    view: ProdexRichStringView,
    bounds: Array[Int64, 2],
) -> Bool:
    return (
        session_raw_present(bounds)
        and deepseek_json_byte(view, bounds[0]) == 34
        and deepseek_json_byte(view, bounds[1] - 1) == 34
    )


def session_raw_string_equals_literal(
    view: ProdexRichStringView,
    bounds: Array[Int64, 2],
    literal: StringSlice,
) -> Bool:
    if not session_raw_string_token(view, bounds):
        return False
    var source = rich_view_ptr(view)
    var expected = literal.unsafe_ptr()
    var expected_length = Int64(literal.byte_length())
    var expected_index: Int64 = 0
    var index = bounds[0] + 1
    var end = bounds[1] - 1
    while index < end:
        if expected_index >= expected_length:
            return False
        var value = source[unsafe_offset=index]
        var decoded: Int64
        if value == 92:
            if index + 1 >= end:
                return False
            var escaped = source[unsafe_offset=index + 1]
            if escaped == 117:
                if index + 5 >= end:
                    return False
                decoded = 0
                for offset in range(2, 6):
                    var digit = session_json_hex(
                        source[unsafe_offset=index + Int64(offset)]
                    )
                    if digit < 0:
                        return False
                    decoded = decoded * 16 + digit
                index += 6
                if decoded >= 55296 and decoded <= 57343:
                    return False
            else:
                if escaped == 34 or escaped == 92 or escaped == 47:
                    decoded = Int64(escaped)
                elif escaped == 98:
                    decoded = 8
                elif escaped == 102:
                    decoded = 12
                elif escaped == 110:
                    decoded = 10
                elif escaped == 114:
                    decoded = 13
                elif escaped == 116:
                    decoded = 9
                else:
                    return False
                index += 2
        else:
            if value >= 128:
                return False
            decoded = Int64(value)
            index += 1
        if decoded != Int64(expected[unsafe_offset=expected_index]):
            return False
        expected_index += 1
    return expected_index == expected_length


def session_raw_string_field(
    view: ProdexRichStringView,
    object: Array[Int64, 2],
    name: StringSlice,
) -> Array[Int64, 2]:
    if not session_raw_present(object):
        return Array[Int64, 2](fill=-1)^
    var field = deepseek_json_object_member(
        view, object[0], object[1], name
    )
    if not session_raw_string_nonblank(view, field):
        return Array[Int64, 2](fill=-1)^
    return field^


def session_raw_nested_string5(
    view: ProdexRichStringView,
    object: Array[Int64, 2],
    first: StringSlice,
    second: StringSlice,
    third: StringSlice,
    fourth: StringSlice,
    name: StringSlice,
) -> Array[Int64, 2]:
    var one = session_raw_object_field(view, object, first)
    if not session_raw_present(one):
        return Array[Int64, 2](fill=-1)^
    var two = session_raw_object_field(view, one, second)
    if not session_raw_present(two):
        return Array[Int64, 2](fill=-1)^
    var three = session_raw_object_field(view, two, third)
    if not session_raw_present(three):
        return Array[Int64, 2](fill=-1)^
    var four = session_raw_object_field(view, three, fourth)
    if not session_raw_present(four):
        return Array[Int64, 2](fill=-1)^
    return session_raw_string_field(view, four, name)


def session_raw_write_span(
    output: Pointer[mut=True, Int64, _],
    offset: Int64,
    bounds: Array[Int64, 2],
):
    if session_raw_present(bounds):
        output[unsafe_offset=offset] = bounds[0]
        output[unsafe_offset=offset + 1] = bounds[1]


def session_digit(value: ProdexRichStringView, index: Int64) -> Int64:
    if index < 0 or index >= Int64(value.len):
        return -1
    var byte = rich_view_ptr(value)[unsafe_offset=index]
    return Int64(byte - UInt8(48)) if byte >= 48 and byte <= 57 else -1

def session_pair(value: ProdexRichStringView, index: Int64) -> Int64:
    var first = session_digit(value, index)
    var second = session_digit(value, index + 1)
    return first * 10 + second if first >= 0 and second >= 0 else -1

def session_parse_i64(value: ProdexRichStringView) -> Tuple[Bool, Int64]:
    var length = Int64(value.len)
    if length == 0:
        return False, 0
    var ptr = rich_view_ptr(value)
    var index: Int64 = 0
    var negative = False
    if ptr[unsafe_offset=0] == 45 or ptr[unsafe_offset=0] == 43:
        negative = ptr[unsafe_offset=0] == 45
        index = 1
    if index == length:
        return False, 0

    var limit: UInt64 = 9223372036854775807
    if negative:
        limit = 9223372036854775808
    var magnitude: UInt64 = 0
    while index < length:
        var digit = session_digit(value, index)
        if digit < 0 or magnitude > (limit - UInt64(digit)) // 10:
            return False, 0
        magnitude = magnitude * 10 + UInt64(digit)
        index += 1
    if negative:
        if magnitude == 9223372036854775808:
            return True, -9223372036854775807 - 1
        return True, -Int64(magnitude)
    return True, Int64(magnitude)

def session_rfc3339_sort_key(value: ProdexRichStringView) -> Tuple[Bool, Int64]:
    var length = Int64(value.len)
    if length < 20:
        return False, 0
    var ptr = rich_view_ptr(value)
    var year: Int64 = 0
    for index in range(4):
        var digit = session_digit(value, Int64(index))
        if digit < 0:
            return False, 0
        year = year * 10 + digit
    if ptr[unsafe_offset=4] != 45 or ptr[unsafe_offset=7] != 45:
        return False, 0
    var month = session_pair(value, 5)
    var day = session_pair(value, 8)
    if month < 1 or month > 12 or day < 1:
        return False, 0
    var month_days: Int64 = 31
    if month == 4 or month == 6 or month == 9 or month == 11:
        month_days = 30
    elif month == 2:
        month_days = 28
        if year % 4 == 0 and (year % 100 != 0 or year % 400 == 0):
            month_days = 29
    if day > month_days:
        return False, 0
    if ptr[unsafe_offset=10] != 84 and ptr[unsafe_offset=10] != 116 and ptr[unsafe_offset=10] != 32:
        return False, 0
    var hour = session_pair(value, 11)
    var minute = session_pair(value, 14)
    var second = session_pair(value, 17)
    if (
        ptr[unsafe_offset=13] != 58
        or ptr[unsafe_offset=16] != 58
        or hour < 0
        or hour > 23
        or minute < 0
        or minute > 59
        or second < 0
        or second > 60
    ):
        return False, 0

    var zone_index: Int64 = 19
    if ptr[unsafe_offset=zone_index] == 46:
        zone_index += 1
        var fraction_start = zone_index
        while zone_index < length and session_digit(value, zone_index) >= 0:
            zone_index += 1
        if zone_index == fraction_start:
            return False, 0

    var offset_seconds: Int64 = 0
    if zone_index < length and (ptr[unsafe_offset=zone_index] == 90 or ptr[unsafe_offset=zone_index] == 122):
        if zone_index + 1 != length:
            return False, 0
    elif zone_index < length and (ptr[unsafe_offset=zone_index] == 43 or ptr[unsafe_offset=zone_index] == 45):
        if zone_index + 6 != length or ptr[unsafe_offset=zone_index + 3] != 58:
            return False, 0
        var offset_hour = session_pair(value, zone_index + 1)
        var offset_minute = session_pair(value, zone_index + 4)
        if offset_hour < 0 or offset_hour > 23 or offset_minute < 0 or offset_minute > 59:
            return False, 0
        offset_seconds = offset_hour * 3600 + offset_minute * 60
        if ptr[unsafe_offset=zone_index] == 45:
            offset_seconds = -offset_seconds
    else:
        return False, 0

    var adjusted_year = year + 4800
    var month_adjustment: Int64 = 0
    if month <= 2:
        adjusted_year -= 1
        month_adjustment = 12
    var adjusted_month = month + month_adjustment - 3
    var julian_day = (
        day
        + (153 * adjusted_month + 2) // 5
        + 365 * adjusted_year
        + adjusted_year // 4
        - adjusted_year // 100
        + adjusted_year // 400
        - 32045
    )
    var days_since_epoch = julian_day - 2440588
    var normalized_second = second if second < 60 else 59
    var local_seconds = (
        days_since_epoch * 86400 + hour * 3600 + minute * 60 + normalized_second
    )
    return True, local_seconds - offset_seconds

def session_timestamp_sort_key(value: ProdexRichStringView) -> Tuple[Bool, Int64]:
    var parsed = session_rfc3339_sort_key(value)
    if parsed[0]:
        return parsed
    return session_parse_i64(value)

def session_raw_numeric_field(
    view: ProdexRichStringView,
    object: Array[Int64, 2],
    name: StringSlice,
) -> Tuple[Bool, Int64]:
    if not session_raw_present(object):
        return False, 0
    var field = deepseek_json_object_member(
        view, object[0], object[1], name
    )
    if not session_raw_present(field):
        return False, 0
    var token = ProdexRichStringView(
        view.ptr + UInt(field[0]), UInt(field[1] - field[0])
    )
    return session_parse_i64(token)

@fieldwise_init
struct SessionReportSortKey(Copyable):
    var updated_sort_key: Int64
    var id: ProdexRichStringView
    var path: ProdexRichStringView

def session_report_before(
    keys: Pointer[mut=False, SessionReportSortKey, ImmUntrackedOrigin],
    left: Int64,
    right: Int64,
) -> Bool:
    var left_key = keys[unsafe_offset=left].updated_sort_key
    var right_key = keys[unsafe_offset=right].updated_sort_key
    if left_key != right_key:
        return left_key > right_key
    var left_id = keys[unsafe_offset=left].id.copy()
    var right_id = keys[unsafe_offset=right].id.copy()
    if not pj_equal(left_id, right_id):
        return pj_less(left_id, right_id)
    var left_path = keys[unsafe_offset=left].path.copy()
    var right_path = keys[unsafe_offset=right].path.copy()
    if not pj_equal(left_path, right_path):
        return pj_less(left_path, right_path)
    return left < right

def session_report_swap(
    indices: Pointer[mut=True, Int64, MutUntrackedOrigin],
    left: Int64,
    right: Int64,
) -> None:
    var selected = indices[unsafe_offset=left]
    indices[unsafe_offset=left] = indices[unsafe_offset=right]
    indices[unsafe_offset=right] = selected

def session_report_sift_down(
    keys: Pointer[mut=False, SessionReportSortKey, ImmUntrackedOrigin],
    indices: Pointer[mut=True, Int64, MutUntrackedOrigin],
    root_index: Int64,
    end: Int64,
) -> None:
    var root = root_index
    while True:
        var child = root * 2 + 1
        if child > end:
            break
        if child + 1 <= end and session_report_before(
            keys,
            indices[unsafe_offset=child],
            indices[unsafe_offset=child + 1],
        ):
            child += 1
        if not session_report_before(
            keys,
            indices[unsafe_offset=root],
            indices[unsafe_offset=child],
        ):
            break
        session_report_swap(indices, root, child)
        root = child

def session_report_heap_sort(
    keys: Pointer[mut=False, SessionReportSortKey, ImmUntrackedOrigin],
    indices: Pointer[mut=True, Int64, MutUntrackedOrigin],
    count: Int64,
) -> None:
    var start = count // 2
    while start > 0:
        start -= 1
        session_report_sift_down(keys, indices, start, count - 1)
    var end = count
    while end > 1:
        end -= 1
        session_report_swap(indices, 0, end)
        session_report_sift_down(keys, indices, 0, end - 1)

def session_report_raw_metadata(
    view: ProdexRichStringView,
    root: Array[Int64, 2],
    output: Pointer[mut=True, Int64, _],
):
    var payload = session_raw_object_field(
        view, root, StringSlice("payload")
    )
    var metadata = session_raw_object_field(
        view, root, StringSlice("metadata")
    )
    var payload_metadata = session_raw_object_field(
        view, payload, StringSlice("metadata")
    )

    var type_token = deepseek_json_object_member(
        view, root[0], root[1], StringSlice("type")
    )
    var type_class: Int64 = 0
    if session_raw_string_token(view, type_token):
        if deepseek_json_raw_equals(
            view, type_token[0], type_token[1], StringSlice("session_meta")
        ):
            type_class = 1
        elif deepseek_json_raw_equals(
            view, type_token[0], type_token[1], StringSlice("turn_context")
        ):
            type_class = 2
        else:
            type_class = 3
    output[unsafe_offset=0] = type_class

    var resume = session_raw_string_field(
        view, payload, StringSlice("id")
    )
    if not session_raw_present(resume):
        resume = session_raw_string_field(
            view, payload, StringSlice("session_id")
        )
    if not session_raw_present(resume):
        resume = session_raw_string_field(
            view, root, StringSlice("id")
        )
    if not session_raw_present(resume):
        resume = session_raw_string_field(
            view, root, StringSlice("session_id")
        )
    session_raw_write_span(output, 1, resume)

    if type_class == 2:
        var model = session_raw_string_field(
            view, payload, StringSlice("model")
        )
        if not session_raw_present(model):
            model = session_raw_string_field(
                view, root, StringSlice("model")
            )
        session_raw_write_span(output, 3, model)

        var effort = session_raw_string_field(
            view, payload, StringSlice("effort")
        )
        if not session_raw_present(effort):
            effort = session_raw_string_field(
                view, payload, StringSlice("reasoning_effort")
            )
        if not session_raw_present(effort):
            effort = session_raw_string_field(
                view, root, StringSlice("effort")
            )
        if not session_raw_present(effort):
            effort = session_raw_string_field(
                view, root, StringSlice("reasoning_effort")
            )
        session_raw_write_span(output, 5, effort)

    var thread_name = session_raw_string_field(
        view, payload, StringSlice("thread_name")
    )
    if not session_raw_present(thread_name):
        thread_name = session_raw_string_field(
            view, payload, StringSlice("title")
        )
    if not session_raw_present(thread_name):
        thread_name = session_raw_string_field(
            view, payload_metadata, StringSlice("thread_name")
        )
    if not session_raw_present(thread_name):
        thread_name = session_raw_string_field(
            view, root, StringSlice("thread_name")
        )
    if not session_raw_present(thread_name):
        thread_name = session_raw_string_field(
            view, root, StringSlice("title")
        )
    if not session_raw_present(thread_name):
        thread_name = session_raw_string_field(
            view, metadata, StringSlice("thread_name")
        )
    session_raw_write_span(output, 7, thread_name)

    var cwd = session_raw_string_field(
        view, payload, StringSlice("cwd")
    )
    if not session_raw_present(cwd):
        cwd = session_raw_string_field(
            view, payload_metadata, StringSlice("cwd")
        )
    if not session_raw_present(cwd):
        cwd = session_raw_string_field(
            view, payload, StringSlice("workdir")
        )
    if not session_raw_present(cwd):
        cwd = session_raw_string_field(
            view, root, StringSlice("cwd")
        )
    if not session_raw_present(cwd):
        cwd = session_raw_string_field(
            view, metadata, StringSlice("cwd")
        )
    if not session_raw_present(cwd):
        cwd = session_raw_string_field(
            view, root, StringSlice("workdir")
        )
    session_raw_write_span(output, 9, cwd)

    var updated = session_raw_string_field(
        view, root, StringSlice("updated_at")
    )
    if not session_raw_present(updated):
        updated = session_raw_string_field(
            view, root, StringSlice("timestamp")
        )
    if not session_raw_present(updated):
        updated = session_raw_string_field(
            view, payload, StringSlice("updated_at")
        )
    if not session_raw_present(updated):
        updated = session_raw_string_field(
            view, payload, StringSlice("timestamp")
        )
    session_raw_write_span(output, 11, updated)

    var parent = session_raw_nested_string5(
        view,
        root,
        StringSlice("payload"),
        StringSlice("source"),
        StringSlice("subagent"),
        StringSlice("thread_spawn"),
        StringSlice("parent_thread_id"),
    )
    if not session_raw_present(parent):
        var source = session_raw_object_field(
            view, root, StringSlice("source")
        )
        var subagent = session_raw_object_field(
            view, source, StringSlice("subagent")
        )
        var spawn = session_raw_object_field(
            view, subagent, StringSlice("thread_spawn")
        )
        parent = session_raw_string_field(
            view, spawn, StringSlice("parent_thread_id")
        )
    if not session_raw_present(parent):
        parent = session_raw_string_field(
            view, payload, StringSlice("parent_thread_id")
        )
    if not session_raw_present(parent):
        parent = session_raw_string_field(
            view, root, StringSlice("parent_thread_id")
        )
    session_raw_write_span(output, 13, parent)

    var provider = session_raw_string_field(
        view, payload, StringSlice("model_provider")
    )
    if not session_raw_present(provider):
        provider = session_raw_string_field(
            view, payload_metadata, StringSlice("model_provider")
        )
    if not session_raw_present(provider):
        provider = session_raw_string_field(
            view, root, StringSlice("model_provider")
        )
    if not session_raw_present(provider):
        provider = session_raw_string_field(
            view, metadata, StringSlice("model_provider")
        )
    session_raw_write_span(output, 15, provider)


@export("prodex_session_report_update_json_v2")
def prodex_session_report_update_json_v2(
    abi_version: Int64,
    raw_address: UInt,
    raw_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != 2
        or raw_length <= 0
        or raw_address == 0
        or output_address == 0
    ):
        return SESSION_REPORT_INVALID

    var view = ProdexRichStringView(raw_address, UInt(raw_length))
    if (
        not rich_view_valid(view, 0x7FFFFFFFFFFFFFFF)
        or not deepseek_json_fragment_valid(view)
    ):
        return SESSION_REPORT_INVALID
    var root = session_raw_root(view)
    if not session_raw_present(root):
        return SESSION_REPORT_INVALID

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(28):
        output[unsafe_offset=index] = -1
    output[unsafe_offset=0] = 0
    output[unsafe_offset=19] = 0
    output[unsafe_offset=20] = 0

    session_report_raw_metadata(view, root, output)

    if (
        (output[unsafe_offset=0] == 0 or output[unsafe_offset=0] == 1)
        and output[unsafe_offset=1] >= 0
    ):
        output[unsafe_offset=17] = output[unsafe_offset=1]
        output[unsafe_offset=18] = output[unsafe_offset=2]

    if output[unsafe_offset=11] < 0:
        var payload = session_raw_object_field(
            view, root, StringSlice("payload")
        )
        var parsed = session_raw_numeric_field(
            view, root, StringSlice("updated_at")
        )
        if not parsed[0]:
            parsed = session_raw_numeric_field(
                view, root, StringSlice("ts")
            )
        if not parsed[0]:
            parsed = session_raw_numeric_field(
                view, root, StringSlice("timestamp")
            )
        if not parsed[0]:
            parsed = session_raw_numeric_field(
                view, payload, StringSlice("updated_at")
            )
        if not parsed[0]:
            parsed = session_raw_numeric_field(
                view, payload, StringSlice("ts")
            )
        if not parsed[0]:
            parsed = session_raw_numeric_field(
                view, payload, StringSlice("timestamp")
            )
        if parsed[0]:
            output[unsafe_offset=19] = 1
            output[unsafe_offset=20] = parsed[1]
    session_meta_repair_fields(view, root, output)
    return SESSION_REPORT_OK


def session_meta_repair_fields(
    view: ProdexRichStringView,
    root: Array[Int64, 2],
    output: Pointer[mut=True, Int64, _],
):
    var payload = session_raw_object_field(
        view, root, StringSlice("payload")
    )
    var root_timestamp_token = deepseek_json_object_member(
        view, root[0], root[1], StringSlice("timestamp")
    )
    var root_type_token = deepseek_json_object_member(
        view, root[0], root[1], StringSlice("type")
    )
    var structural = (
        session_raw_string_token(view, root_timestamp_token)
        and session_raw_string_equals_literal(
            view, root_type_token, StringSlice("session_meta")
        )
        and session_raw_present(payload)
    )
    if structural:
        for name_index in range(5):
            var field = Array[Int64, 2](fill=-1)
            if name_index == 0:
                field = deepseek_json_object_member(
                    view, payload[0], payload[1], StringSlice("id")
                )
            elif name_index == 1:
                field = deepseek_json_object_member(
                    view, payload[0], payload[1], StringSlice("timestamp")
                )
            elif name_index == 2:
                field = deepseek_json_object_member(
                    view, payload[0], payload[1], StringSlice("cwd")
                )
            elif name_index == 3:
                field = deepseek_json_object_member(
                    view, payload[0], payload[1], StringSlice("originator")
                )
            else:
                field = deepseek_json_object_member(
                    view, payload[0], payload[1], StringSlice("cli_version")
                )
            if not session_raw_string_token(view, field):
                structural = False
                break
    output[unsafe_offset=21] = Int64(structural)

    var repair_timestamp = session_raw_string_field(
        view, root, StringSlice("timestamp")
    )
    if not session_raw_present(repair_timestamp):
        repair_timestamp = session_raw_string_field(
            view, payload, StringSlice("timestamp")
        )
    session_raw_write_span(output, 22, repair_timestamp)

    var repair_cwd = session_raw_string_field(
        view, payload, StringSlice("cwd")
    )
    if not session_raw_present(repair_cwd):
        repair_cwd = session_raw_string_field(
            view, root, StringSlice("cwd")
        )
    session_raw_write_span(output, 24, repair_cwd)

    var repair_provider = session_raw_string_field(
        view, payload, StringSlice("model_provider")
    )
    if not session_raw_present(repair_provider):
        repair_provider = session_raw_string_field(
            view, root, StringSlice("model_provider")
        )
    session_raw_write_span(output, 26, repair_provider)


@export("prodex_session_report_record_shape_v1")
def prodex_session_report_record_shape_v1(
    abi_version: Int64,
    raw_address: UInt,
    raw_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != SESSION_REPORT_ABI_VERSION
        or raw_length <= 0
        or raw_address == 0
        or output_address == 0
    ):
        return SESSION_REPORT_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = 0
    var view = ProdexRichStringView(raw_address, UInt(raw_length))
    if not rich_view_valid(view, 0x7FFFFFFFFFFFFFFF):
        return SESSION_REPORT_INVALID
    if not deepseek_json_fragment_valid(view):
        return SESSION_REPORT_OK
    var root = session_raw_root(view)
    if not session_raw_present(root):
        return SESSION_REPORT_OK
    var type_token = deepseek_json_object_member(
        view, root[0], root[1], StringSlice("type")
    )
    var payload = deepseek_json_object_member(
        view, root[0], root[1], StringSlice("payload")
    )
    if not session_raw_string_token(view, type_token) or not session_raw_present(payload):
        return SESSION_REPORT_OK
    if deepseek_json_raw_equals(
        view, type_token[0], type_token[1], StringSlice("event_msg")
    ) or deepseek_json_raw_equals(
        view, type_token[0], type_token[1], StringSlice("response_item")
    ):
        if deepseek_json_byte(view, payload[0]) != 123:
            return SESSION_REPORT_OK
        var payload_type = deepseek_json_object_member(
            view, payload[0], payload[1], StringSlice("type")
        )
        output[] = Int64(session_raw_string_token(view, payload_type))
        return SESSION_REPORT_OK
    if deepseek_json_raw_equals(
        view, type_token[0], type_token[1], StringSlice("session_meta")
    ) or deepseek_json_raw_equals(
        view, type_token[0], type_token[1], StringSlice("turn_context")
    ):
        output[] = Int64(deepseek_json_byte(view, payload[0]) == 123)
        return SESSION_REPORT_OK
    output[] = 1
    return SESSION_REPORT_OK


@export("prodex_session_report_sort_v1")
def prodex_session_report_sort_v1(
    keys_address: UInt,
    keys_count: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if keys_count < 0 or keys_count > 0x7FFFFFFFFFFFFFFF // 40:
        return SESSION_REPORT_INVALID
    if keys_count == 0:
        return SESSION_REPORT_OK
    if keys_address == 0 or output_address == 0:
        return SESSION_REPORT_INVALID
    var keys = Pointer[mut=False, SessionReportSortKey, ImmUntrackedOrigin](
        unsafe_from_address=Int(keys_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(keys_count):
        var key = keys[unsafe_offset=index].copy()
        if not rich_view_valid(key.id, 0x7FFFFFFFFFFFFFFF) or not rich_view_valid(
            key.path, 0x7FFFFFFFFFFFFFFF
        ):
            return SESSION_REPORT_INVALID
        output[unsafe_offset=index] = index
    session_report_heap_sort(keys, output, keys_count)
    return SESSION_REPORT_OK

@export("prodex_session_report_timestamp_sort_key_v1")
def prodex_session_report_timestamp_sort_key_v1(
    abi_version: Int64,
    value_address: UInt,
    value_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != SESSION_REPORT_ABI_VERSION
        or value_length < 0
        or output_address == 0
        or (value_length > 0 and value_address == 0)
    ):
        return SESSION_REPORT_INVALID
    var value = ProdexRichStringView(UInt(value_address), UInt(value_length))
    if not rich_view_valid(value, 0x7FFFFFFFFFFFFFFF):
        return SESSION_REPORT_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = 0
    var parsed = session_timestamp_sort_key(value)
    if parsed[0]:
        output[unsafe_offset=0] = 1
        output[unsafe_offset=1] = parsed[1]
    return SESSION_REPORT_OK


comptime SESSION_CLI_ABI_VERSION: Int64 = 1
comptime SESSION_CLI_MODE_TEXT: Int64 = 0
comptime SESSION_CLI_MODE_JSON: Int64 = 1
comptime SESSION_CLI_MODE_ID_ONLY: Int64 = 2
comptime SESSION_CLI_MODE_RESUME: Int64 = 3
comptime SESSION_CLI_CONFLICT: Int64 = 2
comptime SESSION_RESUME_INSPECT: Int64 = 0
comptime SESSION_RESUME_CONTINUE: Int64 = 1
comptime SESSION_RESUME_REJECT: Int64 = 2


@export("prodex_session_cli_output_mode_v1")
def prodex_session_cli_output_mode_v1(
    abi_version: Int64,
    json: Int64,
    id_only: Int64,
    resume_command: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != SESSION_CLI_ABI_VERSION
        or (json != 0 and json != 1)
        or (id_only != 0 and id_only != 1)
        or (resume_command != 0 and resume_command != 1)
        or output_address == 0
    ):
        return SESSION_REPORT_INVALID
    if json + id_only + resume_command > 1:
        return SESSION_CLI_CONFLICT
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = SESSION_CLI_MODE_TEXT
    if json == 1:
        output[unsafe_offset=0] = SESSION_CLI_MODE_JSON
    elif id_only == 1:
        output[unsafe_offset=0] = SESSION_CLI_MODE_ID_ONLY
    elif resume_command == 1:
        output[unsafe_offset=0] = SESSION_CLI_MODE_RESUME
    return SESSION_REPORT_OK


@export("prodex_session_resume_repair_action_v1")
def prodex_session_resume_repair_action_v1(
    abi_version: Int64,
    repaired: Int64,
    inspected_unrepairable: Int64,
    unrepairable_found: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != SESSION_CLI_ABI_VERSION
        or (repaired != 0 and repaired != 1)
        or (inspected_unrepairable != 0 and inspected_unrepairable != 1)
        or (unrepairable_found != 0 and unrepairable_found != 1)
        or output_address == 0
    ):
        return SESSION_REPORT_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    if repaired == 1:
        output[unsafe_offset=0] = SESSION_RESUME_CONTINUE
    elif inspected_unrepairable == 0:
        output[unsafe_offset=0] = SESSION_RESUME_INSPECT
    elif unrepairable_found == 1:
        output[unsafe_offset=0] = SESSION_RESUME_REJECT
    else:
        output[unsafe_offset=0] = SESSION_RESUME_CONTINUE
    return SESSION_REPORT_OK


@export("prodex_session_report_scroll_update_v1")
def prodex_session_report_scroll_update_v1(
    abi_version: Int64,
    key_code: Int64,
    control: Int64,
    offset: Int64,
    visible: Int64,
    max_scroll: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != SESSION_CLI_ABI_VERSION
        or (control != 0 and control != 1)
        or offset < 0
        or visible <= 0
        or max_scroll < offset
        or output_address == 0
    ):
        return SESSION_REPORT_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = offset
    if (
        key_code == 113
        or key_code == -7
        or key_code == -8
        or (control == 1 and (key_code == 99 or key_code == 122))
    ):
        output[unsafe_offset=0] = 1
    elif key_code == 106 or key_code == -1:
        if offset < max_scroll:
            output[unsafe_offset=1] = offset + 1
    elif key_code == 107 or key_code == -2:
        if offset > 0:
            output[unsafe_offset=1] = offset - 1
    elif key_code == -3:
        var remaining = max_scroll - offset
        if visible >= remaining:
            output[unsafe_offset=1] = max_scroll
        else:
            output[unsafe_offset=1] = offset + visible
    elif key_code == -4:
        if visible >= offset:
            output[unsafe_offset=1] = 0
        else:
            output[unsafe_offset=1] = offset - visible
    elif key_code == -5:
        output[unsafe_offset=1] = 0
    elif key_code == -6:
        output[unsafe_offset=1] = max_scroll
    return SESSION_REPORT_OK


comptime SESSION_PROMPT_WRITE_QUEUE_REJECTED: Int64 = 0
comptime SESSION_PROMPT_WRITE_QUEUE_PREFLIGHT: Int64 = 1
comptime SESSION_PROMPT_WRITE_QUEUE_ACCEPTED: Int64 = 2
comptime SESSION_PROMPT_WRITE_QUEUE_AMBIGUOUS: Int64 = 3
comptime SESSION_PROMPT_WRITE_ACTION_QUEUE_FAILED: Int64 = 0
comptime SESSION_PROMPT_WRITE_ACTION_NOT_ADDRESSABLE: Int64 = 1
comptime SESSION_PROMPT_WRITE_ACTION_AMBIGUOUS: Int64 = 2
comptime SESSION_PROMPT_WRITE_ACTION_PENDING: Int64 = 3
comptime SESSION_PROMPT_WRITE_ACTION_AWAIT_ROLLOUT: Int64 = 4


@export("prodex_session_prompt_write_queue_plan_v1")
def prodex_session_prompt_write_queue_plan_v1(
    abi_version: Int64,
    outcome: Int64,
    queued: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != SESSION_CLI_ABI_VERSION
        or outcome < SESSION_PROMPT_WRITE_QUEUE_REJECTED
        or outcome > SESSION_PROMPT_WRITE_QUEUE_AMBIGUOUS
        or (queued != 0 and queued != 1)
        or output_address == 0
    ):
        return SESSION_REPORT_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = Int64(
        outcome == SESSION_PROMPT_WRITE_QUEUE_REJECTED
        or outcome == SESSION_PROMPT_WRITE_QUEUE_PREFLIGHT
    )
    if outcome == SESSION_PROMPT_WRITE_QUEUE_REJECTED:
        output[unsafe_offset=1] = SESSION_PROMPT_WRITE_ACTION_QUEUE_FAILED
    elif outcome == SESSION_PROMPT_WRITE_QUEUE_PREFLIGHT:
        output[unsafe_offset=1] = SESSION_PROMPT_WRITE_ACTION_NOT_ADDRESSABLE
    elif outcome == SESSION_PROMPT_WRITE_QUEUE_AMBIGUOUS:
        output[unsafe_offset=1] = SESSION_PROMPT_WRITE_ACTION_AMBIGUOUS
    elif queued == 1:
        output[unsafe_offset=1] = SESSION_PROMPT_WRITE_ACTION_PENDING
    else:
        output[unsafe_offset=1] = SESSION_PROMPT_WRITE_ACTION_AWAIT_ROLLOUT
    return SESSION_REPORT_OK


comptime SESSION_PROMPT_WRITE_POLICY_ABI_VERSION: Int64 = 1
comptime SESSION_PROMPT_WRITE_POLICY_PROCESS_ROLE: Int64 = 1
comptime SESSION_PROMPT_WRITE_POLICY_RESOLUTION: Int64 = 2
comptime SESSION_PROMPT_WRITE_POLICY_OUTPUT_LINE: Int64 = 3
comptime SESSION_PROMPT_WRITE_POLICY_GAP_TEXT: Int64 = 4
comptime SESSION_PROMPT_WRITE_POLICY_VERIFICATION: Int64 = 5
comptime SESSION_PROMPT_WRITE_POLICY_RECORD_SHAPE: Int64 = 6
comptime SESSION_PROMPT_WRITE_POLICY_ENDPOINT_ARGS: Int64 = 7
comptime SESSION_PROMPT_WRITE_POLICY_USER_VISIBILITY: Int64 = 8
comptime SESSION_PROMPT_WRITE_ROLE_PLAIN_PRODEX: Int64 = 0
comptime SESSION_PROMPT_WRITE_ROLE_CODEX_WRITER: Int64 = 1
comptime SESSION_PROMPT_WRITE_LINE_PROCESS: Int64 = 0
comptime SESSION_PROMPT_WRITE_LINE_LIMIT: Int64 = 1
comptime SESSION_PROMPT_WRITE_LINE_OVERSIZED: Int64 = 2
comptime SESSION_PROMPT_WRITE_LINE_INVALID_UTF8: Int64 = 3
comptime SESSION_PROMPT_WRITE_LINE_MALFORMED: Int64 = 4
comptime SESSION_PROMPT_WRITE_GAP_OVERSIZED: Int64 = 0
comptime SESSION_PROMPT_WRITE_GAP_INVALID_UTF8: Int64 = 1
comptime SESSION_PROMPT_WRITE_GAP_MALFORMED: Int64 = 2
comptime SESSION_PROMPT_WRITE_VERIFY_QUEUE_FAILED: Int64 = 0
comptime SESSION_PROMPT_WRITE_VERIFY_NOT_ADDRESSABLE: Int64 = 1
comptime SESSION_PROMPT_WRITE_VERIFY_AMBIGUOUS: Int64 = 2
comptime SESSION_PROMPT_WRITE_VERIFY_PENDING: Int64 = 3
comptime SESSION_PROMPT_WRITE_VERIFY_ROLLOUT: Int64 = 4


def session_prompt_write_valid_bool(value: Int64) -> Bool:
    return value == 0 or value == 1


@export("prodex_session_prompt_write_policy_v1")
def prodex_session_prompt_write_policy_v1(
    abi_version: Int64,
    operation: Int64,
    first: Int64,
    second: Int64,
    third: Int64,
    fourth: Int64,
    fifth: Int64,
    sixth: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != SESSION_PROMPT_WRITE_POLICY_ABI_VERSION
        or operation < SESSION_PROMPT_WRITE_POLICY_PROCESS_ROLE
        or operation > SESSION_PROMPT_WRITE_POLICY_USER_VISIBILITY
        or output_address == 0
    ):
        return SESSION_REPORT_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var line_facts = output[unsafe_offset=7]
    for index in range(7):
        output[unsafe_offset=index] = -1

    if operation == SESSION_PROMPT_WRITE_POLICY_PROCESS_ROLE:
        if (
            (first != SESSION_PROMPT_WRITE_ROLE_PLAIN_PRODEX and first != SESSION_PROMPT_WRITE_ROLE_CODEX_WRITER)
            or not session_prompt_write_valid_bool(second)
            or third < 0
            or third > 3
            or not session_prompt_write_valid_bool(fourth)
            or not session_prompt_write_valid_bool(fifth)
        ):
            return SESSION_REPORT_INVALID
        var accepted = second == 1
        if first == SESSION_PROMPT_WRITE_ROLE_PLAIN_PRODEX:
            accepted = accepted and third == 1 and fourth == 0
        else:
            accepted = accepted and fifth == 0 and (third == 0 or third == 1 or third == 2)
        output[unsafe_offset=0] = Int64(accepted)
        return SESSION_REPORT_OK

    if operation == SESSION_PROMPT_WRITE_POLICY_RESOLUTION:
        if (
            not session_prompt_write_valid_bool(first)
            or not session_prompt_write_valid_bool(second)
            or not session_prompt_write_valid_bool(third)
        ):
            return SESSION_REPORT_INVALID
        # first is a targeted request, second is the observed no-session result,
        # and third reports whether the caller may poll before failing closed.
        output[unsafe_offset=0] = Int64(second == 1 and first == 1)
        output[unsafe_offset=1] = Int64(second == 1 and first == 0)
        output[unsafe_offset=2] = Int64(third == 1)
        return SESSION_REPORT_OK

    if operation == SESSION_PROMPT_WRITE_POLICY_OUTPUT_LINE:
        if (
            first < 0
            or second < 0
            or third < 0
            or not session_prompt_write_valid_bool(fourth)
            or not session_prompt_write_valid_bool(fifth)
            or not session_prompt_write_valid_bool(sixth)
        ):
            return SESSION_REPORT_INVALID
        # first is the raw line length, second the read limit, third the
        # verification limit, fourth UTF-8 validity, fifth JSON validity, and
        # sixth record-shape validity. A visible user message is passed in the
        # seventh slot through the output scratch word by the Rust adapter.
        var visible_user = line_facts == 1 or line_facts == 3
        var limit_reached = line_facts == 2 or line_facts == 3
        if limit_reached:
            output[unsafe_offset=0] = SESSION_PROMPT_WRITE_LINE_LIMIT
        elif first > second and (not visible_user or first > third):
            output[unsafe_offset=0] = SESSION_PROMPT_WRITE_LINE_OVERSIZED
        elif fourth == 0:
            output[unsafe_offset=0] = SESSION_PROMPT_WRITE_LINE_INVALID_UTF8
        elif fifth == 0 or sixth == 0:
            output[unsafe_offset=0] = SESSION_PROMPT_WRITE_LINE_MALFORMED
        else:
            output[unsafe_offset=0] = SESSION_PROMPT_WRITE_LINE_PROCESS
        return SESSION_REPORT_OK

    if operation == SESSION_PROMPT_WRITE_POLICY_GAP_TEXT:
        if first < SESSION_PROMPT_WRITE_GAP_OVERSIZED or first > SESSION_PROMPT_WRITE_GAP_MALFORMED:
            return SESSION_REPORT_INVALID
        output[unsafe_offset=0] = first
        return SESSION_REPORT_OK

    if operation == SESSION_PROMPT_WRITE_POLICY_RECORD_SHAPE:
        if (
            first < 0 or first > 5
            or not session_prompt_write_valid_bool(second)
            or not session_prompt_write_valid_bool(third)
            or not session_prompt_write_valid_bool(fourth)
        ):
            return SESSION_REPORT_INVALID
        if second == 0:
            output[unsafe_offset=0] = 0
        elif first == 1 or first == 2:
            output[unsafe_offset=0] = Int64(third == 1 and fourth == 1)
        elif first == 3 or first == 4:
            output[unsafe_offset=0] = Int64(third == 1)
        else:
            output[unsafe_offset=0] = 1
        return SESSION_REPORT_OK

    if operation == SESSION_PROMPT_WRITE_POLICY_ENDPOINT_ARGS:
        if (
            not session_prompt_write_valid_bool(first)
            or not session_prompt_write_valid_bool(second)
            or not session_prompt_write_valid_bool(third)
            or not session_prompt_write_valid_bool(fourth)
        ):
            return SESSION_REPORT_INVALID
        if first == 0:
            output[unsafe_offset=0] = 0
        elif second == 1 and third == 0:
            output[unsafe_offset=0] = 1
        elif second == 0 and third == 1 and fourth == 1:
            output[unsafe_offset=0] = 2
        else:
            output[unsafe_offset=0] = 0
        return SESSION_REPORT_OK

    if operation == SESSION_PROMPT_WRITE_POLICY_USER_VISIBILITY:
        if (
            first < 0 or first > 2
            or not session_prompt_write_valid_bool(second)
            or not session_prompt_write_valid_bool(third)
            or not session_prompt_write_valid_bool(fourth)
            or not session_prompt_write_valid_bool(fifth)
            or not session_prompt_write_valid_bool(sixth)
        ):
            return SESSION_REPORT_INVALID
        if first == 1:
            output[unsafe_offset=0] = Int64(second == 1)
        elif first == 2:
            output[unsafe_offset=0] = Int64(
                second == 1
                and third == 1
                and (fourth == 0 or (fifth == 1 and sixth == 1))
            )
        else:
            output[unsafe_offset=0] = 0
        return SESSION_REPORT_OK

    if first < SESSION_PROMPT_WRITE_VERIFY_QUEUE_FAILED or first > SESSION_PROMPT_WRITE_VERIFY_ROLLOUT:
        return SESSION_REPORT_INVALID
    output[unsafe_offset=0] = first
    return SESSION_REPORT_OK


@export("prodex_session_prompt_write_gap_text_v1")
def prodex_session_prompt_write_gap_text_v1(
    abi_version: Int64,
    reason: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != SESSION_PROMPT_WRITE_POLICY_ABI_VERSION
        or reason < SESSION_PROMPT_WRITE_GAP_OVERSIZED
        or reason > SESSION_PROMPT_WRITE_GAP_MALFORMED
        or output_capacity < 0
        or written_address == 0
        or (output_capacity > 0 and output_address == 0)
    ):
        return SESSION_REPORT_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var reason_text = StringSlice("oversized_record")
    if reason == SESSION_PROMPT_WRITE_GAP_INVALID_UTF8:
        reason_text = StringSlice("invalid_utf8")
    elif reason == SESSION_PROMPT_WRITE_GAP_MALFORMED:
        reason_text = StringSlice("malformed_record")
    var prefix = StringSlice("output gap: ")
    var suffix = StringSlice("; record omitted")
    var required = Int64(prefix.byte_length()) + Int64(reason_text.byte_length()) + Int64(suffix.byte_length())
    if output_capacity < required:
        return SESSION_REPORT_CAPACITY
    var cursor: Int64 = 0
    var prefix_ptr = prefix.unsafe_ptr()
    for index in range(Int64(prefix.byte_length())):
        output[unsafe_offset=cursor] = prefix_ptr[unsafe_offset=index]
        cursor += 1
    var reason_ptr = reason_text.unsafe_ptr()
    for index in range(Int64(reason_text.byte_length())):
        output[unsafe_offset=cursor] = reason_ptr[unsafe_offset=index]
        cursor += 1
    var suffix_ptr = suffix.unsafe_ptr()
    for index in range(Int64(suffix.byte_length())):
        output[unsafe_offset=cursor] = suffix_ptr[unsafe_offset=index]
        cursor += 1
    written[] = cursor
    return SESSION_REPORT_OK
