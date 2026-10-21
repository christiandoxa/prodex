# Direct compiled ownership of overlay state_ SQLite filename classification.
from std.memory.alloc import alloc, dealloc, Layout
from optional_tools_policy import prodex_optional_tool_overlay_rollout_name_v1


def main() raises:
    var output_store = alloc(Layout[Int64](count=1))
    var output = output_store.unsafe_ptr()
    var output_address = UInt(Int(output))
    var owned = StringSlice("state_123.sqlite-wal")
    var ignored = StringSlice("state_123.sqlite-wal.bak")
    var mixed = StringSlice("STATE_123.sqlite")

    assert prodex_optional_tool_overlay_rollout_name_v1(
        1, UInt(Int(owned.unsafe_ptr())),
        Int64(owned.byte_length()), output_address
    ) == 0
    assert output[unsafe_offset=0] == 1
    assert prodex_optional_tool_overlay_rollout_name_v1(
        1, UInt(Int(ignored.unsafe_ptr())),
        Int64(ignored.byte_length()), output_address
    ) == 0
    assert output[unsafe_offset=0] == 0
    assert prodex_optional_tool_overlay_rollout_name_v1(
        1, UInt(Int(mixed.unsafe_ptr())),
        Int64(mixed.byte_length()), output_address
    ) == 0
    assert output[unsafe_offset=0] == 0
    assert prodex_optional_tool_overlay_rollout_name_v1(
        1, 0, 0, output_address
    ) == 0
    assert output[unsafe_offset=0] == 0

    # Reject unsupported ABI and oversized input without changing output.
    output[unsafe_offset=0] = 77
    assert prodex_optional_tool_overlay_rollout_name_v1(
        2, UInt(Int(owned.unsafe_ptr())),
        Int64(owned.byte_length()), output_address
    ) == 4
    assert prodex_optional_tool_overlay_rollout_name_v1(
        1, 0, 100, output_address
    ) == 1
    assert prodex_optional_tool_overlay_rollout_name_v1(
        1, UInt(Int(owned.unsafe_ptr())), 1_048_577, output_address
    ) == 1
    assert output[unsafe_offset=0] == 77
    dealloc(output_store^)
