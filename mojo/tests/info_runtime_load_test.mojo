# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/info_runtime_load_test.mojo`.
from std.memory.alloc import alloc, dealloc, Layout
from parsed_json import pj_literal
from info_runtime_load import InfoLoadRow
from info_render import prodex_info_runtime_load_plan_v1


def main() raises:
    var rows_storage = alloc(Layout[InfoLoadRow](count=3))
    var output_storage = alloc(Layout[UInt64](count=8))
    var rows = rows_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    rows[unsafe_offset=0] = InfoLoadRow(0, 100, 0, pj_literal(StringSlice("β")))
    rows[unsafe_offset=1] = InfoLoadRow(0, 99, 0, pj_literal(StringSlice("a")))
    rows[unsafe_offset=2] = InfoLoadRow(0, 100, 0, pj_literal(StringSlice("a")))
    assert prodex_info_runtime_load_plan_v1(1, UInt(Int(rows)), 3, 0, 0, 100, 0, 1, UInt(Int(output)), 8) == 0
    assert output[unsafe_offset=0] == 3
    assert output[unsafe_offset=2] == 2
    assert output[unsafe_offset=5] == 1
    assert output[unsafe_offset=6] == 2
    assert output[unsafe_offset=7] == 0
    assert prodex_info_runtime_load_plan_v1(2, 0, 0, 0, 0, 0, 0, 0, UInt(Int(output)), 8) == 4
    dealloc(rows_storage^)
    dealloc(output_storage^)
