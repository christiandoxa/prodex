# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/info_provider_test.mojo`.
from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from parsed_json import pj_literal, pj_equal
from rich_types import ProdexRichStringView
from json_sink import JsonSink
from info_provider import info_provider_groups
from info_render import prodex_terminal_info_render_v1


def main() raises:
    var view_storage = alloc(Layout[ProdexRichStringView](count=3))
    var order_storage = alloc(Layout[Int64](count=3))
    var output_storage = alloc(Layout[UInt8](count=128))
    var views = Pointer[mut=True, ProdexRichStringView, MutUntrackedOrigin](unsafe_from_address=Int(view_storage.unsafe_ptr()))
    var order = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(order_storage.unsafe_ptr()))
    var buffer = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_storage.unsafe_ptr()))
    views[unsafe_offset=0] = pj_literal(StringSlice("openai"))
    views[unsafe_offset=1] = pj_literal(StringSlice("gemini"))
    views[unsafe_offset=2] = pj_literal(StringSlice("openai"))
    for index in range(3):
        order[unsafe_offset=index] = Int64(index)
    var sink = JsonSink(buffer, 128, 0, False, False)
    info_provider_groups(Pointer(to=sink), views, order, 3)
    assert not sink.failed
    assert pj_equal(ProdexRichStringView(UInt(Int(buffer)), UInt(sink.written)), pj_literal(StringSlice("gemini=1, openai=2")))
    assert prodex_terminal_info_render_v1(2, 30, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0) == 1
    dealloc(view_storage^)
    dealloc(order_storage^)
    dealloc(output_storage^)
