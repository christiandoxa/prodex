# Direct compiled quota state profile precedence and fallback ABI.
from std.memory.alloc import alloc, dealloc, Layout
from quota import prodex_quota_state_current_profile_v1


def main() raises:
    var rows_storage = alloc(Layout[Int64](count=16))
    var output_storage = alloc(Layout[Int64](count=1))
    var rows = rows_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var names = [
        StringSlice("z"), StringSlice("a"),
        StringSlice("雪"), StringSlice("deleted"),
    ]
    for index in range(4):
        rows[unsafe_offset=index * 4] = Int64(UInt(Int(names[index].unsafe_ptr())))
        rows[unsafe_offset=index * 4 + 1] = Int64(names[index].byte_length())
        rows[unsafe_offset=index * 4 + 2] = 20
        rows[unsafe_offset=index * 4 + 3] = 1
    rows[unsafe_offset=14] = 100
    rows[unsafe_offset=15] = 0
    var source = UInt(Int(rows))
    var result = UInt(Int(output))
    assert prodex_quota_state_current_profile_v1(
        1, source, 4, 1, result,
    ) == 0
    assert output[unsafe_offset=0] == 2

    rows[unsafe_offset=3] = 0
    rows[unsafe_offset=7] = 0
    rows[unsafe_offset=11] = 0
    assert prodex_quota_state_current_profile_v1(
        1, source, 4, 1, result,
    ) == 0
    assert output[unsafe_offset=0] == -1
    assert prodex_quota_state_current_profile_v1(
        1, source, 4, 0, result,
    ) == 0
    assert output[unsafe_offset=0] == -2

    output[unsafe_offset=0] = 77
    assert prodex_quota_state_current_profile_v1(
        2, source, 4, 1, result,
    ) == 4
    assert prodex_quota_state_current_profile_v1(
        1, source, 4, 2, result,
    ) == 1
    rows[unsafe_offset=15] = 2
    assert prodex_quota_state_current_profile_v1(
        1, source, 4, 1, result,
    ) == 1
    assert output[unsafe_offset=0] == 77
    assert prodex_quota_state_current_profile_v1(
        1, 0, 0, 0, result,
    ) == 0
    assert output[unsafe_offset=0] == -2
    dealloc(output_storage^)
    dealloc(rows_storage^)
