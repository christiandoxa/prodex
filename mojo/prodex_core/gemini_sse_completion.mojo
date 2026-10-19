from std.memory import Pointer

# Gemini streaming tool-call completion order and dedup are compiled policies.
# Rust owns the mutable tool map and the generated Responses event objects.
comptime GEMINI_SSE_COMPLETION_ABI: Int64 = 1


@export("prodex_gemini_sse_pending_tool_positions_v1")
def prodex_gemini_sse_pending_tool_positions_v1(
    abi_version: Int64,
    done_address: UInt,
    tool_count: Int64,
    indices_address: UInt,
    indices_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != GEMINI_SSE_COMPLETION_ABI:
        return 4
    if (
        tool_count < 0
        or (tool_count > 0 and done_address == 0)
        or indices_address == 0
        or indices_capacity < tool_count
        or written_address == 0
    ):
        return 1
    var done = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(done_address)
    )
    # Reject invalid flags before writing anything to caller-owned buffers.
    for index in range(tool_count):
        var state = done[unsafe_offset=index]
        if state != 0 and state != 1:
            return 1

    var indices = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(indices_address)
    )
    var count: Int64 = 0
    for index in range(tool_count):
        if done[unsafe_offset=index] == 0:
            indices[unsafe_offset=count] = UInt64(index)
            count += 1
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )[] = count
    return 0
