from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from std.sys.info import size_of
from parsed_json import pj_literal
from rich_types import ProdexRichStringView
from runtime_doctor_report_render import (
    RuntimeDoctorReportInput, RuntimeDoctorReportList,
    prodex_mojo_runtime_doctor_report_render_v1,
)


def main() raises:
    assert size_of[RuntimeDoctorReportInput]() == 128
    var value_storage = alloc(Layout[ProdexRichStringView](count=15))
    var values = value_storage.unsafe_ptr()
    for index in range(15):
        values[unsafe_offset=index] = pj_literal("")
    values[unsafe_offset=0] = pj_literal("fixture")
    values[unsafe_offset=7] = pj_literal("binding")
    var lists = Array[RuntimeDoctorReportList, 3](fill=RuntimeDoctorReportList(0, 0))
    var input_storage = alloc(Layout[RuntimeDoctorReportInput](count=1))
    var input = input_storage.unsafe_ptr()
    input[] = RuntimeDoctorReportInput(
        UInt(Int(values)), 253, 1, 0, 0, 0, 0, 0, 0, 0, lists^,
    )
    var storage = alloc(Layout[UInt8](count=2048))
    var output = storage.unsafe_ptr()
    var address = UInt(Int(input))
    var target = UInt(Int(output))
    var written_storage = alloc(Layout[Int64](count=1))
    var written = written_storage.unsafe_ptr()
    written[] = -1
    var written_address = UInt(Int(written))
    assert prodex_mojo_runtime_doctor_report_render_v1(1, address, target, 2048, written_address) == 0
    var expected = StringSlice("[[\"Log pointer\",\"fixture (exists)\"],[\"Latest log\",\"- (missing)\"],[\"Log sample\",\"0 lines\"],[\"Diagnosis\",\"\"],[\"Selection pressure\",\"\"],[\"Transport pressure\",\"\"],[\"Persistence pressure\",\"\"],[\"Quota freshness pressure\",\"\"],[\"Profiles\",\"0\"],[\"Binding state\",\"binding\"],[\"Degraded routes\",\"-\"],[\"Suspect continuations\",\"-\"],[\"Runtime brokers\",\"-\"]]")
    assert written[] == Int64(expected.byte_length())
    for index in range(written[]):
        assert output[unsafe_offset=index] == expected.unsafe_ptr()[unsafe_offset=index]
    input[].legacy_previous = 5
    input[].compact_final = 7
    input[].sync_probe = 9
    input[].probe_pressure = 11
    input[].presence |= UInt64(1 << 8) | UInt64(1 << 9) | UInt64(1 << 11) | UInt64(1 << 12) | UInt64(1 << 14)
    values[unsafe_offset=9] = pj_literal("overload")
    values[unsafe_offset=11] = pj_literal("2")
    values[unsafe_offset=12] = pj_literal("3")
    values[unsafe_offset=14] = pj_literal("東京 (5)")
    assert prodex_mojo_runtime_doctor_report_render_v1(1, address, target, 2048, written_address) == 0
    var suffix = StringSlice(",[\"Legacy prev recovery\",\"5\"],[\"Compact final\",\"7\"],[\"Compact exit\",\"\"],[\"Compact reason\",\"overload\"],[\"Compact last fail\",\"-\"],[\"Sync-probe deferred\",\"2 job(s)\"],[\"Probe pressure profile\",\"-\"],[\"Hot reason\",\"東京 (5)\"]]")
    assert written[] == Int64(expected.byte_length() - 1 + suffix.byte_length())
    for index in range(Int64(suffix.byte_length())):
        assert output[unsafe_offset=written[] - Int64(suffix.byte_length()) + index] == suffix.unsafe_ptr()[unsafe_offset=index]
    for index in range(2048):
        output[unsafe_offset=index] = 73
    written[] = 83
    assert prodex_mojo_runtime_doctor_report_render_v1(2, address, target, 2048, written_address) == 4
    assert prodex_mojo_runtime_doctor_report_render_v1(1, address + 1, target, 2048, written_address) == 1
    assert prodex_mojo_runtime_doctor_report_render_v1(1, address, target, -1, written_address) == 1
    assert prodex_mojo_runtime_doctor_report_render_v1(1, address, target, 1, written_address) == 3
    assert prodex_mojo_runtime_doctor_report_render_v1(1, address, 0, 2048, written_address) == 1
    assert prodex_mojo_runtime_doctor_report_render_v1(1, address, target, 2048, written_address + 1) == 1
    input[].presence = 0
    assert prodex_mojo_runtime_doctor_report_render_v1(1, address, target, 2048, written_address) == 1
    for index in range(2048):
        assert output[unsafe_offset=index] == 73
    assert written[] == 83
    dealloc(storage^)
    dealloc(value_storage^)
    dealloc(input_storage^)
    dealloc(written_storage^)
