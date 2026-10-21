# Direct offline UTF-8 marked instruction block stripping through linked Mojo.
from std.memory.alloc import alloc, dealloc, Layout
from optional_tools_policy import prodex_optional_tool_marked_block_strip_v1


def main() raises:
    var input_text = StringSlice("keep 🌍[BEGIN]first[END]中[BEGIN]second[END]tail")
    var begin = StringSlice("[BEGIN]")
    var end = StringSlice("[END]")
    var dst_store = alloc(Layout[UInt8](count=128))
    var length_store = alloc(Layout[Int64](count=1))
    var dst = dst_store.unsafe_ptr()
    var length = length_store.unsafe_ptr()
    var out = UInt(Int(dst))
    var written = UInt(Int(length))
    var b = UInt(Int(begin.unsafe_ptr()))
    var e = UInt(Int(end.unsafe_ptr()))
    assert prodex_optional_tool_marked_block_strip_v1(
        1, UInt(Int(input_text.unsafe_ptr())), Int64(input_text.byte_length()),
        b, Int64(begin.byte_length()), e, Int64(end.byte_length()), out, 128, written
    ) == 0
    var expected = StringSlice("keep 🌍中tail")
    assert length[unsafe_offset=0] == Int64(expected.byte_length())
    for i in range(Int64(expected.byte_length())):
        assert dst[unsafe_offset=i] == expected.unsafe_ptr()[unsafe_offset=i]

    var incomplete = StringSlice("start[BEGIN]oops")
    assert prodex_optional_tool_marked_block_strip_v1(
        1, UInt(Int(incomplete.unsafe_ptr())), Int64(incomplete.byte_length()),
        b, Int64(begin.byte_length()), e, Int64(end.byte_length()), out, 128, written
    ) == 2
    var orphan = StringSlice("start[END]oops")
    assert prodex_optional_tool_marked_block_strip_v1(
        1, UInt(Int(orphan.unsafe_ptr())), Int64(orphan.byte_length()),
        b, Int64(begin.byte_length()), e, Int64(end.byte_length()), out, 128, written
    ) == 3

    length[unsafe_offset=0] = 77
    assert prodex_optional_tool_marked_block_strip_v1(
        2, UInt(Int(input_text.unsafe_ptr())), Int64(input_text.byte_length()),
        b, Int64(begin.byte_length()), e, Int64(end.byte_length()), out, 128, written
    ) == 4
    assert prodex_optional_tool_marked_block_strip_v1(
        1, UInt(Int(input_text.unsafe_ptr())), Int64(input_text.byte_length()),
        b, Int64(begin.byte_length()), e, Int64(end.byte_length()), out, 2, written
    ) == 1
    assert length[unsafe_offset=0] == 77
    dealloc(length_store^)
    dealloc(dst_store^)
