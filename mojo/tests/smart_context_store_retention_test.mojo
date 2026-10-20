# Direct compiled scope-store retention policy: ordering, age, disk cap.
# Run with mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/smart_context_store_retention_test.mojo
from std.memory.alloc import alloc, dealloc, Layout
from smart_context_policy import prodex_smart_context_store_retention_v1


def main() raises:
    var rows_storage = alloc(Layout[UInt64](count=36))
    var output_storage = alloc(Layout[Int64](count=4))
    var result_storage = alloc(Layout[Int64](count=1))
    var rows = rows_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var written = result_storage.unsafe_ptr()
    for i in range(36):
        rows[unsafe_offset=i] = 0
    var names = [
        StringSlice("z"), StringSlice("b"),
        StringSlice("a"), StringSlice("y"),
    ]
    for index in range(4):
        rows[unsafe_offset=index * 9] = UInt64(UInt(Int(names[index].unsafe_ptr())))
        rows[unsafe_offset=index * 9 + 1] = UInt64(names[index].byte_length())
    rows[unsafe_offset=3] = 100
    rows[unsafe_offset=5] = 10
    rows[unsafe_offset=7] = 4
    rows[unsafe_offset=12] = 10
    rows[unsafe_offset=14] = 100
    rows[unsafe_offset=16] = 8
    rows[unsafe_offset=21] = 10
    rows[unsafe_offset=23] = 100
    rows[unsafe_offset=25] = 3
    rows[unsafe_offset=30] = 150
    rows[unsafe_offset=32] = 0
    rows[unsafe_offset=34] = 3
    var source = UInt(Int(rows))
    var target = UInt(Int(output))
    var result = UInt(Int(written))
    assert prodex_smart_context_store_retention_v1(
        1, source, 4, 50, 0, 12, target, 4, result,
    ) == 0
    assert written[unsafe_offset=0] == 2
    assert output[unsafe_offset=0] == 2
    assert output[unsafe_offset=1] == 1
    assert prodex_smart_context_store_retention_v1(
        1, source, 4, 200, 0, 18, target, 4, result,
    ) == 0
    assert written[unsafe_offset=0] == 0
    assert prodex_smart_context_store_retention_v1(
        1, source, 4, 200, 0, 12, target, 4, result,
    ) == 0
    assert written[unsafe_offset=0] == 2
    assert output[unsafe_offset=0] == 2
    assert output[unsafe_offset=1] == 1

    for index in range(4):
        output[unsafe_offset=index] = 73
    written[unsafe_offset=0] = 91
    assert prodex_smart_context_store_retention_v1(
        2, source, 4, 50, 0, 12, target, 4, result,
    ) == 4
    assert prodex_smart_context_store_retention_v1(
        1, source, 4, 50, 0, 12, target, 3, result,
    ) == 1
    rows[unsafe_offset=33] = 1_000_000_000
    assert prodex_smart_context_store_retention_v1(
        1, source, 4, 50, 0, 12, target, 4, result,
    ) == 1
    assert written[unsafe_offset=0] == 91
    for index in range(4):
        assert output[unsafe_offset=index] == 73
    assert prodex_smart_context_store_retention_v1(
        1, 0, 0, 50, 0, 12, target, 4, result,
    ) == 0
    assert written[unsafe_offset=0] == 0
    dealloc(result_storage^)
    dealloc(output_storage^)
    dealloc(rows_storage^)
