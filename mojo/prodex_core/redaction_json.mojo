# Shared selection for inspection and replacement. Paths hide arbitrary argument
# keys; schema-only wrappers deliberately retain "$" and its tool-schema exclusion.
from std.memory import Pointer
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_NULL, JSON_STRING, JSON_ARRAY, JSON_OBJECT,
    pj_child, pj_next, pj_field, pj_kind, pj_text, pj_valid, pj_is,
)
from rich_types import ProdexRichStringView
from redaction import (
    reaction_presidio_inspect_mode, reaction_presidio_sensitive_kind,
    reaction_presidio_known_metadata, reaction_presidio_unsupported_modality,
    reaction_view_equals_literal,
)
from json_sink import JsonSink, js_byte, js_literal, js_string, js_u64, js_view

comptime JSON_INSPECTION_MAX_DEPTH: Int64 = 32
comptime JSON_INSPECTION_MAX_VALUES: Int64 = 256
comptime JSON_INSPECTION_MAX_TEXT_BYTES: UInt = 1_048_576
# ABI v1 reserves 64 sixteen-byte scratch units. Each of at most 33
# path segments needs at most 22 bytes for an index or 13 for a known key.
comptime JSON_INSPECTION_PATH_CAPACITY: Int64 = 1024


@fieldwise_init
struct JsonInspectionState(Copyable):
    var count: Int64
    var text_bytes: UInt
    var partial: Bool
    var error: Int64


def inspection_walk(
    tree: ParsedJson, node: Int64, mode: Int64, sensitive: Int64,
    path: Pointer[mut=True, UInt8, MutUntrackedOrigin], path_length: Int64, depth: Int64,
    state: Pointer[mut=True, JsonInspectionState, _],
    sink: Pointer[mut=True, JsonSink, _],
):
    if state[].error != 0:
        return
    if depth > JSON_INSPECTION_MAX_DEPTH:
        state[].error = 1
        return
    var kind = pj_kind(tree, node)
    if kind == JSON_STRING:
        if mode == 0:
            return
        if state[].count >= JSON_INSPECTION_MAX_VALUES:
            state[].error = 2
            return
        var length = pj_text(tree, node).len
        if length > JSON_INSPECTION_MAX_TEXT_BYTES - state[].text_bytes:
            state[].error = 3
            return
        state[].text_bytes += length
        if state[].count > 0:
            js_byte(sink, 44)
        state[].count += 1
        js_literal(sink, StringSlice('{"node":'))
        js_u64(sink, UInt64(node))
        js_literal(sink, StringSlice(',"path":'))
        js_string(sink, ProdexRichStringView(
            UInt(Int(path)), UInt(path_length)
        ))
        js_literal(sink, StringSlice(',"sensitive_kind":'))
        if sensitive == 1:
            js_literal(sink, StringSlice('"private_key"'))
        elif sensitive == 2:
            js_literal(sink, StringSlice('"api_key"'))
        elif sensitive == 3:
            js_literal(sink, StringSlice('"access_token"'))
        elif sensitive == 4:
            js_literal(sink, StringSlice('"password"'))
        else:
            js_literal(sink, StringSlice("null"))
        js_byte(sink, 125)
    elif kind == JSON_ARRAY:
        var child = pj_child(tree, node)
        var index: Int64 = 0
        while child >= 0 and state[].error == 0:
            var child_path = JsonSink(path, Int64(JSON_INSPECTION_PATH_CAPACITY), path_length, False, False)
            js_byte(Pointer(to=child_path), 91)
            js_u64(Pointer(to=child_path), UInt64(index))
            js_byte(Pointer(to=child_path), 93)
            inspection_walk(
                tree, child, mode, sensitive, path, child_path.written,
                depth + 1, state, sink,
            )
            index += 1
            child = pj_next(tree, child)
    elif kind == JSON_OBJECT:
        var skip_tools = path_length == 1 or pj_is["developer"](
            tree, pj_field(tree, node, StringSlice("role"))
        )
        var child = pj_child(tree, node)
        while child >= 0 and state[].error == 0:
            var key = tree.nodes[unsafe_offset=child].key.copy()
            var child_kind = pj_kind(tree, child)
            if skip_tools and reaction_view_equals_literal(key, StringSlice("tools")):
                child = pj_next(tree, child)
                continue
            if reaction_presidio_unsupported_modality(key) and child_kind != JSON_NULL:
                state[].partial = True
                child = pj_next(tree, child)
                continue
            var field_mode = reaction_presidio_inspect_mode(key)
            if mode != 2 and field_mode == 0 and child_kind != JSON_NULL and not reaction_presidio_known_metadata(key):
                state[].partial = True
            var child_mode = 2 if mode == 2 else field_mode
            var child_sensitive = reaction_presidio_sensitive_kind(key)
            if child_sensitive == 0:
                child_sensitive = sensitive
            var child_path = JsonSink(path, Int64(JSON_INSPECTION_PATH_CAPACITY), path_length, False, False)
            if field_mode != 0:
                js_byte(Pointer(to=child_path), 46)
                js_view(Pointer(to=child_path), key)
            elif mode == 2:
                js_literal(Pointer(to=child_path), StringSlice(".*"))
            inspection_walk(
                tree, child, child_mode, child_sensitive, path, child_path.written,
                depth + 1, state, sink,
            )
            child = pj_next(tree, child)


@export("prodex_redaction_json_content_plan_v1")
def prodex_redaction_json_content_plan_v1(
    abi: Int64, operation: Int64, flag: Int64,
    nodes_address: UInt64, nodes_count: Int64,
    raw_address: UInt64, raw_length: Int64,
    scratch_address: UInt64, scratch_count: Int64,
    measuring: Int64, output_address: UInt64, capacity: Int64,
    metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (
        operation != 0 or flag != 0 or measuring < 0 or measuring > 1
        or raw_length < 0 or nodes_address == 0 or metadata_address == 0
        or capacity < 0 or scratch_count < 64 or scratch_address == 0
        or (measuring == 1 and (output_address != 0 or capacity != 0))
        or (measuring == 0 and output_address == 0)
    ):
        return 1
    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](
            unsafe_from_address=Int(nodes_address)
        ), nodes_count, ProdexRichStringView(UInt(raw_address), UInt(raw_length)),
    )
    if not pj_valid(tree):
        return 1
    var sink = JsonSink(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)),
        capacity, 0, measuring == 1, False,
    )
    var state = JsonInspectionState(0, 0, False, 0)
    js_literal(Pointer(to=sink), StringSlice('{"slots":['))
    var path = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(scratch_address))
    path[unsafe_offset=0] = 36
    inspection_walk(tree, 0, 0, 0, path, 1, 0, Pointer(to=state), Pointer(to=sink))
    js_literal(Pointer(to=sink), StringSlice('],"coverage":'))
    if state.count == 0:
        js_literal(Pointer(to=sink), StringSlice('"unsupported"'))
    elif state.partial:
        js_literal(Pointer(to=sink), StringSlice('"partial"'))
    else:
        js_literal(Pointer(to=sink), StringSlice('"full"'))
    js_literal(Pointer(to=sink), StringSlice(',"error":'))
    js_u64(Pointer(to=sink), UInt64(state.error))
    js_byte(Pointer(to=sink), 125)
    if sink.failed:
        return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = sink.written
    return 0
