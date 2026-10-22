from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid, rich_view_matches_literal, rich_scalar_prefix_end
from super_expose_exec import super_expose_exec_path_valid
from json_sink import JsonSink, js_view, js_literal, js_byte


@fieldwise_init
struct OperationalDetailText(Copyable):
    var label: ProdexRichStringView
    var original: ProdexRichStringView
    var prepared: ProdexRichStringView
    var format_kind: Int64


def operational_details_render(
    sink: Pointer[mut=True, JsonSink, _], title: ProdexRichStringView,
    fields: Pointer[mut=False, OperationalDetailText, _], count: Int64,
):
    js_view(sink, title)
    var first = True
    for index in range(count):
        var field = fields[unsafe_offset=index].copy()
        if not super_expose_exec_path_valid(field.original, 67_108_864):
            continue
        if field.format_kind == 0 and (field.prepared.len == 0
            or rich_view_matches_literal["-"](field.prepared, False)):
            continue
        js_literal(sink, StringSlice("  ") if first else StringSlice(" · "))
        first = False
        js_view(sink, field.label)
        js_byte(sink, 61)
        var end = rich_scalar_prefix_end(field.prepared, UInt64(32 if field.format_kind == 1 else 192))
        js_view(sink, ProdexRichStringView(field.prepared.ptr, UInt(end)))
        if end < Int64(field.prepared.len):
            js_literal(sink, StringSlice("…"))
        if field.format_kind == 1:
            js_byte(sink, 37)


@export("prodex_mojo_operational_log_details_v1")
def prodex_mojo_operational_log_details_v1(
    abi_version: Int64, title_address: UInt, title_length: Int64,
    fields_address: UInt, field_count: Int64, measuring: Int64,
    output_address: UInt, output_capacity: Int64, written_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return 4
    if (title_length < 0 or title_length > 67_108_864
        or field_count < 0 or field_count > 64
        or (field_count > 0 and (fields_address == 0 or fields_address % 8 != 0))
        or (measuring != 0 and measuring != 1) or output_capacity < 0
        or (measuring == 1 and (output_address != 0 or output_capacity != 0))
        or (measuring == 0 and output_address == 0)
        or written_address == 0 or written_address % 8 != 0):
        return 1
    var title = ProdexRichStringView(title_address, UInt(title_length))
    if not rich_view_valid(title, 67_108_864):
        return 1
    var fields = Pointer[mut=False, OperationalDetailText, ImmUntrackedOrigin](unsafe_from_address=Int(fields_address))
    var decoded_bytes = title.len
    for index in range(field_count):
        var field = fields[unsafe_offset=index].copy()
        if (field.format_kind < 0 or field.format_kind > 2
            or not rich_view_valid(field.label, 67_108_864)
            or not rich_view_valid(field.original, 67_108_864)
            or not rich_view_valid(field.prepared, 67_108_864)):
            return 1
        decoded_bytes += field.label.len + field.original.len + field.prepared.len
        if decoded_bytes > 67_108_864:
            return 1
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var probe = JsonSink(output, 0, 0, True, False)
    operational_details_render(Pointer(to=probe), title, fields, field_count)
    if probe.failed or probe.written > 67_108_864:
        return 1
    if measuring == 0 and output_capacity < probe.written:
        return 2
    if measuring == 0:
        var sink = JsonSink(output, output_capacity, 0, False, False)
        operational_details_render(Pointer(to=sink), title, fields, field_count)
        if sink.failed:
            return 2
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))
    written[] = probe.written
    return 0
