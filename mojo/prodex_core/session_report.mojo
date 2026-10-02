from std.memory import Pointer

from parsed_json import (
    JSON_OBJECT,
    JSON_NUMBER,
    ParsedJson,
    ParsedJsonNode,
    pj_field,
    pj_equal,
    pj_is,
    pj_kind,
    pj_less,
    pj_nonblank,
    pj_trim,
    pj_string_field,
    pj_valid,
)
from rich_types import ProdexRichStringView
from rich_text import rich_view_ptr, rich_view_valid

comptime SESSION_REPORT_ABI_VERSION: Int64 = 1
comptime SESSION_REPORT_OK: Int64 = 0
comptime SESSION_REPORT_INVALID: Int64 = 1

def session_string_field(tree: ParsedJson, object: Int64, name: StringSlice) -> Int64:
    var field = pj_string_field(tree, object, name)
    return field if field >= 0 and pj_nonblank(tree, field) else -1

def session_object_field(tree: ParsedJson, object: Int64, name: StringSlice) -> Int64:
    var field = pj_field(tree, object, name)
    return field if pj_kind(tree, field) == JSON_OBJECT else -1

def session_nested_string2(
    tree: ParsedJson,
    object: Int64,
    parent: StringSlice,
    name: StringSlice,
) -> Int64:
    var nested = session_object_field(tree, object, parent)
    return session_string_field(tree, nested, name) if nested >= 0 else -1

def session_nested_string5(
    tree: ParsedJson,
    object: Int64,
    first: StringSlice,
    second: StringSlice,
    third: StringSlice,
    fourth: StringSlice,
    name: StringSlice,
) -> Int64:
    var one = session_object_field(tree, object, first)
    if one < 0:
        return -1
    var two = session_object_field(tree, one, second)
    if two < 0:
        return -1
    var three = session_object_field(tree, two, third)
    if three < 0:
        return -1
    var four = session_object_field(tree, three, fourth)
    return session_string_field(tree, four, name) if four >= 0 else -1

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

def session_numeric_timestamp_field(
    tree: ParsedJson,
    number_values: Pointer[mut=False, Int64, ImmUntrackedOrigin],
    number_valid: Pointer[mut=False, Int64, ImmUntrackedOrigin],
    object: Int64,
    name: StringSlice,
) -> Int64:
    var field = pj_field(tree, object, name)
    if field < 0 or pj_kind(tree, field) != JSON_NUMBER:
        return -1
    return field if number_valid[unsafe_offset=field] == 1 else -1

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

@export("prodex_session_report_metadata_v1")
def prodex_session_report_metadata_v1(
    abi_version: Int64,
    nodes_address: UInt,
    nodes_count: Int64,
    raw_address: UInt,
    raw_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != SESSION_REPORT_ABI_VERSION
        or nodes_address == 0
        or nodes_count <= 0
        or raw_length < 0
        or output_address == 0
        or (raw_length > 0 and raw_address == 0)
    ):
        return SESSION_REPORT_INVALID

    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](
            unsafe_from_address=Int(nodes_address)
        ),
        nodes_count,
        ProdexRichStringView(UInt(raw_address), UInt(raw_length)),
    )
    if not pj_valid(tree) or pj_kind(tree, 0) != JSON_OBJECT:
        return SESSION_REPORT_INVALID

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(9):
        output[unsafe_offset=index] = -1

    var payload = session_object_field(tree, 0, StringSlice("payload"))
    var metadata = session_object_field(tree, 0, StringSlice("metadata"))
    var payload_metadata = (
        session_object_field(tree, payload, StringSlice("metadata"))
        if payload >= 0
        else -1
    )

    var type_node = pj_string_field(tree, 0, StringSlice("type"))
    var type_class: Int64 = 0
    if type_node >= 0:
        if pj_is["session_meta"](tree, type_node):
            type_class = 1
        elif pj_is["turn_context"](tree, type_node):
            type_class = 2
        else:
            type_class = 3
    output[0] = type_class

    var id_node: Int64 = -1
    if payload >= 0:
        id_node = session_string_field(tree, payload, StringSlice("id"))
        if id_node < 0:
            id_node = session_string_field(tree, payload, StringSlice("session_id"))
    if id_node < 0:
        id_node = session_string_field(tree, 0, StringSlice("id"))
    if id_node < 0:
        id_node = session_string_field(tree, 0, StringSlice("session_id"))
    output[1] = id_node

    if type_class == 2:
        var model = (
            session_string_field(tree, payload, StringSlice("model"))
            if payload >= 0
            else -1
        )
        if model < 0:
            model = session_string_field(tree, 0, StringSlice("model"))
        output[2] = model

        var effort = (
            session_string_field(tree, payload, StringSlice("effort"))
            if payload >= 0
            else -1
        )
        if effort < 0 and payload >= 0:
            effort = session_string_field(tree, payload, StringSlice("reasoning_effort"))
        if effort < 0:
            effort = session_string_field(tree, 0, StringSlice("effort"))
        if effort < 0:
            effort = session_string_field(tree, 0, StringSlice("reasoning_effort"))
        output[3] = effort

    var thread_name: Int64 = -1
    if payload >= 0:
        thread_name = session_string_field(tree, payload, StringSlice("thread_name"))
        if thread_name < 0:
            thread_name = session_string_field(tree, payload, StringSlice("title"))
        if thread_name < 0 and payload_metadata >= 0:
            thread_name = session_string_field(
                tree, payload_metadata, StringSlice("thread_name")
            )
    if thread_name < 0:
        thread_name = session_string_field(tree, 0, StringSlice("thread_name"))
    if thread_name < 0:
        thread_name = session_string_field(tree, 0, StringSlice("title"))
    if thread_name < 0 and metadata >= 0:
        thread_name = session_string_field(tree, metadata, StringSlice("thread_name"))
    output[4] = thread_name

    var cwd: Int64 = -1
    if payload >= 0:
        cwd = session_string_field(tree, payload, StringSlice("cwd"))
        if cwd < 0 and payload_metadata >= 0:
            cwd = session_string_field(tree, payload_metadata, StringSlice("cwd"))
        if cwd < 0:
            cwd = session_string_field(tree, payload, StringSlice("workdir"))
    if cwd < 0:
        cwd = session_string_field(tree, 0, StringSlice("cwd"))
    if cwd < 0 and metadata >= 0:
        cwd = session_string_field(tree, metadata, StringSlice("cwd"))
    if cwd < 0:
        cwd = session_string_field(tree, 0, StringSlice("workdir"))
    output[5] = cwd

    var updated: Int64 = session_string_field(tree, 0, StringSlice("updated_at"))
    if updated < 0:
        updated = session_string_field(tree, 0, StringSlice("timestamp"))
    if updated < 0 and payload >= 0:
        updated = session_string_field(tree, payload, StringSlice("updated_at"))
    if updated < 0 and payload >= 0:
        updated = session_string_field(tree, payload, StringSlice("timestamp"))
    output[6] = updated

    var parent: Int64 = -1
    if payload >= 0:
        parent = session_nested_string5(
            tree,
            0,
            StringSlice("payload"),
            StringSlice("source"),
            StringSlice("subagent"),
            StringSlice("thread_spawn"),
            StringSlice("parent_thread_id"),
        )
    if parent < 0:
        var source = session_object_field(tree, 0, StringSlice("source"))
        var subagent = (
            session_object_field(tree, source, StringSlice("subagent"))
            if source >= 0
            else -1
        )
        var spawn = (
            session_object_field(tree, subagent, StringSlice("thread_spawn"))
            if subagent >= 0
            else -1
        )
        if spawn >= 0:
            parent = session_string_field(
                tree, spawn, StringSlice("parent_thread_id")
            )
    if parent < 0 and payload >= 0:
        parent = session_string_field(tree, payload, StringSlice("parent_thread_id"))
    if parent < 0:
        parent = session_string_field(tree, 0, StringSlice("parent_thread_id"))
    output[7] = parent

    var provider: Int64 = -1
    if payload >= 0:
        provider = session_string_field(tree, payload, StringSlice("model_provider"))
        if provider < 0 and payload_metadata >= 0:
            provider = session_string_field(
                tree, payload_metadata, StringSlice("model_provider")
            )
    if provider < 0:
        provider = session_string_field(tree, 0, StringSlice("model_provider"))
    if provider < 0 and metadata >= 0:
        provider = session_string_field(tree, metadata, StringSlice("model_provider"))
    output[8] = provider
    return SESSION_REPORT_OK

@export("prodex_session_report_update_v1")
def prodex_session_report_update_v1(
    abi_version: Int64,
    nodes_address: UInt,
    nodes_count: Int64,
    raw_address: UInt,
    raw_length: Int64,
    number_values_address: UInt,
    number_valid_address: UInt,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != SESSION_REPORT_ABI_VERSION
        or nodes_address == 0
        or nodes_count <= 0
        or raw_length < 0
        or output_address == 0
        or number_values_address == 0
        or number_valid_address == 0
        or (raw_length > 0 and raw_address == 0)
    ):
        return SESSION_REPORT_INVALID
    var metadata_status = prodex_session_report_metadata_v1(
        abi_version,
        nodes_address,
        nodes_count,
        raw_address,
        raw_length,
        output_address,
    )
    if metadata_status != SESSION_REPORT_OK:
        return metadata_status

    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](
            unsafe_from_address=Int(nodes_address)
        ),
        nodes_count,
        ProdexRichStringView(UInt(raw_address), UInt(raw_length)),
    )
    var values = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(number_values_address)
    )
    var valid = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(number_valid_address)
    )
    for index in range(nodes_count):
        var present = valid[unsafe_offset=index]
        if present < 0 or present > 1 or (present == 1 and pj_kind(tree, index) != JSON_NUMBER):
            return SESSION_REPORT_INVALID

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=9] = -1
    output[unsafe_offset=10] = -1
    output[unsafe_offset=11] = 0
    output[unsafe_offset=12] = 0
    if output[unsafe_offset=0] == 0 or output[unsafe_offset=0] == 1:
        output[unsafe_offset=9] = output[unsafe_offset=1]

    if output[unsafe_offset=6] >= 0:
        var timestamp = pj_trim(tree.nodes[unsafe_offset=output[unsafe_offset=6]].text.copy())
        var parsed = session_timestamp_sort_key(timestamp)
        if parsed[0]:
            output[unsafe_offset=11] = 1
            output[unsafe_offset=12] = parsed[1]
    else:
        var payload = session_object_field(tree, 0, StringSlice("payload"))
        var timestamp_node = session_numeric_timestamp_field(
            tree, values, valid, 0, StringSlice("updated_at")
        )
        if timestamp_node < 0:
            timestamp_node = session_numeric_timestamp_field(
                tree, values, valid, 0, StringSlice("ts")
            )
        if timestamp_node < 0:
            timestamp_node = session_numeric_timestamp_field(
                tree, values, valid, 0, StringSlice("timestamp")
            )
        if timestamp_node < 0 and payload >= 0:
            timestamp_node = session_numeric_timestamp_field(
                tree, values, valid, payload, StringSlice("updated_at")
            )
        if timestamp_node < 0 and payload >= 0:
            timestamp_node = session_numeric_timestamp_field(
                tree, values, valid, payload, StringSlice("ts")
            )
        if timestamp_node < 0 and payload >= 0:
            timestamp_node = session_numeric_timestamp_field(
                tree, values, valid, payload, StringSlice("timestamp")
            )
        if timestamp_node >= 0:
            output[unsafe_offset=10] = timestamp_node
            output[unsafe_offset=11] = 1
            output[unsafe_offset=12] = values[unsafe_offset=timestamp_node]
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
