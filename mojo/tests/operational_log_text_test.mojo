from std.memory.alloc import alloc, dealloc, Layout
from operational_log_stream import prodex_mojo_operational_log_text_span_v1
from super_expose_exec import prodex_mojo_super_expose_exec_validation_v1


def main() raises:
    var storage = alloc(Layout[Int64](count=2))
    var output = storage.unsafe_ptr()
    var target = UInt(Int(output))
    var text = StringSlice("é🦀é")
    var address = UInt(Int(text.unsafe_ptr()))
    assert prodex_mojo_operational_log_text_span_v1(1, address, 9, 3, target) == 0
    assert output[unsafe_offset=0] == 7 and output[unsafe_offset=1] == 1
    assert prodex_mojo_operational_log_text_span_v1(1, address, 9, 0, target) == 0
    assert output[unsafe_offset=0] == 0 and output[unsafe_offset=1] == 1
    assert prodex_mojo_operational_log_text_span_v1(1, address, 9, UInt64(0xFFFFFFFFFFFFFFFF), target) == 0
    assert output[unsafe_offset=0] == 9 and output[unsafe_offset=1] == 0
    assert prodex_mojo_operational_log_text_span_v1(1, 0, 0, 0, target) == 0
    assert output[unsafe_offset=0] == 0 and output[unsafe_offset=1] == 0
    output[unsafe_offset=0] = 73
    output[unsafe_offset=1] = 83
    assert prodex_mojo_operational_log_text_span_v1(2, address, 9, 3, target) == 4
    assert prodex_mojo_operational_log_text_span_v1(1, 0, 1, 3, target) == 1
    assert prodex_mojo_operational_log_text_span_v1(1, address, -1, 3, target) == 1
    assert prodex_mojo_operational_log_text_span_v1(1, address, 9, 3, 0) == 1
    assert output[unsafe_offset=0] == 73 and output[unsafe_offset=1] == 83
    text = StringSlice("a\u0085b")
    assert prodex_mojo_super_expose_exec_validation_v1(1, 9, UInt(Int(text.unsafe_ptr())), Int64(text.byte_length()),
        0, 0, UInt64(0xFFFFFFFFFFFFFFFF), target) == 0
    assert output[unsafe_offset=0] == 2 and output[unsafe_offset=1] == 0
    text = StringSlice(" \u00a0é\u200b")
    assert prodex_mojo_super_expose_exec_validation_v1(1, 9, UInt(Int(text.unsafe_ptr())), Int64(text.byte_length()),
        0, 0, UInt64(0xFFFFFFFFFFFFFFFF), target) == 0
    assert output[unsafe_offset=0] == 1 and output[unsafe_offset=1] == 0
    dealloc(storage^)
