# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/gemini_retry_text_test.mojo`.
from std.memory.alloc import alloc, dealloc, Layout
from gemini_quota_body import prodex_gemini_retry_text_plan_v1, prodex_gemini_retry_text_finish_v1


def main() raises:
    var storage = alloc(Layout[UInt64](count=9))
    var output = storage.unsafe_ptr()
    var address = UInt(Int(output))
    var text = String("suggested retry after 1s; please retry in .s")
    assert prodex_gemini_retry_text_plan_v1(1, 1, UInt(Int(text.unsafe_ptr())), Int64(text.byte_length()), address, 9) == 0
    assert output[unsafe_offset=0] == 2 and output[unsafe_offset=2] == 2
    assert output[unsafe_offset=5] == 1000 and output[unsafe_offset=8] == 1000
    assert prodex_gemini_retry_text_finish_v1(1, 2, 2, 0.0, 1.0, 1000, 1000, address, 2) == 0
    assert output[unsafe_offset=0] == 1 and output[unsafe_offset=1] == 1000
    assert prodex_gemini_retry_text_finish_v1(1, 1, 1, 1.0e308, 0.0, 1000, 0, address, 2) == 0
    assert output[unsafe_offset=1] == 18446744073709551615
    dealloc(storage^)
