# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/info_token_summary_test.mojo`.
from std.memory.alloc import alloc, dealloc, Layout
from rich_types import ProdexRichStringView
from parsed_json import pj_literal
from info_render import prodex_info_token_usage_summary_v1


def main() raises:
    var profile_storage = alloc(Layout[ProdexRichStringView](count=2))
    var counts_storage = alloc(Layout[UInt64](count=8))
    var output_storage = alloc(Layout[UInt64](count=20))
    var profiles = profile_storage.unsafe_ptr()
    var counts = counts_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    profiles[unsafe_offset=0] = pj_literal(StringSlice("β"))
    profiles[unsafe_offset=1] = pj_literal(StringSlice("a"))
    for index in range(8):
        counts[unsafe_offset=index] = 1
    assert prodex_info_token_usage_summary_v1(1, UInt(Int(profiles)), UInt(Int(counts)), 2, UInt(Int(output)), 20) == 0
    assert output[unsafe_offset=0] == 2
    assert output[unsafe_offset=1] == 2
    assert output[unsafe_offset=2] == 2
    assert output[unsafe_offset=6] == 1
    assert output[unsafe_offset=12] == 0
    dealloc(profile_storage^)
    dealloc(counts_storage^)
    dealloc(output_storage^)
