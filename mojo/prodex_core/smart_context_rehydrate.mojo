from std.memory import Pointer

comptime SMART_CONTEXT_REHYDRATE_MAX_COUNT: Int64 = 256
comptime SMART_CONTEXT_REHYDRATE_MINIMAL_TIER: Int64 = 0
comptime SMART_CONTEXT_REHYDRATE_CONDENSED_TIER: Int64 = 1
comptime SMART_CONTEXT_REHYDRATE_LARGE_TIER: Int64 = 2
comptime SMART_CONTEXT_REHYDRATE_EXACT_TIER: Int64 = 3
comptime SMART_CONTEXT_REHYDRATE_ACTION_REHYDRATE: Int64 = 0
comptime SMART_CONTEXT_REHYDRATE_ACTION_MISSING: Int64 = 1
comptime SMART_CONTEXT_REHYDRATE_ACTION_BUDGET: Int64 = 2
comptime SMART_CONTEXT_REHYDRATE_ACTION_MINIMAL: Int64 = 3


@export("prodex_smart_context_rehydrate_plan_batch")
def prodex_smart_context_rehydrate_plan_batch(
    token_costs: Pointer[mut=False, UInt64, _],
    required: Pointer[mut=False, Int64, _],
    available: Pointer[mut=False, Int64, _],
    action_tags: Pointer[mut=True, Int64, _],
    used_tokens: Pointer[mut=True, UInt64, _],
    count: Int64,
    token_budget: UInt64,
    tier: Int64,
) abi("C") -> Int64:
    if count < 0 or count > SMART_CONTEXT_REHYDRATE_MAX_COUNT:
        return 1
    if (
        tier < SMART_CONTEXT_REHYDRATE_MINIMAL_TIER
        or tier > SMART_CONTEXT_REHYDRATE_EXACT_TIER
    ):
        return 2

    var used: UInt64 = 0
    for index in range(count):
        var required_value = required[unsafe_offset=index]
        var available_value = available[unsafe_offset=index]
        if (required_value != 0 and required_value != 1) or (
            available_value != 0 and available_value != 1
        ):
            return 2
        var cost = token_costs[unsafe_offset=index]
        if available_value == 0:
            action_tags[
                unsafe_offset=index
            ] = SMART_CONTEXT_REHYDRATE_ACTION_MISSING
        elif (
            tier == SMART_CONTEXT_REHYDRATE_MINIMAL_TIER and required_value == 0
        ):
            action_tags[
                unsafe_offset=index
            ] = SMART_CONTEXT_REHYDRATE_ACTION_MINIMAL
        elif cost > token_budget - used:
            action_tags[
                unsafe_offset=index
            ] = SMART_CONTEXT_REHYDRATE_ACTION_BUDGET
        else:
            used += cost
            action_tags[
                unsafe_offset=index
            ] = SMART_CONTEXT_REHYDRATE_ACTION_REHYDRATE
    used_tokens[unsafe_offset=0] = used
    return 0
