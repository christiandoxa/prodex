# Gemini conversation content normalization over a borrowed Serde tree.
# This owner never executes tools or invents tool results; subsequent repair
# operations preserve the existing explicit missing-result sentinel contract.
from std.memory import Pointer
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_ARRAY, JSON_STRING,
    pj_child, pj_next, pj_field, pj_kind, pj_text, pj_literal, pj_equal, pj_valid,
)
from json_sink import JsonSink, js_byte, js_literal, js_string, js_raw
from rich_types import ProdexRichStringView


def gch_parts(tree: ParsedJson, content: Int64) -> Int64:
    var parts = pj_field(tree, content, StringSlice("parts"))
    return parts if pj_kind(tree, parts) == JSON_ARRAY else -1


def gch_role(tree: ParsedJson, content: Int64) -> ProdexRichStringView:
    var role = pj_field(tree, content, StringSlice("role"))
    if pj_kind(tree, role) == JSON_STRING:
        return pj_text(tree, role)
    return pj_literal(StringSlice("user"))


def gch_next_content(tree: ParsedJson, start: Int64) -> Int64:
    var current = start
    while current >= 0:
        if pj_child(tree, gch_parts(tree, current)) >= 0:
            return current
        current = pj_next(tree, current)
    return -1


def gch_write_group(
    sink: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson, first: Int64, end: Int64,
):
    js_byte(sink, 123)
    # Preserve all non-role/parts metadata of the first retained content.
    var field = pj_child(tree, first)
    while field >= 0:
        var key = tree.nodes[unsafe_offset=field].key.copy()
        if not pj_equal(key, pj_literal(StringSlice("role"))) and not pj_equal(
            key, pj_literal(StringSlice("parts"))
        ):
            js_string(sink, key)
            js_byte(sink, 58)
            js_raw(sink, tree, field)
            js_byte(sink, 44)
        field = pj_next(tree, field)
    js_literal(sink, StringSlice('"role":'))
    js_string(sink, gch_role(tree, first))
    js_literal(sink, StringSlice(',"parts":['))
    var current = first
    var emitted = False
    while current >= 0 and current != end:
        var part = pj_child(tree, gch_parts(tree, current))
        while part >= 0:
            if emitted:
                js_byte(sink, 44)
            js_raw(sink, tree, part)
            emitted = True
            part = pj_next(tree, part)
        current = pj_next(tree, current)
    js_literal(sink, StringSlice(']}'))


def gch_coalesce(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson):
    js_byte(sink, 91)
    var current = gch_next_content(tree, pj_child(tree, 0))
    var emitted = False
    while current >= 0:
        var role = gch_role(tree, current)
        var next = gch_next_content(tree, pj_next(tree, current))
        while next >= 0 and pj_equal(role, gch_role(tree, next)):
            next = gch_next_content(tree, pj_next(tree, next))
        if emitted:
            js_byte(sink, 44)
        gch_write_group(sink, tree, current, next)
        emitted = True
        current = next
    js_byte(sink, 93)


@export("prodex_mojo_gemini_content_hardening_v1")
def prodex_mojo_gemini_content_hardening_v1(
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
        or capacity < 0 or scratch_count < nodes_count or scratch_address == 0
    ):
        return 1
    if measuring == 1:
        if output_address != 0 or capacity != 0:
            return 1
    elif output_address == 0:
        return 1
    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](
            unsafe_from_address=Int(nodes_address)
        ), nodes_count, ProdexRichStringView(UInt(raw_address), UInt(raw_length)),
    )
    if not pj_valid(tree) or pj_kind(tree, 0) != JSON_ARRAY:
        return 1
    var sink = JsonSink(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)),
        capacity, 0, measuring == 1, False,
    )
    gch_coalesce(Pointer(to=sink), tree)
    if sink.failed:
        return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(metadata_address)
    )
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = sink.written
    return 0
