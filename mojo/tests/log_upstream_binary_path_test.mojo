# Direct read-only precedence and trimming for seven upstream binary path fields.
from std.memory.alloc import alloc, dealloc, Layout
from log_semantics import prodex_mojo_upstream_binary_path_plan_v1


def main() raises:
    var fields_store = alloc(Layout[UInt64](count=21))
    var output_store = alloc(Layout[Int64](count=3))
    var fields = fields_store.unsafe_ptr()
    var output = output_store.unsafe_ptr()
    for index in range(21):
        fields[unsafe_offset=index] = 0
    var rows = UInt(Int(fields))
    var answer = UInt(Int(output))
    var empty = StringSlice("  ")
    var file_path = StringSlice("  /tmp/metadata.json  ")
    var first = StringSlice(" x ")
    var later = StringSlice("path/ignored")

    # A present-but-empty higher-priority key must block all later paths.
    fields[unsafe_offset=0] = UInt64(UInt(Int(empty.unsafe_ptr())))
    fields[unsafe_offset=1] = UInt64(empty.byte_length())
    fields[unsafe_offset=2] = 1
    fields[unsafe_offset=3] = UInt64(UInt(Int(file_path.unsafe_ptr())))
    fields[unsafe_offset=4] = UInt64(file_path.byte_length())
    fields[unsafe_offset=5] = 1
    fields[unsafe_offset=18] = UInt64(UInt(Int(later.unsafe_ptr())))
    fields[unsafe_offset=19] = UInt64(later.byte_length())
    fields[unsafe_offset=20] = 1
    assert prodex_mojo_upstream_binary_path_plan_v1(1, rows, 7, answer) == 0
    for index in range(3):
        assert output[unsafe_offset=index] == -1

    fields[unsafe_offset=0] = 0
    fields[unsafe_offset=1] = 0
    fields[unsafe_offset=2] = 0
    assert prodex_mojo_upstream_binary_path_plan_v1(1, rows, 7, answer) == 0
    assert output[unsafe_offset=0] == 1
    assert output[unsafe_offset=1] == 2
    assert output[unsafe_offset=2] == Int64(file_path.byte_length()) - 2

    fields[unsafe_offset=0] = UInt64(UInt(Int(first.unsafe_ptr())))
    fields[unsafe_offset=1] = UInt64(first.byte_length())
    fields[unsafe_offset=2] = 1
    assert prodex_mojo_upstream_binary_path_plan_v1(1, rows, 7, answer) == 0
    assert output[unsafe_offset=0] == 0
    assert output[unsafe_offset=1] == 1
    assert output[unsafe_offset=2] == 2

    output[unsafe_offset=0] = 77
    assert prodex_mojo_upstream_binary_path_plan_v1(2, rows, 7, answer) == 4
    assert prodex_mojo_upstream_binary_path_plan_v1(1, rows, 6, answer) == 1
    fields[unsafe_offset=19] = 1_048_577
    assert prodex_mojo_upstream_binary_path_plan_v1(1, rows, 7, answer) == 1
    assert output[unsafe_offset=0] == 77
    dealloc(output_store^)
    dealloc(fields_store^)
