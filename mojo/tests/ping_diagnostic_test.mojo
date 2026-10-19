# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/ping_diagnostic_test.mojo`.
from std.memory.alloc import alloc, dealloc, Layout
from ping_protocol import prodex_mojo_ping_diagnostic_v1


def main() raises:
    var output_storage = alloc(Layout[UInt8](count=128))
    var meta_storage = alloc(Layout[Int64](count=2))
    var output = output_storage.unsafe_ptr()
    var meta = meta_storage.unsafe_ptr()
    var raw = String("clean the diagnostic directory")
    var redacted = String("\u2003safe\r\ntext\u3000")
    assert prodex_mojo_ping_diagnostic_v1(1, 1, UInt(Int(raw.unsafe_ptr())), Int64(raw.byte_length()), UInt(Int(redacted.unsafe_ptr())), Int64(redacted.byte_length()), 0, 0, UInt(Int(output)), 128, UInt(Int(meta))) == 0
    var expected = StringSlice("OpenAI application ping cleanup failed: safe  text")
    assert meta[unsafe_offset=0] == 12 and meta[unsafe_offset=1] == Int64(expected.byte_length())
    var source = expected.unsafe_ptr()
    for index in range(Int64(expected.byte_length())):
        assert output[unsafe_offset=index] == source[unsafe_offset=index]
    output[unsafe_offset=0] = 77
    meta[unsafe_offset=0] = 71
    assert prodex_mojo_ping_diagnostic_v1(1, 1, UInt(Int(raw.unsafe_ptr())), Int64(raw.byte_length()), UInt(Int(redacted.unsafe_ptr())), Int64(redacted.byte_length()), 0, 0, UInt(Int(output)), 1, UInt(Int(meta))) == 3
    assert output[unsafe_offset=0] == 77 and meta[unsafe_offset=0] == 71
    dealloc(output_storage^)
    dealloc(meta_storage^)
