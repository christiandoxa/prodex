# Direct version selection, minimum comparison, and ABI validation; no process probes.
from std.memory.alloc import alloc, dealloc, Layout
from optional_tool_probe_version import prodex_optional_tool_probe_version_v1


def main() raises:
    var raw = StringSlice("rtk (vv0.50.0) build 999.0.0")
    var minimum = StringSlice("0.50.0")
    var output_store = alloc(Layout[Int64](count=3))
    var output = output_store.unsafe_ptr()
    var address = UInt(Int(raw.unsafe_ptr()))
    var minimum_address = UInt(Int(minimum.unsafe_ptr()))
    var result_address = UInt(Int(output))
    assert prodex_optional_tool_probe_version_v1(
        1, address, Int64(raw.byte_length()), minimum_address,
        Int64(minimum.byte_length()), result_address,
    ) == 0
    assert output[unsafe_offset=0] == 2
    var start = output[unsafe_offset=1]
    var end = output[unsafe_offset=2]
    assert end - start == 6
    assert raw.unsafe_ptr()[unsafe_offset=start] == 48
    assert raw.unsafe_ptr()[unsafe_offset=start + 1] == 46

    var newer = StringSlice("1.0.0")
    assert prodex_optional_tool_probe_version_v1(
        1, address, Int64(raw.byte_length()), UInt(Int(newer.unsafe_ptr())),
        Int64(newer.byte_length()), result_address,
    ) == 0
    assert output[unsafe_offset=0] == 1
    assert output[unsafe_offset=1] == start
    assert output[unsafe_offset=2] == end
    assert prodex_optional_tool_probe_version_v1(
        1, 0, 0, minimum_address, Int64(minimum.byte_length()), result_address,
    ) == 0
    assert output[unsafe_offset=0] == 0
    assert output[unsafe_offset=1] == 0
    assert output[unsafe_offset=2] == 0

    # ABI errors must not write the caller's result.
    output[unsafe_offset=0] = 77
    assert prodex_optional_tool_probe_version_v1(
        2, address, Int64(raw.byte_length()), minimum_address,
        Int64(minimum.byte_length()), result_address,
    ) == 4
    assert prodex_optional_tool_probe_version_v1(
        1, address, -1, minimum_address, Int64(minimum.byte_length()), result_address,
    ) == 1
    assert prodex_optional_tool_probe_version_v1(
        1, 0, 1, minimum_address, Int64(minimum.byte_length()), result_address,
    ) == 1
    assert prodex_optional_tool_probe_version_v1(
        1, address, Int64(raw.byte_length()), minimum_address, 1_048_577, result_address,
    ) == 1
    assert prodex_optional_tool_probe_version_v1(
        1, address, Int64(raw.byte_length()), minimum_address,
        Int64(minimum.byte_length()), result_address + 1,
    ) == 1
    assert output[unsafe_offset=0] == 77
    dealloc(output_store^)
