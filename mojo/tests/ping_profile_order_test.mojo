# Direct ABI for stable Unicode profile ordering after parallel ping probes.
# Run: mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/ping_profile_order_test.mojo
from std.memory.alloc import alloc, dealloc, Layout
from ping_protocol import prodex_mojo_ping_profile_order_v1


def main() raises:
    var views_storage = alloc(Layout[UInt64](count=10))
    var output_storage = alloc(Layout[Int64](count=5))
    var views = views_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var snow = StringSlice("雪")
    var zed = StringSlice("z")
    var alpha = StringSlice("a")
    var accent = StringSlice("é")
    var names = [snow, zed, alpha, accent, alpha]
    for i in range(5):
        views[unsafe_offset=i * 2] = UInt64(UInt(Int(names[i].unsafe_ptr())))
        views[unsafe_offset=i * 2 + 1] = UInt64(names[i].byte_length())
    var input = UInt(Int(views))
    var result = UInt(Int(output))
    assert prodex_mojo_ping_profile_order_v1(1, input, 5, result, 5) == 0
    assert output[unsafe_offset=0] == 2
    assert output[unsafe_offset=1] == 4
    assert output[unsafe_offset=2] == 1
    assert output[unsafe_offset=3] == 3
    assert output[unsafe_offset=4] == 0

    for i in range(5):
        output[unsafe_offset=i] = 73
    assert prodex_mojo_ping_profile_order_v1(2, input, 5, result, 5) == 4
    assert prodex_mojo_ping_profile_order_v1(1, input, 5, result, 4) == 1
    views[unsafe_offset=9] = UInt64(9223372036854775808)
    assert prodex_mojo_ping_profile_order_v1(1, input, 5, result, 5) == 1
    for i in range(5):
        assert output[unsafe_offset=i] == 73
    assert prodex_mojo_ping_profile_order_v1(1, 0, 0, result, 5) == 0
    dealloc(output_storage^)
    dealloc(views_storage^)
