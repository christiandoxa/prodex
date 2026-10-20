# Direct ABI: binary last LF and post-lossy UTF-8 line lengths.
# Run with mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/live_log_follow_framing_test.mojo.
from std.memory.alloc import alloc, dealloc, Layout
from live_log_record import (
    prodex_live_log_follow_last_newline_v1,
    prodex_live_log_follow_line_lengths_v1,
)


def main() raises:
    var lengths_storage = alloc(Layout[Int64](count=3))
    var output_storage = alloc(Layout[Int64](count=1))
    var lengths = lengths_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var binary = StringSlice("a\nb\npartial")
    var address = UInt(Int(binary.unsafe_ptr()))
    var result = UInt(Int(output))
    assert prodex_live_log_follow_last_newline_v1(
        1, address, Int64(binary.byte_length()), result,
    ) == 0
    assert output[unsafe_offset=0] == 4

    var text = StringSlice("é\n雪\n")
    address = UInt(Int(text.unsafe_ptr()))
    var selected = UInt(Int(lengths))
    assert prodex_live_log_follow_line_lengths_v1(
        1, address, Int64(text.byte_length()), 0, 0, 1, result,
    ) == 0
    assert output[unsafe_offset=0] == 2
    for i in range(3):
        lengths[unsafe_offset=i] = 71
    assert prodex_live_log_follow_line_lengths_v1(
        1, address, Int64(text.byte_length()), selected, 1, 0, result,
    ) == 2
    for i in range(3):
        assert lengths[unsafe_offset=i] == 71
    assert output[unsafe_offset=0] == 2

    assert prodex_live_log_follow_line_lengths_v1(
        1, address, Int64(text.byte_length()), selected, 2, 0, result,
    ) == 0
    assert output[unsafe_offset=0] == 2
    assert lengths[unsafe_offset=0] == 3
    assert lengths[unsafe_offset=1] == 4
    assert lengths[unsafe_offset=2] == 71

    output[unsafe_offset=0] = 83
    lengths[unsafe_offset=0] = 77
    assert prodex_live_log_follow_last_newline_v1(
        2, address, Int64(text.byte_length()), result,
    ) == 4
    assert prodex_live_log_follow_last_newline_v1(
        1, 0, Int64(text.byte_length()), result,
    ) == 1
    assert prodex_live_log_follow_line_lengths_v1(
        2, address, Int64(text.byte_length()), selected, 2, 0, result,
    ) == 4
    assert prodex_live_log_follow_line_lengths_v1(
        1, 0, Int64(text.byte_length()), selected, 2, 0, result,
    ) == 1
    assert output[unsafe_offset=0] == 83
    assert lengths[unsafe_offset=0] == 77

    assert prodex_live_log_follow_last_newline_v1(1, 0, 0, result) == 0
    assert output[unsafe_offset=0] == 0
    assert prodex_live_log_follow_line_lengths_v1(
        1, 0, 0, 0, 0, 1, result,
    ) == 0
    assert output[unsafe_offset=0] == 0
    dealloc(output_storage^)
    dealloc(lengths_storage^)
