from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from rich_types import ProdexRichStringView
from parsed_json import pj_literal
from ping_report import PingReportRow, prodex_mojo_ping_report_v1


def check(output: Pointer[mut=True, UInt8, _], length: Int64, expected: StringSlice):
    assert length == Int64(expected.byte_length())
    for index in range(length):
        assert output[unsafe_offset=index] == expected.unsafe_ptr()[unsafe_offset=index]


def main() raises:
    var rows_storage = alloc(Layout[PingReportRow](count=2))
    var output_storage = alloc(Layout[UInt8](count=8192))
    var metadata_storage = alloc(Layout[Int64](count=2))
    var rows = rows_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var metadata = metadata_storage.unsafe_ptr()
    var empty = ProdexRichStringView(0, 0)
    rows[] = PingReportRow(0, 0, pj_literal(StringSlice("synthetic-missing")), empty.copy(), empty.copy(), empty.copy(), empty.copy(), 0, 0, 0, 0)
    var input = UInt64(Int(rows))
    var target = UInt64(Int(output))
    var meta = UInt64(Int(metadata))
    assert prodex_mojo_ping_report_v1(1, 0, input, 1, 0, 0, 0, target, 8192, meta) == 0
    check(output, metadata[unsafe_offset=1], StringSlice("synthetic-missing  OK                   first=unavailable completion=0ms  requested=configured/default effort=configured/default effective=unavailable"))
    assert prodex_mojo_ping_report_v1(1, 2, 0, 0, 0, 0, 0, target, 8192, meta) == 0
    check(output, metadata[unsafe_offset=1], StringSlice("{\"detail\":\"0/0 profiles healthy\",\"effective_model\":null,\"effort\":null,\"latency_ms\":0,\"model\":null,\"profiles\":[],\"provider\":\"openai\",\"requested_effort\":null,\"requested_model\":null,\"status\":\"failed\",\"summary\":{\"auth_failures\":0,\"duration_ms\":0,\"exhausted\":0,\"healthy\":0,\"other_failures\":0,\"pool_usable\":false,\"profiles_discovered\":0,\"profiles_tested\":0,\"temporary_failures\":0}}"))
    var unicode_row = PingReportRow(4, 31, pj_literal(StringSlice("profile-雪")), pj_literal(StringSlice("model-\"雪")), pj_literal(StringSlice("")), pj_literal(StringSlice("effective\nmodel")), pj_literal(StringSlice("synthetic reason\n雪\tquoted\"")), 0, 0, 18_446_744_073_709_551_615, 0)
    rows[] = unicode_row.copy()
    assert prodex_mojo_ping_report_v1(1, 0, input, 1, 0, 0, 0, target, 8192, meta) == 0
    check(output, metadata[unsafe_offset=1], StringSlice("profile-雪  AUTH_FAILED          first=0ms completion=18446744073709551615ms  requested=model-\"雪 effort= effective=effective\nmodel\n  reason: synthetic reason\n雪\tquoted\""))
    rows[].first_low = 18_446_744_073_709_551_615
    rows[].first_high = 18_446_744_073_709_551_615
    rows[].completion_high = 18_446_744_073_709_551_615
    assert prodex_mojo_ping_report_v1(1, 0, input, 1, 0, 0, 0, target, 8192, meta) == 0
    check(output, metadata[unsafe_offset=1], StringSlice("profile-雪  AUTH_FAILED          first=340282366920938463463374607431768211455ms completion=340282366920938463463374607431768211455ms  requested=model-\"雪 effort= effective=effective\nmodel\n  reason: synthetic reason\n雪\tquoted\""))
    rows[] = unicode_row.copy()
    rows[unsafe_offset=1] = PingReportRow(9, 0, pj_literal(StringSlice("synthetic-quota")), empty.copy(), empty.copy(), empty.copy(), pj_literal(StringSlice("synthetic detail")), 0, 0, 0, 0)
    assert prodex_mojo_ping_report_v1(1, 2, input, 2, 18_446_744_073_709_551_615, 0, 0, target, 8192, meta) == 0
    check(output, metadata[unsafe_offset=1], StringSlice("{\"detail\":\"0/2 profiles healthy\",\"effective_model\":\"effective\\nmodel\",\"effort\":\"\",\"latency_ms\":18446744073709551615,\"model\":\"model-\\\"雪\",\"profiles\":[{\"completion_latency_ms\":18446744073709551615,\"credential_validation\":\"failed\",\"detail\":\"synthetic reason\\n雪\\tquoted\\\"\",\"effective_model\":\"effective\\nmodel\",\"effort\":\"\",\"first_response_latency_ms\":0,\"latency_ms\":18446744073709551615,\"model\":\"model-\\\"雪\",\"profile\":\"profile-雪\",\"requested_effort\":\"\",\"requested_model\":\"model-\\\"雪\",\"status\":\"auth_failed\"},{\"completion_latency_ms\":null,\"credential_validation\":\"unknown\",\"detail\":\"synthetic detail\",\"effective_model\":null,\"effort\":null,\"first_response_latency_ms\":null,\"latency_ms\":null,\"model\":null,\"profile\":\"synthetic-quota\",\"requested_effort\":null,\"requested_model\":null,\"status\":\"exhausted\"}],\"provider\":\"openai\",\"requested_effort\":\"\",\"requested_model\":\"model-\\\"雪\",\"status\":\"failed\",\"summary\":{\"auth_failures\":1,\"duration_ms\":18446744073709551615,\"exhausted\":1,\"healthy\":0,\"other_failures\":0,\"pool_usable\":false,\"profiles_discovered\":2,\"profiles_tested\":2,\"temporary_failures\":0}}"))
    assert prodex_mojo_ping_report_v1(1, 1, input, 2, 18_446_744_073_709_551_615, 999, 0, target, 8192, meta) == 0
    check(output, metadata[unsafe_offset=1], StringSlice("Profiles discovered: 2\nProfiles tested: 2\nHealthy: 0\nExhausted: 1\nAuth failures: 1\nTemporary failures: 0\nOther failures: 0\nTotal duration: 18446744073709551615999ms\nPool usable: no"))
    assert prodex_mojo_ping_report_v1(1, 2, input, 2, 0, 0, 1, 0, 0, meta) == 0
    var required = metadata[unsafe_offset=1]
    for index in range(8192):
        output[unsafe_offset=index] = 93
    metadata[unsafe_offset=0] = 77
    metadata[unsafe_offset=1] = 88
    assert prodex_mojo_ping_report_v1(1, 2, input, 2, 0, 0, 0, target, required - 1, meta) == 3
    assert prodex_mojo_ping_report_v1(2, 2, input, 2, 0, 0, 0, target, 8192, meta) == 4
    assert prodex_mojo_ping_report_v1(1, 1, input, 65537, 0, 0, 0, target, 8192, meta) == 1
    assert prodex_mojo_ping_report_v1(1, 2, input, 2, 0, 1, 0, target, 8192, meta) == 1
    rows[].first_high = 1
    assert prodex_mojo_ping_report_v1(1, 2, input, 2, 0, 0, 0, target, 8192, meta) == 1
    rows[] = unicode_row.copy()
    rows[].presence = 32
    assert prodex_mojo_ping_report_v1(1, 2, input, 2, 0, 0, 0, target, 8192, meta) == 1
    assert output[] == 93 and metadata[unsafe_offset=0] == 77 and metadata[unsafe_offset=1] == 88
    dealloc(rows_storage^)
    dealloc(output_storage^)
    dealloc(metadata_storage^)
