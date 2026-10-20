# Direct compiled log follower budget, per-source reservation and duplicates.
# Run with mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/log_follow_selection_test.mojo
from std.memory.alloc import alloc, dealloc, Layout
from log_semantics import prodex_mojo_log_follow_selection_v1


def main() raises:
    var rows_storage = alloc(Layout[UInt64](count=24))
    var scratch_storage = alloc(Layout[Int64](count=4))
    var flags_storage = alloc(Layout[Int64](count=4))
    var total_storage = alloc(Layout[Int64](count=1))
    var rows = rows_storage.unsafe_ptr()
    var scratch = scratch_storage.unsafe_ptr()
    var flags = flags_storage.unsafe_ptr()
    var total = total_storage.unsafe_ptr()
    for index in range(24):
        rows[unsafe_offset=index] = 0
    var a = StringSlice("same")
    var b = StringSlice("other")
    for index in range(4):
        rows[unsafe_offset=index * 6] = UInt64(UInt(Int(a.unsafe_ptr())))
        rows[unsafe_offset=index * 6 + 1] = UInt64(a.byte_length())
        rows[unsafe_offset=index * 6 + 4] = UInt64(index + 1)
    rows[unsafe_offset=2] = 0
    rows[unsafe_offset=8] = 0
    rows[unsafe_offset=14] = 1
    rows[unsafe_offset=18] = UInt64(UInt(Int(b.unsafe_ptr())))
    rows[unsafe_offset=19] = UInt64(b.byte_length())
    rows[unsafe_offset=20] = 1
    var input = UInt(Int(rows))
    var work = UInt(Int(scratch))
    var chosen = UInt(Int(flags))
    var count = UInt(Int(total))
    assert prodex_mojo_log_follow_selection_v1(
        1, input, 4, 2, work, 4, chosen, 4, count,
    ) == 0
    assert total[unsafe_offset=0] == 2
    assert flags[unsafe_offset=0] == 1
    assert flags[unsafe_offset=1] == 1
    assert flags[unsafe_offset=2] == 0
    assert flags[unsafe_offset=3] == 1
    assert prodex_mojo_log_follow_selection_v1(
        1, input, 4, 4, work, 4, chosen, 4, count,
    ) == 0
    assert total[unsafe_offset=0] == 3
    for index in range(4):
        assert flags[unsafe_offset=index] == 1

    for index in range(4):
        flags[unsafe_offset=index] = 73
        scratch[unsafe_offset=index] = 71
    total[unsafe_offset=0] = 91
    assert prodex_mojo_log_follow_selection_v1(
        2, input, 4, 2, work, 4, chosen, 4, count,
    ) == 4
    assert prodex_mojo_log_follow_selection_v1(
        1, input, 4, 2, work, 3, chosen, 4, count,
    ) == 1
    rows[unsafe_offset=20] = 2
    assert prodex_mojo_log_follow_selection_v1(
        1, input, 4, 2, work, 4, chosen, 4, count,
    ) == 1
    assert total[unsafe_offset=0] == 91
    for index in range(4):
        assert flags[unsafe_offset=index] == 73
        assert scratch[unsafe_offset=index] == 71
    assert prodex_mojo_log_follow_selection_v1(
        1, 0, 0, 32, work, 4, chosen, 4, count,
    ) == 0
    assert total[unsafe_offset=0] == 0
    dealloc(total_storage^)
    dealloc(flags_storage^)
    dealloc(scratch_storage^)
    dealloc(rows_storage^)
