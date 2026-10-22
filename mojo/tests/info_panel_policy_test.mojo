# Compiled info-panel value color and optional 5h quota-field visibility.
from std.memory.alloc import alloc, dealloc, Layout
from info_render import prodex_terminal_info_render_v1


def main() raises:
    var label = StringSlice("Codex quota data")
    var healthy = StringSlice("unavailable=0")
    var blocked = StringSlice("unavailable=1")
    var views_store = alloc(Layout[UInt64](count=4))
    var output_store = alloc(Layout[UInt8](count=64))
    var written_store = alloc(Layout[Int64](count=1))
    var count_store = alloc(Layout[UInt64](count=1))
    var views = views_store.unsafe_ptr()
    var output = output_store.unsafe_ptr()
    var written = written_store.unsafe_ptr()
    var count = count_store.unsafe_ptr()
    views[unsafe_offset=0] = UInt64(UInt(Int(label.unsafe_ptr())))
    views[unsafe_offset=1] = UInt64(label.byte_length())
    views[unsafe_offset=2] = UInt64(UInt(Int(healthy.unsafe_ptr())))
    views[unsafe_offset=3] = UInt64(healthy.byte_length())
    var texts_address = UInt(Int(views))
    var dst = UInt(Int(output))
    var result = UInt(Int(written))
    var unsigned_address = UInt(Int(count))
    assert prodex_terminal_info_render_v1(
        1, 35, 0, 0, 0, 0, texts_address, 2, 0, dst, 64, result
    ) == 0
    assert written[unsafe_offset=0] == 1
    assert output[unsafe_offset=0] == 50
    views[unsafe_offset=2] = UInt64(UInt(Int(blocked.unsafe_ptr())))
    views[unsafe_offset=3] = UInt64(blocked.byte_length())
    assert prodex_terminal_info_render_v1(
        1, 35, 0, 0, 0, 0, texts_address, 2, 0, dst, 64, result
    ) == 0
    assert output[unsafe_offset=0] == 49

    var fivehour = StringSlice("Codex 5h pool")
    views[unsafe_offset=0] = UInt64(UInt(Int(fivehour.unsafe_ptr())))
    views[unsafe_offset=1] = UInt64(fivehour.byte_length())
    count[unsafe_offset=0] = 0
    assert prodex_terminal_info_render_v1(
        1, 36, 0, 0, unsigned_address, 1,
        texts_address, 1, 0, dst, 64, result
    ) == 0
    assert written[unsafe_offset=0] == 1
    assert output[unsafe_offset=0] == 48
    count[unsafe_offset=0] = 1
    assert prodex_terminal_info_render_v1(
        1, 36, 0, 0, unsigned_address, 1,
        texts_address, 1, 0, dst, 64, result
    ) == 0
    assert output[unsafe_offset=0] == 49

    written[unsafe_offset=0] = 77
    assert prodex_terminal_info_render_v1(
        1, 35, 0, 0, 0, 0, texts_address, 1, 0, dst, 64, result
    ) == 1
    assert prodex_terminal_info_render_v1(
        1, 36, 0, 0, unsigned_address, 0,
        texts_address, 1, 0, dst, 64, result
    ) == 1
    assert prodex_terminal_info_render_v1(
        2, 35, 0, 0, 0, 0, texts_address, 2, 0, dst, 64, result
    ) == 1
    assert prodex_terminal_info_render_v1(
        1, 37, 0, 0, 0, 0, texts_address, 2, 0, dst, 64, result
    ) == 1
    assert written[unsafe_offset=0] == 77
    assert output[unsafe_offset=0] == 49
    dealloc(count_store^)
    dealloc(written_store^)
    dealloc(output_store^)
    dealloc(views_store^)
