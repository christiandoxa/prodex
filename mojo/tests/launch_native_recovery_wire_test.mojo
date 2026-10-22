from std.memory.alloc import alloc, dealloc, Layout
from launch_args_common import LaunchArgView
from launch_config import (
    prodex_launch_native_recovery_wire_v1, prodex_launch_in_place_recovery_v2,
    NATIVE_GOAL_PRESENT, NATIVE_BUDGET_PRESENT, NATIVE_BUDGET_NULL,
    NATIVE_BUDGET_INTEGER, NATIVE_USED_INTEGER, NATIVE_EXPECTED_GOAL,
)


def view(text: StringSlice) -> LaunchArgView:
    return LaunchArgView(UInt64(Int(text.unsafe_ptr())), UInt64(text.byte_length()), 1)


def action(thread: StringSlice, goal: StringSlice, flags: Int64, limit: Int64 = 100, used: Int64 = 10) -> Int64:
    var storage = alloc(Layout[LaunchArgView](count=2))
    var statuses = storage.unsafe_ptr()
    statuses[unsafe_offset=0] = view(thread)
    statuses[unsafe_offset=1] = view(goal)
    var result = prodex_launch_native_recovery_wire_v1(1, UInt64(Int(statuses)), 2, flags, limit, used)
    dealloc(storage^)
    return result


def main() raises:
    var bounded = NATIVE_GOAL_PRESENT | NATIVE_BUDGET_PRESENT | NATIVE_BUDGET_INTEGER | NATIVE_USED_INTEGER | NATIVE_EXPECTED_GOAL
    var idle = StringSlice("idle")
    var blocked = StringSlice("blocked")
    for thread in [idle, StringSlice("systemError")]:
        for goal in [StringSlice("active"), blocked, StringSlice("usageLimited")]:
            assert action(thread, goal, bounded) == 2
        for goal in [StringSlice("paused"), StringSlice("budgetLimited"), StringSlice("complete"), StringSlice("Active"), StringSlice("blocked "), StringSlice("未知"), StringSlice("")]:
            assert action(thread, goal, bounded) == 1
    for thread in [StringSlice("active"), StringSlice("notLoaded"), StringSlice("Idle"), StringSlice("systemError "), StringSlice("未知"), StringSlice("")]:
        assert action(thread, blocked, bounded) == 0
    assert action(idle, blocked, bounded, 100, 0) == 2
    assert action(idle, blocked, bounded, 100, 99) == 2
    assert action(idle, blocked, bounded, 9223372036854775807, 9223372036854775806) == 2
    assert action(idle, blocked, bounded, 100, 100) == 1
    assert action(idle, blocked, bounded, 100, 101) == 1
    assert action(idle, blocked, bounded, 0, 0) == 1
    assert action(idle, blocked, bounded, -1, 0) == 1
    assert action(idle, blocked, bounded, 100, -1) == 1
    assert action(idle, blocked, bounded, -9223372036854775808, -9223372036854775808) == 1
    assert action(idle, blocked, bounded & ~NATIVE_BUDGET_INTEGER) == 1
    assert action(idle, blocked, bounded & ~NATIVE_USED_INTEGER) == 1
    assert action(idle, blocked, NATIVE_GOAL_PRESENT | NATIVE_EXPECTED_GOAL, 0, -10) == 2
    assert action(idle, blocked, NATIVE_GOAL_PRESENT | NATIVE_EXPECTED_GOAL | NATIVE_BUDGET_PRESENT | NATIVE_BUDGET_NULL, 0, -10) == 2
    assert action(idle, StringSlice(""), NATIVE_EXPECTED_GOAL) == 1
    assert action(idle, StringSlice(""), 0) == 3

    var storage = alloc(Layout[LaunchArgView](count=2))
    var statuses = storage.unsafe_ptr()
    statuses[unsafe_offset=0] = view(idle)
    statuses[unsafe_offset=1] = LaunchArgView(0, 0, 0)
    var address = UInt64(Int(statuses))
    assert prodex_launch_native_recovery_wire_v1(1, address, 2, NATIVE_GOAL_PRESENT, 0, 0) == 1
    assert prodex_launch_native_recovery_wire_v1(1, address, 2, 0, 0, 0) == 3
    statuses[unsafe_offset=0] = LaunchArgView(0, 0, 0)
    assert prodex_launch_native_recovery_wire_v1(1, address, 2, 0, 0, 0) == 0
    assert prodex_launch_native_recovery_wire_v1(2, 0, 0, 0, 0, 0) == -3
    assert prodex_launch_native_recovery_wire_v1(1, 0, 2, 0, 0, 0) == -2
    assert prodex_launch_native_recovery_wire_v1(1, address + 1, 2, 0, 0, 0) == -2
    assert prodex_launch_native_recovery_wire_v1(1, address, -1, 0, 0, 0) == -2
    assert prodex_launch_native_recovery_wire_v1(1, address, 1, 0, 0, 0) == -2
    assert prodex_launch_native_recovery_wire_v1(1, address, 3, 0, 0, 0) == -2
    assert prodex_launch_native_recovery_wire_v1(1, address, 2, -1, 0, 0) == -2
    assert prodex_launch_native_recovery_wire_v1(1, address, 2, 64, 0, 0) == -2
    statuses[unsafe_offset=0] = LaunchArgView(0, 1, 1)
    assert prodex_launch_native_recovery_wire_v1(1, address, 2, 0, 0, 0) == -2
    statuses[unsafe_offset=0] = view(idle)
    statuses[unsafe_offset=0].length = UInt64(9223372036854775808)
    assert prodex_launch_native_recovery_wire_v1(1, address, 2, 0, 0, 0) == -2
    statuses[unsafe_offset=0] = view(idle)
    statuses[unsafe_offset=0].valid_utf8 = 2
    assert prodex_launch_native_recovery_wire_v1(1, address, 2, 0, 0, 0) == -2
    statuses[unsafe_offset=0].valid_utf8 = 0
    assert prodex_launch_native_recovery_wire_v1(1, address, 2, 0, 0, 0) == -2
    dealloc(storage^)

    # The wire projection delegates to the existing action owner.
    assert prodex_launch_in_place_recovery_v2(2, 1, 1, 1, 2, 1) == 2
    assert prodex_launch_in_place_recovery_v2(2, 0, 1, 1, 2, 1) == 0
