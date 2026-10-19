# Prompt-cache affinity decisions. The host owns identifiers, locking and maps.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_trim_bounds, rich_view_valid


@export("prodex_prompt_cache_key_v1")
def prodex_prompt_cache_key_v1(
    version: Int64, hit: Int64, tokens: UInt64, key_address: UInt, key_length: Int64,
    output_address: UInt, capacity: Int64,
) abi("C") -> Int64:
    if version != 1:
        return 4
    if hit < 0 or hit > 1 or output_address == 0 or output_address % 8 != 0:
        return 1
    if capacity < 3:
        return 3
    var kind: UInt64 = 1
    var start: UInt64 = 0
    var end: UInt64 = 0
    # Zero-token observations precede key acquisition and leave the map untouched.
    if hit == 0 or tokens > 0:
        if key_length < 0:
            return 1
        var key = ProdexRichStringView(key_address, UInt(key_length))
        if not rich_view_valid(key, key_length):
            return 1
        var bounds = rich_trim_bounds(key)
        if bounds[0] < bounds[1]:
            kind = 2
            start = UInt64(bounds[0])
            end = UInt64(bounds[1])
        else:
            kind = 0
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    output[unsafe_offset=0] = kind
    output[unsafe_offset=1] = start
    output[unsafe_offset=2] = end
    return 0


@export("prodex_prompt_cache_update_v1")
def prodex_prompt_cache_update_v1(
    version: Int64, hit: Int64, present: Int64, same_owner: Int64,
    previous_bound_at: Int64, previous_tokens: UInt64, tokens: UInt64, now: Int64,
    output_address: UInt, capacity: Int64,
) abi("C") -> Int64:
    if version != 1:
        return 4
    if hit < 0 or hit > 1 or present < 0 or present > 1 or same_owner < 0 or same_owner > present or output_address == 0 or output_address % 8 != 0 or (hit == 1 and tokens == 0):
        return 1
    if capacity < 3:
        return 3
    var action: UInt64 = 0  # inserted, changed, unchanged, preserved
    var bound_at = now
    var cached = tokens if hit == 1 else UInt64(0)
    if present == 1:
        bound_at = max(previous_bound_at, now)
        cached = previous_tokens
        if same_owner == 1:
            action = 2
            if hit == 1:
                cached = max(previous_tokens, tokens)
        elif (hit == 0 and previous_tokens > 0) or (hit == 1 and tokens <= previous_tokens):
            action = 3
        else:
            action = 1
            bound_at = now
            if hit == 1:
                cached = tokens
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    output[unsafe_offset=0] = action
    output[unsafe_offset=1] = UInt64(bound_at)
    output[unsafe_offset=2] = cached
    return 0
