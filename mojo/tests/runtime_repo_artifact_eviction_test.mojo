# Direct ABI: oldest-first artifact eviction under count and byte caps.
from std.memory.alloc import alloc, dealloc, Layout
from runtime_repo_map import prodex_runtime_repo_artifact_eviction_v1


def main() raises:
    var rows_storage = alloc(Layout[UInt64](count=16))
    var scratch_storage = alloc(Layout[Int64](count=4))
    var output_storage = alloc(Layout[Int64](count=4))
    var result_storage = alloc(Layout[Int64](count=1))
    var rows = rows_storage.unsafe_ptr()
    var scratch = scratch_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var written = result_storage.unsafe_ptr()
    var z = StringSlice("z")
    var a = StringSlice("a")
    var n = StringSlice("newer")
    var l = StringSlice("latest")
    var names = [z, a, n, l]
    for index in range(4):
        rows[unsafe_offset=index * 4] = UInt64(index if index > 1 else 1)
        rows[unsafe_offset=index * 4 + 1] = UInt64(UInt(Int(names[index].unsafe_ptr())))
        rows[unsafe_offset=index * 4 + 2] = UInt64(names[index].byte_length())
        rows[unsafe_offset=index * 4 + 3] = 1
    rows[unsafe_offset=3] = 4
    rows[unsafe_offset=7] = 3
    rows[unsafe_offset=11] = 10
    var input = UInt(Int(rows))
    var work = UInt(Int(scratch))
    var selected = UInt(Int(output))
    var count = UInt(Int(written))
    assert prodex_runtime_repo_artifact_eviction_v1(
        1, input, 4, 2, 18, 12, work, 4, selected, 4, count,
    ) == 0
    assert written[unsafe_offset=0] == 2
    assert output[unsafe_offset=0] == 1
    assert output[unsafe_offset=1] == 0
    assert prodex_runtime_repo_artifact_eviction_v1(
        1, input, 4, 4, 18, 18, work, 4, selected, 4, count,
    ) == 0
    assert written[unsafe_offset=0] == 0
    assert prodex_runtime_repo_artifact_eviction_v1(
        1, input, 4, 4, 18, 0, work, 4, selected, 4, count,
    ) == 0
    assert written[unsafe_offset=0] == 4

    for index in range(4):
        output[unsafe_offset=index] = 77
        scratch[unsafe_offset=index] = 73
    written[unsafe_offset=0] = 91
    assert prodex_runtime_repo_artifact_eviction_v1(
        2, input, 4, 2, 18, 12, work, 4, selected, 4, count,
    ) == 4
    assert prodex_runtime_repo_artifact_eviction_v1(
        1, input, 4, 2, 18, 12, work, 3, selected, 4, count,
    ) == 1
    rows[unsafe_offset=14] = UInt64(9223372036854775808)
    assert prodex_runtime_repo_artifact_eviction_v1(
        1, input, 4, 2, 18, 12, work, 4, selected, 4, count,
    ) == 1
    assert written[unsafe_offset=0] == 91
    for index in range(4):
        assert output[unsafe_offset=index] == 77
        assert scratch[unsafe_offset=index] == 73
    assert prodex_runtime_repo_artifact_eviction_v1(
        1, 0, 0, 0, 0, 0, work, 4, selected, 4, count,
    ) == 0
    assert written[unsafe_offset=0] == 0
    dealloc(result_storage^)
    dealloc(output_storage^)
    dealloc(scratch_storage^)
    dealloc(rows_storage^)
