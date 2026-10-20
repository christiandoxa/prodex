# Direct ABI: newest artifact order first, id lexical ascending on ties.
from std.memory.alloc import alloc, dealloc, Layout
from runtime_repo_map import prodex_runtime_repo_artifact_order_v1


def main() raises:
    var rows_storage = alloc(Layout[UInt64](count=15))
    var output_storage = alloc(Layout[Int64](count=5))
    var rows = rows_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var last = StringSlice("last")
    var snow = StringSlice("雪")
    var first = StringSlice("a")
    var accent = StringSlice("é")
    var ids = [last, snow, first, accent, first]
    for index in range(5):
        rows[unsafe_offset=index * 3] = 10 if index > 0 else 1
        rows[unsafe_offset=index * 3 + 1] = UInt64(UInt(Int(ids[index].unsafe_ptr())))
        rows[unsafe_offset=index * 3 + 2] = UInt64(ids[index].byte_length())
    var source = UInt(Int(rows))
    var dest = UInt(Int(output))
    assert prodex_runtime_repo_artifact_order_v1(1, source, 5, dest, 5) == 0
    assert output[unsafe_offset=0] == 2
    assert output[unsafe_offset=1] == 4
    assert output[unsafe_offset=2] == 3
    assert output[unsafe_offset=3] == 1
    assert output[unsafe_offset=4] == 0

    for index in range(5):
        output[unsafe_offset=index] = 77
    assert prodex_runtime_repo_artifact_order_v1(2, source, 5, dest, 5) == 4
    assert prodex_runtime_repo_artifact_order_v1(1, source, 5, dest, 4) == 1
    rows[unsafe_offset=14] = UInt64(9223372036854775808)
    assert prodex_runtime_repo_artifact_order_v1(1, source, 5, dest, 5) == 1
    for index in range(5):
        assert output[unsafe_offset=index] == 77
    assert prodex_runtime_repo_artifact_order_v1(1, 0, 0, dest, 5) == 0
    for index in range(5):
        assert output[unsafe_offset=index] == 77
    dealloc(output_storage^)
    dealloc(rows_storage^)
