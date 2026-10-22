from std.memory import Pointer
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_NULL, JSON_ARRAY, JSON_OBJECT,
    pj_child, pj_next, pj_field, pj_kind, pj_text, pj_trim, pj_valid,
)
from rich_types import ProdexRichStringView
from rich_text import rich_view_matches_literal
from json_sink import JsonSink, js_byte, js_literal, js_string, js_escaped, js_u64, js_raw
from provider_usage import usage_estimate_after_subtree
from kiro_model_catalog import kiro_catalog_positive_u64


def copilot_model_list_key(key: ProdexRichStringView) -> Bool:
    return (
        rich_view_matches_literal["models"](key, True)
        or rich_view_matches_literal["available_models"](key, True)
        or rich_view_matches_literal["model_catalog"](key, True)
        or rich_view_matches_literal["chat_models"](key, True)
        or rich_view_matches_literal["data"](key, True)
    )


# Matching model arrays contribute raw entries and stop descent into those entries.
def copilot_collect_models(tree: ParsedJson, limit: Int64, output: Pointer[mut=True, Int64, _]) -> Int64:
    var cursor: Int64 = 0
    var count: Int64 = 0
    while cursor >= 0:
        var node = tree.nodes[unsafe_offset=cursor].copy()
        if node.kind == JSON_ARRAY and pj_kind(tree, node.parent) == JSON_OBJECT and copilot_model_list_key(node.key):
            var child = pj_child(tree, cursor)
            while child >= 0:
                if count >= limit:
                    return -1
                output[unsafe_offset=count] = child
                count += 1
                child = pj_next(tree, child)
            cursor = usage_estimate_after_subtree(tree, cursor, 0)
        else:
            var child = pj_child(tree, cursor)
            cursor = child if child >= 0 else usage_estimate_after_subtree(tree, cursor, 0)
    return count


# Object null fields disappear recursively; null array elements retain their positions.
def copilot_emit_capabilities(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, index: Int64):
    var kind = pj_kind(tree, index)
    if kind != JSON_OBJECT and kind != JSON_ARRAY:
        js_raw(sink, tree, index)
        return
    js_byte(sink, UInt8(123 if kind == JSON_OBJECT else 91))
    var child = pj_child(tree, index)
    var first = True
    while child >= 0:
        if kind != JSON_OBJECT or pj_kind(tree, child) != JSON_NULL:
            if not first:
                js_byte(sink, 44)
            first = False
            if kind == JSON_OBJECT:
                js_string(sink, tree.nodes[unsafe_offset=child].key)
                js_byte(sink, 58)
            copilot_emit_capabilities(sink, tree, child)
        child = pj_next(tree, child)
    js_byte(sink, UInt8(125 if kind == JSON_OBJECT else 93))


def copilot_emit_model(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, index: Int64, comma: Bool) -> Bool:
    if pj_kind(tree, index) != JSON_OBJECT:
        return False
    var id_node = pj_field(tree, index, StringSlice("id"))
    if id_node < 0:
        id_node = pj_field(tree, index, StringSlice("model"))
    if id_node < 0:
        id_node = pj_field(tree, index, StringSlice("slug"))
    if id_node < 0:
        id_node = pj_field(tree, index, StringSlice("name"))
    var identifier = pj_trim(pj_text(tree, id_node))
    if identifier.len == 0:
        return False
    var name_node = pj_field(tree, index, StringSlice("name"))
    if name_node < 0:
        name_node = pj_field(tree, index, StringSlice("display_name"))
    if name_node < 0:
        name_node = pj_field(tree, index, StringSlice("label"))
    var name = pj_trim(pj_text(tree, name_node))
    if name.len == 0:
        name = identifier.copy()
    var capabilities = pj_field(tree, index, StringSlice("capabilities"))
    var limits = pj_field(tree, capabilities, StringSlice("limits"))
    var context_node = pj_field(tree, index, StringSlice("context_window"))
    if context_node < 0:
        context_node = pj_field(tree, index, StringSlice("context_window_tokens"))
    if context_node < 0:
        context_node = pj_field(tree, index, StringSlice("max_context_tokens"))
    if context_node < 0:
        context_node = pj_field(tree, index, StringSlice("max_input_tokens"))
    if context_node < 0:
        context_node = pj_field(tree, limits, StringSlice("max_context_window_tokens"))
    var prompt_node = pj_field(tree, index, StringSlice("max_prompt_tokens"))
    if prompt_node < 0:
        prompt_node = pj_field(tree, limits, StringSlice("max_prompt_tokens"))
    var context_limit = kiro_catalog_positive_u64(tree, context_node)
    var prompt_limit = kiro_catalog_positive_u64(tree, prompt_node)
    var effective: UInt64 = 200_000
    if prompt_limit > 1:
        effective = prompt_limit
    elif context_limit > 1:
        effective = context_limit
    if comma:
        js_byte(sink, 44)
    js_literal(sink, StringSlice('{"id":'))
    js_string(sink, identifier)
    js_literal(sink, StringSlice(',"object":"model","owned_by":"github-copilot","display_name":'))
    js_string(sink, name)
    js_literal(sink, StringSlice(',"description":"GitHub Copilot model available for this account: '))
    js_escaped(sink, name)
    js_literal(sink, StringSlice('.","context_window":'))
    js_u64(sink, effective)
    js_literal(sink, StringSlice(',"input_cost_per_million_microusd":0,"output_cost_per_million_microusd":0'))
    if context_limit > 1:
        js_literal(sink, StringSlice(',"max_context_window":'))
        js_u64(sink, context_limit)
    if prompt_limit > 1:
        js_literal(sink, StringSlice(',"max_prompt_tokens":'))
        js_u64(sink, prompt_limit)
    if capabilities >= 0 and pj_kind(tree, capabilities) != JSON_NULL:
        js_literal(sink, StringSlice(',"capabilities":'))
        copilot_emit_capabilities(sink, tree, capabilities)
    js_byte(sink, 125)
    return True


def copilot_emit_catalog(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, indices: Pointer[mut=True, Int64, _], count: Int64, limit: Int64):
    if count < 0:
        js_literal(sink, StringSlice('{"Err":"Copilot model catalog exceeds the hard limit of '))
        js_u64(sink, UInt64(limit))
        js_literal(sink, StringSlice(' entries"}'))
        return
    js_literal(sink, StringSlice('{"Ok":['))
    var emitted = False
    for offset in range(count):
        if copilot_emit_model(sink, tree, indices[unsafe_offset=offset], emitted):
            emitted = True
    js_literal(sink, StringSlice(']}'))


@export("prodex_mojo_copilot_model_catalog_v1")
def prodex_mojo_copilot_model_catalog_v1(
    abi: Int64, max_entries: Int64, flag: Int64,
    nodes_address: UInt64, nodes_count: Int64,
    raw_address: UInt64, raw_length: Int64,
    scratch_address: UInt64, scratch_count: Int64,
    measuring: Int64, output_address: UInt64, capacity: Int64, metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (
        max_entries < 0 or flag != 0
        or nodes_address == 0 or nodes_address % 8 != 0
        or nodes_count <= 0 or nodes_count > 0x7FFFFFFFFFFFFFFF // 80
        or raw_address == 0 or raw_length <= 0
        or scratch_address == 0 or scratch_address % 8 != 0 or scratch_count < nodes_count
        or measuring < 0 or measuring > 1 or capacity < 0
        or metadata_address == 0 or metadata_address % 8 != 0
    ):
        return 1
    if measuring == 1:
        if output_address != 0 or capacity != 0:
            return 1
    elif output_address == 0:
        return 1
    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](unsafe_from_address=Int(nodes_address)),
        nodes_count, ProdexRichStringView(UInt(raw_address), UInt(raw_length)),
    )
    if not pj_valid(tree):
        return 1
    var indices = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(scratch_address))
    var count = copilot_collect_models(tree, max_entries, indices)
    # Measuring never dereferences the sink's required non-null pointer.
    var sink = JsonSink(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(scratch_address)),
        0, 0, True, False,
    )
    copilot_emit_catalog(Pointer(to=sink), tree, indices, count, max_entries)
    if sink.failed:
        return 3
    var required = sink.written
    if measuring == 0:
        if capacity < required:
            return 3
        sink = JsonSink(
            Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)),
            capacity, 0, False, False,
        )
        copilot_emit_catalog(Pointer(to=sink), tree, indices, count, max_entries)
        if sink.failed or sink.written != required:
            return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = required
    return 0
