# Run with mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/anthropic_stream_data_line_test.mojo.
from std.memory.alloc import alloc, dealloc, Layout
from anthropic_stream_policy import prodex_anthropic_stream_data_line_v1


def main() raises:
    var result_storage = alloc(Layout[Int64](count=3))
    var result = result_storage.unsafe_ptr()
    var output = UInt(Int(result))
    var event = StringSlice("event: ping\r\ndata:  雪 \r\ndata: later\n")
    var before = StringSlice("event: ping\r\ndata: ")
    var data = StringSlice(" 雪 ")
    assert prodex_anthropic_stream_data_line_v1(
        1, UInt(Int(event.unsafe_ptr())), Int64(event.byte_length()), output,
    ) == 0
    assert result[unsafe_offset=0] == 1
    assert result[unsafe_offset=1] == Int64(before.byte_length())
    assert result[unsafe_offset=2] == Int64(before.byte_length() + data.byte_length())

    var blank = StringSlice("data: \r")
    assert prodex_anthropic_stream_data_line_v1(
        1, UInt(Int(blank.unsafe_ptr())), Int64(blank.byte_length()), output,
    ) == 0
    assert result[unsafe_offset=0] == 1
    assert result[unsafe_offset=1] == 6 and result[unsafe_offset=2] == 6

    var unsupported = StringSlice("DATA: nope\ndata:missing-space\n")
    assert prodex_anthropic_stream_data_line_v1(
        1, UInt(Int(unsupported.unsafe_ptr())), Int64(unsupported.byte_length()), output,
    ) == 0
    assert result[unsafe_offset=0] == 0
    assert result[unsafe_offset=1] == 0 and result[unsafe_offset=2] == 0

    result[unsafe_offset=0] = 81
    result[unsafe_offset=1] = 82
    result[unsafe_offset=2] = 83
    assert prodex_anthropic_stream_data_line_v1(
        2, UInt(Int(event.unsafe_ptr())), Int64(event.byte_length()), output,
    ) == 4
    assert prodex_anthropic_stream_data_line_v1(
        1, 0, Int64(event.byte_length()), output,
    ) == 1
    assert prodex_anthropic_stream_data_line_v1(
        1, UInt(Int(event.unsafe_ptr())), -1, output,
    ) == 1
    assert result[unsafe_offset=0] == 81
    assert result[unsafe_offset=1] == 82
    assert result[unsafe_offset=2] == 83
    dealloc(result_storage^)
