# Error-event selection over Serde-acquired JSONL records, before native redaction.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_ARRAY, JSON_STRING, pj_valid, pj_kind,
    pj_child, pj_next, pj_field, pj_text, pj_is, pj_trim,
)
from json_sink import JsonSink, js_byte, js_literal, js_view


def ping_failure_event(tree: ParsedJson) -> Int64:
    var event = pj_child(tree, 0)
    while event >= 0:
        var kind = pj_field(tree, event, StringSlice("type"))
        if pj_is["turn.failed"](tree, kind) or pj_is["error"](tree, kind):
            return event
        event = pj_next(tree, event)
    return -1


def ping_failure_text(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, event: Int64):
    var count: Int64 = 0
    var error = pj_field(tree, event, StringSlice("error"))
    for field in range(5):
        var node: Int64
        if field == 0:
            node = pj_field(tree, event, StringSlice("message"))
        elif field == 1:
            node = pj_field(tree, event, StringSlice("code"))
        elif field == 2:
            node = error
        elif field == 3:
            node = pj_field(tree, error, StringSlice("message"))
        else:
            node = pj_field(tree, error, StringSlice("code"))
        if pj_kind(tree, node) != JSON_STRING:
            continue
        var text = pj_text(tree, node)
        if pj_trim(text).len == 0:
            continue
        if count > 0:
            js_byte(sink, 32)
        js_view(sink, text)
        count += 1
    if count == 0:
        var kind = pj_field(tree, event, StringSlice("type"))
        js_literal(sink, StringSlice("turn failed") if pj_is["turn.failed"](tree, kind) else StringSlice("Codex error"))


@export("prodex_mojo_ping_failure_detail_v1")
def prodex_mojo_ping_failure_detail_v1(
    abi: Int64, operation: Int64, flag: Int64,
    nodes_address: UInt64, nodes_count: Int64, raw_address: UInt64, raw_length: Int64,
    scratch_address: UInt64, scratch_count: Int64,
    measuring: Int64, output_address: UInt64, capacity: Int64, metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (operation != 0 or flag != 0 or nodes_count <= 0 or nodes_count > 0x7FFFFFFFFFFFFFFF // 80
        or nodes_address == 0 or nodes_address % 8 != 0 or raw_length < 0
        or scratch_address == 0 or scratch_address % 8 != 0 or scratch_count != nodes_count
        or capacity < 0 or metadata_address == 0 or metadata_address % 8 != 0
        or (measuring != 0 and measuring != 1)
        or (measuring == 1 and (output_address != 0 or capacity != 0))
        or (measuring == 0 and output_address == 0)):
        return 1
    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](unsafe_from_address=Int(nodes_address)),
        nodes_count, ProdexRichStringView(UInt(raw_address), UInt(raw_length)),
    )
    if not pj_valid(tree) or pj_kind(tree, 0) != JSON_ARRAY:
        return 1
    var event = ping_failure_event(tree)
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var probe = JsonSink(output, 0, 0, True, False)
    if event >= 0:
        ping_failure_text(Pointer(to=probe), tree, event)
    if probe.failed or (measuring == 0 and capacity < probe.written):
        return 3
    var sink = probe.copy()
    if measuring == 0 and event >= 0:
        sink = JsonSink(output, capacity, 0, False, False)
        ping_failure_text(Pointer(to=sink), tree, event)
    if sink.failed:
        return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = Int64(event >= 0)
    metadata[unsafe_offset=1] = sink.written
    return 0
