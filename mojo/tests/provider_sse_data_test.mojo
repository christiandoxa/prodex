# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/provider_sse_data_test.mojo`.
from std.memory.alloc import alloc, dealloc, Layout
from provider_sse_data import prodex_sse_buffered_data_spans_v1


def main() raises:
    var text = String("event: error\n\u2003data:\u3000{\"code\":1} \r\n data: [DONE]\ndata:\nDATA: ignored\n")
    var rows_storage = alloc(Layout[UInt64](count=6))
    var meta_storage = alloc(Layout[UInt64](count=1))
    var rows = rows_storage.unsafe_ptr()
    var meta = meta_storage.unsafe_ptr()
    var address = UInt(Int(text.unsafe_ptr()))
    var length = Int64(text.byte_length())
    assert prodex_sse_buffered_data_spans_v1(1, address, length, 0, 0, UInt(Int(meta))) == 3
    assert meta[] == 3
    assert prodex_sse_buffered_data_spans_v1(1, address, length, UInt(Int(rows)), 3, UInt(Int(meta))) == 0
    assert rows[unsafe_offset=0] == 24 and rows[unsafe_offset=1] == 34
    assert rows[unsafe_offset=2] == 44 and rows[unsafe_offset=3] == 50
    assert rows[unsafe_offset=4] == 56 and rows[unsafe_offset=5] == 56
    dealloc(rows_storage^)
    dealloc(meta_storage^)
