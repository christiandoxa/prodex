comptime RUNTIME_PROXY_BODY_LIMIT_ABI_VERSION: Int64 = 1


@export("prodex_runtime_proxy_body_limit_exceeds_v1")
def prodex_runtime_proxy_body_limit_exceeds_v1(
    abi_version: Int64,
    limit_bytes: UInt64,
    observed_bytes: UInt64,
) abi("C") -> Int64:
    if abi_version != RUNTIME_PROXY_BODY_LIMIT_ABI_VERSION:
        return -1
    return Int64(observed_bytes > limit_bytes)
