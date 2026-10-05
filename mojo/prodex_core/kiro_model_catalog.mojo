from std.memory import Pointer
from std.collections import Array

from parsed_json import (
    JSON_ARRAY,
    JSON_NUMBER,
    JSON_OBJECT,
    JSON_STRING,
    ParsedJson,
    ParsedJsonNode,
    pj_child,
    pj_field,
    pj_kind,
    pj_next,
    pj_string_field,
    pj_text,
    pj_trim_bounds,
    pj_valid,
)
from rich_types import ProdexRichStringView
from rich_text import rich_view_ptr

comptime KIRO_CATALOG_ABI_VERSION: Int64 = 1
comptime KIRO_CATALOG_STATUS_OK: Int64 = 0
comptime KIRO_CATALOG_STATUS_INVALID: Int64 = 1
comptime KIRO_CATALOG_STATUS_ABI: Int64 = 4
comptime KIRO_CATALOG_ISSUE_READY: Int64 = 0
comptime KIRO_CATALOG_ISSUE_MISSING_ARRAY: Int64 = 1
comptime KIRO_CATALOG_ISSUE_TOO_MANY: Int64 = 2
comptime KIRO_CATALOG_ISSUE_NO_USABLE_MODELS: Int64 = 3


@fieldwise_init
struct ProdexKiroCatalogRecord(Copyable):
    var source_index: Int64
    var id_node: Int64
    var id_start: Int64
    var id_length: Int64
    var name_node: Int64
    var name_start: Int64
    var name_length: Int64
    var description_node: Int64
    var context_window_tokens: UInt64
    var context_present: Int64


def kiro_catalog_nonempty_array(
    tree: ParsedJson, object_index: Int64, name: StringSlice
) -> Int64:
    var index = pj_field(tree, object_index, name)
    if pj_kind(tree, index) == JSON_ARRAY and pj_child(tree, index) >= 0:
        return index
    return -1


def kiro_catalog_array(tree: ParsedJson) -> Int64:
    if pj_kind(tree, 0) == JSON_ARRAY:
        return 0
    if pj_kind(tree, 0) != JSON_OBJECT:
        return -1

    var selected = kiro_catalog_nonempty_array(tree, 0, StringSlice("models"))
    if selected < 0:
        selected = kiro_catalog_nonempty_array(
            tree, 0, StringSlice("availableModels")
        )
    if selected < 0:
        selected = kiro_catalog_nonempty_array(
            tree, 0, StringSlice("available_models")
        )
    if selected < 0:
        selected = kiro_catalog_nonempty_array(
            tree, 0, StringSlice("supportedModels")
        )
    if selected < 0:
        selected = kiro_catalog_nonempty_array(
            tree, 0, StringSlice("supported_models")
        )
    if selected >= 0:
        return selected

    var nested = pj_field(tree, 0, StringSlice("models"))
    if pj_kind(tree, nested) != JSON_OBJECT:
        return -1
    selected = kiro_catalog_nonempty_array(
        tree, nested, StringSlice("availableModels")
    )
    if selected < 0:
        selected = kiro_catalog_nonempty_array(
            tree, nested, StringSlice("available_models")
        )
    if selected < 0:
        selected = kiro_catalog_nonempty_array(
            tree, nested, StringSlice("supportedModels")
        )
    if selected < 0:
        selected = kiro_catalog_nonempty_array(
            tree, nested, StringSlice("supported_models")
        )
    return selected


def kiro_catalog_nonblank_string(
    tree: ParsedJson, object_index: Int64, name: StringSlice
) -> Int64:
    var index = pj_string_field(tree, object_index, name)
    if index < 0:
        return -1
    var bounds = pj_trim_bounds(pj_text(tree, index))
    return index if bounds[1] > bounds[0] else -1


def kiro_catalog_first_id(tree: ParsedJson, object_index: Int64) -> Int64:
    var index = kiro_catalog_nonblank_string(
        tree, object_index, StringSlice("id")
    )
    if index < 0:
        index = kiro_catalog_nonblank_string(
            tree, object_index, StringSlice("model_id")
        )
    if index < 0:
        index = kiro_catalog_nonblank_string(
            tree, object_index, StringSlice("modelId")
        )
    if index < 0:
        index = kiro_catalog_nonblank_string(
            tree, object_index, StringSlice("slug")
        )
    if index < 0:
        index = kiro_catalog_nonblank_string(
            tree, object_index, StringSlice("model")
        )
    return index


def kiro_catalog_first_name(tree: ParsedJson, object_index: Int64) -> Int64:
    var index = kiro_catalog_nonblank_string(
        tree, object_index, StringSlice("name")
    )
    if index < 0:
        index = kiro_catalog_nonblank_string(
            tree, object_index, StringSlice("model_name")
        )
    if index < 0:
        index = kiro_catalog_nonblank_string(
            tree, object_index, StringSlice("modelName")
        )
    return index


def kiro_catalog_positive_u64(tree: ParsedJson, index: Int64) -> UInt64:
    if pj_kind(tree, index) != JSON_NUMBER:
        return 0
    var node = tree.nodes[unsafe_offset=index].copy()
    var text = ProdexRichStringView(
        tree.raw.ptr + UInt(node.raw_start), UInt(node.raw_length)
    )
    if text.len == 0:
        return 0
    var ptr = rich_view_ptr(text)
    var value: UInt64 = 0
    var maximum: UInt64 = 0xFFFFFFFFFFFFFFFF
    for offset in range(Int64(text.len)):
        var byte = ptr[unsafe_offset=offset]
        if byte < 48 or byte > 57:
            return 0
        var digit = UInt64(byte - 48)
        if value > (maximum - digit) / 10:
            return 0
        value = value * 10 + digit
    return value


def kiro_catalog_context_window(
    tree: ParsedJson, object_index: Int64
) -> UInt64:
    var index = pj_field(
        tree, object_index, StringSlice("context_window_tokens")
    )
    var value = kiro_catalog_positive_u64(tree, index)
    if value == 0:
        index = pj_field(tree, object_index, StringSlice("contextWindowTokens"))
        value = kiro_catalog_positive_u64(tree, index)
    return value


def kiro_catalog_span(tree: ParsedJson, index: Int64) -> Array[Int64, 2]:
    var bounds = pj_trim_bounds(pj_text(tree, index))
    var span = Array[Int64, 2](fill=0)
    span[0] = bounds[0]
    span[1] = bounds[1] - bounds[0]
    return span^


def kiro_model_catalog_normalize_v1(
    abi_version: Int64,
    nodes_address: UInt,
    nodes_count: Int64,
    raw_address: UInt,
    raw_length: Int64,
    max_entries: Int64,
    records_address: UInt,
    records_capacity: Int64,
    result_address: UInt,
) -> Int64:
    if result_address == 0:
        return KIRO_CATALOG_STATUS_INVALID
    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[unsafe_offset=0] = KIRO_CATALOG_ISSUE_MISSING_ARRAY
    result[unsafe_offset=1] = 0
    result[unsafe_offset=2] = 0
    if abi_version != KIRO_CATALOG_ABI_VERSION:
        return KIRO_CATALOG_STATUS_ABI
    if (
        nodes_address == 0
        or nodes_count <= 0
        or raw_length < 0
        or (raw_length > 0 and raw_address == 0)
        or max_entries <= 0
        or records_address == 0
        or records_capacity < max_entries
    ):
        return KIRO_CATALOG_STATUS_INVALID

    var nodes = Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](
        unsafe_from_address=Int(nodes_address)
    )
    var raw = ProdexRichStringView(raw_address, UInt(raw_length))
    var tree = ParsedJson(nodes, nodes_count, raw^)
    if not pj_valid(tree):
        return KIRO_CATALOG_STATUS_INVALID

    var models = kiro_catalog_array(tree)
    if models < 0:
        return KIRO_CATALOG_STATUS_OK

    var count: Int64 = 0
    var item = pj_child(tree, models)
    while item >= 0:
        count += 1
        if count > max_entries:
            result[unsafe_offset=0] = KIRO_CATALOG_ISSUE_TOO_MANY
            result[unsafe_offset=1] = count
            return KIRO_CATALOG_STATUS_OK
        item = pj_next(tree, item)
    result[unsafe_offset=1] = count

    var records = Pointer[
        mut=True, ProdexKiroCatalogRecord, MutUntrackedOrigin
    ](unsafe_from_address=Int(records_address))
    var written: Int64 = 0
    var source_index: Int64 = 0
    item = pj_child(tree, models)
    while item >= 0:
        if pj_kind(tree, item) == JSON_OBJECT:
            var id_node = kiro_catalog_first_id(tree, item)
            if id_node >= 0:
                var name_node = kiro_catalog_first_name(tree, item)
                if name_node < 0:
                    name_node = id_node
                var id_span = kiro_catalog_span(tree, id_node)
                var name_span = kiro_catalog_span(tree, name_node)
                var description = pj_string_field(
                    tree, item, StringSlice("description")
                )
                var context_window = kiro_catalog_context_window(tree, item)
                records[unsafe_offset=written] = ProdexKiroCatalogRecord(
                    source_index,
                    id_node,
                    id_span[0],
                    id_span[1],
                    name_node,
                    name_span[0],
                    name_span[1],
                    description,
                    context_window,
                    Int64(context_window > 0),
                )
                written += 1
        source_index += 1
        item = pj_next(tree, item)

    result[unsafe_offset=2] = written
    if written == 0:
        result[unsafe_offset=0] = KIRO_CATALOG_ISSUE_NO_USABLE_MODELS
    else:
        result[unsafe_offset=0] = KIRO_CATALOG_ISSUE_READY
    return KIRO_CATALOG_STATUS_OK
