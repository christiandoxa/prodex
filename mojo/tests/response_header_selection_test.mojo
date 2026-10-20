# Direct compiled response header batch semantics and invalid shape tests.
from std.memory.alloc import alloc, dealloc, Layout
from response_forwarding import prodex_runtime_response_header_select_v1


def main() raises:
    var rows_storage = alloc(Layout[UInt64](count=12))
    var output_storage = alloc(Layout[Int64](count=3))
    var rows = rows_storage.unsafe_ptr()
    var result = output_storage.unsafe_ptr()
    var empty_header = StringSlice("X-Value")
    var empty_value = StringSlice("   ")
    var target = StringSlice("x-value")
    var content_header = StringSlice("CONTENT-TYPE")
    var content_value = StringSlice("TEXT/EVENT-STREAM")
    for index in range(12):
        rows[unsafe_offset=index] = 0
    rows[unsafe_offset=0] = UInt64(UInt(Int(empty_header.unsafe_ptr())))
    rows[unsafe_offset=1] = UInt64(empty_header.byte_length())
    rows[unsafe_offset=2] = UInt64(UInt(Int(empty_value.unsafe_ptr())))
    rows[unsafe_offset=3] = UInt64(empty_value.byte_length())
    rows[unsafe_offset=4] = UInt64(UInt(Int(content_header.unsafe_ptr())))
    rows[unsafe_offset=5] = UInt64(content_header.byte_length())
    rows[unsafe_offset=6] = UInt64(UInt(Int(content_value.unsafe_ptr())))
    rows[unsafe_offset=7] = UInt64(content_value.byte_length())
    rows[unsafe_offset=8] = rows[unsafe_offset=0]
    rows[unsafe_offset=9] = rows[unsafe_offset=1]
    rows[unsafe_offset=10] = rows[unsafe_offset=6]
    rows[unsafe_offset=11] = rows[unsafe_offset=7]
    var source = UInt(Int(rows))
    var output = UInt(Int(result))
    assert prodex_runtime_response_header_select_v1(
        1, 0, source, 3,
        UInt(Int(target.unsafe_ptr())), Int64(target.byte_length()), output,
    ) == 0
    assert result[unsafe_offset=0] == -1
    assert result[unsafe_offset=1] == 0 and result[unsafe_offset=2] == 0
    assert prodex_runtime_response_header_select_v1(
        1, 1, source, 3, 0, 0, output,
    ) == 0
    assert result[unsafe_offset=0] == 1
    assert result[unsafe_offset=1] == 0
    assert result[unsafe_offset=2] == Int64(content_value.byte_length())
    assert prodex_runtime_response_header_select_v1(
        1, 2, source, 3, 0, 0, output,
    ) == 0
    assert result[unsafe_offset=0] == 1
    result[unsafe_offset=0] = 81
    result[unsafe_offset=1] = 82
    result[unsafe_offset=2] = 83
    assert prodex_runtime_response_header_select_v1(
        2, 0, source, 3,
        UInt(Int(target.unsafe_ptr())), Int64(target.byte_length()), output,
    ) == 4
    assert result[unsafe_offset=0] == 81
    rows[unsafe_offset=11] = UInt64(9223372036854775808)
    assert prodex_runtime_response_header_select_v1(
        1, 2, source, 3, 0, 0, output,
    ) == 1
    assert result[unsafe_offset=0] == 81
    assert result[unsafe_offset=1] == 82
    assert result[unsafe_offset=2] == 83
    dealloc(output_storage^)
    dealloc(rows_storage^)
