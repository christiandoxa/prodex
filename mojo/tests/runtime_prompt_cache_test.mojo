# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_prompt_cache_test.mojo`.
from std.memory.alloc import alloc, dealloc, Layout
from runtime_prompt_cache import prodex_prompt_cache_key_v1, prodex_prompt_cache_update_v1


def main() raises:
    var storage = alloc(Layout[UInt64](count=3))
    var output = storage.unsafe_ptr()
    var destination = UInt(Int(output))
    var key = String("\u2003cache\u3000")
    assert prodex_prompt_cache_key_v1(1, 0, 0, UInt(Int(key.unsafe_ptr())), Int64(key.byte_length()), destination, 3) == 0
    assert output[unsafe_offset=0] == 2 and output[unsafe_offset=1] == 3 and output[unsafe_offset=2] == 8
    assert prodex_prompt_cache_key_v1(1, 1, 0, 1, -1, destination, 3) == 0
    assert output[unsafe_offset=0] == 1
    assert prodex_prompt_cache_update_v1(1, 1, 1, 0, 10, 18446744073709551614, 18446744073709551615, -1, destination, 3) == 0
    assert output[unsafe_offset=0] == 1 and output[unsafe_offset=1] == 18446744073709551615 and output[unsafe_offset=2] == 18446744073709551615
    dealloc(storage^)
