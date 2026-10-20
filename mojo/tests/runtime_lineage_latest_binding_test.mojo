# Direct ABI: exact parsed response/profile binding and newest signed time.
# Run: mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_lineage_latest_binding_test.mojo
from std.memory.alloc import alloc, dealloc, Layout
from runtime_lineage import prodex_runtime_lineage_latest_turn_binding_v1


def main() raises:
    var views_storage = alloc(Layout[UInt64](count=20))
    var times_storage = alloc(Layout[Int64](count=4))
    var selected_storage = alloc(Layout[Int64](count=1))
    var views = views_storage.unsafe_ptr()
    var times = times_storage.unsafe_ptr()
    var selected = selected_storage.unsafe_ptr()
    for i in range(20):
        views[unsafe_offset=i] = 0
    var response = StringSlice("resp:雪")
    var main_profile = StringSlice("雪")
    var other_profile = StringSlice("other")
    var query = UInt(Int(response.unsafe_ptr()))
    var profile = UInt(Int(main_profile.unsafe_ptr()))
    for i in range(4):
        views[unsafe_offset=i * 5 + 2] = UInt64(profile)
        views[unsafe_offset=i * 5 + 3] = UInt64(main_profile.byte_length())
    for i in [0, 1, 3]:
        views[unsafe_offset=i * 5] = UInt64(query)
        views[unsafe_offset=i * 5 + 1] = UInt64(response.byte_length())
        views[unsafe_offset=i * 5 + 4] = 1
    views[unsafe_offset=7] = UInt64(UInt(Int(other_profile.unsafe_ptr())))
    views[unsafe_offset=8] = UInt64(other_profile.byte_length())
    times[unsafe_offset=0] = 10
    times[unsafe_offset=1] = 100
    times[unsafe_offset=2] = 9223372036854775807
    times[unsafe_offset=3] = 10
    var source = UInt(Int(views))
    var timestamp_ptr = UInt(Int(times))
    var destination = UInt(Int(selected))
    assert prodex_runtime_lineage_latest_turn_binding_v1(
        1, source, timestamp_ptr, 4, query, Int64(response.byte_length()),
        profile, Int64(main_profile.byte_length()), 1, destination,
    ) == 0
    assert selected[unsafe_offset=0] == 3
    assert prodex_runtime_lineage_latest_turn_binding_v1(
        1, source, timestamp_ptr, 4, query, Int64(response.byte_length()),
        0, 0, 0, destination,
    ) == 0
    assert selected[unsafe_offset=0] == 1
    var missing = StringSlice("not-found")
    assert prodex_runtime_lineage_latest_turn_binding_v1(
        1, source, timestamp_ptr, 4,
        UInt(Int(missing.unsafe_ptr())), Int64(missing.byte_length()),
        0, 0, 0, destination,
    ) == 0
    assert selected[unsafe_offset=0] == -1

    selected[unsafe_offset=0] = 91
    assert prodex_runtime_lineage_latest_turn_binding_v1(
        2, source, timestamp_ptr, 4, query, Int64(response.byte_length()),
        profile, Int64(main_profile.byte_length()), 1, destination,
    ) == 4
    assert prodex_runtime_lineage_latest_turn_binding_v1(
        1, 0, timestamp_ptr, 4, query, Int64(response.byte_length()),
        profile, Int64(main_profile.byte_length()), 1, destination,
    ) == 1
    assert prodex_runtime_lineage_latest_turn_binding_v1(
        1, source, timestamp_ptr, 4, query, Int64(response.byte_length()),
        profile, Int64(main_profile.byte_length()), 2, destination,
    ) == 1
    views[unsafe_offset=19] = 2
    assert prodex_runtime_lineage_latest_turn_binding_v1(
        1, source, timestamp_ptr, 4, query, Int64(response.byte_length()),
        profile, Int64(main_profile.byte_length()), 1, destination,
    ) == 1
    assert selected[unsafe_offset=0] == 91

    assert prodex_runtime_lineage_latest_turn_binding_v1(
        1, 0, 0, 0, query, Int64(response.byte_length()),
        0, 0, 0, destination,
    ) == 0
    assert selected[unsafe_offset=0] == -1
    dealloc(selected_storage^)
    dealloc(times_storage^)
    dealloc(views_storage^)
