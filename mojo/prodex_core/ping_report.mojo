from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid
from json_sink import JsonSink, js_byte, js_literal, js_view, js_string, js_u64, js_u128
from ping_protocol import (
    ping_status_label_view, ping_summary_counts, PingSummaryCounts,
    PING_STATUS_OK, PING_STATUS_AUTH_FAILED, PING_STATUS_CANCELLED, PING_SUMMARY_MAX_STATUSES,
)

comptime PING_REPORT_MODEL: UInt64 = 1
comptime PING_REPORT_EFFORT: UInt64 = 2
comptime PING_REPORT_EFFECTIVE: UInt64 = 4
comptime PING_REPORT_FIRST: UInt64 = 8
comptime PING_REPORT_COMPLETION: UInt64 = 16
comptime PING_REPORT_ROW_WORDS: Int64 = 16


@fieldwise_init
struct PingReportRow(Copyable):
    var status: Int64
    var presence: UInt64
    var profile: ProdexRichStringView
    var model: ProdexRichStringView
    var effort: ProdexRichStringView
    var effective_model: ProdexRichStringView
    var detail: ProdexRichStringView
    var first_low: UInt64
    var first_high: UInt64
    var completion_low: UInt64
    var completion_high: UInt64


def ping_report_wide(low: UInt64, high: UInt64) -> UInt128:
    return (UInt128(high) << 64) | UInt128(low)


def ping_report_row_valid(row: PingReportRow, json: Bool) -> Bool:
    if row.status < PING_STATUS_OK or row.status > PING_STATUS_CANCELLED or row.presence > 31:
        return False
    if (
        not rich_view_valid(row.profile, 0x7FFFFFFFFFFFFFFF)
        or not rich_view_valid(row.model, 0x7FFFFFFFFFFFFFFF)
        or not rich_view_valid(row.effort, 0x7FFFFFFFFFFFFFFF)
        or not rich_view_valid(row.effective_model, 0x7FFFFFFFFFFFFFFF)
        or not rich_view_valid(row.detail, 0x7FFFFFFFFFFFFFFF)
    ):
        return False
    return not json or (
        (row.presence & PING_REPORT_FIRST == 0 or row.first_high == 0)
        and (row.presence & PING_REPORT_COMPLETION == 0 or row.completion_high == 0)
    )


def ping_report_optional(sink: Pointer[mut=True, JsonSink, _], value: ProdexRichStringView, present: Bool):
    if present:
        js_string(sink, value)
    else:
        js_literal(sink, StringSlice("null"))


def ping_report_first[mask: UInt64](
    sink: Pointer[mut=True, JsonSink, _], rows: Pointer[mut=False, PingReportRow, _], count: Int64,
):
    if count == 0:
        js_literal(sink, StringSlice("null"))
    elif mask == PING_REPORT_MODEL:
        ping_report_optional(sink, rows[].model, rows[].presence & mask != 0)
    elif mask == PING_REPORT_EFFORT:
        ping_report_optional(sink, rows[].effort, rows[].presence & mask != 0)
    else:
        ping_report_optional(sink, rows[].effective_model, rows[].presence & mask != 0)


def ping_report_profile_text(sink: Pointer[mut=True, JsonSink, _], row: PingReportRow):
    js_view(sink, row.profile)
    js_literal(sink, StringSlice("  "))
    var label = ping_status_label_view(row.status, False)
    js_view(sink, label)
    for _ in range(max(Int64(20) - Int64(label.len), Int64(0))):
        js_byte(sink, 32)
    js_literal(sink, StringSlice(" first="))
    if row.presence & PING_REPORT_FIRST != 0:
        js_u128(sink, ping_report_wide(row.first_low, row.first_high))
        js_literal(sink, StringSlice("ms"))
    else:
        js_literal(sink, StringSlice("unavailable"))
    js_literal(sink, StringSlice(" completion="))
    js_u128(sink, ping_report_wide(row.completion_low, row.completion_high) if row.presence & PING_REPORT_COMPLETION != 0 else UInt128(0))
    js_literal(sink, StringSlice("ms  requested="))
    if row.presence & PING_REPORT_MODEL != 0:
        js_view(sink, row.model)
    else:
        js_literal(sink, StringSlice("configured/default"))
    js_literal(sink, StringSlice(" effort="))
    if row.presence & PING_REPORT_EFFORT != 0:
        js_view(sink, row.effort)
    else:
        js_literal(sink, StringSlice("configured/default"))
    js_literal(sink, StringSlice(" effective="))
    if row.presence & PING_REPORT_EFFECTIVE != 0:
        js_view(sink, row.effective_model)
    else:
        js_literal(sink, StringSlice("unavailable"))
    if row.status != PING_STATUS_OK:
        js_literal(sink, StringSlice("\n  reason: "))
        js_view(sink, row.detail)


def ping_report_profile_json(sink: Pointer[mut=True, JsonSink, _], row: PingReportRow):
    js_literal(sink, StringSlice('{"completion_latency_ms":'))
    if row.presence & PING_REPORT_COMPLETION != 0:
        js_u64(sink, row.completion_low)
    else:
        js_literal(sink, StringSlice("null"))
    js_literal(sink, StringSlice(',"credential_validation":'))
    if row.status == PING_STATUS_AUTH_FAILED:
        js_literal(sink, StringSlice('"failed"'))
    elif row.status == PING_STATUS_OK:
        js_literal(sink, StringSlice('"valid"'))
    else:
        js_literal(sink, StringSlice('"unknown"'))
    js_literal(sink, StringSlice(',"detail":'))
    js_string(sink, row.detail)
    js_literal(sink, StringSlice(',"effective_model":'))
    ping_report_optional(sink, row.effective_model, row.presence & PING_REPORT_EFFECTIVE != 0)
    js_literal(sink, StringSlice(',"effort":'))
    ping_report_optional(sink, row.effort, row.presence & PING_REPORT_EFFORT != 0)
    js_literal(sink, StringSlice(',"first_response_latency_ms":'))
    if row.presence & PING_REPORT_FIRST != 0:
        js_u64(sink, row.first_low)
    else:
        js_literal(sink, StringSlice("null"))
    js_literal(sink, StringSlice(',"latency_ms":'))
    if row.presence & PING_REPORT_COMPLETION != 0:
        js_u64(sink, row.completion_low)
    else:
        js_literal(sink, StringSlice("null"))
    js_literal(sink, StringSlice(',"model":'))
    ping_report_optional(sink, row.model, row.presence & PING_REPORT_MODEL != 0)
    js_literal(sink, StringSlice(',"profile":'))
    js_string(sink, row.profile)
    js_literal(sink, StringSlice(',"requested_effort":'))
    ping_report_optional(sink, row.effort, row.presence & PING_REPORT_EFFORT != 0)
    js_literal(sink, StringSlice(',"requested_model":'))
    ping_report_optional(sink, row.model, row.presence & PING_REPORT_MODEL != 0)
    js_literal(sink, StringSlice(',"status":'))
    js_string(sink, ping_status_label_view(row.status, True))
    js_byte(sink, 125)


def ping_report_count_line(sink: Pointer[mut=True, JsonSink, _], label: StringSlice, value: UInt64):
    js_literal(sink, label)
    js_u64(sink, value)
    js_byte(sink, 10)


def ping_report_summary_text(sink: Pointer[mut=True, JsonSink, _], counts: PingSummaryCounts, elapsed: UInt128):
    ping_report_count_line(sink, StringSlice("Profiles discovered: "), counts.total)
    ping_report_count_line(sink, StringSlice("Profiles tested: "), counts.total)
    ping_report_count_line(sink, StringSlice("Healthy: "), counts.healthy)
    ping_report_count_line(sink, StringSlice("Exhausted: "), counts.exhausted)
    ping_report_count_line(sink, StringSlice("Auth failures: "), counts.auth_failures)
    ping_report_count_line(sink, StringSlice("Temporary failures: "), counts.temporary_failures)
    ping_report_count_line(sink, StringSlice("Other failures: "), counts.other_failures)
    js_literal(sink, StringSlice("Total duration: "))
    js_u128(sink, elapsed)
    js_literal(sink, StringSlice("ms\nPool usable: "))
    if counts.pool_usable:
        js_literal(sink, StringSlice("yes"))
    else:
        js_literal(sink, StringSlice("no"))


def ping_report_summary_json(
    sink: Pointer[mut=True, JsonSink, _], rows: Pointer[mut=False, PingReportRow, _],
    count: Int64, counts: PingSummaryCounts, elapsed: UInt64,
):
    js_literal(sink, StringSlice('{"detail":"'))
    js_u64(sink, counts.healthy)
    js_byte(sink, 47)
    js_u64(sink, counts.total)
    js_literal(sink, StringSlice(' profiles healthy","effective_model":'))
    ping_report_first[PING_REPORT_EFFECTIVE](sink, rows, count)
    js_literal(sink, StringSlice(',"effort":'))
    ping_report_first[PING_REPORT_EFFORT](sink, rows, count)
    js_literal(sink, StringSlice(',"latency_ms":'))
    js_u64(sink, elapsed)
    js_literal(sink, StringSlice(',"model":'))
    ping_report_first[PING_REPORT_MODEL](sink, rows, count)
    js_literal(sink, StringSlice(',"profiles":['))
    for index in range(count):
        if index > 0:
            js_byte(sink, 44)
        ping_report_profile_json(sink, rows[unsafe_offset=index])
    js_literal(sink, StringSlice('],"provider":"openai","requested_effort":'))
    ping_report_first[PING_REPORT_EFFORT](sink, rows, count)
    js_literal(sink, StringSlice(',"requested_model":'))
    ping_report_first[PING_REPORT_MODEL](sink, rows, count)
    js_literal(sink, StringSlice(',"status":'))
    if counts.healthy == counts.total and counts.total > 0:
        js_literal(sink, StringSlice('"ok"'))
    else:
        js_literal(sink, StringSlice('"failed"'))
    js_literal(sink, StringSlice(',"summary":{"auth_failures":'))
    js_u64(sink, counts.auth_failures)
    js_literal(sink, StringSlice(',"duration_ms":'))
    js_u64(sink, elapsed)
    js_literal(sink, StringSlice(',"exhausted":'))
    js_u64(sink, counts.exhausted)
    js_literal(sink, StringSlice(',"healthy":'))
    js_u64(sink, counts.healthy)
    js_literal(sink, StringSlice(',"other_failures":'))
    js_u64(sink, counts.other_failures)
    js_literal(sink, StringSlice(',"pool_usable":'))
    if counts.pool_usable:
        js_literal(sink, StringSlice("true"))
    else:
        js_literal(sink, StringSlice("false"))
    js_literal(sink, StringSlice(',"profiles_discovered":'))
    js_u64(sink, counts.total)
    js_literal(sink, StringSlice(',"profiles_tested":'))
    js_u64(sink, counts.total)
    js_literal(sink, StringSlice(',"temporary_failures":'))
    js_u64(sink, counts.temporary_failures)
    js_literal(sink, StringSlice('}}'))


def ping_report_emit(
    sink: Pointer[mut=True, JsonSink, _], rows_address: UInt64, count: Int64,
    kind: Int64, elapsed_low: UInt64, elapsed_high: UInt64,
) -> Bool:
    var rows = Pointer[mut=False, PingReportRow, ImmUntrackedOrigin](unsafe_from_address=Int(rows_address))
    if kind == 0:
        ping_report_profile_text(sink, rows[])
        return True
    var counts = ping_summary_counts(
        Pointer[mut=False, Int64, ImmUntrackedOrigin](unsafe_from_address=Int(rows_address)),
        count, PING_REPORT_ROW_WORDS,
    )
    if not counts.valid:
        return False
    if kind == 1:
        ping_report_summary_text(sink, counts, ping_report_wide(elapsed_low, elapsed_high))
    else:
        ping_report_summary_json(sink, rows, count, counts, elapsed_low)
    return True


@export("prodex_mojo_ping_report_v1")
def prodex_mojo_ping_report_v1(
    abi: Int64, kind: Int64, rows_address: UInt64, count: Int64,
    elapsed_low: UInt64, elapsed_high: UInt64, measuring: Int64,
    output_address: UInt64, capacity: Int64, metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (
        kind < 0 or kind > 2 or count < 0 or count > PING_SUMMARY_MAX_STATUSES
        or rows_address % 8 != 0 or (count > 0 and rows_address == 0)
        or measuring < 0 or measuring > 1 or capacity < 0
        or metadata_address == 0 or metadata_address % 8 != 0
        or (kind == 0 and (count != 1 or elapsed_low != 0 or elapsed_high != 0))
        or (kind == 2 and elapsed_high != 0)
    ):
        return 1
    if measuring == 1:
        if output_address != 0 or capacity != 0:
            return 1
    elif output_address == 0:
        return 1
    var rows = Pointer[mut=False, PingReportRow, ImmUntrackedOrigin](unsafe_from_address=Int(rows_address))
    for index in range(count):
        if not ping_report_row_valid(rows[unsafe_offset=index], kind == 2):
            return 1
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var sink = JsonSink(output, capacity, 0, True, False)
    if not ping_report_emit(Pointer(to=sink), rows_address, count, kind, elapsed_low, elapsed_high):
        return 1
    if sink.failed:
        return 3
    var required = sink.written
    if measuring == 0:
        if capacity < required:
            return 3
        sink = JsonSink(output, capacity, 0, False, False)
        if not ping_report_emit(Pointer(to=sink), rows_address, count, kind, elapsed_low, elapsed_high) or sink.failed or sink.written != required:
            return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = required
    return 0
