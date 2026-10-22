from std.memory.alloc import alloc, dealloc, Layout
from rich_types import ProdexRichStringView
from parsed_json import pj_literal
from operational_metrics import prodex_mojo_operational_metric_label_order_v1


def main() raises:
    var fs = alloc(Layout[ProdexRichStringView](count=8))
    var os = alloc(Layout[Int64](count=4))
    var fields = fs.unsafe_ptr()
    var output = os.unsafe_ptr()
    fields[unsafe_offset=0] = pj_literal(StringSlice("route"))
    fields[unsafe_offset=1] = pj_literal(StringSlice("z"))
    fields[unsafe_offset=2] = pj_literal(StringSlice("route"))
    fields[unsafe_offset=3] = pj_literal(StringSlice("a"))
    fields[unsafe_offset=4] = pj_literal(StringSlice("route"))
    fields[unsafe_offset=5] = pj_literal(StringSlice("a"))
    fields[unsafe_offset=6] = pj_literal(StringSlice("é"))
    fields[unsafe_offset=7] = pj_literal(StringSlice("value"))
    var table = UInt(Int(fields))
    var target = UInt(Int(output))
    assert prodex_mojo_operational_metric_label_order_v1(1, table, 4, target, 4) == 0
    assert output[unsafe_offset=0] == 1 and output[unsafe_offset=1] == 2
    assert output[unsafe_offset=2] == 0 and output[unsafe_offset=3] == 3
    assert prodex_mojo_operational_metric_label_order_v1(1, 0, 0, 0, 0) == 0
    for index in range(4):
        output[unsafe_offset=index] = 73
    assert prodex_mojo_operational_metric_label_order_v1(2, 0, 0, target, 4) == 4
    assert prodex_mojo_operational_metric_label_order_v1(1, table, 4, target, 3) == 2
    fields[unsafe_offset=7] = ProdexRichStringView(fields[unsafe_offset=6].ptr, 1)
    assert prodex_mojo_operational_metric_label_order_v1(1, table, 4, target, 4) == 1
    for index in range(4):
        assert output[unsafe_offset=index] == 73
    dealloc(fs^)
    dealloc(os^)
