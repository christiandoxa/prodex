# Direct SSE response-ID sort-and-dedup batch ABI test.
# Run: mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_sse_response_id_order_test.mojo
from std.memory.alloc import alloc, dealloc, Layout
from sse_line import prodex_runtime_sse_response_id_order_v1


def main() raises:
    var views_storage = alloc(Layout[UInt64](count=10))
    var indices_storage = alloc(Layout[Int64](count=5))
    var result_storage = alloc(Layout[Int64](count=1))
    var views = views_storage.unsafe_ptr()
    var indices = indices_storage.unsafe_ptr()
    var written = result_storage.unsafe_ptr()
    var snow = StringSlice("雪")
    var bee = StringSlice("b")
    var aye = StringSlice("a")
    var accent = StringSlice("é")
    views[unsafe_offset=0] = UInt64(UInt(Int(snow.unsafe_ptr())))
    views[unsafe_offset=1] = UInt64(snow.byte_length())
    views[unsafe_offset=2] = UInt64(UInt(Int(bee.unsafe_ptr())))
    views[unsafe_offset=3] = UInt64(bee.byte_length())
    views[unsafe_offset=4] = UInt64(UInt(Int(aye.unsafe_ptr())))
    views[unsafe_offset=5] = UInt64(aye.byte_length())
    views[unsafe_offset=6] = views[unsafe_offset=2]
    views[unsafe_offset=7] = views[unsafe_offset=3]
    views[unsafe_offset=8] = UInt64(UInt(Int(accent.unsafe_ptr())))
    views[unsafe_offset=9] = UInt64(accent.byte_length())
    var input_address = UInt(Int(views))
    var output_address = UInt(Int(indices))
    var result_address = UInt(Int(written))
    assert prodex_runtime_sse_response_id_order_v1(
        1, input_address, 5, output_address, 5, result_address,
    ) == 0
    assert written[unsafe_offset=0] == 4
    assert indices[unsafe_offset=0] == 2
    assert indices[unsafe_offset=1] == 1
    assert indices[unsafe_offset=2] == 4
    assert indices[unsafe_offset=3] == 0

    for index in range(5):
        indices[unsafe_offset=index] = 77
    written[unsafe_offset=0] = 81
    assert prodex_runtime_sse_response_id_order_v1(
        2, input_address, 5, output_address, 5, result_address,
    ) == 4
    assert prodex_runtime_sse_response_id_order_v1(
        1, input_address, 5, output_address, 4, result_address,
    ) == 1
    assert written[unsafe_offset=0] == 81
    for index in range(5):
        assert indices[unsafe_offset=index] == 77
    views[unsafe_offset=9] = UInt64(9223372036854775808)
    assert prodex_runtime_sse_response_id_order_v1(
        1, input_address, 5, output_address, 5, result_address,
    ) == 1
    assert written[unsafe_offset=0] == 81
    for index in range(5):
        assert indices[unsafe_offset=index] == 77

    assert prodex_runtime_sse_response_id_order_v1(
        1, 0, 0, output_address, 5, result_address,
    ) == 0
    assert written[unsafe_offset=0] == 0
    dealloc(result_storage^)
    dealloc(indices_storage^)
    dealloc(views_storage^)
