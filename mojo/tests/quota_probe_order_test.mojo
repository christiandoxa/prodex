# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/quota_probe_order_test.mojo`.
from std.memory.alloc import alloc, dealloc, Layout
from quota_pressure import prodex_runtime_quota_probe_order_v1


def main() raises:
    var input_storage = alloc(Layout[Int64](count=18))
    var scratch_storage = alloc(Layout[Int64](count=20))
    var output_storage = alloc(Layout[Int64](count=2))
    var rows = input_storage.unsafe_ptr()
    var scratch = scratch_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    for index in range(18):
        rows[unsafe_offset=index] = 0
    rows[unsafe_offset=0] = 1
    rows[unsafe_offset=10] = 1
    rows[unsafe_offset=14] = 50
    rows[unsafe_offset=15] = 50
    assert prodex_runtime_quota_probe_order_v1(1, 0, 1, 0, 0, 0, 0) == 0
    assert prodex_runtime_quota_probe_order_v1(1, 0, 2, 0, 0, 0, 0) == 5
    assert prodex_runtime_quota_probe_order_v1(1, UInt(Int(rows)), 2, UInt(Int(scratch)), 20, UInt(Int(output)), 2) == 0
    assert output[unsafe_offset=0] == 1 and output[unsafe_offset=1] == 0
    dealloc(input_storage^)
    dealloc(scratch_storage^)
    dealloc(output_storage^)
