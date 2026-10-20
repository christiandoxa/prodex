# Direct ABI: missing metadata first, chronological old-to-new, path ties.
from std.memory.alloc import alloc, dealloc, Layout
from quota import prodex_quota_runtime_log_order_v1


def main() raises:
    var rows_storage = alloc(Layout[UInt64](count=24))
    var output_storage = alloc(Layout[Int64](count=4))
    var rows = rows_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    for index in range(24):
        rows[unsafe_offset=index] = 0
    var z = StringSlice("z")
    var missing = StringSlice("missing")
    var snow = StringSlice("雪")
    var a = StringSlice("a")
    var names = [z, missing, snow, a]
    for index in range(4):
        rows[unsafe_offset=index * 6] = UInt64(UInt(Int(names[index].unsafe_ptr())))
        rows[unsafe_offset=index * 6 + 1] = UInt64(names[index].byte_length())
    rows[unsafe_offset=2] = 1
    rows[unsafe_offset=4] = 1
    rows[unsafe_offset=14] = 1
    rows[unsafe_offset=15] = 1
    rows[unsafe_offset=16] = 2
    rows[unsafe_offset=20] = 1
    rows[unsafe_offset=22] = 1
    var source = UInt(Int(rows))
    var result = UInt(Int(output))
    assert prodex_quota_runtime_log_order_v1(1, source, 4, result, 4) == 0
    assert output[unsafe_offset=0] == 1
    assert output[unsafe_offset=1] == 2
    assert output[unsafe_offset=2] == 3
    assert output[unsafe_offset=3] == 0

    for index in range(4):
        output[unsafe_offset=index] = 77
    assert prodex_quota_runtime_log_order_v1(2, source, 4, result, 4) == 4
    assert prodex_quota_runtime_log_order_v1(1, source, 4, result, 3) == 1
    rows[unsafe_offset=11] = 1
    assert prodex_quota_runtime_log_order_v1(1, source, 4, result, 4) == 1
    for index in range(4):
        assert output[unsafe_offset=index] == 77
    assert prodex_quota_runtime_log_order_v1(1, 0, 0, result, 4) == 0
    dealloc(output_storage^)
    dealloc(rows_storage^)
