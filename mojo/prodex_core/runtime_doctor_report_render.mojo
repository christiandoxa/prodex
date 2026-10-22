# Ordered human-report fields; the host supplies paths, maps and binding text.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid
from parsed_json import pj_literal
from json_sink import JsonSink, js_byte, js_literal, js_string, js_escaped, js_u64
from runtime_doctor_render import runtime_doctor_report_field_enabled, runtime_doctor_report_sync_source


@fieldwise_init
struct RuntimeDoctorReportList(Copyable):
    var values: UInt
    var count: Int64


@fieldwise_init
struct RuntimeDoctorReportInput(Copyable):
    var values: UInt
    var presence: UInt64
    var pointer_exists: UInt64
    var log_exists: UInt64
    var line_count: UInt64
    var profile_count: UInt64
    var legacy_previous: UInt64
    var compact_final: UInt64
    var sync_probe: UInt64
    var probe_pressure: UInt64
    var lists: Array[RuntimeDoctorReportList, 3]


def report_value(input: RuntimeDoctorReportInput, index: Int) -> ProdexRichStringView:
    return Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(input.values)
    )[unsafe_offset=index].copy()


def report_present(input: RuntimeDoctorReportInput, index: Int) -> Bool:
    return input.presence & (UInt64(1) << UInt64(index)) != 0


def report_fallback(input: RuntimeDoctorReportInput, index: Int) -> ProdexRichStringView:
    return report_value(input, index) if report_present(input, index) else pj_literal("-")


def report_row(writer: Pointer[mut=True, JsonSink, _], label: StringSlice):
    if writer[].written > 1:
        js_byte(writer, 44)
    js_byte(writer, 91)
    js_string(writer, pj_literal(label))
    js_literal(writer, StringSlice(",\""))


def report_row_end(writer: Pointer[mut=True, JsonSink, _]):
    js_literal(writer, StringSlice("\"]"))


def report_text_row(
    writer: Pointer[mut=True, JsonSink, _], label: StringSlice, value: ProdexRichStringView,
):
    report_row(writer, label)
    js_escaped(writer, value)
    report_row_end(writer)


def report_count_row(
    writer: Pointer[mut=True, JsonSink, _], label: StringSlice, value: UInt64,
):
    report_row(writer, label)
    js_u64(writer, value)
    report_row_end(writer)


def report_list_row(
    writer: Pointer[mut=True, JsonSink, _], label: StringSlice, values: RuntimeDoctorReportList,
):
    report_row(writer, label)
    if values.count == 0:
        js_byte(writer, 45)
        report_row_end(writer)
        return
    var views = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(values.values)
    )
    for index in range(min(values.count, 5)):
        if index > 0:
            js_literal(writer, StringSlice("; "))
        js_escaped(writer, views[unsafe_offset=index])
    report_row_end(writer)


def report_render(writer: Pointer[mut=True, JsonSink, _], input: RuntimeDoctorReportInput):
    js_byte(writer, 91)
    report_row(writer, "Log pointer")
    js_escaped(writer, report_value(input, 0))
    js_literal(writer, StringSlice(" (exists)" if input.pointer_exists != 0 else " (missing)"))
    report_row_end(writer)
    report_row(writer, "Latest log")
    js_escaped(writer, report_fallback(input, 1))
    js_literal(writer, StringSlice(" (exists)" if input.log_exists != 0 else " (missing)"))
    report_row_end(writer)
    report_row(writer, "Log sample")
    js_u64(writer, input.line_count)
    js_literal(writer, StringSlice(" lines"))
    report_row_end(writer)
    report_text_row(writer, "Diagnosis", report_value(input, 2))
    report_text_row(writer, "Selection pressure", report_value(input, 3))
    report_text_row(writer, "Transport pressure", report_value(input, 4))
    report_text_row(writer, "Persistence pressure", report_value(input, 5))
    report_text_row(writer, "Quota freshness pressure", report_value(input, 6))
    report_count_row(writer, "Profiles", input.profile_count)
    report_text_row(writer, "Binding state", report_value(input, 7))
    report_list_row(writer, "Degraded routes", input.lists[0])
    report_list_row(writer, "Suspect continuations", input.lists[1])
    report_list_row(writer, "Runtime brokers", input.lists[2])

    var bits = Int64(input.legacy_previous > 0) | (Int64(input.compact_final > 0) << 1)
    bits |= (Int64(input.sync_probe > 0) << 2) | (Int64(input.probe_pressure > 0) << 3)
    bits |= (Int64(report_present(input, 14)) << 4) | (Int64(report_present(input, 11)) << 5)
    bits |= Int64(report_present(input, 12)) << 6
    var source = runtime_doctor_report_sync_source(bits)
    for field in range(Int64(8)):
        if not runtime_doctor_report_field_enabled(bits, field):
            continue
        if field == 0:
            report_count_row(writer, "Legacy prev recovery", input.legacy_previous)
        elif field == 1:
            report_count_row(writer, "Compact final", input.compact_final)
        elif field == 2:
            report_text_row(writer, "Compact exit", report_fallback(input, 8))
        elif field == 3:
            report_text_row(writer, "Compact reason", report_fallback(input, 9))
        elif field == 4:
            report_text_row(writer, "Compact last fail", report_fallback(input, 10))
        elif field == 5:
            report_row(writer, "Sync-probe deferred")
            if source == 1 or source == 2:
                js_escaped(writer, report_value(input, 11 if source == 1 else 12))
                js_literal(writer, StringSlice(" job(s)" if source == 1 else " profile(s)"))
            else:
                js_byte(writer, 45)
            report_row_end(writer)
        elif field == 6:
            report_text_row(writer, "Probe pressure profile", report_fallback(input, 13))
        elif field == 7:
            report_text_row(writer, "Hot reason", report_value(input, 14))
    js_byte(writer, 93)


@export("prodex_mojo_runtime_doctor_report_render_v1")
def prodex_mojo_runtime_doctor_report_render_v1(
    version: Int64, input_address: UInt, output_address: UInt,
    capacity: Int64, written_address: UInt,
) abi("C") -> Int64:
    if version != 1:
        return 4
    if (input_address == 0 or input_address % 8 != 0
        or capacity < 0 or written_address == 0 or written_address % 8 != 0):
        return 1
    var input = Pointer[mut=False, RuntimeDoctorReportInput, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_address)
    )[].copy()
    if (input.values == 0 or input.values % 8 != 0 or input.presence > 32767
        or input.presence & 253 != 253 or input.pointer_exists > 1 or input.log_exists > 1):
        return 1
    for index in range(15):
        if not rich_view_valid(report_value(input, index), 0x7FFFFFFFFFFFFFFF):
            return 1
    for index in range(3):
        var values = input.lists[index].copy()
        if (values.count < 0 or values.count > 0x7FFFFFFFFFFFFFFF // 16
            or values.count > 0 and (values.values == 0 or values.values % 8 != 0)):
            return 1
        if values.count > 0:
            var views = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
                unsafe_from_address=Int(values.values)
            )
            for item in range(values.count):
                if not rich_view_valid(views[unsafe_offset=item], 0x7FFFFFFFFFFFFFFF):
                    return 1
    var writer = JsonSink(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=1), 0, 0, True, False,
    )
    report_render(Pointer(to=writer), input)
    if writer.failed:
        return 3
    if writer.written > capacity:
        return 3
    if writer.written > 0 and output_address == 0:
        return 1
    writer.output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    writer.capacity = capacity
    writer.written = 0
    writer.measuring = False
    report_render(Pointer(to=writer), input)
    if writer.failed:
        return 3
    Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))[] = writer.written
    return 0
