# Direct ABI coverage for lexical Copilot instruction paths and UTF-8 cut.
# Run with mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/launch_copilot_instruction_policy_test.mojo.
from std.memory.alloc import alloc, dealloc, Layout
from launch_config import (
    prodex_launch_instruction_path_order_v1,
    prodex_launch_instruction_text_cut_v1,
)


def main() raises:
    var words_storage = alloc(Layout[UInt64](count=8))
    var indices_storage = alloc(Layout[Int64](count=4))
    var cut_storage = alloc(Layout[Int64](count=1))
    var words = words_storage.unsafe_ptr()
    var indices = indices_storage.unsafe_ptr()
    var cut = cut_storage.unsafe_ptr()
    var a = StringSlice("a")
    var z = StringSlice("z")
    var snow = StringSlice("雪")
    var accent = StringSlice("é")
    var names = [snow, z, a, accent]
    for i in range(4):
        words[unsafe_offset=i * 2] = UInt64(UInt(Int(names[i].unsafe_ptr())))
        words[unsafe_offset=i * 2 + 1] = UInt64(names[i].byte_length())
    var input = UInt(Int(words))
    var result = UInt(Int(indices))
    var cut_address = UInt(Int(cut))
    assert prodex_launch_instruction_path_order_v1(
        1, input, 4, result, 4,
    ) == 0
    assert indices[unsafe_offset=0] == 2
    assert indices[unsafe_offset=1] == 1
    assert indices[unsafe_offset=2] == 3
    assert indices[unsafe_offset=3] == 0

    var text = StringSlice("abé🦀")
    var text_address = UInt(Int(text.unsafe_ptr()))
    assert prodex_launch_instruction_text_cut_v1(
        1, text_address, Int64(text.byte_length()), 3, cut_address,
    ) == 0
    assert cut[unsafe_offset=0] == 2
    assert prodex_launch_instruction_text_cut_v1(
        1, text_address, Int64(text.byte_length()), 5, cut_address,
    ) == 0
    assert cut[unsafe_offset=0] == 4
    assert prodex_launch_instruction_text_cut_v1(
        1, text_address, Int64(text.byte_length()), 8, cut_address,
    ) == 0
    assert cut[unsafe_offset=0] == 8

    for i in range(4):
        indices[unsafe_offset=i] = 71
    cut[unsafe_offset=0] = 83
    assert prodex_launch_instruction_path_order_v1(
        2, input, 4, result, 4,
    ) == 4
    assert prodex_launch_instruction_path_order_v1(
        1, input, 4, result, 3,
    ) == 1
    words[unsafe_offset=7] = UInt64(9223372036854775808)
    assert prodex_launch_instruction_path_order_v1(
        1, input, 4, result, 4,
    ) == 1
    assert prodex_launch_instruction_text_cut_v1(
        2, text_address, Int64(text.byte_length()), 3, cut_address,
    ) == 4
    assert prodex_launch_instruction_text_cut_v1(
        1, 0, Int64(text.byte_length()), 3, cut_address,
    ) == 1
    for i in range(4):
        assert indices[unsafe_offset=i] == 71
    assert cut[unsafe_offset=0] == 83

    assert prodex_launch_instruction_path_order_v1(
        1, 0, 0, result, 4,
    ) == 0
    dealloc(cut_storage^)
    dealloc(indices_storage^)
    dealloc(words_storage^)
