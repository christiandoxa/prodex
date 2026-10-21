from std.memory.alloc import alloc, dealloc, Layout
from mcp_stdio_policy import prodex_mcp_line_chunk_v1


def main() raises:
    var store = alloc(Layout[UInt64](count=5))
    var output = store.unsafe_ptr()
    var address = UInt(Int(output))
    var text = StringSlice(" \t{\nignored")
    var source = UInt(Int(text.unsafe_ptr()))
    var length = Int64(text.byte_length())
    assert prodex_mcp_line_chunk_v1(1, 0, source, length, 0, -1, 0, address) == 0
    assert output[unsafe_offset=0] == 4 and output[unsafe_offset=1] == 123
    assert output[unsafe_offset=2] == 67_108_864
    assert output[unsafe_offset=3] == 0 and output[unsafe_offset=4] == 1
    assert prodex_mcp_line_chunk_v1(1, 1, source, length, 1, -1, 4, address) == 0
    assert output[unsafe_offset=2] == 4 and output[unsafe_offset=3] == 1
    assert prodex_mcp_line_chunk_v1(
        1, 1, source, length, 18_446_744_073_709_551_615, -1, 4, address
    ) == 0
    assert output[unsafe_offset=3] == 1
    var vertical_tab = StringSlice("\v{\n")
    assert prodex_mcp_line_chunk_v1(
        1, 0, UInt(Int(vertical_tab.unsafe_ptr())), Int64(vertical_tab.byte_length()), 0, -1, 0, address
    ) == 0
    assert output[unsafe_offset=1] == 11 and output[unsafe_offset=2] == 4_096
    for index in range(5):
        output[unsafe_offset=index] = 77
    assert prodex_mcp_line_chunk_v1(2, 0, source, length, 0, -1, 0, address) == 4
    assert prodex_mcp_line_chunk_v1(1, 2, source, length, 0, -1, 0, address) == 1
    assert prodex_mcp_line_chunk_v1(1, 0, 0, 1, 0, -1, 0, address) == 1
    assert prodex_mcp_line_chunk_v1(1, 0, source, length, 0, 256, 0, address) == 1
    assert prodex_mcp_line_chunk_v1(1, 0, source, length, 0, -1, 0, address + 1) == 1
    for index in range(5):
        assert output[unsafe_offset=index] == 77
    assert prodex_mcp_line_chunk_v1(1, 1, 0, 0, 0, -1, 0, address) == 0
    assert output[unsafe_offset=0] == 0 and output[unsafe_offset=1] == 256
    dealloc(store^)
