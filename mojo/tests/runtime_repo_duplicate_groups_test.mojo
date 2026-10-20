# Direct ABI: sorted fingerprint groups, occurrence caps and invalid late rows.
# Run: mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_repo_duplicate_groups_test.mojo
from std.memory.alloc import alloc, dealloc, Layout
from runtime_repo_map import prodex_runtime_repo_duplicate_groups_v1


def main() raises:
    var rows_storage = alloc(Layout[UInt64](count=18))
    var indices_storage = alloc(Layout[Int64](count=6))
    var groups_storage = alloc(Layout[Int64](count=9))
    var result_storage = alloc(Layout[Int64](count=2))
    var rows = rows_storage.unsafe_ptr()
    var indices = indices_storage.unsafe_ptr()
    var groups = groups_storage.unsafe_ptr()
    var result = result_storage.unsafe_ptr()
    var a = StringSlice("hash-a")
    var b = StringSlice("hash-b")
    var names = [b, a, b, a, a, a]
    for i in range(6):
        rows[unsafe_offset=i * 3] = UInt64(UInt(Int(names[i].unsafe_ptr())))
        rows[unsafe_offset=i * 3 + 1] = UInt64(names[i].byte_length())
        rows[unsafe_offset=i * 3 + 2] = 3 if i < 4 else 5
    var source = UInt(Int(rows))
    var out = UInt(Int(indices))
    var grouped = UInt(Int(groups))
    var summary = UInt(Int(result))
    assert prodex_runtime_repo_duplicate_groups_v1(
        1, source, 6, 3, 8, out, 6, grouped, 9, summary,
    ) == 0
    assert result[unsafe_offset=0] == 3
    assert result[unsafe_offset=1] == 1
    assert indices[unsafe_offset=0] == 1
    assert indices[unsafe_offset=1] == 3
    assert indices[unsafe_offset=2] == 4
    assert indices[unsafe_offset=3] == 5
    assert indices[unsafe_offset=4] == 0
    assert indices[unsafe_offset=5] == 2
    assert groups[unsafe_offset=0] == 0
    assert groups[unsafe_offset=1] == 2
    assert groups[unsafe_offset=2] == 2
    assert groups[unsafe_offset=3] == 2
    assert groups[unsafe_offset=4] == 2
    assert groups[unsafe_offset=5] == 2
    assert groups[unsafe_offset=6] == 4
    assert groups[unsafe_offset=7] == 2
    assert groups[unsafe_offset=8] == 2

    assert prodex_runtime_repo_duplicate_groups_v1(
        1, source, 6, 1, 8, out, 6, grouped, 9, summary,
    ) == 0
    assert result[unsafe_offset=0] == 1
    assert result[unsafe_offset=1] == 0
    assert prodex_runtime_repo_duplicate_groups_v1(
        1, source, 6, 3, 1, out, 6, grouped, 9, summary,
    ) == 0
    assert result[unsafe_offset=0] == 3
    assert result[unsafe_offset=1] == 0
    assert groups[unsafe_offset=2] == 1

    for i in range(6):
        indices[unsafe_offset=i] = 73
    for i in range(9):
        groups[unsafe_offset=i] = 79
    result[unsafe_offset=0] = 91
    result[unsafe_offset=1] = 91
    assert prodex_runtime_repo_duplicate_groups_v1(
        2, source, 6, 3, 8, out, 6, grouped, 9, summary,
    ) == 4
    assert prodex_runtime_repo_duplicate_groups_v1(
        1, source, 6, 3, 8, out, 5, grouped, 9, summary,
    ) == 1
    rows[unsafe_offset=17] = UInt64(9223372036854775808)
    assert prodex_runtime_repo_duplicate_groups_v1(
        1, source, 6, 3, 8, out, 6, grouped, 9, summary,
    ) == 1
    for i in range(6):
        assert indices[unsafe_offset=i] == 73
    for i in range(9):
        assert groups[unsafe_offset=i] == 79
    assert result[unsafe_offset=0] == 91
    assert result[unsafe_offset=1] == 91
    assert prodex_runtime_repo_duplicate_groups_v1(
        1, 0, 0, 3, 8, out, 6, grouped, 9, summary,
    ) == 0
    assert result[unsafe_offset=0] == 0
    assert result[unsafe_offset=1] == 1

    dealloc(result_storage^)
    dealloc(groups_storage^)
    dealloc(indices_storage^)
    dealloc(rows_storage^)
