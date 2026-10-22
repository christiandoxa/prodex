# Session/turn context and reasoning text from the shared borrowed JSON tree.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid
from log_transcript_operation import transcript_operation_render
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_STRING, JSON_OBJECT, JSON_ARRAY,
    pj_valid, pj_kind, pj_field, pj_string_field, pj_text, pj_trim,
    pj_child, pj_next, pj_literal,
)
from json_sink import JsonSink, js_byte, js_literal, js_string, js_escaped


def transcript_context_field(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, node: Int64,
    label: StringSlice, count: Pointer[mut=True, Int64, _],
):
    if pj_kind(tree, node) != JSON_STRING:
        return
    if count[] > 0:
        js_byte(sink, 32)
    js_escaped(sink, pj_literal(label))
    js_byte(sink, 61)
    js_escaped(sink, pj_text(tree, node))
    count[] += 1


def transcript_context_fields(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, operation: Int64):
    var count: Int64 = 0
    if operation == 0:
        # Presence blocks the provider fallback even when the primary is not a string.
        var provider = pj_field(tree, 0, StringSlice("model_provider"))
        if provider < 0:
            provider = pj_field(tree, 0, StringSlice("provider"))
        transcript_context_field(sink, tree, provider, StringSlice("provider"), Pointer(to=count))
        for key in [StringSlice("source"), StringSlice("originator"), StringSlice("cwd")]:
            transcript_context_field(sink, tree, pj_field(tree, 0, key), key, Pointer(to=count))
    else:
        for key, label in [
            (StringSlice("model"), StringSlice("model")),
            (StringSlice("effort"), StringSlice("effort")),
            (StringSlice("summary"), StringSlice("summary")),
            (StringSlice("approval_policy"), StringSlice("approval")),
            (StringSlice("cwd"), StringSlice("cwd")),
        ]:
            transcript_context_field(sink, tree, pj_field(tree, 0, key), label, Pointer(to=count))


def transcript_reasoning_part(tree: ParsedJson, node: Int64) -> ProdexRichStringView:
    var selected = node
    if pj_kind(tree, node) == JSON_OBJECT:
        var text = pj_field(tree, node, StringSlice("text"))
        if text < 0:
            text = pj_field(tree, node, StringSlice("summary_text"))
        selected = text
    var part = pj_text(tree, selected)
    return part.copy() if pj_trim(part).len > 0 else ProdexRichStringView(0, 0)


def transcript_reasoning_text(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson):
    var summary = pj_field(tree, 0, StringSlice("summary"))
    if pj_kind(tree, summary) != JSON_ARRAY:
        js_escaped(sink, transcript_reasoning_part(tree, summary))
        return
    var child = pj_child(tree, summary)
    var first = True
    while child >= 0:
        var part = transcript_reasoning_part(tree, child)
        if part.len > 0:
            if not first:
                js_literal(sink, StringSlice("\\n"))
            js_escaped(sink, part)
            first = False
        child = pj_next(tree, child)


def transcript_context_record_begin(sink: Pointer[mut=True, JsonSink, _], source: StringSlice):
    if sink[].written > 1:
        js_byte(sink, 44)
    js_byte(sink, 91)
    js_string(sink, pj_literal(source))
    js_literal(sink, StringSlice(",\""))


def transcript_context_render(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, operation: Int64):
    js_byte(sink, 91)
    if operation == 0:
        var base = pj_field(tree, 0, StringSlice("base_instructions"))
        var text = pj_text(tree, pj_string_field(tree, base, StringSlice("text")))
        if pj_trim(text).len > 0:
            transcript_context_record_begin(sink, StringSlice("prompt-engineering"))
            js_escaped(sink, text)
            js_literal(sink, StringSlice("\"]"))
    var probe = JsonSink(sink[].output, 0, 0, True, False)
    if operation == 2:
        transcript_reasoning_text(Pointer(to=probe), tree)
    else:
        transcript_context_fields(Pointer(to=probe), tree, operation)
    if probe.written > 0:
        var source = StringSlice("session-context")
        if operation == 1:
            source = StringSlice("turn-context")
        elif operation == 2:
            source = StringSlice("reasoning")
        transcript_context_record_begin(sink, source)
        if operation == 2:
            transcript_reasoning_text(sink, tree)
        else:
            transcript_context_fields(sink, tree, operation)
        js_literal(sink, StringSlice("\"]"))
    js_byte(sink, 93)


@export("prodex_mojo_log_transcript_context_v1")
def prodex_mojo_log_transcript_context_v1(
    abi: Int64, operation: Int64, flag: Int64,
    nodes_address: UInt64, nodes_count: Int64, raw_address: UInt64, raw_length: Int64,
    scratch_address: UInt64, scratch_count: Int64,
    measuring: Int64, output_address: UInt64, capacity: Int64, metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (operation < 0 or operation > 16 or flag != 0 or nodes_count <= 0 or nodes_count > 0x7FFFFFFFFFFFFFFF // 80
        or nodes_address == 0 or nodes_address % 8 != 0 or raw_length < 0
        or scratch_count != nodes_count * Int64(2 if operation >= 3 else 1) or scratch_address == 0 or scratch_address % 8 != 0
        or capacity < 0 or metadata_address == 0 or metadata_address % 8 != 0
        or (measuring != 0 and measuring != 1)
        or (measuring == 1 and (output_address != 0 or capacity != 0))
        or (measuring == 0 and output_address == 0)):
        return 1
    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](unsafe_from_address=Int(nodes_address)),
        nodes_count, ProdexRichStringView(UInt(raw_address), UInt(raw_length)),
    )
    if not pj_valid(tree):
        return 1
    var values = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(scratch_address))
    if operation >= 3:
        for index in range(scratch_count):
            if not rich_view_valid(values[unsafe_offset=index], 0x7FFFFFFFFFFFFFFF):
                return 1
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var probe = JsonSink(output, 0, 0, True, False)
    if operation <= 2:
        transcript_context_render(Pointer(to=probe), tree, operation)
    else:
        transcript_operation_render(Pointer(to=probe), tree, operation, values)
    if probe.failed or (measuring == 0 and capacity < probe.written):
        return 3
    var sink = probe.copy()
    if measuring == 0:
        sink = JsonSink(output, capacity, 0, False, False)
        if operation <= 2:
            transcript_context_render(Pointer(to=sink), tree, operation)
        else:
            transcript_operation_render(Pointer(to=sink), tree, operation, values)
    if sink.failed:
        return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = sink.written
    return 0
