# Direct selected-profile exact-match visibility ABI.
from std.memory.alloc import alloc, dealloc, Layout
from profile_schedule import prodex_runtime_profile_exact_visibility_v1


def main() raises:
    var first = StringSlice("main")
    var second = StringSlice("main-extra")
    var third = StringSlice("雪")
    var names = [first, second, third]
    var views_storage = alloc(Layout[UInt64](count=6))
    var output_storage = alloc(Layout[Int64](count=3))
    var views = views_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    for index in range(3):
        views[unsafe_offset=index * 2] = UInt64(UInt(Int(names[index].unsafe_ptr())))
        views[unsafe_offset=index * 2 + 1] = UInt64(names[index].byte_length())
    var names_address = UInt(Int(views))
    var output_address = UInt(Int(output))
    assert prodex_runtime_profile_exact_visibility_v1(
        1, names_address, 3,
        UInt(Int(third.unsafe_ptr())), Int64(third.byte_length()),
        output_address, 3,
    ) == 0
    assert output[unsafe_offset=0] == 0
    assert output[unsafe_offset=1] == 0
    assert output[unsafe_offset=2] == 1
    for index in range(3):
        output[unsafe_offset=index] = 79
    assert prodex_runtime_profile_exact_visibility_v1(
        2, names_address, 3,
        UInt(Int(third.unsafe_ptr())), Int64(third.byte_length()),
        output_address, 3,
    ) == 4
    assert prodex_runtime_profile_exact_visibility_v1(
        1, names_address, 3,
        UInt(Int(third.unsafe_ptr())), Int64(third.byte_length()),
        output_address, 2,
    ) == 1
    views[unsafe_offset=5] = 9223372036854775808
    assert prodex_runtime_profile_exact_visibility_v1(
        1, names_address, 3,
        UInt(Int(third.unsafe_ptr())), Int64(third.byte_length()),
        output_address, 3,
    ) == 1
    for index in range(3):
        assert output[unsafe_offset=index] == 79
    views[unsafe_offset=5] = UInt64(third.byte_length())
    assert prodex_runtime_profile_exact_visibility_v1(
        1, 0, 0, 0, 0, 0, 0
    ) == 0
    dealloc(output_storage^)
    dealloc(views_storage^)
