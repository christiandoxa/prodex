# Offline direct ABI test for persisted quota snapshot usage reconstruction.
from std.memory.alloc import alloc, dealloc, Layout
from runtime_state_quota import prodex_runtime_quota_snapshot_rebuild_v1


def main() raises:
    var facts_storage = alloc(Layout[Int64](count=6))
    var result_storage = alloc(Layout[Int64](count=6))
    var facts = facts_storage.unsafe_ptr()
    var result = result_storage.unsafe_ptr()
    # Ready primary window, unknown weekly window.
    facts[unsafe_offset=0] = 0
    facts[unsafe_offset=1] = 60
    facts[unsafe_offset=2] = 3600
    facts[unsafe_offset=3] = 4
    facts[unsafe_offset=4] = 9223372036854775807
    facts[unsafe_offset=5] = 9223372036854775807
    var address = UInt(Int(facts))
    var output_address = UInt(Int(result))
    assert prodex_runtime_quota_snapshot_rebuild_v1(
        1, address, 6, output_address, 6
    ) == 0
    assert result[unsafe_offset=0] == 1
    assert result[unsafe_offset=1] == 40
    assert result[unsafe_offset=2] == 1
    assert result[unsafe_offset=3] == 0
    assert result[unsafe_offset=4] == 0
    assert result[unsafe_offset=5] == 0

    facts[unsafe_offset=0] = 3
    facts[unsafe_offset=1] = -9223372036854775808
    facts[unsafe_offset=2] = 9223372036854775807
    facts[unsafe_offset=3] = 1
    facts[unsafe_offset=4] = 9223372036854775807
    facts[unsafe_offset=5] = 4000
    assert prodex_runtime_quota_snapshot_rebuild_v1(
        1, address, 6, output_address, 6
    ) == 0
    assert result[unsafe_offset=0] == 1
    assert result[unsafe_offset=1] == 100
    assert result[unsafe_offset=2] == 0
    assert result[unsafe_offset=3] == 1
    assert result[unsafe_offset=4] == 0
    assert result[unsafe_offset=5] == 1

    for index in range(6):
        result[unsafe_offset=index] = 77
    facts[unsafe_offset=3] = 5
    assert prodex_runtime_quota_snapshot_rebuild_v1(
        1, address, 6, output_address, 6
    ) == 1
    assert prodex_runtime_quota_snapshot_rebuild_v1(
        2, address, 6, output_address, 6
    ) == 4
    assert prodex_runtime_quota_snapshot_rebuild_v1(
        1, address, 6, output_address, 5
    ) == 1
    for index in range(6):
        assert result[unsafe_offset=index] == 77
    dealloc(result_storage^)
    dealloc(facts_storage^)
