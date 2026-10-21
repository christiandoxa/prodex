# Direct ABI regression for raw file-key sorting and stable de-duplication.
# Run: mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/core_file_lexical_order_test.mojo
from std.memory.alloc import alloc, dealloc, Layout
from core_file_policy import prodex_core_file_lexical_order_v1


def main() raises:
    var view_storage = alloc(Layout[UInt64](count=12))
    var order_storage = alloc(Layout[Int64](count=6))
    var count_storage = alloc(Layout[Int64](count=1))
    var views = view_storage.unsafe_ptr()
    var order = order_storage.unsafe_ptr()
    var count = count_storage.unsafe_ptr()
    var snow = StringSlice("雪")
    var a = StringSlice("a")
    var z = StringSlice("z")
    var accent = StringSlice("é")
    var values = [snow, a, z, a, accent]
    for index in range(5):
        views[unsafe_offset=index * 2] = UInt64(UInt(Int(values[index].unsafe_ptr())))
        views[unsafe_offset=index * 2 + 1] = UInt64(values[index].byte_length())
    views[unsafe_offset=10] = 0
    views[unsafe_offset=11] = 0
    var source = UInt(Int(views))
    var result = UInt(Int(order))
    var written = UInt(Int(count))
    assert prodex_core_file_lexical_order_v1(
        1, source, 6, 0, result, 6, written,
    ) == 0
    assert count[unsafe_offset=0] == 6
    assert order[unsafe_offset=0] == 5
    assert order[unsafe_offset=1] == 1
    assert order[unsafe_offset=2] == 3
    assert order[unsafe_offset=3] == 2
    assert order[unsafe_offset=4] == 4
    assert order[unsafe_offset=5] == 0

    assert prodex_core_file_lexical_order_v1(
        1, source, 6, 1, result, 6, written,
    ) == 0
    assert count[unsafe_offset=0] == 5
    assert order[unsafe_offset=0] == 5
    assert order[unsafe_offset=1] == 1
    assert order[unsafe_offset=2] == 2
    assert order[unsafe_offset=3] == 4
    assert order[unsafe_offset=4] == 0

    for index in range(6):
        order[unsafe_offset=index] = 77
    count[unsafe_offset=0] = 89
    assert prodex_core_file_lexical_order_v1(
        2, source, 6, 1, result, 6, written,
    ) == 4
    assert prodex_core_file_lexical_order_v1(
        1, source, 6, 1, result, 5, written,
    ) == 1
    views[unsafe_offset=11] = UInt64(9223372036854775808)
    assert prodex_core_file_lexical_order_v1(
        1, source, 6, 1, result, 6, written,
    ) == 1
    assert count[unsafe_offset=0] == 89
    for index in range(6):
        assert order[unsafe_offset=index] == 77
    assert prodex_core_file_lexical_order_v1(
        1, 0, 0, 1, result, 6, written,
    ) == 0
    assert count[unsafe_offset=0] == 0
    dealloc(count_storage^)
    dealloc(order_storage^)
    dealloc(view_storage^)
