@export("prodex_context_signal_diff")
def prodex_context_signal_diff(
    before: Pointer[mut=False, Int64, _],
    after: Pointer[mut=False, Int64, _],
    lost: Pointer[mut=True, Int64, _],
    gained: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    for index in range(7):
        var before_value = before[unsafe_offset=index]
        var after_value = after[unsafe_offset=index]
        if before_value < 0 or after_value < 0:
            return 1
        if before_value > after_value:
            lost[unsafe_offset=index] = before_value - after_value
            gained[unsafe_offset=index] = 0
        else:
            lost[unsafe_offset=index] = 0
            gained[unsafe_offset=index] = after_value - before_value
    return 0
