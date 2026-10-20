# Direct compiled quota sorting ABI test for all report ordering contracts.
from std.memory.alloc import alloc, dealloc, Layout
from quota import prodex_quota_report_sort_batch_v1


def main() raises:
    var rows_storage = alloc(Layout[Int64](count=44))
    var output_storage = alloc(Layout[Int64](count=4))
    var rows = rows_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    for index in range(44):
        rows[unsafe_offset=index] = 0
    var zulu = StringSlice("Zulu")
    var alpha = StringSlice("alpha")
    var upper = StringSlice("Alpha")
    var unicode = StringSlice("α")
    rows[unsafe_offset=0] = 0
    rows[unsafe_offset=1] = 2
    rows[unsafe_offset=2] = 100
    rows[unsafe_offset=3] = Int64(UInt(Int(zulu.unsafe_ptr())))
    rows[unsafe_offset=4] = Int64(zulu.byte_length())
    rows[unsafe_offset=11] = 1
    rows[unsafe_offset=12] = 1
    rows[unsafe_offset=13] = 50
    rows[unsafe_offset=14] = Int64(UInt(Int(alpha.unsafe_ptr())))
    rows[unsafe_offset=15] = Int64(alpha.byte_length())
    rows[unsafe_offset=22] = 1
    rows[unsafe_offset=23] = 1
    rows[unsafe_offset=24] = 20
    rows[unsafe_offset=25] = Int64(UInt(Int(upper.unsafe_ptr())))
    rows[unsafe_offset=26] = Int64(upper.byte_length())
    rows[unsafe_offset=33] = 0
    rows[unsafe_offset=34] = 0
    rows[unsafe_offset=35] = 10
    rows[unsafe_offset=36] = Int64(UInt(Int(unicode.unsafe_ptr())))
    rows[unsafe_offset=37] = Int64(unicode.byte_length())
    var source = UInt(Int(rows))
    var dest = UInt(Int(output))

    assert prodex_quota_report_sort_batch_v1(1, 0, source, 4, dest, 4) == 0
    assert output[unsafe_offset=0] == 2
    assert output[unsafe_offset=1] == 1
    assert output[unsafe_offset=2] == 3
    assert output[unsafe_offset=3] == 0

    assert prodex_quota_report_sort_batch_v1(1, 1, source, 4, dest, 4) == 0
    assert output[unsafe_offset=0] == 3
    assert output[unsafe_offset=1] == 2
    assert output[unsafe_offset=2] == 1
    assert output[unsafe_offset=3] == 0

    assert prodex_quota_report_sort_batch_v1(1, 2, source, 4, dest, 4) == 0
    assert output[unsafe_offset=0] == 2
    assert output[unsafe_offset=1] == 1
    assert output[unsafe_offset=2] == 0
    assert output[unsafe_offset=3] == 3

    for index in range(4):
        output[unsafe_offset=index] = 87
    assert prodex_quota_report_sort_batch_v1(2, 2, source, 4, dest, 4) == 4
    assert prodex_quota_report_sort_batch_v1(1, 6, source, 4, dest, 4) == 1
    assert prodex_quota_report_sort_batch_v1(1, 2, source, 4, dest, 3) == 1
    rows[unsafe_offset=34] = -1
    assert prodex_quota_report_sort_batch_v1(1, 2, source, 4, dest, 4) == 1
    for index in range(4):
        assert output[unsafe_offset=index] == 87
    dealloc(output_storage^)
    dealloc(rows_storage^)
