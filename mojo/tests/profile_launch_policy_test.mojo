# Direct compiled profile launch policy ABI contract.
# Run: mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/profile_launch_policy_test.mojo
from std.memory import Pointer
from std.collections.list import List
from profile_schedule import (
    prodex_runtime_profile_next_selection_at_v1,
    prodex_runtime_profile_name_resolution_v1,
)


def main() raises:
    var timestamps = List[Int64]()
    timestamps.extend([Int64(-100), Int64(9), Int64(20)])
    var selected: Int64 = -99
    var selected_pointer = Pointer(to=selected)
    var address = UInt(Int(timestamps.unsafe_ptr()))
    var output_address = UInt(Int(selected_pointer))

    assert prodex_runtime_profile_next_selection_at_v1(
        1, address, 3, 2, output_address
    ) == 0
    assert selected == 21
    assert prodex_runtime_profile_next_selection_at_v1(
        1, address, 3, 24, output_address
    ) == 0
    assert selected == 24
    selected = -99
    assert prodex_runtime_profile_next_selection_at_v1(
        1, 0, 0, -100, output_address
    ) == 0
    assert selected == 1

    timestamps[2] = 9223372036854775807
    assert prodex_runtime_profile_next_selection_at_v1(
        1, address, 3, 0, output_address
    ) == 0
    assert selected == 9223372036854775807
    assert prodex_runtime_profile_next_selection_at_v1(
        2, address, 3, 0, output_address
    ) == 4
    assert prodex_runtime_profile_next_selection_at_v1(
        1, 0, 3, 0, output_address
    ) == 1
    assert selected == 9223372036854775807

    var resolution: Int64 = -1
    var resolution_address = UInt(Int(Pointer(to=resolution)))
    assert prodex_runtime_profile_name_resolution_v1(
        1, 1, 1, 1, 1, 2, resolution_address
    ) == 0
    assert resolution == 0
    assert prodex_runtime_profile_name_resolution_v1(
        1, 1, 0, 1, 1, 2, resolution_address
    ) == 0
    assert resolution == 3
    assert prodex_runtime_profile_name_resolution_v1(
        1, 0, 0, 1, 1, 1, resolution_address
    ) == 0
    assert resolution == 1
    assert prodex_runtime_profile_name_resolution_v1(
        1, 0, 0, 1, 0, 1, resolution_address
    ) == 0
    assert resolution == 4
    assert prodex_runtime_profile_name_resolution_v1(
        1, 0, 0, 0, 0, 1, resolution_address
    ) == 0
    assert resolution == 2
    assert prodex_runtime_profile_name_resolution_v1(
        1, 0, 0, 0, 0, 2, resolution_address
    ) == 0
    assert resolution == 5
    assert prodex_runtime_profile_name_resolution_v1(
        1, 0, 1, 0, 0, 2, resolution_address
    ) == 1
    assert prodex_runtime_profile_name_resolution_v1(
        2, 0, 0, 0, 0, 1, resolution_address
    ) == 4
    assert resolution == 5
