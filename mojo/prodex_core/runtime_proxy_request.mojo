from std.collections import Array
from std.memory import Pointer

from parsed_json import (
    JSON_ARRAY,
    JSON_OBJECT,
    JSON_STRING,
    ParsedJson,
    ParsedJsonNode,
    pj_child,
    pj_field,
    pj_is,
    pj_kind,
    pj_next,
    pj_nonblank,
    pj_string_field,
    pj_text,
    pj_trim,
    pj_valid,
)
from json_view import (
    deepseek_json_byte,
    deepseek_json_object_member,
    deepseek_json_raw_equals,
    deepseek_json_skip_ws,
    deepseek_json_value_end,
)
from launch_args_common import launch_rust_space
from rich_text import (
    rich_codepoint,
    rich_codepoint_width,
    rich_trim_bounds,
    rich_view_ptr,
    rich_view_valid,
)
from rich_types import ProdexRichStringView

comptime RUNTIME_PROXY_REQUEST_ABI_VERSION: Int64 = 1
comptime RUNTIME_PROXY_REQUEST_OK: Int64 = 0
comptime RUNTIME_PROXY_REQUEST_INVALID: Int64 = 1
comptime RUNTIME_PROXY_REQUEST_CAPACITY: Int64 = 3

comptime FALLBACK_TOOL_OUTPUT_ONLY: Int64 = 0
comptime FALLBACK_CONTEXT_DEPENDENT: Int64 = 1
comptime FALLBACK_SESSION_SCOPED: Int64 = 2
comptime FALLBACK_EMPTY_INPUT: Int64 = 3


def runtime_proxy_string_field_nonblank(
    tree: ParsedJson, object: Int64, name: StringSlice
) -> Int64:
    var field = pj_string_field(tree, object, name)
    return field if field >= 0 and pj_nonblank(tree, field) else -1


def runtime_proxy_nested_string_field_nonblank(
    tree: ParsedJson,
    object: Int64,
    direct_name: StringSlice,
    nested_object_name: StringSlice,
    nested_name: StringSlice,
) -> Int64:
    var direct = runtime_proxy_string_field_nonblank(tree, object, direct_name)
    if direct >= 0:
        return direct
    var nested = pj_field(tree, object, nested_object_name)
    if pj_kind(tree, nested) != JSON_OBJECT:
        return -1
    return runtime_proxy_string_field_nonblank(tree, nested, nested_name)


def runtime_proxy_view_ends_with(
    view: ProdexRichStringView, suffix: StringSlice
) -> Bool:
    var suffix_length = Int64(suffix.byte_length())
    if suffix_length > Int64(view.len):
        return False
    var source = rich_view_ptr(view)
    var target = suffix.unsafe_ptr()
    var start = Int64(view.len) - suffix_length
    for index in range(suffix_length):
        if source[unsafe_offset=start + index] != target[unsafe_offset=index]:
            return False
    return True


def runtime_proxy_input_item_is_tool_output(
    tree: ParsedJson, item: Int64
) -> Bool:
    if pj_kind(tree, item) != JSON_OBJECT:
        return False
    var kind = pj_string_field(tree, item, StringSlice("type"))
    var call_id = pj_string_field(tree, item, StringSlice("call_id"))
    if kind < 0 or call_id < 0 or not pj_nonblank(tree, call_id):
        return False
    return runtime_proxy_view_ends_with(
        pj_text(tree, kind), StringSlice("_call_output")
    )


def runtime_proxy_input_shape(
    tree: ParsedJson,
    request: Int64,
    session_present: Bool,
    shape_out: Pointer[mut=True, Int64, _],
    requires_affinity_out: Pointer[mut=True, Int64, _],
    full_history_out: Pointer[mut=True, Int64, _],
):
    var previous = runtime_proxy_string_field_nonblank(
        tree, request, StringSlice("previous_response_id")
    )
    var input = pj_field(tree, request, StringSlice("input"))
    var has_array = pj_kind(tree, input) == JSON_ARRAY

    var count: Int64 = 0
    var tool_output_count: Int64 = 0
    var context_dependent = False
    var reconstructable = False

    if has_array:
        var item = pj_child(tree, input)
        var index: Int64 = 0
        while item >= 0:
            var tool_output = runtime_proxy_input_item_is_tool_output(tree, item)
            if tool_output:
                tool_output_count += 1
            else:
                context_dependent = True

            if pj_is["compaction"](tree, pj_field(tree, item, StringSlice("type"))):
                if pj_next(tree, item) >= 0:
                    reconstructable = True

            if pj_is["user"](tree, pj_field(tree, item, StringSlice("role"))):
                var later = pj_next(tree, item)
                while later >= 0:
                    var has_after = pj_next(tree, later) >= 0
                    if has_after and (
                        pj_is["assistant"](
                            tree, pj_field(tree, later, StringSlice("role"))
                        )
                        or pj_is["function_call"](
                            tree, pj_field(tree, later, StringSlice("type"))
                        )
                    ):
                        reconstructable = True
                        break
                    later = pj_next(tree, later)

            count += 1
            index += 1
            item = pj_next(tree, item)

    requires_affinity_out[] = Int64(previous >= 0 and tool_output_count > 0)
    full_history_out[] = Int64(reconstructable)

    if previous < 0:
        shape_out[] = -1
    elif count > 0 and tool_output_count == count:
        shape_out[] = FALLBACK_TOOL_OUTPUT_ONLY
    elif context_dependent:
        shape_out[] = FALLBACK_CONTEXT_DEPENDENT
    elif session_present:
        shape_out[] = FALLBACK_SESSION_SCOPED
    else:
        shape_out[] = FALLBACK_EMPTY_INPUT



def runtime_proxy_raw_present(bounds: Array[Int64, 2]) -> Bool:
    return bounds[0] >= 0 and bounds[1] > bounds[0]


def runtime_proxy_raw_root(view: ProdexRichStringView) -> Array[Int64, 2]:
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


def runtime_proxy_raw_member(
    view: ProdexRichStringView,
    object: Array[Int64, 2],
    name: StringSlice,
) -> Array[Int64, 2]:
    if not runtime_proxy_raw_present(object):
        return Array[Int64, 2](fill=-1)^
    return deepseek_json_object_member(view, object[0], object[1], name)


def runtime_proxy_json_hex(value: UInt8) -> Int64:
    if value >= 48 and value <= 57:
        return Int64(value - 48)
    if value >= 65 and value <= 70:
        return Int64(value - 65 + 10)
    if value >= 97 and value <= 102:
        return Int64(value - 97 + 10)
    return -1


def runtime_proxy_raw_string_nonblank(
    view: ProdexRichStringView,
    bounds: Array[Int64, 2],
) -> Bool:
    if (
        not runtime_proxy_raw_present(bounds)
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
                    var digit = runtime_proxy_json_hex(
                        ptr[unsafe_offset=index + Int64(offset)]
                    )
                    if digit < 0:
                        return False
                    codepoint = codepoint * 16 + digit
                index += 6
            else:
                return True
        else:
            var width = rich_codepoint_width(value)
            if index + width > end:
                return False
            codepoint = rich_codepoint(ptr, index, width)
            index += width
        if not launch_rust_space(codepoint):
            return True
    return False


def runtime_proxy_raw_string_ends_with(
    view: ProdexRichStringView,
    bounds: Array[Int64, 2],
    suffix: StringSlice,
) -> Bool:
    if (
        not runtime_proxy_raw_present(bounds)
        or deepseek_json_byte(view, bounds[0]) != 34
        or deepseek_json_byte(view, bounds[1] - 1) != 34
    ):
        return False
    var suffix_length = Int64(suffix.byte_length())
    var value_length = bounds[1] - bounds[0] - 2
    if suffix_length > value_length:
        return False
    var source = rich_view_ptr(view)
    var expected = suffix.unsafe_ptr()
    var start = bounds[1] - 1 - suffix_length
    for offset in range(suffix_length):
        if source[unsafe_offset=start + offset] != expected[unsafe_offset=offset]:
            return False
    return True


def runtime_proxy_raw_input_item_is_tool_output(
    view: ProdexRichStringView,
    item: Array[Int64, 2],
) -> Bool:
    if (
        not runtime_proxy_raw_present(item)
        or deepseek_json_byte(view, item[0]) != 123
    ):
        return False
    var kind = runtime_proxy_raw_member(view, item, StringSlice("type"))
    var call_id = runtime_proxy_raw_member(view, item, StringSlice("call_id"))
    return (
        runtime_proxy_raw_string_nonblank(view, call_id)
        and runtime_proxy_raw_string_ends_with(
            view, kind, StringSlice("_call_output")
        )
    )


def runtime_proxy_raw_next_array_item(
    view: ProdexRichStringView,
    current_end: Int64,
    array_end: Int64,
) -> Int64:
    var index = deepseek_json_skip_ws(view, current_end, array_end - 1)
    if index >= array_end - 1:
        return -1
    if deepseek_json_byte(view, index) != 44:
        return -1
    index = deepseek_json_skip_ws(view, index + 1, array_end - 1)
    return index if index < array_end - 1 else -1


def runtime_proxy_raw_item_string_is(
    view: ProdexRichStringView,
    item: Array[Int64, 2],
    field: StringSlice,
    literal: StringSlice,
) -> Bool:
    var value = runtime_proxy_raw_member(view, item, field)
    return (
        runtime_proxy_raw_present(value)
        and deepseek_json_raw_equals(view, value[0], value[1], literal)
    )


def runtime_proxy_raw_input_shape(
    view: ProdexRichStringView,
    root: Array[Int64, 2],
    session_present: Bool,
    output: Pointer[mut=True, Int64, _],
) -> Bool:
    var previous = runtime_proxy_raw_member(
        view, root, StringSlice("previous_response_id")
    )
    var previous_present = runtime_proxy_raw_string_nonblank(view, previous)
    var input = runtime_proxy_raw_member(view, root, StringSlice("input"))
    var has_array = (
        runtime_proxy_raw_present(input)
        and deepseek_json_byte(view, input[0]) == 91
    )

    var count: Int64 = 0
    var tool_output_count: Int64 = 0
    var context_dependent = False
    var reconstructable = False

    if has_array:
        var cursor = deepseek_json_skip_ws(view, input[0] + 1, input[1] - 1)
        while cursor < input[1] - 1:
            var item_end = deepseek_json_value_end(
                view, cursor, input[1] - 1, 0
            )
            if item_end < 0:
                return False
            var item = Array[Int64, 2](fill=-1)
            item[0] = cursor
            item[1] = item_end
            if runtime_proxy_raw_input_item_is_tool_output(view, item):
                tool_output_count += 1
            else:
                context_dependent = True

            var next_item = runtime_proxy_raw_next_array_item(
                view, item_end, input[1]
            )
            if (
                runtime_proxy_raw_item_string_is(
                    view, item, StringSlice("type"), StringSlice("compaction")
                )
                and next_item >= 0
            ):
                reconstructable = True

            if runtime_proxy_raw_item_string_is(
                view, item, StringSlice("role"), StringSlice("user")
            ):
                var later = next_item
                while later >= 0:
                    var later_end = deepseek_json_value_end(
                        view, later, input[1] - 1, 0
                    )
                    if later_end < 0:
                        return False
                    var later_item = Array[Int64, 2](fill=-1)
                    later_item[0] = later
                    later_item[1] = later_end
                    var later_next = runtime_proxy_raw_next_array_item(
                        view, later_end, input[1]
                    )
                    if later_next >= 0 and (
                        runtime_proxy_raw_item_string_is(
                            view,
                            later_item,
                            StringSlice("role"),
                            StringSlice("assistant"),
                        )
                        or runtime_proxy_raw_item_string_is(
                            view,
                            later_item,
                            StringSlice("type"),
                            StringSlice("function_call"),
                        )
                    ):
                        reconstructable = True
                        break
                    later = later_next

            count += 1
            cursor = next_item
            if cursor < 0:
                break

    output[unsafe_offset=0] = Int64(
        previous_present and tool_output_count > 0
    )
    output[unsafe_offset=2] = Int64(reconstructable)

    if not previous_present:
        output[unsafe_offset=1] = -1
    elif count > 0 and tool_output_count == count:
        output[unsafe_offset=1] = FALLBACK_TOOL_OUTPUT_ONLY
    elif context_dependent:
        output[unsafe_offset=1] = FALLBACK_CONTEXT_DEPENDENT
    elif session_present:
        output[unsafe_offset=1] = FALLBACK_SESSION_SCOPED
    else:
        output[unsafe_offset=1] = FALLBACK_EMPTY_INPUT
    return True


@export("prodex_runtime_proxy_request_shape_v1")
def prodex_runtime_proxy_request_shape_v1(
    abi_version: Int64,
    raw_address: UInt,
    raw_length: Int64,
    session_present: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != RUNTIME_PROXY_REQUEST_ABI_VERSION
        or raw_length <= 0
        or raw_address == 0
        or output_address == 0
        or (session_present != 0 and session_present != 1)
    ):
        return RUNTIME_PROXY_REQUEST_INVALID

    var view = ProdexRichStringView(raw_address, UInt(raw_length))
    if not rich_view_valid(view, 0x7FFFFFFFFFFFFFFF):
        return RUNTIME_PROXY_REQUEST_INVALID
    var root = runtime_proxy_raw_root(view)
    if not runtime_proxy_raw_present(root):
        return RUNTIME_PROXY_REQUEST_INVALID

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = -1
    output[unsafe_offset=2] = 0
    if not runtime_proxy_raw_input_shape(
        view, root, session_present == 1, output
    ):
        return RUNTIME_PROXY_REQUEST_INVALID
    return RUNTIME_PROXY_REQUEST_OK


@export("prodex_runtime_proxy_request_metadata_v1")
def prodex_runtime_proxy_request_metadata_v1(
    abi_version: Int64,
    nodes_address: UInt,
    nodes_count: Int64,
    raw_address: UInt,
    raw_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != RUNTIME_PROXY_REQUEST_ABI_VERSION
        or nodes_address == 0
        or nodes_count <= 0
        or raw_length < 0
        or output_address == 0
        or (raw_length > 0 and raw_address == 0)
    ):
        return RUNTIME_PROXY_REQUEST_INVALID

    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](
            unsafe_from_address=Int(nodes_address)
        ),
        nodes_count,
        ProdexRichStringView(UInt(raw_address), UInt(raw_length)),
    )
    if not pj_valid(tree) or pj_kind(tree, 0) != JSON_OBJECT:
        return RUNTIME_PROXY_REQUEST_INVALID

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(10):
        output[unsafe_offset=index] = -1

    var previous = runtime_proxy_string_field_nonblank(
        tree, 0, StringSlice("previous_response_id")
    )
    var session = runtime_proxy_nested_string_field_nonblank(
        tree,
        0,
        StringSlice("session_id"),
        StringSlice("client_metadata"),
        StringSlice("session_id"),
    )
    var prompt_cache = runtime_proxy_string_field_nonblank(
        tree, 0, StringSlice("prompt_cache_key")
    )
    var turn_state = runtime_proxy_nested_string_field_nonblank(
        tree,
        0,
        StringSlice("x-codex-turn-state"),
        StringSlice("client_metadata"),
        StringSlice("x-codex-turn-state"),
    )
    var turn_id = runtime_proxy_nested_string_field_nonblank(
        tree,
        0,
        StringSlice("turn_id"),
        StringSlice("client_metadata"),
        StringSlice("turn_id"),
    )
    var thread_id = runtime_proxy_nested_string_field_nonblank(
        tree,
        0,
        StringSlice("thread_id"),
        StringSlice("client_metadata"),
        StringSlice("thread_id"),
    )
    var window_id = runtime_proxy_nested_string_field_nonblank(
        tree,
        0,
        StringSlice("window_id"),
        StringSlice("client_metadata"),
        StringSlice("x-codex-window-id"),
    )

    output[unsafe_offset=0] = previous
    output[unsafe_offset=1] = session
    output[unsafe_offset=2] = prompt_cache
    output[unsafe_offset=3] = turn_state
    output[unsafe_offset=4] = turn_id
    output[unsafe_offset=5] = thread_id
    output[unsafe_offset=6] = window_id

    var shape: Int64 = -1
    var requires_affinity: Int64 = 0
    var full_history: Int64 = 0
    runtime_proxy_input_shape(
        tree,
        0,
        session >= 0,
        Pointer(to=shape),
        Pointer(to=requires_affinity),
        Pointer(to=full_history),
    )
    output[unsafe_offset=7] = requires_affinity
    output[unsafe_offset=8] = shape
    output[unsafe_offset=9] = full_history
    return RUNTIME_PROXY_REQUEST_OK


def runtime_proxy_path_prefix(
    source: Pointer[mut=False, UInt8, _],
    length: Int64,
    literal: StringSlice,
) -> Bool:
    var target_length = Int64(literal.byte_length())
    if target_length > length:
        return False
    var target = literal.unsafe_ptr()
    for index in range(target_length):
        if source[unsafe_offset=index] != target[unsafe_offset=index]:
            return False
    return True


def runtime_proxy_range_equal(
    source: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    var target_length = Int64(literal.byte_length())
    if start < 0 or end < start or end - start != target_length:
        return False
    var target = literal.unsafe_ptr()
    for index in range(target_length):
        if source[unsafe_offset=start + index] != target[unsafe_offset=index]:
            return False
    return True


def runtime_proxy_range_ends(
    source: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    var target_length = Int64(literal.byte_length())
    if start < 0 or end < start or target_length > end - start:
        return False
    return runtime_proxy_range_equal(source, end - target_length, end, literal)


def runtime_proxy_legacy_version_segment(
    source: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    if end <= start:
        return False
    var has_digit = False
    for index in range(start, end):
        var value = source[unsafe_offset=index]
        if value >= 48 and value <= 57:
            has_digit = True
        elif value != 46:
            return False
    return has_digit


def runtime_proxy_mount_suffix_start(
    source: Pointer[mut=False, UInt8, _], path_end: Int64
) -> Int64:
    comptime LEGACY = StringSlice("/backend-api/prodex/v")
    comptime MOUNT = StringSlice("/backend-api/prodex")
    var legacy_length = Int64(LEGACY.byte_length())
    if runtime_proxy_path_prefix(source, path_end, LEGACY):
        var slash = legacy_length
        while slash < path_end and source[unsafe_offset=slash] != 47:
            slash += 1
        if slash < path_end and runtime_proxy_legacy_version_segment(source, legacy_length, slash):
            return slash

    var mount_length = Int64(MOUNT.byte_length())
    if runtime_proxy_path_prefix(source, path_end, MOUNT):
        if path_end == mount_length or source[unsafe_offset=mount_length] == 47:
            return mount_length
    return -1


def runtime_proxy_mount_path_copy(
    source: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    if start < 0 or end < start or capacity < 0 or written[] < 0:
        return False
    var mount_start = start
    var mount_end = end
    while mount_start < mount_end and source[unsafe_offset=mount_start] == 47:
        mount_start += 1
    while mount_end > mount_start and source[unsafe_offset=mount_end - 1] == 47:
        mount_end -= 1
    if mount_start == mount_end:
        if written[] >= capacity:
            return False
        output[unsafe_offset=written[]] = 47
        written[] += 1
        return True
    var length = mount_end - mount_start + 1
    if length > capacity - written[]:
        return False
    output[unsafe_offset=written[]] = 47
    written[] += 1
    for index in range(mount_start, mount_end):
        output[unsafe_offset=written[]] = source[unsafe_offset=index]
        written[] += 1
    return True


@export("prodex_runtime_proxy_mount_path_v1")
def prodex_runtime_proxy_mount_path_v1(
    abi_version: Int64,
    mount_address: UInt,
    mount_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != RUNTIME_PROXY_REQUEST_ABI_VERSION
        or mount_length < 0
        or (mount_length > 0 and mount_address == 0)
        or output_address == 0
        or output_capacity < 1
        or written_address == 0
    ):
        return RUNTIME_PROXY_REQUEST_INVALID
    var view = ProdexRichStringView(mount_address, UInt(mount_length))
    if not rich_view_valid(view, mount_length):
        return RUNTIME_PROXY_REQUEST_INVALID
    var bounds = rich_trim_bounds(view)
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if bounds[1] <= bounds[0] or (
        bounds[1] - bounds[0] == 1
        and rich_view_ptr(view)[unsafe_offset=bounds[0]] == 47
    ):
        return RUNTIME_PROXY_REQUEST_OK
    if not runtime_proxy_mount_path_copy(
        rich_view_ptr(view), bounds[0], bounds[1], output, output_capacity, written
    ):
        return RUNTIME_PROXY_REQUEST_CAPACITY
    return RUNTIME_PROXY_REQUEST_OK


def runtime_proxy_effective_ends(
    source: Pointer[mut=False, UInt8, _],
    path_end: Int64,
    suffix_start: Int64,
    literal: StringSlice,
) -> Bool:
    var start = 0 if suffix_start < 0 else suffix_start
    return runtime_proxy_range_ends(source, start, path_end, literal)


def runtime_proxy_effective_responses(
    source: Pointer[mut=False, UInt8, _],
    path_end: Int64,
    suffix_start: Int64,
) -> Bool:
    if suffix_start >= 0:
        return (
            runtime_proxy_range_equal(source, suffix_start, path_end, StringSlice("/responses"))
            or runtime_proxy_range_ends(
                source, suffix_start, path_end, StringSlice("/codex/responses")
            )
        )
    return runtime_proxy_range_ends(
        source, 0, path_end, StringSlice("/codex/responses")
    )


def runtime_proxy_effective_live_call(
    source: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
) -> Bool:
    comptime MARKER = StringSlice("/live/")
    var marker = MARKER.unsafe_ptr()
    var marker_length = Int64(MARKER.byte_length())
    if end - start <= marker_length:
        return False
    var index = start
    while index + marker_length < end:
        var matches = True
        for offset in range(marker_length):
            if source[unsafe_offset=index + offset] != marker[unsafe_offset=offset]:
                matches = False
                break
        if matches:
            var call_start = index + marker_length
            if call_start < end:
                var cursor = call_start
                var has_slash = False
                while cursor < end:
                    if source[unsafe_offset=cursor] == 47:
                        has_slash = True
                        break
                    cursor += 1
                if not has_slash:
                    return True
        index += 1
    return False


@export("prodex_runtime_proxy_path_plan_v1")
def prodex_runtime_proxy_path_plan_v1(
    abi_version: Int64,
    path_address: UInt,
    path_length: Int64,
    websocket: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != RUNTIME_PROXY_REQUEST_ABI_VERSION
        or path_length < 0
        or (path_length > 0 and path_address == 0)
        or (websocket != 0 and websocket != 1)
        or output_address == 0
    ):
        return RUNTIME_PROXY_REQUEST_INVALID

    var view = ProdexRichStringView(path_address, UInt(path_length))
    if not rich_view_valid(view, path_length):
        return RUNTIME_PROXY_REQUEST_INVALID
    var source = rich_view_ptr(view)
    var path_end = path_length
    var query_mark: Int64 = -1
    for index in range(path_length):
        if source[unsafe_offset=index] == 63:
            path_end = index
            query_mark = index
            break

    var suffix_start = runtime_proxy_mount_suffix_start(source, path_end)
    var effective_start = 0 if suffix_start < 0 else suffix_start
    var responses = runtime_proxy_effective_responses(source, path_end, suffix_start)
    var chat = runtime_proxy_effective_ends(
        source, path_end, suffix_start, StringSlice("/chat/completions")
    )
    var compact = runtime_proxy_effective_ends(
        source, path_end, suffix_start, StringSlice("/responses/compact")
    )
    var realtime_call = (
        runtime_proxy_effective_ends(
            source, path_end, suffix_start, StringSlice("/realtime/calls")
        )
        or runtime_proxy_effective_ends(
            source, path_end, suffix_start, StringSlice("/live")
        )
    )
    var realtime_websocket = (
        runtime_proxy_effective_ends(
            source, path_end, suffix_start, StringSlice("/realtime")
        )
        or runtime_proxy_effective_ends(
            source, path_end, suffix_start, StringSlice("/live")
        )
        or runtime_proxy_effective_live_call(source, effective_start, path_end)
    )

    var route: Int64 = 3
    if websocket == 1:
        route = 2
    elif compact:
        route = 1
    elif responses or chat:
        route = 0

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = Int64(suffix_start >= 0)
    output[unsafe_offset=1] = suffix_start
    output[unsafe_offset=2] = path_end
    output[unsafe_offset=3] = query_mark
    output[unsafe_offset=4] = Int64(responses)
    output[unsafe_offset=5] = Int64(chat)
    output[unsafe_offset=6] = Int64(compact)
    output[unsafe_offset=7] = Int64(realtime_call)
    output[unsafe_offset=8] = Int64(realtime_websocket)
    output[unsafe_offset=9] = route
    output[unsafe_offset=10] = Int64(websocket == 1 or responses or chat)
    return RUNTIME_PROXY_REQUEST_OK


comptime RUNTIME_PROVIDER_ROUTE_RESPONSES: Int64 = 0
comptime RUNTIME_PROVIDER_ROUTE_RESPONSES_COMPACT: Int64 = 1
comptime RUNTIME_PROVIDER_ROUTE_CHAT_COMPLETIONS: Int64 = 2
comptime RUNTIME_PROVIDER_ROUTE_MESSAGES: Int64 = 3
comptime RUNTIME_PROVIDER_ROUTE_EMBEDDINGS: Int64 = 4
comptime RUNTIME_PROVIDER_ROUTE_MODELS_LIST: Int64 = 5
comptime RUNTIME_PROVIDER_ROUTE_MODELS_SINGLE: Int64 = 6


@export("prodex_runtime_provider_route_plan_v1")
def prodex_runtime_provider_route_plan_v1(
    abi_version: Int64,
    path_address: UInt,
    path_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != RUNTIME_PROXY_REQUEST_ABI_VERSION
        or path_length < 0
        or (path_length > 0 and path_address == 0)
        or output_address == 0
    ):
        return RUNTIME_PROXY_REQUEST_INVALID

    var view = ProdexRichStringView(path_address, UInt(path_length))
    if not rich_view_valid(view, path_length):
        return RUNTIME_PROXY_REQUEST_INVALID
    var source = rich_view_ptr(view)
    var path_end = path_length
    for index in range(path_length):
        if source[unsafe_offset=index] == 63:
            path_end = index
            break

    var route: Int64 = -1
    var model_start: Int64 = -1
    var model_end: Int64 = -1
    if (
        runtime_proxy_range_equal(source, 0, path_end, StringSlice("/v1/responses"))
        or runtime_proxy_range_equal(source, 0, path_end, StringSlice("/responses"))
    ):
        route = RUNTIME_PROVIDER_ROUTE_RESPONSES
    elif (
        runtime_proxy_range_equal(source, 0, path_end, StringSlice("/v1/responses/compact"))
        or runtime_proxy_range_equal(source, 0, path_end, StringSlice("/responses/compact"))
    ):
        route = RUNTIME_PROVIDER_ROUTE_RESPONSES_COMPACT
    elif (
        runtime_proxy_range_equal(source, 0, path_end, StringSlice("/v1/chat/completions"))
        or runtime_proxy_range_equal(source, 0, path_end, StringSlice("/chat/completions"))
    ):
        route = RUNTIME_PROVIDER_ROUTE_CHAT_COMPLETIONS
    elif (
        runtime_proxy_range_equal(source, 0, path_end, StringSlice("/v1/messages"))
        or runtime_proxy_range_equal(source, 0, path_end, StringSlice("/messages"))
    ):
        route = RUNTIME_PROVIDER_ROUTE_MESSAGES
    elif (
        runtime_proxy_range_equal(source, 0, path_end, StringSlice("/v1/embeddings"))
        or runtime_proxy_range_equal(source, 0, path_end, StringSlice("/embeddings"))
    ):
        route = RUNTIME_PROVIDER_ROUTE_EMBEDDINGS
    elif (
        runtime_proxy_range_equal(source, 0, path_end, StringSlice("/v1/models"))
        or runtime_proxy_range_equal(source, 0, path_end, StringSlice("/models"))
    ):
        route = RUNTIME_PROVIDER_ROUTE_MODELS_LIST
    else:
        comptime V1_MODELS = StringSlice("/v1/models/")
        comptime MODELS = StringSlice("/models/")
        var v1_length = Int64(V1_MODELS.byte_length())
        var models_length = Int64(MODELS.byte_length())
        if (
            path_end > v1_length
            and runtime_proxy_path_prefix(source, path_end, V1_MODELS)
        ):
            route = RUNTIME_PROVIDER_ROUTE_MODELS_SINGLE
            model_start = v1_length
            model_end = path_end
        elif (
            path_end > models_length
            and runtime_proxy_path_prefix(source, path_end, MODELS)
        ):
            route = RUNTIME_PROVIDER_ROUTE_MODELS_SINGLE
            model_start = models_length
            model_end = path_end

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = route
    output[unsafe_offset=1] = model_start
    output[unsafe_offset=2] = model_end
    output[unsafe_offset=3] = path_end
    return RUNTIME_PROXY_REQUEST_OK
