# Direct AGENTS block upsert/remove render ABI, no filesystem dependency.
from std.memory.alloc import alloc, dealloc, Layout
from optional_tools_policy import prodex_optional_tool_agents_block_render_v1


def main() raises:
    var original = StringSlice("intro\n")
    var cleaned = StringSlice("intro  \n")
    var begin = StringSlice("[BEGIN]")
    var end = StringSlice("[END]")
    var block = StringSlice(" 🌍 hello ")
    var expected = StringSlice("intro\n\n[BEGIN]\n🌍 hello\n[END]\n")
    var args = [original, cleaned, begin, end, block]
    var views_store = alloc(Layout[UInt64](count=10))
    var output_store = alloc(Layout[UInt8](count=128))
    var result_store = alloc(Layout[Int64](count=2))
    var views = views_store.unsafe_ptr()
    var output = output_store.unsafe_ptr()
    var result = result_store.unsafe_ptr()
    for i in range(5):
        views[unsafe_offset=i * 2] = UInt64(UInt(Int(args[i].unsafe_ptr())))
        views[unsafe_offset=i * 2 + 1] = UInt64(args[i].byte_length())
    var records = UInt(Int(views))
    var output_address = UInt(Int(output))
    var result_address = UInt(Int(result))
    assert prodex_optional_tool_agents_block_render_v1(
        1, 0, records, 5, output_address, 128, result_address
    ) == 0
    assert result[unsafe_offset=0] == Int64(expected.byte_length())
    assert result[unsafe_offset=1] == 1
    for i in range(Int64(expected.byte_length())):
        assert output[unsafe_offset=i] == expected.unsafe_ptr()[unsafe_offset=i]

    var no_change = StringSlice("intro\n")
    views[unsafe_offset=0] = UInt64(UInt(Int(no_change.unsafe_ptr())))
    views[unsafe_offset=1] = UInt64(no_change.byte_length())
    views[unsafe_offset=2] = UInt64(UInt(Int(no_change.unsafe_ptr())))
    views[unsafe_offset=3] = UInt64(no_change.byte_length())
    assert prodex_optional_tool_agents_block_render_v1(
        1, 1, records, 5, output_address, 128, result_address
    ) == 0
    assert result[unsafe_offset=0] == 0
    assert result[unsafe_offset=1] == 0

    result[unsafe_offset=0] = 77
    assert prodex_optional_tool_agents_block_render_v1(
        2, 0, records, 5, output_address, 128, result_address
    ) == 4
    assert prodex_optional_tool_agents_block_render_v1(
        1, 5, records, 5, output_address, 128, result_address
    ) == 1
    assert prodex_optional_tool_agents_block_render_v1(
        1, 0, records, 5, output_address, 1, result_address
    ) == 3
    assert result[unsafe_offset=0] == 77
    dealloc(result_store^)
    dealloc(output_store^)
    dealloc(views_store^)
