from std.memory import Pointer

from parsed_json import (
    JSON_ARRAY,
    JSON_NUMBER,
    JSON_OBJECT,
    JSON_STRING,
    ParsedJson,
    ParsedJsonNode,
    pj_child,
    pj_equal,
    pj_field,
    pj_kind,
    pj_next,
    pj_text,
    pj_trim,
    pj_valid,
)
from rich_text import rich_view_matches_literal, rich_view_ptr
from rich_types import ProdexRichStringView

comptime RUNTIME_RESPONSE_METADATA_ABI_VERSION: Int64 = 1
comptime RUNTIME_RESPONSE_METADATA_INVALID: Int64 = 1
comptime RUNTIME_RESPONSE_METADATA_SCAN_LIMIT: Int64 = 2048


def runtime_response_metadata_trimmed_string(
    tree: ParsedJson, index: Int64
) -> Array[Int64, 3]:
    var result = Array[Int64, 3](fill=-1)
    if pj_kind(tree, index) != JSON_STRING:
        return result^
    var value = pj_text(tree, index)
    var trimmed = pj_trim(value)
    if trimmed.len == 0:
        return result^
    result[0] = index
    result[1] = Int64(trimmed.ptr - value.ptr)
    result[2] = Int64(trimmed.len)
    return result^


def runtime_response_metadata_header_value(
    tree: ParsedJson, index: Int64
) -> Array[Int64, 3]:
    if pj_kind(tree, index) == JSON_STRING:
        return runtime_response_metadata_trimmed_string(tree, index)
    var result = Array[Int64, 3](fill=-1)
    if pj_kind(tree, index) != JSON_ARRAY:
        return result^
    var item = pj_child(tree, index)
    while item >= 0:
        result = runtime_response_metadata_header_value(tree, item)
        if result[0] >= 0:
            return result^
        item = pj_next(tree, item)
    return result^


def runtime_response_metadata_headers_state(
    tree: ParsedJson, headers: Int64
) -> Array[Int64, 3]:
    var result = Array[Int64, 3](fill=-1)
    if pj_kind(tree, headers) == JSON_OBJECT:
        var header = pj_child(tree, headers)
        while header >= 0:
            if rich_view_matches_literal["x-codex-turn-state"](
                tree.nodes[unsafe_offset=header].key, True
            ):
                result = runtime_response_metadata_header_value(tree, header)
                if result[0] >= 0:
                    return result^
            header = pj_next(tree, header)
    elif pj_kind(tree, headers) == JSON_ARRAY:
        var entry = pj_child(tree, headers)
        while entry >= 0:
            var name: Int64 = -1
            var value: Int64 = -1
            if pj_kind(tree, entry) == JSON_ARRAY:
                name = pj_child(tree, entry)
                if name >= 0:
                    value = pj_next(tree, name)
            elif pj_kind(tree, entry) == JSON_OBJECT:
                name = pj_field(tree, entry, StringSlice("name"))
                if name < 0:
                    name = pj_field(tree, entry, StringSlice("key"))
                value = pj_field(tree, entry, StringSlice("value"))
                if value < 0:
                    value = pj_field(tree, entry, StringSlice("values"))
            if (
                pj_kind(tree, name) == JSON_STRING
                and rich_view_matches_literal["x-codex-turn-state"](pj_text(tree, name), True)
            ):
                result = runtime_response_metadata_header_value(tree, value)
                if result[0] >= 0:
                    return result^
            entry = pj_next(tree, entry)
    return result^


def runtime_response_metadata_add_id(
    tree: ParsedJson,
    output: Pointer[mut=True, Int64, _],
    index: Int64,
):
    if pj_kind(tree, index) != JSON_STRING:
        return
    var count = output[unsafe_offset=0]
    for slot in range(count):
        var prior = output[unsafe_offset=slot + 1]
        if pj_equal(pj_text(tree, prior), pj_text(tree, index)):
            return
    if count < 3:
        output[unsafe_offset=count + 1] = index
        output[unsafe_offset=0] = count + 1


def runtime_response_metadata_ends_with(
    value: ProdexRichStringView, suffix: StringSlice
) -> Bool:
    var suffix_length = Int64(suffix.byte_length())
    if suffix_length > Int64(value.len):
        return False
    var source = rich_view_ptr(value)
    var target = suffix.unsafe_ptr()
    var start = Int64(value.len) - suffix_length
    for index in range(suffix_length):
        if source[unsafe_offset=start + index] != target[unsafe_offset=index]:
            return False
    return True


def runtime_response_metadata_u64_number(tree: ParsedJson, index: Int64) -> Bool:
    if pj_kind(tree, index) != JSON_NUMBER:
        return False
    var value = tree.nodes[unsafe_offset=index].text.copy()
    if value.len == 0:
        return False
    var bytes = rich_view_ptr(value)
    if bytes[unsafe_offset=0] == 45:
        return value.len == 2 and bytes[unsafe_offset=1] == 48
    if value.len > 20:
        return False
    var parsed: UInt64 = 0
    for offset in range(Int64(value.len)):
        var byte = bytes[unsafe_offset=offset]
        if byte < 48 or byte > 57:
            return False
        var digit = UInt64(byte - 48)
        if parsed > 1844674407370955161 or (
            parsed == 1844674407370955161 and digit > 5
        ):
            return False
        parsed = parsed * 10 + digit
    return True


def runtime_response_metadata_u64_field(
    tree: ParsedJson, object: Int64, name: StringSlice
) -> Int64:
    var field = pj_field(tree, object, name)
    return field if runtime_response_metadata_u64_number(tree, field) else -1


def runtime_response_metadata_u64_nested(
    tree: ParsedJson,
    object: Int64,
    object_name: StringSlice,
    field_name: StringSlice,
) -> Int64:
    var nested = pj_field(tree, object, object_name)
    return runtime_response_metadata_u64_field(tree, nested, field_name)


def runtime_response_metadata_usage(tree: ParsedJson, value: Int64) -> Array[Int64, 5]:
    var result = Array[Int64, 5](fill=-1)
    var input_tokens = runtime_response_metadata_u64_field(
        tree, value, StringSlice("input_tokens")
    )
    if input_tokens < 0:
        input_tokens = runtime_response_metadata_u64_field(
            tree, value, StringSlice("prompt_tokens")
        )
    var cached_tokens = runtime_response_metadata_u64_field(
        tree, value, StringSlice("cached_input_tokens")
    )
    if cached_tokens < 0:
        cached_tokens = runtime_response_metadata_u64_nested(
            tree, value, StringSlice("input_tokens_details"), StringSlice("cached_tokens")
        )
    if cached_tokens < 0:
        cached_tokens = runtime_response_metadata_u64_nested(
            tree,
            value,
            StringSlice("input_tokens_details"),
            StringSlice("cached_input_tokens"),
        )
    if cached_tokens < 0:
        cached_tokens = runtime_response_metadata_u64_nested(
            tree, value, StringSlice("prompt_tokens_details"), StringSlice("cached_tokens")
        )
    var output_tokens = runtime_response_metadata_u64_field(
        tree, value, StringSlice("output_tokens")
    )
    if output_tokens < 0:
        output_tokens = runtime_response_metadata_u64_field(
            tree, value, StringSlice("completion_tokens")
        )
    var reasoning_tokens = runtime_response_metadata_u64_field(
        tree, value, StringSlice("reasoning_tokens")
    )
    if reasoning_tokens < 0:
        reasoning_tokens = runtime_response_metadata_u64_nested(
            tree,
            value,
            StringSlice("output_tokens_details"),
            StringSlice("reasoning_tokens"),
        )
    if reasoning_tokens < 0:
        reasoning_tokens = runtime_response_metadata_u64_nested(
            tree,
            value,
            StringSlice("completion_tokens_details"),
            StringSlice("reasoning_tokens"),
        )
    if input_tokens >= 0 or cached_tokens >= 0 or output_tokens >= 0 or reasoning_tokens >= 0:
        result[0] = 1
        result[1] = input_tokens
        result[2] = cached_tokens
        result[3] = output_tokens
        result[4] = reasoning_tokens
    return result^


def runtime_response_metadata_token_usage(tree: ParsedJson) -> Array[Int64, 5]:
    var result = Array[Int64, 5](fill=-1)
    var count = min(tree.count, RUNTIME_RESPONSE_METADATA_SCAN_LIMIT)
    for index in range(count):
        if pj_kind(tree, index) != JSON_OBJECT:
            continue
        var usage = pj_field(tree, index, StringSlice("usage"))
        if usage >= 0:
            result = runtime_response_metadata_usage(tree, usage)
            if result[0] == 1:
                return result^
        result = runtime_response_metadata_usage(tree, index)
        if result[0] == 1:
            return result^
    result[0] = 0
    return result^


@export("prodex_runtime_response_metadata_v1")
def prodex_runtime_response_metadata_v1(
    abi_version: Int64,
    nodes_address: UInt,
    nodes_count: Int64,
    raw_address: UInt,
    raw_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != RUNTIME_RESPONSE_METADATA_ABI_VERSION
        or nodes_address == 0
        or nodes_count <= 0
        or raw_length < 0
        or output_address == 0
        or (raw_length > 0 and raw_address == 0)
    ):
        return RUNTIME_RESPONSE_METADATA_INVALID
    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](
            unsafe_from_address=Int(nodes_address)
        ),
        nodes_count,
        ProdexRichStringView(UInt(raw_address), UInt(raw_length)),
    )
    if not pj_valid(tree):
        return RUNTIME_RESPONSE_METADATA_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(18):
        output[unsafe_offset=index] = -1
    output[unsafe_offset=0] = 0
    output[unsafe_offset=13] = 0

    var direct_headers_state = runtime_response_metadata_headers_state(tree, 0)
    for slot in range(3):
        output[unsafe_offset=10 + slot] = direct_headers_state[slot]

    if pj_kind(tree, 0) == JSON_OBJECT:
        var response = pj_field(tree, 0, StringSlice("response"))
        runtime_response_metadata_add_id(
            tree, output, pj_field(tree, response, StringSlice("id"))
        )
        runtime_response_metadata_add_id(
            tree, output, pj_field(tree, 0, StringSlice("response_id"))
        )
        var object = pj_field(tree, 0, StringSlice("object"))
        if pj_kind(tree, object) == JSON_STRING:
            var object_text = pj_text(tree, object)
            if (
                rich_view_matches_literal["response"](object_text, False)
                or runtime_response_metadata_ends_with(
                    object_text, StringSlice(".response")
                )
            ):
                runtime_response_metadata_add_id(
                    tree, output, pj_field(tree, 0, StringSlice("id"))
                )

        var event_type = runtime_response_metadata_trimmed_string(
            tree, pj_field(tree, 0, StringSlice("type"))
        )
        for slot in range(3):
            output[unsafe_offset=4 + slot] = event_type[slot]
        var headers_state = runtime_response_metadata_headers_state(
            tree, pj_field(tree, 0, StringSlice("headers"))
        )

        var turn_state = runtime_response_metadata_headers_state(
            tree, pj_field(tree, response, StringSlice("headers"))
        )
        if turn_state[0] < 0:
            turn_state = headers_state.copy()
        if turn_state[0] < 0:
            turn_state = runtime_response_metadata_trimmed_string(
                tree, pj_field(tree, response, StringSlice("turn_state"))
            )
        if turn_state[0] < 0:
            turn_state = runtime_response_metadata_trimmed_string(
                tree, pj_field(tree, response, StringSlice("turnState"))
            )
        if turn_state[0] < 0:
            turn_state = runtime_response_metadata_trimmed_string(
                tree, pj_field(tree, 0, StringSlice("turn_state"))
            )
        if turn_state[0] < 0:
            turn_state = runtime_response_metadata_trimmed_string(
                tree, pj_field(tree, 0, StringSlice("turnState"))
            )
        for slot in range(3):
            output[unsafe_offset=7 + slot] = turn_state[slot]

    var usage = runtime_response_metadata_token_usage(tree)
    output[unsafe_offset=13] = usage[0]
    for slot in range(4):
        output[unsafe_offset=14 + slot] = usage[slot + 1]
    return 0
