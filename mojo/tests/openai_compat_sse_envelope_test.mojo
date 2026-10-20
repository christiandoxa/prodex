# Direct strict chat-completions SSE envelope and ABI validation.
from std.memory.alloc import alloc, dealloc, Layout
from rich_abi import prodex_openai_compat_sse_envelope_v1


def main() raises:
    var output_storage = alloc(Layout[Int64](count=3))
    var output = output_storage.unsafe_ptr()
    var dest = UInt(Int(output))
    var done = StringSlice("data: [DONE]\n\n")
    assert prodex_openai_compat_sse_envelope_v1(
        6, UInt(Int(done.unsafe_ptr())), Int64(done.byte_length()), dest
    ) == 0
    assert output[unsafe_offset=0] == 1
    assert output[unsafe_offset=1] == 6
    assert output[unsafe_offset=2] == 12
    var json = StringSlice("data: 雪\n\n")
    assert prodex_openai_compat_sse_envelope_v1(
        6, UInt(Int(json.unsafe_ptr())), Int64(json.byte_length()), dest
    ) == 0
    assert output[unsafe_offset=0] == 2
    assert output[unsafe_offset=1] == 6
    assert output[unsafe_offset=2] == 9
    var wrong = StringSlice("data: [DONE]\r\n\r\n")
    assert prodex_openai_compat_sse_envelope_v1(
        6, UInt(Int(wrong.unsafe_ptr())), Int64(wrong.byte_length()), dest
    ) == 0
    assert output[unsafe_offset=0] == 0
    assert output[unsafe_offset=1] == 0
    assert output[unsafe_offset=2] == 0
    output[unsafe_offset=0] = 77
    output[unsafe_offset=1] = 77
    output[unsafe_offset=2] = 77
    assert prodex_openai_compat_sse_envelope_v1(
        5, UInt(Int(done.unsafe_ptr())), Int64(done.byte_length()), dest
    ) == 4
    assert prodex_openai_compat_sse_envelope_v1(
        6, 0, Int64(done.byte_length()), dest
    ) == 1
    assert output[unsafe_offset=0] == 77
    assert output[unsafe_offset=1] == 77
    assert output[unsafe_offset=2] == 77
    dealloc(output_storage^)
