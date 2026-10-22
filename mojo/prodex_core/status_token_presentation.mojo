from std.memory import Pointer
from std.collections import Array
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid
from json_sink import JsonSink, js_byte, js_literal, js_string, js_escaped, js_u64
from session_tui_presentation import stp_literal
from info_render import (
    InfoRenderWriter, info_write_human_count, info_write_token_efficiency,
    info_status_u64_saturating_add,
)


@fieldwise_init
struct StatusTokenPanelInput(Copyable):
    var presence: UInt64
    var input: UInt64
    var cached: UInt64
    var output: UInt64
    var reasoning: UInt64
    var events: UInt64
    var logs: UInt64
    var profiles: UInt64
    var runtime_profile: ProdexRichStringView
    var active_profile: ProdexRichStringView
    var first: ProdexRichStringView
    var last: ProdexRichStringView


def sto_count(sink: Pointer[mut=True, JsonSink, _], value: UInt64, scratch: Pointer[mut=True, UInt8, MutUntrackedOrigin]):
    var writer = InfoRenderWriter(scratch, 128, 0)
    if not info_write_human_count(Pointer(to=writer), value):
        sink[].failed = True
        return
    js_string(sink, ProdexRichStringView(UInt(Int(scratch)), UInt(writer.written)))


def sto_efficiency(sink: Pointer[mut=True, JsonSink, _], row: StatusTokenPanelInput, scratch: Pointer[mut=True, UInt8, MutUntrackedOrigin]):
    var writer = InfoRenderWriter(scratch, 128, 0)
    if not info_write_token_efficiency(Pointer(to=writer), row.input, row.cached, row.output):
        sink[].failed = True
        return
    js_string(sink, ProdexRichStringView(UInt(Int(scratch)), UInt(writer.written)))


def sto_emit(sink: Pointer[mut=True, JsonSink, _], input_address: UInt64, scratch: Pointer[mut=True, UInt8, MutUntrackedOrigin]):
    js_literal(sink, StringSlice('{"lines":['))
    if input_address == 0:
        js_byte(sink, 91)
        stp_literal(sink, StringSlice("raw"), StringSlice("loading token history…"))
        js_literal(sink, StringSlice(']],"history_title":""}'))
        return
    var row = Pointer[mut=False, StatusTokenPanelInput, ImmUntrackedOrigin](unsafe_from_address=Int(input_address))[].copy()
    js_literal(sink, StringSlice('[["secondary","total "],["metric",'))
    sto_count(sink, info_status_u64_saturating_add(row.input, row.output), scratch)
    js_literal(sink, StringSlice('],["muted","  '))
    js_u64(sink, row.events)
    js_literal(sink, StringSlice(' event(s) / '))
    js_u64(sink, row.logs)
    js_literal(sink, StringSlice(' log(s)"]],[["raw","in '))
    # Existing compiled human-count formatting is inserted inside one JSON string.
    var values: Array[UInt64, 4] = [row.input, row.cached, row.output, row.reasoning]
    for index in range(4):
        if index == 1:
            js_literal(sink, StringSlice("  cached "))
        elif index == 2:
            js_literal(sink, StringSlice("  out "))
        elif index == 3:
            js_literal(sink, StringSlice("  reasoning "))
        var writer = InfoRenderWriter(scratch, 128, 0)
        var value = values[index]
        if not info_write_human_count(Pointer(to=writer), value):
            sink[].failed = True
            return
        js_escaped(sink, ProdexRichStringView(UInt(Int(scratch)), UInt(writer.written)))
    js_literal(sink, StringSlice('"]],[["secondary","token efficiency "],["accent",'))
    sto_efficiency(sink, row, scratch)
    js_literal(sink, StringSlice(']],[["secondary","active/config "],["accent","'))
    js_escaped(sink, row.runtime_profile)
    js_literal(sink, StringSlice(" / "))
    js_escaped(sink, row.active_profile)
    js_literal(sink, StringSlice('"],["muted","  '))
    js_u64(sink, row.profiles)
    js_literal(sink, StringSlice(' profile(s)"]]],"history_title":"'))
    if row.presence == 3:
        js_literal(sink, StringSlice(" recent events "))
        js_escaped(sink, row.first)
        js_literal(sink, StringSlice(" → "))
        js_escaped(sink, row.last)
        js_byte(sink, 32)
    else:
        js_literal(sink, StringSlice(" no token events "))
    js_literal(sink, StringSlice('"}'))


@export("prodex_status_token_panel_v1")
def prodex_status_token_panel_v1(
    abi: Int64, input_address: UInt64, scratch_address: UInt64, scratch_capacity: Int64, measuring: Int64,
    output_address: UInt64, capacity: Int64, metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (
        input_address % 8 != 0 or scratch_address == 0 or scratch_capacity < 128 or measuring < 0 or measuring > 1 or capacity < 0
        or metadata_address == 0 or metadata_address % 8 != 0
        or (measuring == 1 and (output_address != 0 or capacity != 0))
        or (measuring == 0 and output_address == 0)
    ):
        return 1
    if input_address != 0:
        var row = Pointer[mut=False, StatusTokenPanelInput, ImmUntrackedOrigin](unsafe_from_address=Int(input_address))[].copy()
        if (
            row.presence > 3 or not rich_view_valid(row.runtime_profile, 0x7FFFFFFFFFFFFFFF)
            or not rich_view_valid(row.active_profile, 0x7FFFFFFFFFFFFFFF)
            or not rich_view_valid(row.first, 0x7FFFFFFFFFFFFFFF)
            or not rich_view_valid(row.last, 0x7FFFFFFFFFFFFFFF)
        ):
            return 1
    var scratch = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(scratch_address))
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var sink = JsonSink(output, capacity, 0, True, False)
    sto_emit(Pointer(to=sink), input_address, scratch)
    if sink.failed:
        return 3
    var required = sink.written
    if measuring == 0:
        if capacity < required:
            return 3
        sink = JsonSink(output, capacity, 0, False, False)
        sto_emit(Pointer(to=sink), input_address, scratch)
        if sink.failed or sink.written != required:
            return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = required
    return 0
