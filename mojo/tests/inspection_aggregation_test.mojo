from std.memory.alloc import alloc, dealloc, Layout
from governance_inspection import GovernanceInspectionSourceFacts, prodex_mojo_governance_inspection_aggregate_v1


def main() raises:
    var ss = alloc(Layout[GovernanceInspectionSourceFacts](count=2))
    var ks = alloc(Layout[UInt8](count=12))
    var ms = alloc(Layout[UInt8](count=12))
    var os = alloc(Layout[Int64](count=15))
    var sources = ss.unsafe_ptr()
    var kinds = ks.unsafe_ptr()
    var masked = ms.unsafe_ptr()
    var output = os.unsafe_ptr()
    var table = UInt(Int(sources))
    var findings = UInt(Int(kinds))
    var masks = UInt(Int(masked))
    var target = UInt(Int(output))
    kinds[unsafe_offset=0] = 0
    kinds[unsafe_offset=1] = 8
    masked[unsafe_offset=0] = 8
    masked[unsafe_offset=1] = 0
    sources[unsafe_offset=0] = GovernanceInspectionSourceFacts(0, findings, 1, masks, 1, 0, 0)
    sources[unsafe_offset=1] = GovernanceInspectionSourceFacts(2, findings + 1, 1, masks + 1, 1, 0, 0)
    assert prodex_mojo_governance_inspection_aggregate_v1(6, table, 2, 0, target) == 0
    assert output[unsafe_offset=0] == 1
    assert output[unsafe_offset=1] == 3
    assert output[unsafe_offset=2] == 0
    for index in range(12):
        kinds[unsafe_offset=index] = UInt8(index)
        masked[unsafe_offset=index] = UInt8(11 - index)
    sources[unsafe_offset=0] = GovernanceInspectionSourceFacts(0, findings, 12, masks, 12, 0, 0)
    assert prodex_mojo_governance_inspection_aggregate_v1(6, table, 1, 0, target) == 0
    assert output[unsafe_offset=0] == 0 and output[unsafe_offset=1] == 3
    assert output[unsafe_offset=2] == 12
    for index in range(12):
        assert output[unsafe_offset=index + 3] == Int64(index)
    assert prodex_mojo_governance_inspection_aggregate_v1(6, 0, 0, 2, target) == 0
    assert output[unsafe_offset=0] == 2 and output[unsafe_offset=1] == 2
    assert output[unsafe_offset=2] == 0
    for index in range(15):
        output[unsafe_offset=index] = 73
    assert prodex_mojo_governance_inspection_aggregate_v1(7, 0, 0, 0, target) == 4
    assert prodex_mojo_governance_inspection_aggregate_v1(6, 0, 1, 0, target) == 1
    assert prodex_mojo_governance_inspection_aggregate_v1(6, table + 1, 1, 0, target) == 1
    assert prodex_mojo_governance_inspection_aggregate_v1(6, table, -1, 0, target) == 1
    assert prodex_mojo_governance_inspection_aggregate_v1(6, table, 9, 0, target) == 1
    assert prodex_mojo_governance_inspection_aggregate_v1(6, table, 1, 4, target) == 1
    sources[unsafe_offset=0] = GovernanceInspectionSourceFacts(0, findings, 12, masks, 13, 0, 0)
    assert prodex_mojo_governance_inspection_aggregate_v1(6, table, 1, 0, target) == 1
    sources[unsafe_offset=0] = GovernanceInspectionSourceFacts(3, findings, 12, masks, 12, 0, 0)
    assert prodex_mojo_governance_inspection_aggregate_v1(6, table, 1, 0, target) == 1
    sources[unsafe_offset=0] = GovernanceInspectionSourceFacts(0, findings, 12, masks, 12, 0, 0)
    kinds[unsafe_offset=11] = 12
    assert prodex_mojo_governance_inspection_aggregate_v1(6, table, 1, 0, target) == 1
    sources[unsafe_offset=0] = GovernanceInspectionSourceFacts(0, findings, 65_537, 0, 0, 0, 0)
    assert prodex_mojo_governance_inspection_aggregate_v1(6, table, 1, 0, target) == 5
    sources[unsafe_offset=0] = GovernanceInspectionSourceFacts(0, 0, 0, 0, 0, 33, 0)
    assert prodex_mojo_governance_inspection_aggregate_v1(6, table, 1, 0, target) == 5
    sources[unsafe_offset=0] = GovernanceInspectionSourceFacts(0, 0, 0, 0, 0, 0, 33)
    assert prodex_mojo_governance_inspection_aggregate_v1(6, table, 1, 0, target) == 5
    for index in range(15):
        assert output[unsafe_offset=index] == 73
    dealloc(ss^)
    dealloc(ks^)
    dealloc(ms^)
    dealloc(os^)
