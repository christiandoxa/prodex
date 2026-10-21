# Direct offline canonical AGENTS reference idempotence and UTF-8 regression.
from std.memory.alloc import alloc, dealloc, Layout
from optional_tools_policy import prodex_optional_tool_agents_reference_update_v1


def main() raises:
    var source = StringSlice("intro\r\n  @REF  \nother\r\n@REF\n")
    var reference = StringSlice("@REF")
    var expected = StringSlice("intro\nother\n\n@REF\n")
    var output_store = alloc(Layout[UInt8](count=128))
    var result_store = alloc(Layout[Int64](count=2))
    var output = output_store.unsafe_ptr()
    var result = result_store.unsafe_ptr()
    var address = UInt(Int(output))
    var result_address = UInt(Int(result))
    var reference_address = UInt(Int(reference.unsafe_ptr()))
    assert prodex_optional_tool_agents_reference_update_v1(
        1, UInt(Int(source.unsafe_ptr())), Int64(source.byte_length()),
        reference_address, Int64(reference.byte_length()),
        address, 128, result_address
    ) == 0
    assert result[unsafe_offset=0] == Int64(expected.byte_length())
    assert result[unsafe_offset=1] == 1
    for index in range(Int64(expected.byte_length())):
        assert output[unsafe_offset=index] == expected.unsafe_ptr()[unsafe_offset=index]

    assert prodex_optional_tool_agents_reference_update_v1(
        1, UInt(Int(expected.unsafe_ptr())), Int64(expected.byte_length()),
        reference_address, Int64(reference.byte_length()),
        address, 128, result_address
    ) == 0
    assert result[unsafe_offset=1] == 0

    # Bad ABI/capacity must not overwrite caller's result.
    result[unsafe_offset=0] = 77
    assert prodex_optional_tool_agents_reference_update_v1(
        2, UInt(Int(source.unsafe_ptr())), Int64(source.byte_length()),
        reference_address, Int64(reference.byte_length()),
        address, 128, result_address
    ) == 4
    assert prodex_optional_tool_agents_reference_update_v1(
        1, UInt(Int(source.unsafe_ptr())), Int64(source.byte_length()),
        reference_address, Int64(reference.byte_length()),
        address, 1, result_address
    ) == 1
    assert result[unsafe_offset=0] == 77
    dealloc(result_store^)
    dealloc(output_store^)
