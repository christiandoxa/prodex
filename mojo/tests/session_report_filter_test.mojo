# Offline ABI regression: compiled session report gate precedence and query.
from std.memory.alloc import alloc, dealloc, Layout
from session_selector import prodex_session_report_filter_v1


def main() raises:
    var current_profile = StringSlice("Main")
    var requested_profile = StringSlice("Main")
    var query = StringSlice(" istanbul  ")
    var field = StringSlice("istanbul search")
    var empty = StringSlice("")
    var views = [
        current_profile, requested_profile, query,
        field, empty, empty, empty, empty, empty,
    ]
    var flags_store = alloc(Layout[Int64](count=7))
    var views_store = alloc(Layout[UInt64](count=18))
    var output_store = alloc(Layout[Int64](count=1))
    var flags = flags_store.unsafe_ptr()
    var pointers = views_store.unsafe_ptr()
    var output = output_store.unsafe_ptr()
    for i in range(7):
        flags[unsafe_offset=i] = 0
    flags[unsafe_offset=4] = 1
    flags[unsafe_offset=5] = 1
    flags[unsafe_offset=6] = 1
    for i in range(9):
        pointers[unsafe_offset=i * 2] = UInt64(UInt(Int(views[i].unsafe_ptr())))
        pointers[unsafe_offset=i * 2 + 1] = UInt64(views[i].byte_length())

    var facts_address = UInt(Int(flags))
    var views_address = UInt(Int(pointers))
    var output_address = UInt(Int(output))
    assert prodex_session_report_filter_v1(
        1, facts_address, 7, views_address, 9, output_address
    ) == 0
    assert output[unsafe_offset=0] == 1
    flags[unsafe_offset=0] = 1
    flags[unsafe_offset=1] = 0
    assert prodex_session_report_filter_v1(
        1, facts_address, 7, views_address, 9, output_address
    ) == 0
    assert output[unsafe_offset=0] == 0
    flags[unsafe_offset=1] = 1
    flags[unsafe_offset=3] = 1
    assert prodex_session_report_filter_v1(
        1, facts_address, 7, views_address, 9, output_address
    ) == 0
    assert output[unsafe_offset=0] == 0
    flags[unsafe_offset=2] = 1
    assert prodex_session_report_filter_v1(
        1, facts_address, 7, views_address, 9, output_address
    ) == 0
    assert output[unsafe_offset=0] == 1

    var different = StringSlice("main")
    pointers[unsafe_offset=2] = UInt64(UInt(Int(different.unsafe_ptr())))
    assert prodex_session_report_filter_v1(
        1, facts_address, 7, views_address, 9, output_address
    ) == 0
    assert output[unsafe_offset=0] == 0
    pointers[unsafe_offset=2] = UInt64(UInt(Int(requested_profile.unsafe_ptr())))
    flags[unsafe_offset=4] = 0
    assert prodex_session_report_filter_v1(
        1, facts_address, 7, views_address, 9, output_address
    ) == 0
    assert output[unsafe_offset=0] == 0

    # Invalid ABI and facts leave the output untouched.
    output[unsafe_offset=0] = 77
    assert prodex_session_report_filter_v1(
        2, facts_address, 7, views_address, 9, output_address
    ) == 4
    flags[unsafe_offset=0] = 2
    assert prodex_session_report_filter_v1(
        1, facts_address, 7, views_address, 9, output_address
    ) == 1
    assert output[unsafe_offset=0] == 77
    dealloc(output_store^)
    dealloc(views_store^)
    dealloc(flags_store^)
