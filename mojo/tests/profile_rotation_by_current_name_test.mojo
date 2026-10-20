# Direct profile current-name equality and rotation ABI checks.
from std.memory.alloc import alloc, dealloc, Layout
from profile_schedule import prodex_runtime_profile_rotation_by_current_name_v2


def main() raises:
    var names_storage = alloc(Layout[UInt64](count=6))
    var priorities_storage = alloc(Layout[Int64](count=3))
    var output_storage = alloc(Layout[Int64](count=4))
    var written_storage = alloc(Layout[Int64](count=1))
    var names = names_storage.unsafe_ptr()
    var priorities = priorities_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var written = written_storage.unsafe_ptr()
    var first = StringSlice("a")
    var second = StringSlice("雪")
    var third = StringSlice("z")
    var words = [first, second, third]
    for i in range(3):
        names[unsafe_offset=i * 2] = UInt64(UInt(Int(words[i].unsafe_ptr())))
        names[unsafe_offset=i * 2 + 1] = UInt64(words[i].byte_length())
    priorities[unsafe_offset=0] = 2
    priorities[unsafe_offset=1] = 0
    priorities[unsafe_offset=2] = 0
    var names_addr = UInt(Int(names))
    var pri_addr = UInt(Int(priorities))
    var out_addr = UInt(Int(output))
    var count_addr = UInt(Int(written))
    assert prodex_runtime_profile_rotation_by_current_name_v2(
        2, names_addr, pri_addr, 3,
        UInt(Int(second.unsafe_ptr())), Int64(second.byte_length()),
        2, out_addr, 3, count_addr,
    ) == 0
    assert written[unsafe_offset=0] == 3
    assert output[unsafe_offset=0] == 1
    assert output[unsafe_offset=1] == 2
    assert output[unsafe_offset=2] == 0

    var missing = StringSlice("unknown")
    assert prodex_runtime_profile_rotation_by_current_name_v2(
        2, names_addr, pri_addr, 3,
        UInt(Int(missing.unsafe_ptr())), Int64(missing.byte_length()),
        2, out_addr, 3, count_addr,
    ) == 0
    assert output[unsafe_offset=0] == 1
    assert output[unsafe_offset=1] == 2
    assert output[unsafe_offset=2] == 0

    assert prodex_runtime_profile_rotation_by_current_name_v2(
        2, names_addr, pri_addr, 3,
        UInt(Int(second.unsafe_ptr())), Int64(second.byte_length()),
        0, out_addr, 3, count_addr,
    ) == 0
    assert written[unsafe_offset=0] == 2
    assert output[unsafe_offset=0] == 2
    assert output[unsafe_offset=1] == 0
    assert prodex_runtime_profile_rotation_by_current_name_v2(
        2, names_addr, pri_addr, 3,
        UInt(Int(missing.unsafe_ptr())), Int64(missing.byte_length()),
        1, out_addr, 4, count_addr,
    ) == 0
    assert written[unsafe_offset=0] == 4
    assert output[unsafe_offset=3] == 3

    for i in range(4):
        output[unsafe_offset=i] = 79
    written[unsafe_offset=0] = 91
    assert prodex_runtime_profile_rotation_by_current_name_v2(
        3, names_addr, pri_addr, 3,
        UInt(Int(second.unsafe_ptr())), Int64(second.byte_length()),
        2, out_addr, 3, count_addr,
    ) == 4
    assert prodex_runtime_profile_rotation_by_current_name_v2(
        2, names_addr, pri_addr, 3,
        UInt(Int(second.unsafe_ptr())), Int64(second.byte_length()),
        2, out_addr, 2, count_addr,
    ) == 1
    names[unsafe_offset=5] = UInt64(9223372036854775808)
    assert prodex_runtime_profile_rotation_by_current_name_v2(
        2, names_addr, pri_addr, 3,
        UInt(Int(second.unsafe_ptr())), Int64(second.byte_length()),
        2, out_addr, 3, count_addr,
    ) == 1
    assert written[unsafe_offset=0] == 91
    for i in range(4):
        assert output[unsafe_offset=i] == 79
    dealloc(written_storage^)
    dealloc(output_storage^)
    dealloc(priorities_storage^)
    dealloc(names_storage^)
