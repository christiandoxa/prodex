@export("prodex_smart_context_estimate_tokens_from_body_bytes")
def prodex_smart_context_estimate_tokens_from_body_bytes(body_bytes: UInt64) abi("C") -> UInt64:
    if body_bytes > 18446744073709551612:
        return 4611686018427387903
    return (body_bytes + 3) / 4
