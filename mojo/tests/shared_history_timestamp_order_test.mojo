# Direct compiled history timestamp ordering and output-safety ABI test.
# Run: mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/shared_history_timestamp_order_test.mojo
from std.memory.alloc import alloc, dealloc, Layout
from shared_history_policy import prodex_shared_history_timestamp_order_v1


def main() raises:
    var rows_storage = alloc(Layout[Int64](count=14))
    var output_storage = alloc(Layout[Int64](count=7))
    var rows = rows_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    for index in range(7):
        rows[unsafe_offset=index * 2] = 1
    rows[unsafe_offset=1] = 40
    rows[unsafe_offset=3] = 2
    rows[unsafe_offset=4] = 0
    rows[unsafe_offset=5] = 0
    rows[unsafe_offset=7] = 19
    rows[unsafe_offset=9] = -4
    rows[unsafe_offset=10] = 0
    rows[unsafe_offset=11] = 0
    rows[unsafe_offset=13] = 3
    var source = UInt(Int(rows))
    var target = UInt(Int(output))
    assert prodex_shared_history_timestamp_order_v1(
        1, source, 7, target, 7,
    ) == 0
    assert output[unsafe_offset=0] == 1
    assert output[unsafe_offset=1] == 0
    assert output[unsafe_offset=2] == 2
    assert output[unsafe_offset=3] == 4
    assert output[unsafe_offset=4] == 3
    assert output[unsafe_offset=5] == 5
    assert output[unsafe_offset=6] == 6

    for index in range(7):
        output[unsafe_offset=index] = 73
    assert prodex_shared_history_timestamp_order_v1(
        2, source, 7, target, 7,
    ) == 4
    assert prodex_shared_history_timestamp_order_v1(
        1, source, 7, target, 6,
    ) == 1
    rows[unsafe_offset=12] = 2
    assert prodex_shared_history_timestamp_order_v1(
        1, source, 7, target, 7,
    ) == 1
    for index in range(7):
        assert output[unsafe_offset=index] == 73
    assert prodex_shared_history_timestamp_order_v1(
        1, 0, 0, target, 7,
    ) == 0
    for index in range(7):
        assert output[unsafe_offset=index] == 73
    dealloc(output_storage^)
    dealloc(rows_storage^)
