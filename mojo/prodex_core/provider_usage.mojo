from std.memory import Pointer

from json_view import (
    deepseek_json_object_member,
    deepseek_json_skip_ws,
    deepseek_json_value_end,
)
from rich_text import rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView
from parsed_json import (
    JSON_ARRAY,
    JSON_OBJECT,
    JSON_STRING,
    ParsedJson,
    ParsedJsonNode,
    pj_child,
    pj_field,
    pj_kind,
    pj_next,
    pj_text,
    pj_valid,
)
from rich_text import rich_codepoint, rich_codepoint_width, rich_view_matches_literal

comptime PROVIDER_USAGE_ABI_VERSION: Int64 = 1
comptime PROVIDER_USAGE_OK: Int64 = 0
comptime PROVIDER_USAGE_INVALID: Int64 = 1


def usage_root_bounds(view: ProdexRichStringView) -> Tuple[Int64, Int64]:
    var start = deepseek_json_skip_ws(view, 0, Int64(view.len))
    var end = deepseek_json_value_end(view, start, Int64(view.len), 0)
    if end < 0 or deepseek_json_skip_ws(view, end, Int64(view.len)) != Int64(
        view.len
    ):
        return (-1, -1)
    return (start, end)


def usage_parse_u64(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Tuple[Bool, UInt64]:
    if start < 0 or end <= start:
        return (False, UInt64(0))
    var source = rich_view_ptr(view)
    var value = UInt64(0)
    for index in range(start, end):
        var byte = source[unsafe_offset=index]
        if byte < 48 or byte > 57:
            return (False, UInt64(0))
        var digit = UInt64(0)
        if byte == 49:
            digit = UInt64(1)
        elif byte == 50:
            digit = UInt64(2)
        elif byte == 51:
            digit = UInt64(3)
        elif byte == 52:
            digit = UInt64(4)
        elif byte == 53:
            digit = UInt64(5)
        elif byte == 54:
            digit = UInt64(6)
        elif byte == 55:
            digit = UInt64(7)
        elif byte == 56:
            digit = UInt64(8)
        elif byte == 57:
            digit = UInt64(9)
        if value > UInt64(1844674407370955161) or (
            value == UInt64(1844674407370955161) and digit > UInt64(5)
        ):
            return (False, UInt64(0))
        value = value * UInt64(10) + digit
    return (True, value)


def usage_member_u64(
    view: ProdexRichStringView,
    object_start: Int64,
    object_end: Int64,
    key: StringSlice,
) -> Tuple[Bool, UInt64]:
    var bounds = deepseek_json_object_member(
        view, object_start, object_end, key
    )
    return usage_parse_u64(view, bounds[0], bounds[1])


def usage_selected_object(
    view: ProdexRichStringView, root_start: Int64, root_end: Int64
) -> Tuple[Int64, Int64]:
    var usage = deepseek_json_object_member(
        view, root_start, root_end, StringSlice("usage")
    )
    if usage[0] >= 0:
        return (usage[0], usage[1])
    var response = deepseek_json_object_member(
        view, root_start, root_end, StringSlice("response")
    )
    if response[0] >= 0:
        usage = deepseek_json_object_member(
            view, response[0], response[1], StringSlice("usage")
        )
        if usage[0] >= 0:
            return (usage[0], usage[1])
    var message = deepseek_json_object_member(
        view, root_start, root_end, StringSlice("message")
    )
    if message[0] >= 0:
        usage = deepseek_json_object_member(
            view, message[0], message[1], StringSlice("usage")
        )
        if usage[0] >= 0:
            return (usage[0], usage[1])
    return (root_start, root_end)


def usage_input_tokens(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Tuple[Bool, UInt64]:
    var value = usage_member_u64(view, start, end, StringSlice("input_tokens"))
    if value[0]:
        return value
    value = usage_member_u64(view, start, end, StringSlice("prompt_tokens"))
    if value[0]:
        return value
    value = usage_member_u64(view, start, end, StringSlice("promptTokens"))
    if value[0]:
        return value
    value = usage_member_u64(view, start, end, StringSlice("inputTokens"))
    if value[0]:
        return value
    value = usage_member_u64(
        view, start, end, StringSlice("cache_creation_input_tokens")
    )
    return value


def usage_output_tokens(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Tuple[Bool, UInt64]:
    var value = usage_member_u64(view, start, end, StringSlice("output_tokens"))
    if value[0]:
        return value
    value = usage_member_u64(view, start, end, StringSlice("completion_tokens"))
    if value[0]:
        return value
    value = usage_member_u64(view, start, end, StringSlice("completionTokens"))
    if value[0]:
        return value
    value = usage_member_u64(view, start, end, StringSlice("outputTokens"))
    return value


def usage_total_tokens(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Tuple[Bool, UInt64]:
    var value = usage_member_u64(view, start, end, StringSlice("total_tokens"))
    if value[0]:
        return value
    return usage_member_u64(view, start, end, StringSlice("totalTokens"))


def usage_flag(value: Bool) -> UInt64:
    return UInt64(1) if value else UInt64(0)


@export("prodex_provider_usage_extract_v1")
def prodex_provider_usage_extract_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PROVIDER_USAGE_ABI_VERSION
        or length < 0
        or (length > 0 and address == 0)
        or output_address == 0
    ):
        return PROVIDER_USAGE_INVALID

    var view = ProdexRichStringView(address, UInt(length))
    if not rich_view_valid(view, length):
        return PROVIDER_USAGE_INVALID

    var root = usage_root_bounds(view)
    if root[0] < 0:
        return PROVIDER_USAGE_INVALID

    var selected = usage_selected_object(view, root[0], root[1])
    var input = usage_input_tokens(view, selected[0], selected[1])
    var output = usage_output_tokens(view, selected[0], selected[1])
    var total = usage_total_tokens(view, selected[0], selected[1])

    var metadata = deepseek_json_object_member(
        view, root[0], root[1], StringSlice("usageMetadata")
    )
    if not input[0] and metadata[0] >= 0:
        input = usage_member_u64(
            view, metadata[0], metadata[1], StringSlice("promptTokenCount")
        )
    if not output[0] and metadata[0] >= 0:
        output = usage_member_u64(
            view, metadata[0], metadata[1], StringSlice("candidatesTokenCount")
        )
    if not total[0] and metadata[0] >= 0:
        total = usage_member_u64(
            view, metadata[0], metadata[1], StringSlice("totalTokenCount")
        )

    var out = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    out[0] = usage_flag(input[0])
    out[1] = input[1]
    out[2] = usage_flag(output[0])
    out[3] = output[1]
    out[4] = usage_flag(total[0])
    out[5] = total[1]
    return PROVIDER_USAGE_OK


def usage_saturating_mul(left: UInt64, right: UInt64) -> UInt64:
    if left == 0 or right == 0:
        return UInt64(0)
    if left > UInt64(18446744073709551615) // right:
        return UInt64(18446744073709551615)
    return left * right


def usage_saturating_add(left: UInt64, right: UInt64) -> UInt64:
    if UInt64(18446744073709551615) - left < right:
        return UInt64(18446744073709551615)
    return left + right


@export("prodex_provider_usage_cost_v1")
def prodex_provider_usage_cost_v1(
    abi_version: Int64,
    input_present: Int64,
    input_tokens: UInt64,
    input_rate_present: Int64,
    input_rate: UInt64,
    output_present: Int64,
    output_tokens: UInt64,
    output_rate_present: Int64,
    output_rate: UInt64,
    result_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PROVIDER_USAGE_ABI_VERSION
        or result_address == 0
        or (input_present != 0 and input_present != 1)
        or (input_rate_present != 0 and input_rate_present != 1)
        or (output_present != 0 and output_present != 1)
        or (output_rate_present != 0 and output_rate_present != 1)
    ):
        return PROVIDER_USAGE_INVALID

    var known = False
    var total = UInt64(0)
    if input_present == 1 and input_rate_present == 1:
        total = usage_saturating_add(
            total,
            usage_saturating_mul(input_tokens, input_rate) // UInt64(1_000_000),
        )
        known = True
    if output_present == 1 and output_rate_present == 1:
        total = usage_saturating_add(
            total,
            usage_saturating_mul(output_tokens, output_rate)
            // UInt64(1_000_000),
        )
        known = True

    var result = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[0] = usage_flag(known)
    result[1] = total
    return PROVIDER_USAGE_OK


@export("prodex_provider_usage_merged_total_v1")
def prodex_provider_usage_merged_total_v1(
    abi_version: Int64,
    total_present: Int64,
    total_tokens: UInt64,
    input_present: Int64,
    input_tokens: UInt64,
    output_present: Int64,
    output_tokens: UInt64,
    result_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PROVIDER_USAGE_ABI_VERSION
        or result_address == 0
        or (total_present != 0 and total_present != 1)
        or (input_present != 0 and input_present != 1)
        or (output_present != 0 and output_present != 1)
    ):
        return PROVIDER_USAGE_INVALID

    var known = False
    var total = UInt64(0)
    if total_present == 1:
        total = total_tokens
        known = True
    elif input_present == 1:
        total = input_tokens
        if output_present == 1:
            total = usage_saturating_add(total, output_tokens)
        known = True

    var result = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[0] = usage_flag(known)
    result[1] = total
    return PROVIDER_USAGE_OK


def usage_latest_present_value(
    previous_present: Int64,
    previous_tokens: UInt64,
    incoming_present: Int64,
    incoming_tokens: UInt64,
) -> Tuple[Bool, UInt64]:
    if incoming_present == 1:
        return (True, incoming_tokens)
    return (previous_present == 1, previous_tokens)


def usage_estimate_text_tokens(view: ProdexRichStringView) -> UInt64:
    var source = rich_view_ptr(view)
    var cursor: Int64 = 0
    var significant: UInt64 = 0
    while cursor < Int64(view.len):
        var width = rich_codepoint_width(source[unsafe_offset=cursor])
        var codepoint = rich_codepoint(source, cursor, width)
        if codepoint > 31 and (codepoint < 127 or codepoint > 159):
            significant += UInt64(1)
        cursor += width
    return usage_saturating_add(significant, UInt64(3)) // UInt64(4)


def usage_estimate_ignored_key(key: ProdexRichStringView) -> Bool:
    return (
        rich_view_matches_literal["model"](key, False)
        or rich_view_matches_literal["role"](key, False)
        or rich_view_matches_literal["type"](key, False)
        or rich_view_matches_literal["id"](key, False)
        or rich_view_matches_literal["name"](key, False)
        or rich_view_matches_literal["metadata"](key, False)
        or rich_view_matches_literal["temperature"](key, False)
        or rich_view_matches_literal["top_p"](key, False)
        or rich_view_matches_literal["stream"](key, False)
    )


def usage_estimate_after_subtree(
    tree: ParsedJson, index: Int64, root: Int64
) -> Int64:
    var cursor = index
    while cursor != root:
        var next = pj_next(tree, cursor)
        if next >= 0:
            return next
        cursor = tree.nodes[unsafe_offset=cursor].parent
    return -1


def usage_estimate_next(
    tree: ParsedJson, index: Int64, root: Int64
) -> Int64:
    var kind = pj_kind(tree, index)
    if kind == JSON_ARRAY or kind == JSON_OBJECT:
        var child = pj_child(tree, index)
        while child >= 0 and usage_estimate_ignored_key(
            tree.nodes[unsafe_offset=child].key
        ):
            child = usage_estimate_after_subtree(tree, child, root)
        if child >= 0:
            return child
    return usage_estimate_after_subtree(tree, index, root)


def usage_estimate_tree_value(
    tree: ParsedJson, root: Int64
) -> UInt64:
    var total = UInt64(0)
    var index = root
    while index >= 0:
        if usage_estimate_ignored_key(tree.nodes[unsafe_offset=index].key):
            index = usage_estimate_after_subtree(tree, index, root)
            continue
        if pj_kind(tree, index) == JSON_STRING:
            total = usage_saturating_add(
                total, usage_estimate_text_tokens(pj_text(tree, index))
            )
        index = usage_estimate_next(tree, index, root)
    return total


def usage_estimate_root_field(tree: ParsedJson, field: Int64) -> Int64:
    if field == 0:
        return pj_field(tree, 0, StringSlice("input"))
    if field == 1:
        return pj_field(tree, 0, StringSlice("messages"))
    if field == 2:
        return pj_field(tree, 0, StringSlice("contents"))
    if field == 3:
        return pj_field(tree, 0, StringSlice("content"))
    if field == 4:
        return pj_field(tree, 0, StringSlice("parts"))
    if field == 5:
        return pj_field(tree, 0, StringSlice("prompt"))
    if field == 6:
        return pj_field(tree, 0, StringSlice("system"))
    if field == 7:
        return pj_field(tree, 0, StringSlice("instructions"))
    if field == 8:
        return pj_field(tree, 0, StringSlice("tools"))
    return -1


def usage_estimate_json_value(tree: ParsedJson) -> UInt64:
    if pj_kind(tree, 0) != JSON_OBJECT:
        return UInt64(0)
    var total = UInt64(0)
    for field in range(9):
        var index = usage_estimate_root_field(tree, Int64(field))
        if index >= 0:
            total = usage_saturating_add(
                total, usage_estimate_tree_value(tree, index)
            )
    return total


@export("prodex_provider_usage_merge_latest_present_v1")
def prodex_provider_usage_merge_latest_present_v1(
    abi_version: Int64,
    previous_input_present: Int64,
    previous_input_tokens: UInt64,
    incoming_input_present: Int64,
    incoming_input_tokens: UInt64,
    previous_output_present: Int64,
    previous_output_tokens: UInt64,
    incoming_output_present: Int64,
    incoming_output_tokens: UInt64,
    previous_total_present: Int64,
    previous_total_tokens: UInt64,
    incoming_total_present: Int64,
    incoming_total_tokens: UInt64,
    result_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PROVIDER_USAGE_ABI_VERSION
        or result_address == 0
        or (previous_input_present != 0 and previous_input_present != 1)
        or (incoming_input_present != 0 and incoming_input_present != 1)
        or (previous_output_present != 0 and previous_output_present != 1)
        or (incoming_output_present != 0 and incoming_output_present != 1)
        or (previous_total_present != 0 and previous_total_present != 1)
        or (incoming_total_present != 0 and incoming_total_present != 1)
    ):
        return PROVIDER_USAGE_INVALID

    var input = usage_latest_present_value(
        previous_input_present,
        previous_input_tokens,
        incoming_input_present,
        incoming_input_tokens,
    )
    var output = usage_latest_present_value(
        previous_output_present,
        previous_output_tokens,
        incoming_output_present,
        incoming_output_tokens,
    )
    var total = usage_latest_present_value(
        previous_total_present,
        previous_total_tokens,
        incoming_total_present,
        incoming_total_tokens,
    )

    var result = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[0] = usage_flag(input[0])
    result[1] = input[1]
    result[2] = usage_flag(output[0])
    result[3] = output[1]
    result[4] = usage_flag(total[0])
    result[5] = total[1]
    return PROVIDER_USAGE_OK


@export("prodex_provider_usage_estimate_v1")
def prodex_provider_usage_estimate_v1(
    abi_version: Int64,
    operation: Int64,
    nodes_address: UInt,
    nodes_count: Int64,
    raw_address: UInt,
    raw_length: Int64,
    text_address: UInt,
    text_length: Int64,
    body_empty: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PROVIDER_USAGE_ABI_VERSION
        or operation < 1
        or operation > 3
        or nodes_count < 0
        or raw_length < 0
        or text_length < 0
        or (nodes_count > 0 and nodes_address == 0)
        or (raw_length > 0 and raw_address == 0)
        or (text_length > 0 and text_address == 0)
        or (body_empty != 0 and body_empty != 1)
        or output_address == 0
    ):
        return PROVIDER_USAGE_INVALID

    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[0] = 0
    output[1] = 0

    var text = ProdexRichStringView(text_address, UInt(text_length))
    if not rich_view_valid(text, text_length):
        return PROVIDER_USAGE_INVALID

    if operation == 2:
        if nodes_count != 0 or raw_length != 0 or body_empty != 0:
            return PROVIDER_USAGE_INVALID
        output[0] = UInt64(1)
        output[1] = usage_estimate_text_tokens(text)
        return PROVIDER_USAGE_OK

    if operation == 1 or operation == 3:
        var has_tree = nodes_count > 0
        if (nodes_count == 0 and raw_length != 0) or (
            nodes_count > 0 and raw_length == 0
        ):
            return PROVIDER_USAGE_INVALID
        if body_empty == 1 and (has_tree or raw_length != 0 or text_length != 0):
            return PROVIDER_USAGE_INVALID
        if operation == 1 and (not has_tree or text_length != 0):
            return PROVIDER_USAGE_INVALID

        var estimated = UInt64(0)
        if has_tree:
            var tree = ParsedJson(
                Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](
                    unsafe_from_address=Int(nodes_address)
                ),
                nodes_count,
                ProdexRichStringView(raw_address, UInt(raw_length)),
            )
            if not pj_valid(tree):
                return PROVIDER_USAGE_INVALID
            estimated = usage_estimate_json_value(tree)

        if operation == 1:
            output[0] = usage_flag(estimated > 0)
            output[1] = estimated
        elif body_empty == 1:
            output[0] = UInt64(1)
        else:
            output[0] = UInt64(1)
            output[1] = (
                estimated
                if estimated > 0
                else max(usage_estimate_text_tokens(text), UInt64(1))
            )
        return PROVIDER_USAGE_OK

    return PROVIDER_USAGE_INVALID
