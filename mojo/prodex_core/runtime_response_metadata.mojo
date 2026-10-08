from std.collections import Array
from std.memory import Pointer

from json_view import (
    deepseek_json_byte,
    deepseek_json_fragment_valid,
    deepseek_json_object_member,
    deepseek_json_raw_equals,
    deepseek_json_skip_ws,
    deepseek_json_string_end,
    deepseek_json_value_end,
)
from kiro import kiro_raw_string_nonblank
from rich_text import rich_view_matches_literal, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime RUNTIME_RESPONSE_METADATA_ABI_VERSION: Int64 = 1
comptime RUNTIME_RESPONSE_METADATA_INVALID: Int64 = 1
comptime RUNTIME_RESPONSE_METADATA_SCAN_LIMIT: Int64 = 2048
comptime RUNTIME_RESPONSE_METADATA_MAX_DEPTH: Int64 = 256
comptime RUNTIME_RESPONSE_EVENT_KIND_ABI_VERSION: Int64 = 1


def runtime_response_raw_present(bounds: Array[Int64, 2]) -> Bool:
    return bounds[0] >= 0 and bounds[1] > bounds[0]


def runtime_response_raw_string(bounds: Array[Int64, 2], view: ProdexRichStringView) -> Bool:
    return (
        runtime_response_raw_present(bounds)
        and deepseek_json_byte(view, bounds[0]) == 34
        and deepseek_json_byte(view, bounds[1] - 1) == 34
    )


def runtime_response_raw_string_nonblank(
    view: ProdexRichStringView, bounds: Array[Int64, 2]
) -> Bool:
    return runtime_response_raw_string(bounds, view) and kiro_raw_string_nonblank(
        view, bounds
    )


def runtime_response_raw_write_bounds(
    output: Pointer[mut=True, Int64, _],
    offset: Int64,
    bounds: Array[Int64, 2],
):
    if runtime_response_raw_present(bounds):
        output[unsafe_offset=offset] = bounds[0]
        output[unsafe_offset=offset + 1] = bounds[1]


def runtime_response_raw_token_equal(
    view: ProdexRichStringView,
    left: Array[Int64, 2],
    right: Array[Int64, 2],
) -> Bool:
    if (
        not runtime_response_raw_present(left)
        or not runtime_response_raw_present(right)
        or left[1] - left[0] != right[1] - right[0]
    ):
        return False
    var source = rich_view_ptr(view)
    for offset in range(left[1] - left[0]):
        if (
            source[unsafe_offset=left[0] + offset]
            != source[unsafe_offset=right[0] + offset]
        ):
            return False
    return True


def runtime_response_raw_string_ends_with(
    view: ProdexRichStringView,
    bounds: Array[Int64, 2],
    suffix: StringSlice,
) -> Bool:
    if not runtime_response_raw_string(bounds, view):
        return False
    var suffix_length = Int64(suffix.byte_length())
    var text_length = bounds[1] - bounds[0] - 2
    if suffix_length > text_length:
        return False
    var source = rich_view_ptr(view)
    var target = suffix.unsafe_ptr()
    var start = bounds[1] - 1 - suffix_length
    for index in range(suffix_length):
        if source[unsafe_offset=start + index] != target[unsafe_offset=index]:
            return False
    return True


def runtime_response_raw_ascii_casefold_key(
    view: ProdexRichStringView,
    token_start: Int64,
    token_end: Int64,
    literal: StringSlice,
) -> Bool:
    if (
        token_start < 0
        or token_end <= token_start + 1
        or deepseek_json_byte(view, token_start) != 34
        or deepseek_json_byte(view, token_end - 1) != 34
    ):
        return False
    var expected_length = Int64(literal.byte_length())
    if token_end - token_start - 2 != expected_length:
        return False
    var source = rich_view_ptr(view)
    var expected = literal.unsafe_ptr()
    for index in range(expected_length):
        var actual = source[unsafe_offset=token_start + 1 + index]
        if actual >= 65 and actual <= 90:
            actual += 32
        var wanted = expected[unsafe_offset=index]
        if wanted >= 65 and wanted <= 90:
            wanted += 32
        if actual != wanted:
            return False
    return True


def runtime_response_raw_header_value(
    view: ProdexRichStringView,
    bounds: Array[Int64, 2],
    depth: Int64,
) -> Array[Int64, 2]:
    if depth > RUNTIME_RESPONSE_METADATA_MAX_DEPTH:
        return Array[Int64, 2](fill=-1)^
    if runtime_response_raw_string_nonblank(view, bounds):
        var result = Array[Int64, 2](fill=-1)
        result[0] = bounds[0]
        result[1] = bounds[1]
        return result^
    if (
        not runtime_response_raw_present(bounds)
        or deepseek_json_byte(view, bounds[0]) != 91
    ):
        return Array[Int64, 2](fill=-1)^
    var cursor = deepseek_json_skip_ws(view, bounds[0] + 1, bounds[1] - 1)
    while cursor < bounds[1] - 1:
        var item_end = deepseek_json_value_end(view, cursor, bounds[1] - 1, depth + 1)
        if item_end < 0:
            return Array[Int64, 2](fill=-1)^
        var item = Array[Int64, 2](fill=-1)
        item[0] = cursor
        item[1] = item_end
        var result = runtime_response_raw_header_value(view, item, depth + 1)
        if runtime_response_raw_present(result):
            return result^
        cursor = deepseek_json_skip_ws(view, item_end, bounds[1] - 1)
        if cursor < bounds[1] - 1 and deepseek_json_byte(view, cursor) == 44:
            cursor = deepseek_json_skip_ws(view, cursor + 1, bounds[1] - 1)
            continue
        if cursor != bounds[1] - 1:
            return Array[Int64, 2](fill=-1)^
        break
    return Array[Int64, 2](fill=-1)^


def runtime_response_raw_array_first_two(
    view: ProdexRichStringView,
    bounds: Array[Int64, 2],
) -> Array[Int64, 4]:
    var result = Array[Int64, 4](fill=-1)
    if (
        not runtime_response_raw_present(bounds)
        or deepseek_json_byte(view, bounds[0]) != 91
    ):
        return result^
    var cursor = deepseek_json_skip_ws(view, bounds[0] + 1, bounds[1] - 1)
    for slot in range(2):
        if cursor >= bounds[1] - 1:
            return result^
        var item_end = deepseek_json_value_end(view, cursor, bounds[1] - 1, 0)
        if item_end < 0:
            return Array[Int64, 4](fill=-1)^
        result[slot * 2] = cursor
        result[slot * 2 + 1] = item_end
        cursor = deepseek_json_skip_ws(view, item_end, bounds[1] - 1)
        if slot == 0:
            if cursor >= bounds[1] - 1 or deepseek_json_byte(view, cursor) != 44:
                return Array[Int64, 4](fill=-1)^
            cursor = deepseek_json_skip_ws(view, cursor + 1, bounds[1] - 1)
    return result^


def runtime_response_raw_headers_state(
    view: ProdexRichStringView, headers: Array[Int64, 2]
) -> Array[Int64, 2]:
    if not runtime_response_raw_present(headers):
        return Array[Int64, 2](fill=-1)^
    if deepseek_json_byte(view, headers[0]) == 123:
        var cursor = deepseek_json_skip_ws(view, headers[0] + 1, headers[1] - 1)
        while cursor < headers[1] - 1:
            var key_start = cursor
            var key_end = deepseek_json_string_end(view, key_start, headers[1] - 1)
            if key_end < 0:
                return Array[Int64, 2](fill=-1)^
            cursor = deepseek_json_skip_ws(view, key_end, headers[1] - 1)
            if cursor >= headers[1] - 1 or deepseek_json_byte(view, cursor) != 58:
                return Array[Int64, 2](fill=-1)^
            var value_start = deepseek_json_skip_ws(view, cursor + 1, headers[1] - 1)
            var value_end = deepseek_json_value_end(view, value_start, headers[1] - 1, 0)
            if value_end < 0:
                return Array[Int64, 2](fill=-1)^
            if runtime_response_raw_ascii_casefold_key(
                view, key_start, key_end, StringSlice("x-codex-turn-state")
            ):
                var value = Array[Int64, 2](fill=-1)
                value[0] = value_start
                value[1] = value_end
                var result = runtime_response_raw_header_value(view, value, 0)
                if runtime_response_raw_present(result):
                    return result^
            cursor = deepseek_json_skip_ws(view, value_end, headers[1] - 1)
            if cursor < headers[1] - 1 and deepseek_json_byte(view, cursor) == 44:
                cursor = deepseek_json_skip_ws(view, cursor + 1, headers[1] - 1)
                continue
            if cursor != headers[1] - 1:
                return Array[Int64, 2](fill=-1)^
            break
    elif deepseek_json_byte(view, headers[0]) == 91:
        var cursor = deepseek_json_skip_ws(view, headers[0] + 1, headers[1] - 1)
        while cursor < headers[1] - 1:
            var entry_end = deepseek_json_value_end(view, cursor, headers[1] - 1, 0)
            if entry_end < 0:
                return Array[Int64, 2](fill=-1)^
            var entry = Array[Int64, 2](fill=-1)
            entry[0] = cursor
            entry[1] = entry_end
            var name = Array[Int64, 2](fill=-1)
            var value = Array[Int64, 2](fill=-1)
            if deepseek_json_byte(view, entry[0]) == 91:
                var pair = runtime_response_raw_array_first_two(view, entry)
                name[0] = pair[0]
                name[1] = pair[1]
                value[0] = pair[2]
                value[1] = pair[3]
            elif deepseek_json_byte(view, entry[0]) == 123:
                name = deepseek_json_object_member(
                    view, entry[0], entry[1], StringSlice("name")
                )
                if not runtime_response_raw_present(name):
                    name = deepseek_json_object_member(
                        view, entry[0], entry[1], StringSlice("key")
                    )
                value = deepseek_json_object_member(
                    view, entry[0], entry[1], StringSlice("value")
                )
                if not runtime_response_raw_present(value):
                    value = deepseek_json_object_member(
                        view, entry[0], entry[1], StringSlice("values")
                    )
            if (
                runtime_response_raw_string(name, view)
                and runtime_response_raw_ascii_casefold_key(
                    view, name[0], name[1], StringSlice("x-codex-turn-state")
                )
            ):
                var result = runtime_response_raw_header_value(view, value, 0)
                if runtime_response_raw_present(result):
                    return result^
            cursor = deepseek_json_skip_ws(view, entry_end, headers[1] - 1)
            if cursor < headers[1] - 1 and deepseek_json_byte(view, cursor) == 44:
                cursor = deepseek_json_skip_ws(view, cursor + 1, headers[1] - 1)
                continue
            if cursor != headers[1] - 1:
                return Array[Int64, 2](fill=-1)^
            break
    return Array[Int64, 2](fill=-1)^


def runtime_response_raw_add_id(
    view: ProdexRichStringView,
    output: Pointer[mut=True, Int64, _],
    bounds: Array[Int64, 2],
):
    if not runtime_response_raw_string(bounds, view):
        return
    var count = output[unsafe_offset=0]
    for slot in range(count):
        var prior = Array[Int64, 2](fill=-1)
        prior[0] = output[unsafe_offset=1 + slot * 2]
        prior[1] = output[unsafe_offset=2 + slot * 2]
        if runtime_response_raw_token_equal(view, prior, bounds):
            return
    if count < 3:
        output[unsafe_offset=1 + count * 2] = bounds[0]
        output[unsafe_offset=2 + count * 2] = bounds[1]
        output[unsafe_offset=0] = count + 1


def runtime_response_raw_u64_number(
    view: ProdexRichStringView, bounds: Array[Int64, 2]
) -> Bool:
    if not runtime_response_raw_present(bounds):
        return False
    var start = bounds[0]
    var end = bounds[1]
    var source = rich_view_ptr(view)
    if source[unsafe_offset=start] == 45:
        return end - start == 2 and source[unsafe_offset=start + 1] == 48
    if end - start > 20:
        return False
    var parsed: UInt64 = 0
    for index in range(start, end):
        var byte = source[unsafe_offset=index]
        if byte < 48 or byte > 57:
            return False
        var digit = UInt64(byte - 48)
        if parsed > 1844674407370955161 or (
            parsed == 1844674407370955161 and digit > 5
        ):
            return False
        parsed = parsed * 10 + digit
    return end > start


def runtime_response_raw_u64_field(
    view: ProdexRichStringView,
    object: Array[Int64, 2],
    name: StringSlice,
) -> Array[Int64, 2]:
    if (
        not runtime_response_raw_present(object)
        or deepseek_json_byte(view, object[0]) != 123
    ):
        return Array[Int64, 2](fill=-1)^
    var field = deepseek_json_object_member(view, object[0], object[1], name)
    return field^ if runtime_response_raw_u64_number(view, field) else Array[Int64, 2](fill=-1)^


def runtime_response_raw_u64_nested(
    view: ProdexRichStringView,
    object: Array[Int64, 2],
    object_name: StringSlice,
    field_name: StringSlice,
) -> Array[Int64, 2]:
    if (
        not runtime_response_raw_present(object)
        or deepseek_json_byte(view, object[0]) != 123
    ):
        return Array[Int64, 2](fill=-1)^
    var nested = deepseek_json_object_member(
        view, object[0], object[1], object_name
    )
    return runtime_response_raw_u64_field(view, nested, field_name)


def runtime_response_raw_usage(
    view: ProdexRichStringView, value: Array[Int64, 2]
) -> Array[Int64, 9]:
    var result = Array[Int64, 9](fill=-1)
    var input_tokens = runtime_response_raw_u64_field(
        view, value, StringSlice("input_tokens")
    )
    if not runtime_response_raw_present(input_tokens):
        input_tokens = runtime_response_raw_u64_field(
            view, value, StringSlice("prompt_tokens")
        )
    var cached_tokens = runtime_response_raw_u64_field(
        view, value, StringSlice("cached_input_tokens")
    )
    if not runtime_response_raw_present(cached_tokens):
        cached_tokens = runtime_response_raw_u64_nested(
            view, value, StringSlice("input_tokens_details"), StringSlice("cached_tokens")
        )
    if not runtime_response_raw_present(cached_tokens):
        cached_tokens = runtime_response_raw_u64_nested(
            view, value, StringSlice("input_tokens_details"), StringSlice("cached_input_tokens")
        )
    if not runtime_response_raw_present(cached_tokens):
        cached_tokens = runtime_response_raw_u64_nested(
            view, value, StringSlice("prompt_tokens_details"), StringSlice("cached_tokens")
        )
    var output_tokens = runtime_response_raw_u64_field(
        view, value, StringSlice("output_tokens")
    )
    if not runtime_response_raw_present(output_tokens):
        output_tokens = runtime_response_raw_u64_field(
            view, value, StringSlice("completion_tokens")
        )
    var reasoning_tokens = runtime_response_raw_u64_field(
        view, value, StringSlice("reasoning_tokens")
    )
    if not runtime_response_raw_present(reasoning_tokens):
        reasoning_tokens = runtime_response_raw_u64_nested(
            view, value, StringSlice("output_tokens_details"), StringSlice("reasoning_tokens")
        )
    if not runtime_response_raw_present(reasoning_tokens):
        reasoning_tokens = runtime_response_raw_u64_nested(
            view, value, StringSlice("completion_tokens_details"), StringSlice("reasoning_tokens")
        )
    if (
        runtime_response_raw_present(input_tokens)
        or runtime_response_raw_present(cached_tokens)
        or runtime_response_raw_present(output_tokens)
        or runtime_response_raw_present(reasoning_tokens)
    ):
        result[0] = 1
        result[1] = input_tokens[0]
        result[2] = input_tokens[1]
        result[3] = cached_tokens[0]
        result[4] = cached_tokens[1]
        result[5] = output_tokens[0]
        result[6] = output_tokens[1]
        result[7] = reasoning_tokens[0]
        result[8] = reasoning_tokens[1]
    return result^


def runtime_response_raw_copy_usage(
    target: Pointer[mut=True, Int64, _],
    source: Array[Int64, 9],
):
    for index in range(9):
        target[unsafe_offset=index] = source[index]


def runtime_response_raw_scan_usage(
    view: ProdexRichStringView,
    bounds: Array[Int64, 2],
    scanned: Pointer[mut=True, Int64, _],
    output: Pointer[mut=True, Int64, _],
    depth: Int64,
) -> Bool:
    if (
        depth > RUNTIME_RESPONSE_METADATA_MAX_DEPTH
        or not runtime_response_raw_present(bounds)
        or scanned[] >= RUNTIME_RESPONSE_METADATA_SCAN_LIMIT
    ):
        return False
    scanned[] += 1
    var opening = deepseek_json_byte(view, bounds[0])
    if opening == 123:
        var usage_member = deepseek_json_object_member(
            view, bounds[0], bounds[1], StringSlice("usage")
        )
        var usage = runtime_response_raw_usage(view, usage_member)
        if usage[0] == 1:
            runtime_response_raw_copy_usage(output, usage)
            return True
        usage = runtime_response_raw_usage(view, bounds)
        if usage[0] == 1:
            runtime_response_raw_copy_usage(output, usage)
            return True
        var cursor = deepseek_json_skip_ws(view, bounds[0] + 1, bounds[1] - 1)
        while cursor < bounds[1] - 1:
            var key_end = deepseek_json_string_end(view, cursor, bounds[1] - 1)
            if key_end < 0:
                return False
            cursor = deepseek_json_skip_ws(view, key_end, bounds[1] - 1)
            if cursor >= bounds[1] - 1 or deepseek_json_byte(view, cursor) != 58:
                return False
            var child_start = deepseek_json_skip_ws(view, cursor + 1, bounds[1] - 1)
            var child_end = deepseek_json_value_end(view, child_start, bounds[1] - 1, depth + 1)
            if child_end < 0:
                return False
            var child = Array[Int64, 2](fill=-1)
            child[0] = child_start
            child[1] = child_end
            if runtime_response_raw_scan_usage(
                view, child, scanned, output, depth + 1
            ):
                return True
            if scanned[] >= RUNTIME_RESPONSE_METADATA_SCAN_LIMIT:
                return False
            cursor = deepseek_json_skip_ws(view, child_end, bounds[1] - 1)
            if cursor < bounds[1] - 1 and deepseek_json_byte(view, cursor) == 44:
                cursor = deepseek_json_skip_ws(view, cursor + 1, bounds[1] - 1)
                continue
            if cursor != bounds[1] - 1:
                return False
            break
    elif opening == 91:
        var cursor = deepseek_json_skip_ws(view, bounds[0] + 1, bounds[1] - 1)
        while cursor < bounds[1] - 1:
            var child_end = deepseek_json_value_end(view, cursor, bounds[1] - 1, depth + 1)
            if child_end < 0:
                return False
            var child = Array[Int64, 2](fill=-1)
            child[0] = cursor
            child[1] = child_end
            if runtime_response_raw_scan_usage(
                view, child, scanned, output, depth + 1
            ):
                return True
            if scanned[] >= RUNTIME_RESPONSE_METADATA_SCAN_LIMIT:
                return False
            cursor = deepseek_json_skip_ws(view, child_end, bounds[1] - 1)
            if cursor < bounds[1] - 1 and deepseek_json_byte(view, cursor) == 44:
                cursor = deepseek_json_skip_ws(view, cursor + 1, bounds[1] - 1)
                continue
            if cursor != bounds[1] - 1:
                return False
            break
    return False


@export("prodex_runtime_response_metadata_event_kind_v1")
def prodex_runtime_response_metadata_event_kind_v1(
    abi_version: Int64,
    event_address: UInt,
    event_length: Int64,
    event_present: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != RUNTIME_RESPONSE_EVENT_KIND_ABI_VERSION
        or event_present < 0
        or event_present > 1
        or event_length < 0
        or output_address == 0
        or (event_present == 1 and event_length > 0 and event_address == 0)
    ):
        return RUNTIME_RESPONSE_METADATA_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = 0
    if event_present == 0:
        return 0
    if event_length > 4_096:
        return 0
    var event = ProdexRichStringView(event_address, UInt(event_length))
    if not rich_view_valid(event, 4_096):
        return RUNTIME_RESPONSE_METADATA_INVALID
    output[] = Int64(rich_view_matches_literal["response.completed"](event, False))
    return 0


@export("prodex_runtime_response_metadata_json_v1")
def prodex_runtime_response_metadata_json_v1(
    abi_version: Int64,
    raw_address: UInt,
    raw_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != RUNTIME_RESPONSE_METADATA_ABI_VERSION
        or raw_address == 0
        or raw_length <= 0
        or output_address == 0
    ):
        return RUNTIME_RESPONSE_METADATA_INVALID
    var view = ProdexRichStringView(raw_address, UInt(raw_length))
    if (
        not rich_view_valid(view, 0x7FFFFFFFFFFFFFFF)
        or not deepseek_json_fragment_valid(view)
    ):
        return RUNTIME_RESPONSE_METADATA_INVALID
    var root_start = deepseek_json_skip_ws(view, 0, raw_length)
    var root_end = deepseek_json_value_end(view, root_start, raw_length, 0)
    if root_end < 0:
        return RUNTIME_RESPONSE_METADATA_INVALID
    var root = Array[Int64, 2](fill=-1)
    root[0] = root_start
    root[1] = root_end

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(22):
        output[unsafe_offset=index] = -1
    output[unsafe_offset=0] = 0
    output[unsafe_offset=13] = 0

    var direct_headers_state = runtime_response_raw_headers_state(view, root)
    runtime_response_raw_write_bounds(output, 11, direct_headers_state)

    if deepseek_json_byte(view, root[0]) == 123:
        var response = deepseek_json_object_member(
            view, root[0], root[1], StringSlice("response")
        )
        if runtime_response_raw_present(response) and deepseek_json_byte(view, response[0]) == 123:
            runtime_response_raw_add_id(
                view, output, deepseek_json_object_member(
                    view, response[0], response[1], StringSlice("id")
                )
            )
        runtime_response_raw_add_id(
            view, output, deepseek_json_object_member(
                view, root[0], root[1], StringSlice("response_id")
            )
        )
        var object = deepseek_json_object_member(
            view, root[0], root[1], StringSlice("object")
        )
        if (
            deepseek_json_raw_equals(
                view, object[0], object[1], StringSlice("response")
            )
            or runtime_response_raw_string_ends_with(
                view, object, StringSlice(".response")
            )
        ):
            runtime_response_raw_add_id(
                view, output, deepseek_json_object_member(
                    view, root[0], root[1], StringSlice("id")
                )
            )

        var event_type = deepseek_json_object_member(
            view, root[0], root[1], StringSlice("type")
        )
        if runtime_response_raw_string_nonblank(view, event_type):
            runtime_response_raw_write_bounds(output, 7, event_type)

        var headers = deepseek_json_object_member(
            view, root[0], root[1], StringSlice("headers")
        )
        var headers_state = runtime_response_raw_headers_state(view, headers)

        var turn_state = Array[Int64, 2](fill=-1)
        if runtime_response_raw_present(response) and deepseek_json_byte(view, response[0]) == 123:
            turn_state = runtime_response_raw_headers_state(
                view,
                deepseek_json_object_member(
                    view, response[0], response[1], StringSlice("headers")
                ),
            )
        if not runtime_response_raw_present(turn_state):
            turn_state = headers_state.copy()
        if not runtime_response_raw_present(turn_state) and runtime_response_raw_present(response):
            turn_state = deepseek_json_object_member(
                view, response[0], response[1], StringSlice("turn_state")
            )
            if not runtime_response_raw_string_nonblank(view, turn_state):
                turn_state = Array[Int64, 2](fill=-1)
        if not runtime_response_raw_present(turn_state) and runtime_response_raw_present(response):
            turn_state = deepseek_json_object_member(
                view, response[0], response[1], StringSlice("turnState")
            )
            if not runtime_response_raw_string_nonblank(view, turn_state):
                turn_state = Array[Int64, 2](fill=-1)
        if not runtime_response_raw_present(turn_state):
            turn_state = deepseek_json_object_member(
                view, root[0], root[1], StringSlice("turn_state")
            )
            if not runtime_response_raw_string_nonblank(view, turn_state):
                turn_state = Array[Int64, 2](fill=-1)
        if not runtime_response_raw_present(turn_state):
            turn_state = deepseek_json_object_member(
                view, root[0], root[1], StringSlice("turnState")
            )
            if not runtime_response_raw_string_nonblank(view, turn_state):
                turn_state = Array[Int64, 2](fill=-1)
        runtime_response_raw_write_bounds(output, 9, turn_state)

    var usage = Array[Int64, 9](fill=-1)
    usage[0] = 0
    var scanned: Int64 = 0
    _ = runtime_response_raw_scan_usage(
        view, root, Pointer(to=scanned), Pointer(to=usage[0]), 0
    )
    output[unsafe_offset=13] = usage[0]
    for slot in range(4):
        output[unsafe_offset=14 + slot * 2] = usage[1 + slot * 2]
        output[unsafe_offset=15 + slot * 2] = usage[2 + slot * 2]
    return 0
