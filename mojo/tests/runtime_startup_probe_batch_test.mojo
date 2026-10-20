# Direct compiled startup-probe batch ABI tests, including invalid late facts.
from std.memory.alloc import alloc, dealloc, Layout
from runtime_state_quota import prodex_runtime_state_startup_probe_batch_v1


def main() raises:
    var rows_storage = alloc(Layout[Int64](count=10))
    var output_storage = alloc(Layout[Int64](count=5))
    var remaining_storage = alloc(Layout[Int64](count=1))
    var rows = rows_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var remaining = remaining_storage.unsafe_ptr()
    for index in range(10):
        rows[unsafe_offset=index] = 0
    rows[unsafe_offset=0] = 1
    rows[unsafe_offset=3] = 1
    var source = UInt(Int(rows))
    var destination = UInt(Int(output))
    var rest = UInt(Int(remaining))
    assert prodex_runtime_state_startup_probe_batch_v1(
        1, source, 5, 2, destination, 5, rest,
    ) == 0
    assert output[unsafe_offset=0] == 0
    assert output[unsafe_offset=1] == 0
    assert output[unsafe_offset=2] == 1
    assert output[unsafe_offset=3] == 1
    assert output[unsafe_offset=4] == 0
    assert remaining[unsafe_offset=0] == 0
    for index in range(5):
        output[unsafe_offset=index] = 71
    remaining[unsafe_offset=0] = 81
    assert prodex_runtime_state_startup_probe_batch_v1(
        2, source, 5, 2, destination, 5, rest,
    ) == 4
    assert prodex_runtime_state_startup_probe_batch_v1(
        1, source, 5, 2, destination, 4, rest,
    ) == 1
    rows[unsafe_offset=9] = 2
    assert prodex_runtime_state_startup_probe_batch_v1(
        1, source, 5, 2, destination, 5, rest,
    ) == 1
    for index in range(5):
        assert output[unsafe_offset=index] == 71
    assert remaining[unsafe_offset=0] == 81
    assert prodex_runtime_state_startup_probe_batch_v1(
        1, 0, 0, 4, destination, 5, rest,
    ) == 0
    assert remaining[unsafe_offset=0] == 4
    dealloc(remaining_storage^)
    dealloc(output_storage^)
    dealloc(rows_storage^)
