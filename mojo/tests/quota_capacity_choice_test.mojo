# Run with mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/quota_capacity_choice_test.mojo.
from std.memory.alloc import alloc, dealloc, Layout
from quota import prodex_quota_capacity_choice_v1


def main() raises:
    var rows_storage = alloc(Layout[Int64](count=8))
    var output_storage = alloc(Layout[Int64](count=2))
    var rows = rows_storage.unsafe_ptr()
    var result = output_storage.unsafe_ptr()
    for index in range(8):
        rows[unsafe_offset=index] = 0
    rows[unsafe_offset=0] = 2
    rows[unsafe_offset=1] = 0
    rows[unsafe_offset=2] = 0
    rows[unsafe_offset=3] = 0
    rows[unsafe_offset=4] = 1
    rows[unsafe_offset=5] = 1
    rows[unsafe_offset=6] = 1
    rows[unsafe_offset=7] = 1
    var source = UInt(Int(rows))
    var dest = UInt(Int(result))
    assert prodex_quota_capacity_choice_v1(1, source, 4, dest, 2) == 0
    assert result[unsafe_offset=0] == 2
    assert result[unsafe_offset=1] == 1
    rows[unsafe_offset=5] = 0
    rows[unsafe_offset=7] = 0
    assert prodex_quota_capacity_choice_v1(1, source, 4, dest, 2) == 0
    assert result[unsafe_offset=0] == 1
    assert result[unsafe_offset=1] == 0
    result[unsafe_offset=0] = 99
    result[unsafe_offset=1] = 88
    assert prodex_quota_capacity_choice_v1(2, source, 4, dest, 2) == 4
    assert prodex_quota_capacity_choice_v1(1, source, 4, dest, 1) == 1
    assert result[unsafe_offset=0] == 99 and result[unsafe_offset=1] == 88
    rows[unsafe_offset=6] = 3
    assert prodex_quota_capacity_choice_v1(1, source, 4, dest, 2) == 1
    assert result[unsafe_offset=0] == 99 and result[unsafe_offset=1] == 88
    assert prodex_quota_capacity_choice_v1(1, 0, 0, dest, 2) == 0
    assert result[unsafe_offset=0] == -1 and result[unsafe_offset=1] == 0
    dealloc(output_storage^)
    dealloc(rows_storage^)
