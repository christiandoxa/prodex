# Direct compiled ABI for sensitive upstream payload string budget/truncation.
from std.memory.alloc import alloc, dealloc, Layout
from log_semantics import (
    prodex_mojo_upstream_payload_string_budget_v1,
    prodex_mojo_upstream_payload_truncate_v1,
)


def main() raises:
    var inputs_storage = alloc(Layout[UInt64](count=6))
    var flags_storage = alloc(Layout[Int64](count=6))
    var remaining_storage = alloc(Layout[UInt64](count=1))
    var inputs = inputs_storage.unsafe_ptr()
    var flags = flags_storage.unsafe_ptr()
    var remaining = remaining_storage.unsafe_ptr()
    inputs[unsafe_offset=0] = 900
    inputs[unsafe_offset=1] = 200
    inputs[unsafe_offset=2] = 100
    inputs[unsafe_offset=3] = 30
    inputs[unsafe_offset=4] = 0
    inputs[unsafe_offset=5] = 20
    var source = UInt(Int(inputs))
    var dest = UInt(Int(flags))
    var rest = UInt(Int(remaining))
    assert prodex_mojo_upstream_payload_string_budget_v1(
        1, source, 6, 1024, dest, 6, rest,
    ) == 0
    assert flags[unsafe_offset=0] == 0
    assert flags[unsafe_offset=1] == 1
    assert flags[unsafe_offset=2] == 0
    assert flags[unsafe_offset=3] == 1
    assert flags[unsafe_offset=4] == 0
    assert flags[unsafe_offset=5] == 0
    assert remaining[unsafe_offset=0] == 4

    for index in range(6):
        flags[unsafe_offset=index] = 77
    remaining[unsafe_offset=0] = 91
    assert prodex_mojo_upstream_payload_string_budget_v1(
        2, source, 6, 1024, dest, 6, rest,
    ) == 4
    assert prodex_mojo_upstream_payload_string_budget_v1(
        1, source, 6, 1024, dest, 5, rest,
    ) == 1
    for index in range(6):
        assert flags[unsafe_offset=index] == 77
    assert remaining[unsafe_offset=0] == 91

    var span_storage = alloc(Layout[Int64](count=2))
    var span = span_storage.unsafe_ptr()
    var result = UInt(Int(span))
    var text = StringSlice("abé🙂")
    var address = UInt(Int(text.unsafe_ptr()))
    assert prodex_mojo_upstream_payload_truncate_v1(
        1, address, Int64(text.byte_length()), 3, result
    ) == 0
    assert span[unsafe_offset=0] == 2
    assert span[unsafe_offset=1] == 1
    assert prodex_mojo_upstream_payload_truncate_v1(
        1, address, Int64(text.byte_length()), 4, result
    ) == 0
    assert span[unsafe_offset=0] == 4
    assert span[unsafe_offset=1] == 1
    assert prodex_mojo_upstream_payload_truncate_v1(
        1, address, Int64(text.byte_length()), 8, result
    ) == 0
    assert span[unsafe_offset=0] == 8
    assert span[unsafe_offset=1] == 0
    span[unsafe_offset=0] = 79
    span[unsafe_offset=1] = 80
    assert prodex_mojo_upstream_payload_truncate_v1(
        2, address, Int64(text.byte_length()), 4, result
    ) == 4
    assert prodex_mojo_upstream_payload_truncate_v1(
        1, 0, Int64(text.byte_length()), 4, result
    ) == 1
    assert span[unsafe_offset=0] == 79
    assert span[unsafe_offset=1] == 80
    dealloc(span_storage^)
    dealloc(remaining_storage^)
    dealloc(flags_storage^)
    dealloc(inputs_storage^)
