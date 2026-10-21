# Direct version-token lexical extraction ABI tests, no external process.
from std.memory.alloc import alloc, dealloc, Layout
from optional_tools_policy import prodex_optional_tool_probe_token_v1


def main() raises:
    var raw = StringSlice("rtk (vv0.50.0) build")
    var output_store = alloc(Layout[Int64](count=3))
    var output = output_store.unsafe_ptr()
    var address = UInt(Int(raw.unsafe_ptr()))
    var result_address = UInt(Int(output))
    assert prodex_optional_tool_probe_token_v1(
        1, address, Int64(raw.byte_length()), 0, result_address
    ) == 0
    assert output[unsafe_offset=0] == 0
    assert output[unsafe_offset=1] == 3
    var after_tool = output[unsafe_offset=2]
    assert prodex_optional_tool_probe_token_v1(
        1, address, Int64(raw.byte_length()), after_tool, result_address
    ) == 0
    var second_start = output[unsafe_offset=0]
    var second_end = output[unsafe_offset=1]
    assert second_end > second_start
    var text = raw.unsafe_ptr()
    assert text[unsafe_offset=second_start] == 48
    assert text[unsafe_offset=second_start + 1] == 46
    var after_version = output[unsafe_offset=2]
    assert prodex_optional_tool_probe_token_v1(
        1, address, Int64(raw.byte_length()), after_version, result_address
    ) == 0
    assert output[unsafe_offset=1] > output[unsafe_offset=0]
    var after_last = output[unsafe_offset=2]
    assert prodex_optional_tool_probe_token_v1(
        1, address, Int64(raw.byte_length()), after_last, result_address
    ) == 0
    assert output[unsafe_offset=0] == -1
    assert output[unsafe_offset=1] == -1

    # ABI errors must not write the caller's result.
    output[unsafe_offset=0] = 77
    assert prodex_optional_tool_probe_token_v1(
        2, address, Int64(raw.byte_length()), 0, result_address
    ) == 4
    assert prodex_optional_tool_probe_token_v1(
        1, address, Int64(raw.byte_length()), -1, result_address
    ) == 1
    assert output[unsafe_offset=0] == 77
    dealloc(output_store^)
