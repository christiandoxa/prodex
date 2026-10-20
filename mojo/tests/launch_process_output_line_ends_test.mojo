# Direct ABI: all LF positions from a given byte offset, without UTF-8.
# Run with mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/launch_process_output_line_ends_test.mojo.
from std.memory.alloc import alloc, dealloc, Layout
from launch_config import prodex_launch_process_line_ends_v1


def main() raises:
    var output_storage = alloc(Layout[Int64](count=3))
    var count_storage = alloc(Layout[Int64](count=1))
    var output = output_storage.unsafe_ptr()
    var count = count_storage.unsafe_ptr()
    var text = StringSlice("one\ntwo\npartial")
    var source = UInt(Int(text.unsafe_ptr()))
    var result = UInt(Int(output))
    var written = UInt(Int(count))
    assert prodex_launch_process_line_ends_v1(
        1, source, Int64(text.byte_length()), 0, 0, 0, 1, written,
    ) == 0
    assert count[unsafe_offset=0] == 2
    for i in range(3):
        output[unsafe_offset=i] = 71
    assert prodex_launch_process_line_ends_v1(
        1, source, Int64(text.byte_length()), 0, result, 1, 0, written,
    ) == 3
    assert count[unsafe_offset=0] == 2
    assert output[unsafe_offset=0] == 71
    assert prodex_launch_process_line_ends_v1(
        1, source, Int64(text.byte_length()), 0, result, 2, 0, written,
    ) == 0
    assert count[unsafe_offset=0] == 2
    assert output[unsafe_offset=0] == 3
    assert output[unsafe_offset=1] == 7
    assert output[unsafe_offset=2] == 71

    assert prodex_launch_process_line_ends_v1(
        1, source, Int64(text.byte_length()), 4, 0, 0, 1, written,
    ) == 0
    assert count[unsafe_offset=0] == 1
    assert prodex_launch_process_line_ends_v1(
        1, source, Int64(text.byte_length()), 4, result, 1, 0, written,
    ) == 0
    assert output[unsafe_offset=0] == 7

    count[unsafe_offset=0] = 83
    output[unsafe_offset=0] = 79
    assert prodex_launch_process_line_ends_v1(
        2, source, Int64(text.byte_length()), 0, result, 3, 0, written,
    ) == 4
    assert prodex_launch_process_line_ends_v1(
        1, 0, Int64(text.byte_length()), 0, result, 3, 0, written,
    ) == 1
    assert prodex_launch_process_line_ends_v1(
        1, source, Int64(text.byte_length()), -1, result, 3, 0, written,
    ) == 1
    assert prodex_launch_process_line_ends_v1(
        1, source, Int64(text.byte_length()), 0, result, 3, 2, written,
    ) == 1
    assert output[unsafe_offset=0] == 79
    assert count[unsafe_offset=0] == 83
    assert prodex_launch_process_line_ends_v1(
        1, 0, 0, 0, 0, 0, 1, written,
    ) == 0
    assert count[unsafe_offset=0] == 0
    dealloc(count_storage^)
    dealloc(output_storage^)
