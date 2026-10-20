# Direct previous-response text classification and Unicode span ABI checks.
from std.memory.alloc import alloc, dealloc, Layout
from rich_fallback import prodex_mojo_previous_response_text_message_span_v1


def main() raises:
    var result_storage = alloc(Layout[Int64](count=3))
    var output = result_storage.unsafe_ptr()
    var dest = UInt(Int(output))
    var content = StringSlice(" 　previous_response_not_found \t")
    var prefix = StringSlice(" 　")
    var selected = StringSlice("previous_response_not_found")
    assert prodex_mojo_previous_response_text_message_span_v1(
        6, UInt(Int(content.unsafe_ptr())), Int64(content.byte_length()), dest,
    ) == 0
    assert output[unsafe_offset=0] == 1
    assert output[unsafe_offset=1] == Int64(prefix.byte_length())
    assert output[unsafe_offset=2] == Int64(prefix.byte_length() + selected.byte_length())

    var ordinary = StringSlice("ordinary output")
    assert prodex_mojo_previous_response_text_message_span_v1(
        6, UInt(Int(ordinary.unsafe_ptr())), Int64(ordinary.byte_length()), dest,
    ) == 0
    assert output[unsafe_offset=0] == 0
    assert output[unsafe_offset=1] == 0
    assert output[unsafe_offset=2] == 0

    output[unsafe_offset=0] = 73
    output[unsafe_offset=1] = 74
    output[unsafe_offset=2] = 75
    assert prodex_mojo_previous_response_text_message_span_v1(
        5, UInt(Int(content.unsafe_ptr())), Int64(content.byte_length()), dest,
    ) == 4
    assert prodex_mojo_previous_response_text_message_span_v1(
        6, 0, Int64(content.byte_length()), dest,
    ) == 1
    assert output[unsafe_offset=0] == 73
    assert output[unsafe_offset=1] == 74
    assert output[unsafe_offset=2] == 75
    dealloc(result_storage^)
