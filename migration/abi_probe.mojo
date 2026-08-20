from std.memory import Pointer

@export("prodex_abi_probe_sum_u64")
def prodex_abi_probe_sum_u64(
    values: Pointer[mut=False, UInt64, _],
    length: Int64,
    output: Pointer[mut=True, UInt64, _],
) abi("C") -> Int64:
    if length < 0:
        return 1
    if length == 0:
        output[] = 0
        return 0

    var total: UInt64 = 0
    for index in range(length):
        total += values[unsafe_offset=index]
    output[] = total
    return 0
