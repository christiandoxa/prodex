# Measured blocked-message ABI rejects invalid inputs before touching output.
from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from std.collections import Array
from parsed_json import pj_literal
from rich_types import ProdexRichStringView
from quota_blocked_windows import QuotaBlockedWindowRow, prodex_quota_blocked_windows_json_v1


def main() raises:
    var empty = ProdexRichStringView(0, 0)
    var storage = alloc(Layout[QuotaBlockedWindowRow](count=1))
    var row = storage.unsafe_ptr()
    row[] = QuotaBlockedWindowRow(1, 1, 100, 0, 1, 60, empty.copy(), empty.copy(), empty.copy(), pj_literal("-"))
    var output = Array[UInt8, 128](fill=77)
    var written: Int64 = -1
    var rows = UInt(Int(row))
    var dest = UInt(Int(output.unsafe_ptr()))
    var length = UInt(Int(Pointer(to=written)))
    assert prodex_quota_blocked_windows_json_v1(1, 0, 1, 0, rows, 1, 1, 0, 0, length) == 0
    var required = written
    assert required > 2
    written = -1
    assert prodex_quota_blocked_windows_json_v1(1, 0, 1, 0, rows, 1, 0, dest, 128, length) == 0
    assert written == required
    assert output[0] == 91 and output[Int(required) - 1] == 93
    assert prodex_quota_blocked_windows_json_v1(1, 0, 1, 0, rows, 1, 0, dest, required - 1, length) == 2

    for index in range(128):
        output[index] = 77
    written = -1
    assert prodex_quota_blocked_windows_json_v1(2, 0, 1, 0, rows, 1, 0, dest, 128, length) == 4
    assert prodex_quota_blocked_windows_json_v1(1, 2, 1, 0, rows, 1, 0, dest, 128, length) == 1
    assert prodex_quota_blocked_windows_json_v1(1, 0, 2, 0, rows, 1, 0, dest, 128, length) == 1
    assert prodex_quota_blocked_windows_json_v1(1, 0, 1, 2, rows, 1, 0, dest, 128, length) == 1
    assert prodex_quota_blocked_windows_json_v1(1, 0, 1, 0, rows, -1, 0, dest, 128, length) == 1
    assert prodex_quota_blocked_windows_json_v1(1, 0, 1, 0, 0, 1, 0, dest, 128, length) == 1
    assert prodex_quota_blocked_windows_json_v1(1, 0, 1, 0, rows + 1, 1, 0, dest, 128, length) == 1
    assert prodex_quota_blocked_windows_json_v1(1, 0, 1, 0, rows, 1, 1, dest, 128, length) == 1
    assert prodex_quota_blocked_windows_json_v1(1, 0, 1, 0, rows, 1, 0, 0, 128, length) == 1
    assert prodex_quota_blocked_windows_json_v1(1, 0, 1, 0, rows, 1, 0, dest, 128, length + 1) == 1
    row[].kind = 3
    assert prodex_quota_blocked_windows_json_v1(1, 0, 1, 0, rows, 1, 0, dest, 128, length) == 1
    row[].kind = 1
    row[].name = pj_literal("name without presence")
    assert prodex_quota_blocked_windows_json_v1(1, 0, 1, 0, rows, 1, 0, dest, 128, length) == 1
    row[].names_present = 1
    var invalid_utf8 = Array[UInt8, 1](fill=255)
    row[].name = ProdexRichStringView(UInt(Int(invalid_utf8.unsafe_ptr())), 1)
    assert prodex_quota_blocked_windows_json_v1(1, 0, 1, 0, rows, 1, 0, dest, 128, length) == 1
    assert written == -1
    for index in range(128):
        assert output[index] == 77
    dealloc(storage^)
