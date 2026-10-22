from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid, rich_trim_bounds
from parsed_json import pj_literal
from json_sink import JsonSink, js_byte, js_literal, js_string


@fieldwise_init
struct SessionTuiRow(Copyable):
    var presence: UInt64
    var id: ProdexRichStringView
    var title: ProdexRichStringView
    var updated: ProdexRichStringView
    var cwd: ProdexRichStringView
    var profile: ProdexRichStringView
    var provider: ProdexRichStringView


def stp_optional(row: SessionTuiRow, bit: UInt64, value: ProdexRichStringView) -> ProdexRichStringView:
    if row.presence & bit != 0:
        return value.copy()
    return pj_literal(StringSlice("-"))


def stp_span(sink: Pointer[mut=True, JsonSink, _], tone: StringSlice, text: ProdexRichStringView):
    js_byte(sink, 91)
    js_string(sink, pj_literal(tone))
    js_byte(sink, 44)
    js_string(sink, text)
    js_byte(sink, 93)


def stp_literal(sink: Pointer[mut=True, JsonSink, _], tone: StringSlice, text: StringSlice):
    stp_span(sink, tone, pj_literal(text))


def stp_row(sink: Pointer[mut=True, JsonSink, _], row: SessionTuiRow, scroll: Bool):
    var title = row.title.copy()
    var title_bounds = rich_trim_bounds(title)
    if row.presence & 1 == 0 or title_bounds[0] == title_bounds[1]:
        title = pj_literal(StringSlice("Untitled session"))
    js_literal(sink, StringSlice("[["))
    stp_span(sink, StringSlice("hint_bold"), row.id)
    js_byte(sink, 44)
    stp_literal(sink, StringSlice("raw"), StringSlice("  "))
    js_byte(sink, 44)
    stp_span(sink, StringSlice("primary"), title)
    js_literal(sink, StringSlice("],["))
    stp_literal(sink, StringSlice("secondary"), StringSlice("updated "))
    js_byte(sink, 44)
    stp_span(sink, StringSlice("raw"), stp_optional(row, 2, row.updated))
    js_byte(sink, 44)
    stp_literal(sink, StringSlice("secondary"), StringSlice("  profile ") if scroll else StringSlice(" profile "))
    js_byte(sink, 44)
    stp_span(sink, StringSlice("raw"), stp_optional(row, 8, row.profile))
    js_byte(sink, 44)
    stp_literal(sink, StringSlice("secondary"), StringSlice("  provider ") if scroll else StringSlice(" provider "))
    js_byte(sink, 44)
    stp_span(sink, StringSlice("raw"), stp_optional(row, 16, row.provider))
    js_byte(sink, 93)
    if not scroll or row.presence & 4 != 0:
        js_literal(sink, StringSlice(",["))
        stp_literal(sink, StringSlice("secondary"), StringSlice("cwd "))
        js_byte(sink, 44)
        stp_span(sink, StringSlice("raw"), stp_optional(row, 4, row.cwd))
        js_byte(sink, 93)
    js_literal(sink, StringSlice(",[]]"))


def stp_emit(sink: Pointer[mut=True, JsonSink, _], rows_address: UInt64, count: Int64, scroll: Bool):
    var rows = Pointer[mut=False, SessionTuiRow, ImmUntrackedOrigin](unsafe_from_address=Int(rows_address))
    js_byte(sink, 91)
    for index in range(count):
        if index > 0:
            js_byte(sink, 44)
        stp_row(sink, rows[unsafe_offset=index], scroll)
    js_byte(sink, 93)


@export("prodex_session_tui_rows_v1")
def prodex_session_tui_rows_v1(
    abi: Int64, mode: Int64, rows_address: UInt64, count: Int64, measuring: Int64,
    output_address: UInt64, capacity: Int64, metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (
        mode < 0 or mode > 1 or count < 0 or count > 0x7FFFFFFFFFFFFFFF // 104
        or rows_address % 8 != 0 or (count > 0 and rows_address == 0)
        or measuring < 0 or measuring > 1 or capacity < 0
        or metadata_address == 0 or metadata_address % 8 != 0
        or (measuring == 1 and (output_address != 0 or capacity != 0))
        or (measuring == 0 and output_address == 0)
    ):
        return 1
    var rows = Pointer[mut=False, SessionTuiRow, ImmUntrackedOrigin](unsafe_from_address=Int(rows_address))
    for index in range(count):
        var row = rows[unsafe_offset=index].copy()
        if (
            row.presence > 31 or not rich_view_valid(row.id, 0x7FFFFFFFFFFFFFFF)
            or not rich_view_valid(row.title, 0x7FFFFFFFFFFFFFFF)
            or not rich_view_valid(row.updated, 0x7FFFFFFFFFFFFFFF)
            or not rich_view_valid(row.cwd, 0x7FFFFFFFFFFFFFFF)
            or not rich_view_valid(row.profile, 0x7FFFFFFFFFFFFFFF)
            or not rich_view_valid(row.provider, 0x7FFFFFFFFFFFFFFF)
        ):
            return 1
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var sink = JsonSink(output, capacity, 0, True, False)
    stp_emit(Pointer(to=sink), rows_address, count, mode == 1)
    if sink.failed:
        return 3
    var required = sink.written
    if measuring == 0:
        if capacity < required:
            return 3
        sink = JsonSink(output, capacity, 0, False, False)
        stp_emit(Pointer(to=sink), rows_address, count, mode == 1)
        if sink.failed or sink.written != required:
            return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = required
    return 0
