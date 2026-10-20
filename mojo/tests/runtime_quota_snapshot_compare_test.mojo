# Direct compiled state snapshot comparison and persistence ABI regression.
from std.memory.alloc import alloc, dealloc, Layout
from runtime_state_quota import prodex_runtime_state_quota_snapshot_compare_v1


def main() raises:
    var fact_storage = alloc(Layout[Int64](count=15))
    var result_storage = alloc(Layout[Int64](count=2))
    var facts = fact_storage.unsafe_ptr()
    var output = result_storage.unsafe_ptr()
    for index in range(15):
        facts[unsafe_offset=index] = 0
    facts[unsafe_offset=0] = 90
    facts[unsafe_offset=4] = 90
    facts[unsafe_offset=2] = 80
    facts[unsafe_offset=6] = 80
    facts[unsafe_offset=8] = 1
    facts[unsafe_offset=9] = 1
    facts[unsafe_offset=10] = 1
    facts[unsafe_offset=11] = 1
    facts[unsafe_offset=12] = 100
    facts[unsafe_offset=13] = 110
    facts[unsafe_offset=14] = 60
    var source = UInt(Int(facts))
    var dest = UInt(Int(output))
    assert prodex_runtime_state_quota_snapshot_compare_v1(1, 1, source, 15, dest) == 0
    assert output[unsafe_offset=0] == 1
    assert output[unsafe_offset=1] == 0
    facts[unsafe_offset=13] = 161
    assert prodex_runtime_state_quota_snapshot_compare_v1(1, 1, source, 15, dest) == 0
    assert output[unsafe_offset=0] == 1
    assert output[unsafe_offset=1] == 1
    facts[unsafe_offset=4] = 89
    assert prodex_runtime_state_quota_snapshot_compare_v1(1, 0, source, 15, dest) == 0
    assert output[unsafe_offset=0] == 0
    assert output[unsafe_offset=1] == 1
    facts[unsafe_offset=4] = 90
    facts[unsafe_offset=8] = 0
    assert prodex_runtime_state_quota_snapshot_compare_v1(1, 1, source, 15, dest) == 0
    assert output[unsafe_offset=0] == 0
    assert output[unsafe_offset=1] == 1
    output[unsafe_offset=0] = 77
    output[unsafe_offset=1] = 78
    assert prodex_runtime_state_quota_snapshot_compare_v1(2, 1, source, 15, dest) == 4
    assert prodex_runtime_state_quota_snapshot_compare_v1(1, 2, source, 15, dest) == 1
    facts[unsafe_offset=11] = 2
    assert prodex_runtime_state_quota_snapshot_compare_v1(1, 1, source, 15, dest) == 1
    assert output[unsafe_offset=0] == 77
    assert output[unsafe_offset=1] == 78
    dealloc(result_storage^)
    dealloc(fact_storage^)
