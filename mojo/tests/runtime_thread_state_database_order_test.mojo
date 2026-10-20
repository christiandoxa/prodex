# Direct compiled thread-index filename descending and malformed ABI coverage.
# Run: mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_thread_state_database_order_test.mojo
from std.memory.alloc import alloc, dealloc, Layout
from runtime_thread_index import prodex_runtime_thread_index_database_order_v1


def main() raises:
    var rows_storage = alloc(Layout[UInt64](count=8))
    var output_storage = alloc(Layout[Int64](count=4))
    var rows = rows_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var first = StringSlice("state_10.sqlite")
    var last = StringSlice("state_9.sqlite")
    var snow = StringSlice("state_雪.sqlite")
    var accent = StringSlice("state_é.sqlite")
    var names = [first, last, snow, accent]
    for index in range(4):
        rows[unsafe_offset=index * 2] = UInt64(UInt(Int(names[index].unsafe_ptr())))
        rows[unsafe_offset=index * 2 + 1] = UInt64(names[index].byte_length())
    var source = UInt(Int(rows))
    var result = UInt(Int(output))
    assert prodex_runtime_thread_index_database_order_v1(
        1, source, 4, result, 4,
    ) == 0
    assert output[unsafe_offset=0] == 2
    assert output[unsafe_offset=1] == 3
    assert output[unsafe_offset=2] == 1
    assert output[unsafe_offset=3] == 0

    for index in range(4):
        output[unsafe_offset=index] = 73
    assert prodex_runtime_thread_index_database_order_v1(
        2, source, 4, result, 4,
    ) == 4
    assert prodex_runtime_thread_index_database_order_v1(
        1, source, 4, result, 3,
    ) == 1
    rows[unsafe_offset=7] = UInt64(9223372036854775808)
    assert prodex_runtime_thread_index_database_order_v1(
        1, source, 4, result, 4,
    ) == 1
    for index in range(4):
        assert output[unsafe_offset=index] == 73
    assert prodex_runtime_thread_index_database_order_v1(
        1, 0, 0, result, 4,
    ) == 0
    dealloc(output_storage^)
    dealloc(rows_storage^)
