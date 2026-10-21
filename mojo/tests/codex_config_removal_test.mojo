from std.memory.alloc import alloc, dealloc, Layout
from codex_config import CodexConfigArgView, prodex_codex_config_removal_mask_v1


def main() raises:
    var arguments_store = alloc(Layout[CodexConfigArgView](count=4))
    var arguments = arguments_store.unsafe_ptr()
    var output_store = alloc(Layout[UInt8](count=4))
    var output = output_store.unsafe_ptr()
    var flag = StringSlice("-c")
    var value = StringSlice(" model =first")
    var inline = StringSlice("--config=model=second")
    var key = StringSlice("model")
    arguments[unsafe_offset=0] = CodexConfigArgView(UInt64(Int(flag.unsafe_ptr())), UInt64(flag.byte_length()), 1)
    arguments[unsafe_offset=1] = CodexConfigArgView(UInt64(Int(value.unsafe_ptr())), UInt64(value.byte_length()), 1)
    arguments[unsafe_offset=2] = CodexConfigArgView(UInt64(Int(inline.unsafe_ptr())), UInt64(inline.byte_length()), 1)
    arguments[unsafe_offset=3] = CodexConfigArgView(UInt64(Int(flag.unsafe_ptr())), UInt64(flag.byte_length()), 1)
    var source = UInt(Int(arguments))
    var target = UInt(Int(output))
    var key_address = UInt(Int(key.unsafe_ptr()))
    assert prodex_codex_config_removal_mask_v1(1, 0, source, 4, key_address, 5, target, 4) == 0
    assert output[unsafe_offset=0] == 1 and output[unsafe_offset=1] == 1
    assert output[unsafe_offset=2] == 0 and output[unsafe_offset=3] == 0
    assert prodex_codex_config_removal_mask_v1(1, 1, source, 4, key_address, 5, target, 4) == 0
    assert output[unsafe_offset=0] == 1 and output[unsafe_offset=1] == 1
    assert output[unsafe_offset=2] == 1 and output[unsafe_offset=3] == 0
    for index in range(4):
        output[unsafe_offset=index] = 77
    assert prodex_codex_config_removal_mask_v1(2, 0, source, 4, key_address, 5, target, 4) == 4
    assert prodex_codex_config_removal_mask_v1(1, 2, source, 4, key_address, 5, target, 4) == 1
    assert prodex_codex_config_removal_mask_v1(1, 0, source, 4, key_address, 5, target, 3) == 1
    for index in range(4):
        assert output[unsafe_offset=index] == 77
    dealloc(output_store^)
    dealloc(arguments_store^)
