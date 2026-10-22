from std.memory import Pointer
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_STRING, JSON_ARRAY, JSON_FALSE, JSON_TRUE,
    pj_kind, pj_string_field, pj_field, pj_text, pj_trim, pj_child, pj_next, pj_valid,
)
from rich_types import ProdexRichStringView
from rich_text import rich_view_matches_literal
from json_sink import JsonSink, js_byte, js_literal, js_string, js_raw
from runtime_model_catalog import runtime_catalog_unsigned


# Pick the first string before trimming; a blank primary blocks its alias.
def main_catalog_string(tree: ParsedJson, index: Int64, primary: StringSlice, alias: StringSlice) -> ProdexRichStringView:
    var field = pj_string_field(tree, index, primary)
    if field < 0:
        field = pj_string_field(tree, index, alias)
    return pj_trim(pj_text(tree, field))


def main_catalog_model(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, index: Int64):
    var identifier = main_catalog_string(tree, index, StringSlice("slug"), StringSlice("id"))
    var label = main_catalog_string(tree, index, StringSlice("display_name"), StringSlice("displayName"))
    if label.len == 0:
        label = identifier.copy()
    js_literal(sink, StringSlice('{"id":'))
    js_string(sink, identifier)
    js_literal(sink, StringSlice(',"label":'))
    js_string(sink, label)
    js_literal(sink, StringSlice(',"priority":'))
    var priority = pj_field(tree, index, StringSlice("priority"))
    if runtime_catalog_unsigned(tree, priority):
        js_raw(sink, tree, priority)
    else:
        js_literal(sink, StringSlice("9223372036854775807"))
    js_literal(sink, StringSlice(',"supported":'))
    js_literal(sink, StringSlice("false") if pj_kind(tree, pj_field(tree, index, StringSlice("supported_in_api"))) == JSON_FALSE else StringSlice("true"))
    js_literal(sink, StringSlice(',"hidden":'))
    js_literal(sink, StringSlice("true") if pj_kind(tree, pj_field(tree, index, StringSlice("hidden"))) == JSON_TRUE else StringSlice("false"))
    var visibility = pj_field(tree, index, StringSlice("visibility"))
    var listed = pj_kind(tree, visibility) != JSON_STRING or rich_view_matches_literal["list"](pj_text(tree, visibility), True)
    js_literal(sink, StringSlice(',"listed":'))
    js_literal(sink, StringSlice("true") if listed else StringSlice("false"))
    js_literal(sink, StringSlice(',"efforts":['))
    var levels = pj_field(tree, index, StringSlice("supported_reasoning_levels"))
    var child = pj_child(tree, levels) if pj_kind(tree, levels) == JSON_ARRAY else -1
    var first = True
    while child >= 0:
        var effort = pj_string_field(tree, child, StringSlice("effort"))
        if effort >= 0:
            if not first:
                js_byte(sink, 44)
            first = False
            js_raw(sink, tree, effort)
        child = pj_next(tree, child)
    js_literal(sink, StringSlice('],"aliases":['))
    var aliases = pj_field(tree, index, StringSlice("aliases"))
    child = pj_child(tree, aliases) if pj_kind(tree, aliases) == JSON_ARRAY else -1
    first = True
    while child >= 0:
        var alias = pj_trim(pj_text(tree, child))
        if alias.len > 0:
            if not first:
                js_byte(sink, 44)
            first = False
            js_string(sink, alias)
        child = pj_next(tree, child)
    js_literal(sink, StringSlice('],"default_effort":'))
    var default = main_catalog_string(tree, index, StringSlice("default_reasoning_level"), StringSlice("default_reasoning_effort"))
    if default.len > 0:
        js_string(sink, default)
    else:
        js_literal(sink, StringSlice("null"))
    js_byte(sink, 125)


def main_catalog_emit(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson):
    js_byte(sink, 91)
    var child = pj_child(tree, 0)
    var first = True
    while child >= 0:
        if not first:
            js_byte(sink, 44)
        first = False
        main_catalog_model(sink, tree, child)
        child = pj_next(tree, child)
    js_byte(sink, 93)


@export("prodex_mojo_main_catalog_projection_v1")
def prodex_mojo_main_catalog_projection_v1(
    abi: Int64, reserved: Int64, flags: Int64,
    nodes_address: UInt64, nodes_count: Int64, raw_address: UInt64, raw_length: Int64,
    scratch_address: UInt64, scratch_count: Int64,
    measuring: Int64, output_address: UInt64, capacity: Int64, metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (
        reserved != 0 or flags != 0 or nodes_address == 0 or nodes_address % 8 != 0
        or nodes_count <= 0 or nodes_count > 0x7FFFFFFFFFFFFFFF // 80
        or raw_address == 0 or raw_length <= 0 or scratch_address == 0 or scratch_address % 8 != 0
        or scratch_count < nodes_count or measuring < 0 or measuring > 1 or capacity < 0
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
    if not pj_valid(tree) or pj_kind(tree, 0) != JSON_ARRAY:
        return 1
    var sink = JsonSink(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(scratch_address)),
        0, 0, True, False,
    )
    main_catalog_emit(Pointer(to=sink), tree)
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
        main_catalog_emit(Pointer(to=sink), tree)
        if sink.failed or sink.written != required:
            return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = required
    return 0
