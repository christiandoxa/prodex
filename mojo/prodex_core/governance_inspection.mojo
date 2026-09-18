from std.memory import Pointer

comptime GOVERNANCE_INSPECTION_ABI_VERSION: Int64 = 6
comptime GOVERNANCE_INSPECTION_OK: Int64 = 0
comptime GOVERNANCE_INSPECTION_INVALID: Int64 = 1
comptime GOVERNANCE_INSPECTION_ABI: Int64 = 4

def governance_finding_minimum_classification(kind: Int64) -> Int64:
    if (kind >= 0 and kind <= 3) or kind == 11:
        return 2
    if kind >= 4 and kind <= 10:
        return 3
    return -1

@export("prodex_mojo_governance_finding_classification_v1")
def prodex_mojo_governance_finding_classification_v1(
    abi_version: Int64,
    mode: Int64,
    values_address: UInt,
    value_count: Int64,
    classification: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != GOVERNANCE_INSPECTION_ABI_VERSION:
        return GOVERNANCE_INSPECTION_ABI
    if mode < 0 or mode > 1 or value_count < 0 or value_count > 256:
        return GOVERNANCE_INSPECTION_INVALID
    if output_address == 0 or (value_count > 0 and values_address == 0):
        return GOVERNANCE_INSPECTION_INVALID

    var values = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(values_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = 0

    if mode == 0:
        if value_count != 1:
            return GOVERNANCE_INSPECTION_INVALID
        var minimum = governance_finding_minimum_classification(values[0])
        if minimum < 0:
            return GOVERNANCE_INSPECTION_INVALID
        output[] = minimum
        return GOVERNANCE_INSPECTION_OK

    if classification < 0 or classification > 3:
        return GOVERNANCE_INSPECTION_INVALID
    for index in range(value_count):
        var minimum = governance_finding_minimum_classification(values[unsafe_offset=index])
        if minimum < 0:
            return GOVERNANCE_INSPECTION_INVALID
        if classification < minimum:
            output[] = 1
            return GOVERNANCE_INSPECTION_OK
    return GOVERNANCE_INSPECTION_OK
