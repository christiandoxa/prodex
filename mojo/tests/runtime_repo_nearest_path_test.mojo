# Direct Mojo ABI: nearest source-path distance and deterministic tie breaks.
# Run: mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_repo_nearest_path_test.mojo
from std.memory.alloc import alloc, dealloc, Layout
from runtime_repo_map import prodex_runtime_repo_nearest_path_v1


def main() raises:
    var rows_storage = alloc(Layout[Int64](count=15))
    var result_storage = alloc(Layout[Int64](count=1))
    var rows = rows_storage.unsafe_ptr()
    var result = result_storage.unsafe_ptr()
    for index in range(15):
        rows[unsafe_offset=index] = 0
    rows[unsafe_offset=0] = 1
    rows[unsafe_offset=1] = 2
    rows[unsafe_offset=2] = 1
    rows[unsafe_offset=3] = 4
    rows[unsafe_offset=4] = 5
    rows[unsafe_offset=5] = 1
    rows[unsafe_offset=6] = 2
    rows[unsafe_offset=7] = 100
    rows[unsafe_offset=8] = 0
    rows[unsafe_offset=9] = 4
    rows[unsafe_offset=10] = 5
    rows[unsafe_offset=11] = 1
    rows[unsafe_offset=12] = 1
    rows[unsafe_offset=13] = 2
    rows[unsafe_offset=14] = 1
    var input = UInt(Int(rows))
    var dest = UInt(Int(result))
    assert prodex_runtime_repo_nearest_path_v1(1, input, 5, 3, dest) == 0
    assert result[unsafe_offset=0] == 0
    assert prodex_runtime_repo_nearest_path_v1(1, input, 5, 5, dest) == 0
    assert result[unsafe_offset=0] == 1
    assert prodex_runtime_repo_nearest_path_v1(1, input, 5, 0, dest) == 0
    assert result[unsafe_offset=0] == 0

    result[unsafe_offset=0] = 91
    assert prodex_runtime_repo_nearest_path_v1(2, input, 5, 3, dest) == 4
    assert prodex_runtime_repo_nearest_path_v1(1, 0, 5, 3, dest) == 1
    assert prodex_runtime_repo_nearest_path_v1(1, input, -1, 3, dest) == 1
    rows[unsafe_offset=14] = 2
    assert prodex_runtime_repo_nearest_path_v1(1, input, 5, 3, dest) == 1
    assert result[unsafe_offset=0] == 91
    assert prodex_runtime_repo_nearest_path_v1(1, 0, 0, 3, dest) == 0
    assert result[unsafe_offset=0] == -1
    dealloc(result_storage^)
    dealloc(rows_storage^)
