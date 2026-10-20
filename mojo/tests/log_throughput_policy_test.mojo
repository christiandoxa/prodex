from std.collections.list import List
from std.memory.alloc import alloc, dealloc, Layout

from prodex_core.log_throughput_policy import (
    prodex_log_retention_candidates_v1, prodex_log_retention_policy_v1,
)


# The allocations outlive the integer-address ABI call, including under optimization.
def call_retention(operation: Int64, candidates: List[UInt64], names: String, policy: List[UInt64]) -> List[UInt64]:
    var candidate_storage = alloc(Layout[UInt64](count=10))
    var name_storage = alloc(Layout[UInt8](count=10))
    var policy_storage = alloc(Layout[UInt64](count=5))
    var output_storage = alloc(Layout[UInt64](count=4))
    var candidate_ptr = candidate_storage.unsafe_ptr()
    var name_ptr = name_storage.unsafe_ptr()
    var policy_ptr = policy_storage.unsafe_ptr()
    var output_ptr = output_storage.unsafe_ptr()
    for index in range(4):
        output_ptr[unsafe_offset=index] = 0
    for index in range(10):
        candidate_ptr[unsafe_offset=index] = candidates[index]
        name_ptr[unsafe_offset=index] = names.unsafe_ptr()[unsafe_offset=index]
    for index in range(5):
        policy_ptr[unsafe_offset=index] = policy[index]
    var result = prodex_log_retention_candidates_v1(1, operation, 2,
        UInt(Int(candidate_ptr)), UInt(Int(name_ptr)), 10,
        UInt(Int(policy_ptr)), UInt(Int(output_ptr)))
    assert result == 0
    var output = List[UInt64]()
    for index in range(4):
        output.append(output_ptr[unsafe_offset=index])
    dealloc(candidate_storage^)
    dealloc(name_storage^)
    dealloc(policy_storage^)
    dealloc(output_storage^)
    return output^


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

    var output = call_retention(6, candidates, names, policy)
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

    var output = call_retention(7, candidates, names, policy)
    assert output[0] == 1
    assert output[1] == UInt64(0xFFFFFFFFFFFFFFFF)


def test_runtime_log_filename_scan_priority_and_abi() raises:
    var output = List[UInt64]()
    for _ in range(4):
        output.append(UInt64(97))
    var result_address = UInt(Int(output.unsafe_ptr()))
    var prefix = StringSlice("prodex-runtime")
    var name = StringSlice("prodex-runtime-雪.log.lock")
    assert prodex_log_retention_policy_v1(
        1, 8, UInt64(UInt(Int(name.unsafe_ptr()))), UInt64(name.byte_length()),
        UInt64(UInt(Int(prefix.unsafe_ptr()))), UInt64(prefix.byte_length()),
        0, 0, result_address,
    ) == 0
    assert output[0] == 1
    var logfile = StringSlice("prodex-runtime-雪.log")
    assert prodex_log_retention_policy_v1(
        1, 8, UInt64(UInt(Int(logfile.unsafe_ptr()))), UInt64(logfile.byte_length()),
        UInt64(UInt(Int(prefix.unsafe_ptr()))), UInt64(prefix.byte_length()),
        0, 0, result_address,
    ) == 0
    assert output[0] == 2
    var unrelated = StringSlice("unrelated.log")
    assert prodex_log_retention_policy_v1(
        1, 8, UInt64(UInt(Int(unrelated.unsafe_ptr()))), UInt64(unrelated.byte_length()),
        UInt64(UInt(Int(prefix.unsafe_ptr()))), UInt64(prefix.byte_length()),
        0, 0, result_address,
    ) == 0
    assert output[0] == 0
    output[0] = 97
    assert prodex_log_retention_policy_v1(
        2, 8, UInt64(UInt(Int(logfile.unsafe_ptr()))), UInt64(logfile.byte_length()),
        UInt64(UInt(Int(prefix.unsafe_ptr()))), UInt64(prefix.byte_length()),
        0, 0, result_address,
    ) == 4
    assert output[0] == 97


def main() raises:
    test_expiry_uses_a_strict_cutoff()
    test_budget_selection_uses_oldest_name_tiebreak()
    test_runtime_log_filename_scan_priority_and_abi()
