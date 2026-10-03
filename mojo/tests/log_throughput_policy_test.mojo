from std.collections.list import List

from prodex_core.log_throughput_policy import prodex_log_retention_candidates_v1


def test_expiry_uses_a_strict_cutoff() -> None:
    var candidates = List[UInt64]()
    candidates.append(1)
    candidates.append(UInt64(0x8000000000000063))
    candidates.append(1)
    candidates.append(0)
    candidates.append(5)
    candidates.append(1)
    candidates.append(UInt64(0x8000000000000064))
    candidates.append(1)
    candidates.append(5)
    candidates.append(5)

    var names: String = "a.logb.log"
    var policy = List[UInt64]()
    policy.append(UInt64(0x8000000000000064))
    policy.append(0)
    policy.append(0)
    policy.append(0)
    policy.append(0)

    var output = List[UInt64]()
    output.append(0)
    output.append(0)
    output.append(0)
    output.append(0)
    var result = prodex_log_retention_candidates_v1(
        1,
        6,
        2,
        UInt(Int(candidates.unsafe_ptr())),
        UInt(Int(names.unsafe_ptr())),
        10,
        UInt(Int(policy.unsafe_ptr())),
        UInt(Int(output.unsafe_ptr())),
    )
    assert result == 0
    assert output[0] == 1
    assert output[1] == 0


def test_budget_selection_uses_oldest_name_tiebreak() -> None:
    var candidates = List[UInt64]()
    candidates.append(6)
    candidates.append(1)
    candidates.append(1)
    candidates.append(0)
    candidates.append(5)
    candidates.append(5)
    candidates.append(1)
    candidates.append(1)
    candidates.append(5)
    candidates.append(5)

    var names: String = "z.loga.log"
    var policy = List[UInt64]()
    policy.append(0)
    policy.append(2)
    policy.append(1)
    policy.append(11)
    policy.append(6)

    var output = List[UInt64]()
    for _ in range(4):
        output.append(0)
    var result = prodex_log_retention_candidates_v1(
        1,
        7,
        2,
        UInt(Int(candidates.unsafe_ptr())),
        UInt(Int(names.unsafe_ptr())),
        10,
        UInt(Int(policy.unsafe_ptr())),
        UInt(Int(output.unsafe_ptr())),
    )
    assert result == 0
    assert output[0] == 1
    assert output[1] == UInt64(0xFFFFFFFFFFFFFFFF)


def main() raises:
    test_expiry_uses_a_strict_cutoff()
    test_budget_selection_uses_oldest_name_tiebreak()
