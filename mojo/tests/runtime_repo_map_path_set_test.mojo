# Direct ABI: Unicode whitespace path exclusion, lexical sorting and dedup.
# Run: mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_repo_map_path_set_test.mojo
from std.memory.alloc import alloc, dealloc, Layout
from runtime_repo_map import prodex_runtime_repo_map_path_set_v1


def main() raises:
    var rows_storage = alloc(Layout[UInt64](count=18))
    var output_storage = alloc(Layout[Int64](count=6))
    var result_storage = alloc(Layout[Int64](count=1))
    var rows = rows_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var selected = result_storage.unsafe_ptr()
    for index in range(18):
        rows[unsafe_offset=index] = 0
    var snow = StringSlice("src/雪.rs")
    var alpha = StringSlice("src/a.rs")
    var whitespace = StringSlice("　 \t")
    var prefix = StringSlice(" src/a.rs ")
    var names = [snow, alpha, whitespace, snow, prefix]
    for index in range(5):
        rows[unsafe_offset=index * 3] = 1
        rows[unsafe_offset=index * 3 + 1] = UInt64(UInt(Int(names[index].unsafe_ptr())))
        rows[unsafe_offset=index * 3 + 2] = UInt64(names[index].byte_length())
    var source = UInt(Int(rows))
    var dest = UInt(Int(output))
    var written = UInt(Int(selected))
    assert prodex_runtime_repo_map_path_set_v1(
        1, source, 6, dest, 6, written,
    ) == 0
    assert selected[unsafe_offset=0] == 3
    assert output[unsafe_offset=0] == 4
    assert output[unsafe_offset=1] == 1
    assert output[unsafe_offset=2] == 0

    for index in range(6):
        output[unsafe_offset=index] = 77
    selected[unsafe_offset=0] = 91
    assert prodex_runtime_repo_map_path_set_v1(
        2, source, 6, dest, 6, written,
    ) == 4
    assert prodex_runtime_repo_map_path_set_v1(
        1, source, 6, dest, 5, written,
    ) == 1
    rows[unsafe_offset=15] = 2
    assert prodex_runtime_repo_map_path_set_v1(
        1, source, 6, dest, 6, written,
    ) == 1
    assert selected[unsafe_offset=0] == 91
    for index in range(6):
        assert output[unsafe_offset=index] == 77
    assert prodex_runtime_repo_map_path_set_v1(
        1, 0, 0, dest, 6, written,
    ) == 0
    assert selected[unsafe_offset=0] == 0
    dealloc(result_storage^)
    dealloc(output_storage^)
    dealloc(rows_storage^)
