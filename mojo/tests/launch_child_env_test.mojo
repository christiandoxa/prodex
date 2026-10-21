# Direct child-process environment policy classification ABI tests.
from std.memory.alloc import alloc, dealloc, Layout
from launch_child_env import prodex_launch_child_env_flags_v1


def main() raises:
    var first = StringSlice("CODEX_SANDBOX")
    var second = StringSlice("DYLD_CUSTOM")
    var third = StringSlice("PRODEX_ALLOW_UNSAFE_CHILD_ENV")
    var fourth = StringSlice("dyld_other")
    var names = [first, second, third, fourth]
    var views_storage = alloc(Layout[UInt64](count=15))
    var flags_storage = alloc(Layout[Int64](count=5))
    var bypass_storage = alloc(Layout[Int64](count=1))
    var views = views_storage.unsafe_ptr()
    var flags = flags_storage.unsafe_ptr()
    var bypass = bypass_storage.unsafe_ptr()
    for index in range(4):
        views[unsafe_offset=index * 3] = UInt64(UInt(Int(names[index].unsafe_ptr())))
        views[unsafe_offset=index * 3 + 1] = UInt64(names[index].byte_length())
        views[unsafe_offset=index * 3 + 2] = 1
    views[unsafe_offset=12] = 0
    views[unsafe_offset=13] = 0
    views[unsafe_offset=14] = 0
    var source_address = UInt(Int(views))
    var flags_address = UInt(Int(flags))
    var bypass_address = UInt(Int(bypass))

    assert prodex_launch_child_env_flags_v1(
        1, source_address, 5, flags_address, 5, bypass_address
    ) == 0
    assert flags[unsafe_offset=0] == 1
    assert flags[unsafe_offset=1] == 2
    assert flags[unsafe_offset=2] == 4
    assert flags[unsafe_offset=3] == 0
    assert flags[unsafe_offset=4] == 0
    assert bypass[unsafe_offset=0] == 1

    # An invalid ABI or input must leave outputs unchanged.
    bypass[unsafe_offset=0] = 99
    assert prodex_launch_child_env_flags_v1(
        2, source_address, 5, flags_address, 5, bypass_address
    ) == 4
    assert prodex_launch_child_env_flags_v1(
        1, source_address, 5, flags_address, 4, bypass_address
    ) == 1
    views[unsafe_offset=14] = 2
    assert prodex_launch_child_env_flags_v1(
        1, source_address, 5, flags_address, 5, bypass_address
    ) == 1
    assert bypass[unsafe_offset=0] == 99
    views[unsafe_offset=14] = 0
    assert prodex_launch_child_env_flags_v1(
        1, 0, 0, 0, 0, bypass_address
    ) == 0
    assert bypass[unsafe_offset=0] == 0
    dealloc(bypass_storage^)
    dealloc(flags_storage^)
    dealloc(views_storage^)
