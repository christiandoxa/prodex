from std.memory.alloc import alloc, dealloc, Layout
from rich_types import ProdexRichStringView
from runtime_active_logs import active_log_pid, prodex_runtime_log_pid_v1, prodex_active_runtime_logs_v1


def view(text: StringSlice) -> ProdexRichStringView:
    return ProdexRichStringView(UInt(Int(text.unsafe_ptr())), UInt(text.byte_length()))


def check_pid(name: StringSlice, expected: Int64, prefix: StringSlice = "prodex-runtime"):
    var result = active_log_pid(view(name), view(prefix))
    assert result[0] == (expected >= 0)
    if expected >= 0:
        assert result[1] == UInt64(expected)


def main() raises:
    check_pid("prodex-runtime-200-old.log", 200)
    check_pid("prodex-runtime-+000200-", 200)
    check_pid("prodex-runtime-0-anything", 0)
    check_pid("prodex-runtime-4294967295-東京", 4_294_967_295)
    check_pid("prodex-runtime-4294967296-x", -1)
    check_pid("prodex-runtime--1-x", -1)
    check_pid("prodex-runtime-+x-x", -1)
    check_pid("prodex-runtime-200", -1)
    check_pid("prodex-runtime- 200-x", -1)
    check_pid("prodex-runtime-２００-x", -1)
    check_pid("prodex-runtime-200x-x", -1)
    check_pid("PRODEX-runtime-200-x", -1)
    check_pid("-+1-", 1, "")
    check_pid("雪--4-\0", 4, "雪-")

    var row_storage = alloc(Layout[UInt64](count=8))
    var pid_storage = alloc(Layout[UInt64](count=7))
    var scratch_storage = alloc(Layout[Int64](count=11))
    var output_storage = alloc(Layout[Int64](count=7))
    var metadata_storage = alloc(Layout[Int64](count=2))
    var parser_storage = alloc(Layout[UInt64](count=2))
    var rows = row_storage.unsafe_ptr()
    rows[unsafe_offset=0] = 200
    rows[unsafe_offset=1] = 1
    rows[unsafe_offset=2] = 2
    rows[unsafe_offset=3] = 1
    rows[unsafe_offset=4] = 201
    rows[unsafe_offset=5] = 0
    rows[unsafe_offset=6] = 200
    rows[unsafe_offset=7] = 0
    var pids = pid_storage.unsafe_ptr()
    pids[unsafe_offset=0] = 200
    pids[unsafe_offset=1] = 201
    pids[unsafe_offset=2] = 2
    pids[unsafe_offset=3] = 200
    pids[unsafe_offset=4] = 18_446_744_073_709_551_615
    pids[unsafe_offset=5] = 999
    pids[unsafe_offset=6] = 2
    var work = scratch_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var metadata = metadata_storage.unsafe_ptr()
    var source = UInt64(Int(rows))
    var names = UInt64(Int(pids))
    var scratch = UInt64(Int(work))
    var target = UInt64(Int(output))
    var meta = UInt64(Int(metadata))
    assert prodex_active_runtime_logs_v1(1, source, 4, 0, 0, scratch, 4, 0, 0, meta) == 0
    assert metadata[unsafe_offset=0] == 1 and metadata[unsafe_offset=1] == 0
    assert prodex_active_runtime_logs_v1(1, source, 4, names, 7, scratch, 11, target, 7, meta) == 0
    assert metadata[unsafe_offset=0] == 1 and metadata[unsafe_offset=1] == 2
    assert output[unsafe_offset=0] == 6 and output[unsafe_offset=1] == 3
    rows[unsafe_offset=1] = 0
    rows[unsafe_offset=3] = 0
    assert prodex_active_runtime_logs_v1(1, source, 4, names, 7, scratch, 11, target, 7, meta) == 0
    assert metadata[unsafe_offset=0] == 0 and metadata[unsafe_offset=1] == 0
    for index in range(11):
        work[unsafe_offset=index] = 81
    for index in range(7):
        output[unsafe_offset=index] = 83
    metadata[unsafe_offset=0] = 77
    metadata[unsafe_offset=1] = 77
    assert prodex_active_runtime_logs_v1(2, 0, -1, 0, -1, 0, 0, 0, 0, 0) == 4
    assert prodex_active_runtime_logs_v1(1, source, 4, names, 7, scratch, 10, target, 7, meta) == 1
    assert prodex_active_runtime_logs_v1(1, source, 4, names, 7, scratch, 11, target, 6, meta) == 1
    assert prodex_active_runtime_logs_v1(1, source + 1, 4, names, 7, scratch, 11, target, 7, meta) == 1
    rows[unsafe_offset=7] = 2
    assert prodex_active_runtime_logs_v1(1, source, 4, names, 7, scratch, 11, target, 7, meta) == 1
    rows[unsafe_offset=7] = 0
    pids[unsafe_offset=6] = 4_294_967_296
    assert prodex_active_runtime_logs_v1(1, source, 4, names, 7, scratch, 11, target, 7, meta) == 1
    for index in range(11):
        assert work[unsafe_offset=index] == 81
    for index in range(7):
        assert output[unsafe_offset=index] == 83
    assert metadata[unsafe_offset=0] == 77 and metadata[unsafe_offset=1] == 77

    var name = view("prodex-runtime-+2-")
    var prefix = view("prodex-runtime")
    var parser = parser_storage.unsafe_ptr()
    var dest = UInt64(Int(parser))
    assert prodex_runtime_log_pid_v1(1, UInt64(name.ptr), Int64(name.len), UInt64(prefix.ptr), Int64(prefix.len), dest) == 0
    assert parser[unsafe_offset=0] == 1 and parser[unsafe_offset=1] == 2
    parser[unsafe_offset=0] = 79
    parser[unsafe_offset=1] = 79
    assert prodex_runtime_log_pid_v1(2, 0, 0, 0, 0, 0) == 4
    assert prodex_runtime_log_pid_v1(1, 0, 1, UInt64(prefix.ptr), Int64(prefix.len), dest) == 1
    assert prodex_runtime_log_pid_v1(1, UInt64(name.ptr), Int64(name.len), UInt64(prefix.ptr), Int64(prefix.len), dest + 1) == 1
    assert parser[unsafe_offset=0] == 79 and parser[unsafe_offset=1] == 79
    dealloc(row_storage^)
    dealloc(pid_storage^)
    dealloc(scratch_storage^)
    dealloc(output_storage^)
    dealloc(metadata_storage^)
    dealloc(parser_storage^)
