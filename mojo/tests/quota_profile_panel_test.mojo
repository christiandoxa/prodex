# Native quota-panel ABI checks use caller-owned views and exact capacity.
from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from std.collections import Array
from parsed_json import pj_literal
from rich_types import ProdexRichStringView
from quota_watch_presentation import prodex_quota_profile_panel_v1


def main() raises:
    var storage = alloc(Layout[ProdexRichStringView](count=11))
    var fields = storage.unsafe_ptr()
    for index in range(11):
        fields[unsafe_offset=index] = pj_literal("")
    fields[unsafe_offset=0] = pj_literal("main")
    fields[unsafe_offset=6] = pj_literal("Ready")
    fields[unsafe_offset=7] = pj_literal("full")
    var source = UInt64(Int(fields))
    var output = Array[UInt8, 256](fill=77)
    var dest = UInt64(Int(output.unsafe_ptr()))
    var metadata = alloc(Layout[Int64](count=2))
    var meta = metadata.unsafe_ptr()
    var address = UInt64(Int(meta))
    meta[unsafe_offset=0] = 71
    meta[unsafe_offset=1] = 71
    assert prodex_quota_profile_panel_v1(2, 0, 0, source, 193, 0, 0, 0, dest, 256, address) == 4
    assert prodex_quota_profile_panel_v1(1, 4, 0, source, 193, 0, 0, 0, dest, 256, address) == 1
    assert prodex_quota_profile_panel_v1(1, 0, 0, source + 1, 193, 0, 0, 0, dest, 256, address) == 1
    assert prodex_quota_profile_panel_v1(1, 0, 0, source, 0, 0, 0, 0, dest, 256, address) == 1
    assert prodex_quota_profile_panel_v1(1, 0, 0, source, 193, 0, 1, 0, dest, 256, address) == 1
    assert prodex_quota_profile_panel_v1(1, 0, 0, source, 193, 0, 0, 0, dest, 1, address) == 3
    assert meta[unsafe_offset=0] == 71 and meta[unsafe_offset=1] == 71
    for index in range(256):
        assert output[index] == 77
    assert prodex_quota_profile_panel_v1(1, 0, 0, source, 193, 0, 0, 1, 0, 0, address) == 0
    var required = meta[unsafe_offset=1]
    assert required > 2 and required < 256
    assert prodex_quota_profile_panel_v1(1, 0, 0, source, 193, 0, 0, 0, dest, required, address) == 0
    assert meta[unsafe_offset=0] == 1 and meta[unsafe_offset=1] == required
    assert output[0] == 91 and output[Int(required) - 1] == 93
    dealloc(metadata^)
    dealloc(storage^)
