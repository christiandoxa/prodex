# Direct read-only latest-log timestamp ordering ABI regression.
from std.memory.alloc import alloc, dealloc, Layout
from log_semantics import prodex_mojo_log_latest_timestamp_v1


def main() raises:
    var earlier = StringSlice("2026-10-20 10:00:00")
    var later = StringSlice("2026-10-21 12:34:00")
    var output_store = alloc(Layout[Int64](count=1))
    var output = output_store.unsafe_ptr()
    var result_address = UInt(Int(output))
    var old_ptr = UInt(Int(earlier.unsafe_ptr()))
    var new_ptr = UInt(Int(later.unsafe_ptr()))
    assert prodex_mojo_log_latest_timestamp_v1(
        1, old_ptr, Int64(earlier.byte_length()), 1,
        new_ptr, Int64(later.byte_length()), result_address
    ) == 0
    assert output[unsafe_offset=0] == 1
    assert prodex_mojo_log_latest_timestamp_v1(
        1, new_ptr, Int64(later.byte_length()), 1,
        old_ptr, Int64(earlier.byte_length()), result_address
    ) == 0
    assert output[unsafe_offset=0] == 0
    assert prodex_mojo_log_latest_timestamp_v1(
        1, new_ptr, Int64(later.byte_length()), 1,
        new_ptr, Int64(later.byte_length()), result_address
    ) == 0
    assert output[unsafe_offset=0] == 1
    assert prodex_mojo_log_latest_timestamp_v1(
        1, 0, 0, 0, new_ptr, Int64(later.byte_length()), result_address
    ) == 0
    assert output[unsafe_offset=0] == 1

    output[unsafe_offset=0] = 77
    assert prodex_mojo_log_latest_timestamp_v1(
        2, 0, 0, 0, new_ptr, Int64(later.byte_length()), result_address
    ) == 4
    assert prodex_mojo_log_latest_timestamp_v1(
        1, 0, 0, 2, new_ptr, Int64(later.byte_length()), result_address
    ) == 1
    assert prodex_mojo_log_latest_timestamp_v1(
        1, 0, 0, 0, new_ptr, 1_048_577, result_address
    ) == 1
    assert output[unsafe_offset=0] == 77
    dealloc(output_store^)
