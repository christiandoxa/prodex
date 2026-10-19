# Protocol/status field selection over native, already-redacted presentations.
from std.memory import Pointer
from rich_types import ProdexRichStringView, rich_view_ptr
from rich_text import rich_view_prefix
from parsed_json import (
    ParsedJson, JSON_STRING, JSON_NUMBER, JSON_FALSE, JSON_TRUE, JSON_ARRAY, JSON_OBJECT,
    pj_kind, pj_field, pj_text, pj_literal,
)
from json_sink import JsonSink, js_byte, js_literal, js_string, js_escaped


def transcript_operation_value(
    tree: ParsedJson, values: Pointer[mut=False, ProdexRichStringView, _],
    node: Int64, *, output: Bool = False,
) -> ProdexRichStringView:
    var kind = pj_kind(tree, node)
    if (kind == JSON_STRING or kind == JSON_NUMBER or kind == JSON_FALSE or kind == JSON_TRUE
        or (output and (kind == JSON_ARRAY or kind == JSON_OBJECT))):
        return values[unsafe_offset=node * 2 + Int64(output)].copy()
    return ProdexRichStringView(0, 0)


def transcript_operation_field(
    sink: Pointer[mut=True, JsonSink, _], label: StringSlice,
    value: ProdexRichStringView, count: Pointer[mut=True, Int64, _], *, output: Bool = False,
):
    if count[] > 0:
        js_byte(sink, 32)
    js_escaped(sink, pj_literal(label))
    js_literal(sink, StringSlice(":\\n") if output else StringSlice("="))
    js_escaped(sink, value)
    count[] += 1


def transcript_protocol_fields(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson,
    values: Pointer[mut=False, ProdexRichStringView, _], event: ProdexRichStringView,
):
    var count: Int64 = 0
    var has_status = False
    for label in [StringSlice("server"), StringSlice("tool"), StringSlice("name"), StringSlice("status"), StringSlice("phase")]:
        var node = pj_field(tree, 0, label)
        var value = ProdexRichStringView(0, 0)
        if pj_kind(tree, node) == JSON_STRING:
            value = values[unsafe_offset=node * 2].copy()
        if value.len == 0 and label == StringSlice("server"):
            for key in [StringSlice("server_label"), StringSlice("server_name")]:
                node = pj_field(tree, 0, key)
                if value.len == 0 and pj_kind(tree, node) == JSON_STRING:
                    value = values[unsafe_offset=node * 2].copy()
        elif value.len == 0 and label == StringSlice("tool"):
            node = pj_field(tree, 0, StringSlice("tool_name"))
            if pj_kind(tree, node) == JSON_STRING:
                value = values[unsafe_offset=node * 2].copy()
        if value.len > 0:
            transcript_operation_field(sink, label, value, Pointer(to=count))
            has_status = has_status or label == StringSlice("status")
    if not has_status:
        var prefix: Int64 = 0
        if rich_view_prefix["subagent_"](event, False):
            prefix = 9
        elif rich_view_prefix["sub_agent_"](event, False):
            prefix = 10
        if prefix > 0:
            transcript_operation_field(sink, StringSlice("status"),
                ProdexRichStringView(event.ptr + UInt(prefix), event.len - UInt(prefix)), Pointer(to=count))


def transcript_status_fields(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson,
    values: Pointer[mut=False, ProdexRichStringView, _],
):
    var count: Int64 = 0
    for key in [StringSlice("status"), StringSlice("exit_code"), StringSlice("exit_status"), StringSlice("reason"), StringSlice("duration_ms"), StringSlice("message")]:
        var value = transcript_operation_value(tree, values, pj_field(tree, 0, key))
        if value.len > 0:
            transcript_operation_field(sink, key, value, Pointer(to=count))
    for key in [StringSlice("stdout"), StringSlice("stderr"), StringSlice("output")]:
        var value = transcript_operation_value(tree, values, pj_field(tree, 0, key), output=True)
        if value.len > 0:
            transcript_operation_field(sink, key, value, Pointer(to=count), output=True)


def transcript_operation_fallback(sink: Pointer[mut=True, JsonSink, _], event: ProdexRichStringView):
    var bytes = rich_view_ptr(event)
    var start: Int64 = 0
    for index in range(Int64(event.len)):
        if bytes[unsafe_offset=index] == 95:
            js_escaped(sink, ProdexRichStringView(event.ptr + UInt(start), UInt(index - start)))
            js_byte(sink, 32)
            start = index + 1
    js_escaped(sink, ProdexRichStringView(event.ptr + UInt(start), event.len - UInt(start)))


def transcript_operation_render(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, operation: Int64,
    values: Pointer[mut=False, ProdexRichStringView, _],
):
    var event = pj_text(tree, pj_field(tree, 0, StringSlice("type")))
    var probe = JsonSink(sink[].output, 0, 0, True, False)
    if operation <= 5:
        transcript_protocol_fields(Pointer(to=probe), tree, values, event)
    else:
        transcript_status_fields(Pointer(to=probe), tree, values)
    var source = StringSlice("tool")
    if operation == 4:
        source = StringSlice("mcp")
    elif operation == 5:
        source = StringSlice("agent")
    elif operation == 6:
        source = StringSlice("terminal")
    elif operation == 7:
        source = StringSlice("error")
    js_literal(sink, StringSlice("[["))
    js_string(sink, pj_literal(source))
    js_literal(sink, StringSlice(",\""))
    if probe.written == 0:
        transcript_operation_fallback(sink, event)
    elif operation <= 5:
        transcript_protocol_fields(sink, tree, values, event)
    else:
        transcript_status_fields(sink, tree, values)
    js_literal(sink, StringSlice("\"]]"))
