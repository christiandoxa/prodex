# Direct borrow-only patch field precedence and ABI validation.
from std.memory.alloc import alloc, dealloc, Layout
from gemini_tooling_policy import prodex_gemini_apply_patch_sources_v1


def main() raises:
    var fields_storage = alloc(Layout[UInt64](count=51))
    var result_storage = alloc(Layout[Int64](count=7))
    var fields = fields_storage.unsafe_ptr()
    var result = result_storage.unsafe_ptr()
    for index in range(51):
        fields[unsafe_offset=index] = 0
    var prefix = StringSlice("explicit")
    var path = StringSlice(" 雪.rs ")
    var content = StringSlice("fallback")
    fields[unsafe_offset=3] = 1
    fields[unsafe_offset=4] = UInt64(UInt(Int(prefix.unsafe_ptr())))
    fields[unsafe_offset=5] = UInt64(prefix.byte_length())
    fields[unsafe_offset=15] = 1
    fields[unsafe_offset=16] = UInt64(UInt(Int(path.unsafe_ptr())))
    fields[unsafe_offset=17] = UInt64(path.byte_length())
    fields[unsafe_offset=48] = 1
    fields[unsafe_offset=49] = UInt64(UInt(Int(content.unsafe_ptr())))
    fields[unsafe_offset=50] = UInt64(content.byte_length())
    var source = UInt(Int(fields))
    var dest = UInt(Int(result))
    assert prodex_gemini_apply_patch_sources_v1(1, source, 17, dest) == 0
    assert result[unsafe_offset=0] == 1
    assert result[unsafe_offset=1] == 5
    assert result[unsafe_offset=2] == 1
    assert result[unsafe_offset=3] == Int64(path.byte_length() - 1)
    assert result[unsafe_offset=4] == -1
    assert result[unsafe_offset=5] == 16
    assert result[unsafe_offset=6] == 16

    for index in range(7):
        result[unsafe_offset=index] = 91
    assert prodex_gemini_apply_patch_sources_v1(2, source, 17, dest) == 4
    assert prodex_gemini_apply_patch_sources_v1(1, source, 16, dest) == 1
    assert result[unsafe_offset=0] == 91
    fields[unsafe_offset=50] = UInt64(9223372036854775808)
    assert prodex_gemini_apply_patch_sources_v1(1, source, 17, dest) == 1
    assert result[unsafe_offset=0] == 91
    assert result[unsafe_offset=6] == 91
    dealloc(result_storage^)
    dealloc(fields_storage^)
