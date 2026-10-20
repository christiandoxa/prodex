# Direct ABI tests: three production recent-log path sort contracts.
# Run with mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/recent_log_paths_test.mojo.
from std.memory.alloc import alloc, dealloc, Layout
from log_semantics import prodex_mojo_recent_log_paths_v1


def main() raises:
    var rows_storage = alloc(Layout[UInt64](count=24))
    var scratch_storage = alloc(Layout[Int64](count=4))
    var output_storage = alloc(Layout[Int64](count=4))
    var count_storage = alloc(Layout[Int64](count=1))
    var rows = rows_storage.unsafe_ptr()
    var scratch = scratch_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var count = count_storage.unsafe_ptr()
    for index in range(24):
        rows[unsafe_offset=index] = 0
    var z = StringSlice("z")
    var a = StringSlice("a")
    var accent = StringSlice("é")
    var snow = StringSlice("雪")
    rows[unsafe_offset=0] = UInt64(UInt(Int(z.unsafe_ptr())))
    rows[unsafe_offset=1] = UInt64(z.byte_length())
    rows[unsafe_offset=6] = UInt64(UInt(Int(a.unsafe_ptr())))
    rows[unsafe_offset=7] = UInt64(a.byte_length())
    rows[unsafe_offset=12] = UInt64(UInt(Int(accent.unsafe_ptr())))
    rows[unsafe_offset=13] = UInt64(accent.byte_length())
    rows[unsafe_offset=18] = UInt64(UInt(Int(snow.unsafe_ptr())))
    rows[unsafe_offset=19] = UInt64(snow.byte_length())
    for index in range(4):
        rows[unsafe_offset=index * 6 + 4] = 2500
    var source = UInt(Int(rows))
    var work = UInt(Int(scratch))
    var result = UInt(Int(output))
    var written = UInt(Int(count))
    assert prodex_mojo_recent_log_paths_v1(
        1, 0, source, 4, 4, work, 4, result, 4, written,
    ) == 0
    assert count[unsafe_offset=0] == 4
    assert output[unsafe_offset=0] == 1
    assert output[unsafe_offset=1] == 0
    assert output[unsafe_offset=2] == 2
    assert output[unsafe_offset=3] == 3

    rows[unsafe_offset=20] = 1
    rows[unsafe_offset=21] = 0
    rows[unsafe_offset=22] = 3
    rows[unsafe_offset=23] = 0
    assert prodex_mojo_recent_log_paths_v1(
        1, 1, source, 4, 3, work, 4, result, 3, written,
    ) == 0
    assert count[unsafe_offset=0] == 3
    assert output[unsafe_offset=0] == 2
    assert output[unsafe_offset=1] == 0
    assert output[unsafe_offset=2] == 1

    # Unsigned u128 mode has no pre-epoch row. Place snow in the high word.
    rows[unsafe_offset=20] = 0
    rows[unsafe_offset=21] = 1
    assert prodex_mojo_recent_log_paths_v1(
        1, 2, source, 4, 4, work, 4, result, 4, written,
    ) == 0
    assert count[unsafe_offset=0] == 4
    assert output[unsafe_offset=0] == 3
    assert output[unsafe_offset=1] == 2
    assert output[unsafe_offset=2] == 0
    assert output[unsafe_offset=3] == 1

    for index in range(4):
        output[unsafe_offset=index] = 73
        scratch[unsafe_offset=index] = 71
    count[unsafe_offset=0] = 91
    assert prodex_mojo_recent_log_paths_v1(
        2, 1, source, 4, 4, work, 4, result, 4, written,
    ) == 4
    assert prodex_mojo_recent_log_paths_v1(
        1, 3, source, 4, 4, work, 4, result, 4, written,
    ) == 1
    assert prodex_mojo_recent_log_paths_v1(
        1, 1, source, 4, 5, work, 4, result, 4, written,
    ) == 1
    assert prodex_mojo_recent_log_paths_v1(
        1, 1, source, 4, 4, work, 3, result, 4, written,
    ) == 1
    rows[unsafe_offset=23] = 1_000_000_000
    assert prodex_mojo_recent_log_paths_v1(
        1, 1, source, 4, 4, work, 4, result, 4, written,
    ) == 1
    assert count[unsafe_offset=0] == 91
    for index in range(4):
        assert output[unsafe_offset=index] == 73
        assert scratch[unsafe_offset=index] == 71
    assert prodex_mojo_recent_log_paths_v1(
        1, 0, 0, 0, 0, work, 4, result, 4, written,
    ) == 0
    assert count[unsafe_offset=0] == 0
    dealloc(count_storage^)
    dealloc(output_storage^)
    dealloc(scratch_storage^)
    dealloc(rows_storage^)
