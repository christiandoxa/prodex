# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/gemini_normalized_error_test.mojo`.
from std.memory.alloc import alloc, dealloc, Layout
from gemini_quota_body import prodex_gemini_normalized_error_v1


def main() raises:
    var storage = alloc(Layout[Int64](count=2))
    var metadata = storage.unsafe_ptr()
    var address = UInt64(Int(metadata))
    assert prodex_gemini_normalized_error_v1(1, 200, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, address) == 0
    assert metadata[unsafe_offset=0] == 0 and metadata[unsafe_offset=1] == 0
    assert prodex_gemini_normalized_error_v1(1, 429, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, address) == 0
    assert metadata[unsafe_offset=0] == 2 and metadata[unsafe_offset=1] == 0
    assert prodex_gemini_normalized_error_v1(2, 429, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, address) == 4
    assert prodex_gemini_normalized_error_v1(1, 65536, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, address) == 1
    dealloc(storage^)
