# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/launch_quota_plan_test.mojo`.
from std.memory.alloc import alloc, dealloc, Layout
from rich_types import ProdexRichStringView
from parsed_json import pj_literal
from info_render import prodex_terminal_info_render_v1


def main() raises:
    var text_storage = alloc(Layout[ProdexRichStringView](count=5))
    var output_storage = alloc(Layout[UInt8](count=1024))
    var written_storage = alloc(Layout[Int64](count=1))
    var texts = text_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var written = written_storage.unsafe_ptr()
    texts[unsafe_offset=0] = pj_literal(StringSlice("main"))
    texts[unsafe_offset=1] = pj_literal(StringSlice(""))
    texts[unsafe_offset=2] = pj_literal(StringSlice(""))
    texts[unsafe_offset=3] = pj_literal(StringSlice("exhausted"))
    texts[unsafe_offset=4] = pj_literal(StringSlice("βeta"))
    for index in range(1024):
        output[unsafe_offset=index] = 71
    written[] = -1
    var text_address = UInt(Int(texts))
    var output_address = UInt(Int(output))
    var written_address = UInt(Int(written))
    assert prodex_terminal_info_render_v1(1, 32, 0, 0, 0, 0, text_address, 5, 0, output_address, 1024, written_address) == 1
    assert output[unsafe_offset=0] == 71
    assert written[] == -1
    assert prodex_terminal_info_render_v1(1, 33, 0, 0, 0, 0, text_address, 5, 1, output_address, 1024, written_address) == 0
    assert output[unsafe_offset=0] == 48
    assert output[unsafe_offset=22] == 81
    assert prodex_terminal_info_render_v1(1, 33, 0, 0, 0, 0, text_address, 5, 0, output_address, 1024, written_address) == 0
    assert output[unsafe_offset=0] == 49
    assert prodex_terminal_info_render_v1(1, 32, 0, 0, 0, 0, text_address, 4, 1, output_address, 1024, written_address) == 0
    assert output[unsafe_offset=0] == 49
    assert output[unsafe_offset=22] == 87
    dealloc(text_storage^)
    dealloc(output_storage^)
    dealloc(written_storage^)
