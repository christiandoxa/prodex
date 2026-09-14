comptime UINT64_MAX: UInt64 = 18446744073709551615

@export("prodex_context_estimate_tokens")
def prodex_context_estimate_tokens(chars: UInt64, words: UInt64) abi("C") -> UInt64:
    var char_tokens = chars / 4
    if chars % 4 != 0:
        char_tokens += 1

    var word_groups = words / 3
    if word_groups > UINT64_MAX / 4:
        return UINT64_MAX
    var word_tokens = word_groups * 4
    if words % 3 == 1:
        if word_tokens > UINT64_MAX - 2:
            return UINT64_MAX
        word_tokens += 2
    elif words % 3 == 2:
        if word_tokens > UINT64_MAX - 3:
            return UINT64_MAX
        word_tokens += 3

    if char_tokens > word_tokens:
        return char_tokens
    return word_tokens
