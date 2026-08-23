from std.memory import Pointer

comptime POLICY_NUMERIC_NON_ZERO: Int64 = 0
comptime POLICY_NUMERIC_RANGE: Int64 = 1
comptime POLICY_NUMERIC_RELATION_LE: Int64 = 2
comptime UINT64_MAX: UInt64 = 18446744073709551615

@export("prodex_runtime_policy_validate_numeric")
def prodex_runtime_policy_validate_numeric(
    values: Pointer[mut=False, UInt64, _],
    kinds: Pointer[mut=False, Int64, _],
    minimums: Pointer[mut=False, UInt64, _],
    maximums: Pointer[mut=False, UInt64, _],
    related_values: Pointer[mut=False, UInt64, _],
    failed_rules: Pointer[mut=True, Int64, _],
    count: Int64,
) abi("C") -> Int64:
    if count < 0:
        return 1

    for index in range(count):
        var kind = kinds[unsafe_offset=index]
        var value = values[unsafe_offset=index]
        var invalid = False
        if kind == POLICY_NUMERIC_NON_ZERO:
            invalid = value == 0
        elif kind == POLICY_NUMERIC_RANGE:
            invalid = value < minimums[unsafe_offset=index] or value > maximums[unsafe_offset=index]
        elif kind == POLICY_NUMERIC_RELATION_LE:
            invalid = value > related_values[unsafe_offset=index]
        else:
            return 2

        if invalid:
            failed_rules[unsafe_offset=index] = 1
        else:
            failed_rules[unsafe_offset=index] = 0
    return 0
