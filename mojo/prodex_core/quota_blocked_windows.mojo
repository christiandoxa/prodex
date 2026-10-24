# Blocked quota selection and text; Rust acquires observations and local reset times.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid
from parsed_json import pj_literal
from json_sink import JsonSink, js_byte, js_literal, js_string, js_escaped, js_i64
from quota import QUOTA_WINDOW_LABEL_SECONDS, quota_window_label_parts


@fieldwise_init
struct QuotaBlockedWindowRow(Copyable):
    var kind: Int64
    var used_present: Int64
    var used: Int64
    var names_present: Int64
    var seconds_present: Int64
    var seconds: Int64
    var name: ProdexRichStringView
    var feature: ProdexRichStringView
    var id: ProdexRichStringView
    var reset: ProdexRichStringView


def quota_blocked_row_prefix(sink: Pointer[mut=True, JsonSink, _], rows: Int64) -> Int64:
    if rows > 0:
        js_byte(sink, 44)
    return rows + 1


def quota_blocked_row_text(
    sink: Pointer[mut=True, JsonSink, _], row: QuotaBlockedWindowRow,
):
    var label = quota_window_label_parts(row.seconds_present, row.seconds)
    js_byte(sink, 34)
    if row.kind == 2:
        js_literal(sink, StringSlice("code-review "))
    elif row.kind == 1:
        var name = row.name.copy()
        if row.names_present & 1 == 0:
            name = row.feature.copy() if row.names_present & 2 != 0 else row.id.copy()
        if name.len > 0:
            js_escaped(sink, name)
            js_byte(sink, 32)
    if label[0] == QUOTA_WINDOW_LABEL_SECONDS:
        js_i64(sink, row.seconds)
        js_byte(sink, 115)
    else:
        js_escaped(sink, label[1])
    if row.kind == 0 and row.used_present == 0:
        js_literal(sink, StringSlice(" quota unknown"))
    else:
        js_literal(sink, StringSlice(" exhausted until "))
        js_escaped(sink, row.reset)
    js_byte(sink, 34)


@export("prodex_quota_blocked_windows_json_v1")
def prodex_quota_blocked_windows_json_v1(
    abi: Int64, runtime_ready: Int64, main_present: Int64, include_code_review: Int64,
    rows_address: UInt, count: Int64, measuring: Int64,
    output_address: UInt, capacity: Int64, written_address: UInt,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (
        runtime_ready < 0 or runtime_ready > 1 or main_present < 0 or main_present > 1
        or include_code_review < 0 or include_code_review > 1
        or count < 0 or count > 0x7FFFFFFFFFFFFFFF // 112
        or count > 0 and (rows_address == 0 or rows_address % 8 != 0)
        or measuring < 0 or measuring > 1 or capacity < 0
        or written_address == 0 or written_address % 8 != 0
        or measuring == 1 and (output_address != 0 or capacity != 0)
        or measuring == 0 and output_address == 0
    ):
        return 1
    var rows = Pointer[mut=False, QuotaBlockedWindowRow, ImmUntrackedOrigin](unsafe_from_address=Int(rows_address))
    for index in range(count):
        var row = rows[unsafe_offset=index].copy()
        if (
            row.kind < 0 or row.kind > 2 or row.used_present < 0 or row.used_present > 1
            or row.seconds_present < 0 or row.seconds_present > 1
            or row.names_present < 0 or row.names_present > 7
            or not rich_view_valid(row.name, 0x7FFFFFFFFFFFFFFF)
            or not rich_view_valid(row.feature, 0x7FFFFFFFFFFFFFFF)
            or not rich_view_valid(row.id, 0x7FFFFFFFFFFFFFFF)
            or not rich_view_valid(row.reset, 0x7FFFFFFFFFFFFFFF)
            or row.names_present & 1 == 0 and row.name.len != 0
            or row.names_present & 2 == 0 and row.feature.len != 0
            or row.names_present & 4 == 0 and row.id.len != 0
        ):
            return 1
    var sink = JsonSink(Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)), capacity, 0, measuring == 1, False)
    var emitted: Int64 = 0
    js_byte(Pointer(to=sink), 91)
    if runtime_ready == 0 and main_present == 0:
        emitted = quota_blocked_row_prefix(Pointer(to=sink), emitted)
        js_string(Pointer(to=sink), pj_literal("5h quota unavailable"))
        emitted = quota_blocked_row_prefix(Pointer(to=sink), emitted)
        js_string(Pointer(to=sink), pj_literal("weekly quota unavailable"))
    for phase in range(2):
        if phase == 1 and runtime_ready == 0 and main_present == 1 and emitted == 0:
            emitted = quota_blocked_row_prefix(Pointer(to=sink), emitted)
            js_string(Pointer(to=sink), pj_literal("quota unavailable"))
        for index in range(count):
            var row = rows[unsafe_offset=index].copy()
            var blocked = False
            if phase == 0 and row.kind != 2 and runtime_ready == 0:
                blocked = row.kind == 0 and row.used_present == 0 or row.used_present == 1 and row.used >= 100
            elif phase == 1 and row.kind == 2 and include_code_review == 1:
                blocked = row.used_present == 1 and row.used >= 100
            if blocked:
                emitted = quota_blocked_row_prefix(Pointer(to=sink), emitted)
                quota_blocked_row_text(Pointer(to=sink), row)
    js_byte(Pointer(to=sink), 93)
    if sink.failed:
        return 2
    Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))[] = sink.written
    return 0
