# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/gemini_data_url_test.mojo`.
from std.memory.alloc import alloc, dealloc, Layout
from gemini_response_media import prodex_mojo_gemini_data_url_parts_v1


def main() raises:
    var storage = alloc(Layout[Int64](count=6))
    var fields = storage.unsafe_ptr()
    var text = StringSlice("data:画像/png;charset=utf-8;BaSe64,a,b")
    var mime = StringSlice("画像/png")
    var source = UInt(Int(text.unsafe_ptr()))
    var dest = UInt(Int(fields))
    assert prodex_mojo_gemini_data_url_parts_v1(1, source, Int64(text.byte_length()), dest) == 0
    assert fields[unsafe_offset=0] == 1 and fields[unsafe_offset=1] == 0
    assert fields[unsafe_offset=2] == 5 and fields[unsafe_offset=3] == 5 + Int64(mime.byte_length())
    assert fields[unsafe_offset=4] == Int64(text.byte_length()) - 3
    assert fields[unsafe_offset=5] == Int64(text.byte_length())
    fields[unsafe_offset=0] = 71
    assert prodex_mojo_gemini_data_url_parts_v1(2, source, Int64(text.byte_length()), dest) == 4
    assert fields[unsafe_offset=0] == 71
    assert prodex_mojo_gemini_data_url_parts_v1(1, source, -1, dest) == 1
    assert fields[unsafe_offset=0] == 71
    dealloc(storage^)
