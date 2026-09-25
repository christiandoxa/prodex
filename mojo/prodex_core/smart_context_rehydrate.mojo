from std.memory import Pointer

comptime SMART_CONTEXT_REHYDRATE_MAX_COUNT: Int64 = 256
comptime SMART_CONTEXT_REHYDRATE_MAX_IDENTIFIER_BYTES: UInt64 = 4_096
comptime SMART_CONTEXT_REHYDRATE_ORDER_ABI_VERSION: Int64 = 1
comptime SMART_CONTEXT_REHYDRATE_MINIMAL_TIER: Int64 = 0
comptime SMART_CONTEXT_REHYDRATE_CONDENSED_TIER: Int64 = 1
comptime SMART_CONTEXT_REHYDRATE_LARGE_TIER: Int64 = 2
comptime SMART_CONTEXT_REHYDRATE_EXACT_TIER: Int64 = 3
comptime SMART_CONTEXT_REHYDRATE_ACTION_REHYDRATE: Int64 = 0
comptime SMART_CONTEXT_REHYDRATE_ACTION_MISSING: Int64 = 1
comptime SMART_CONTEXT_REHYDRATE_ACTION_BUDGET: Int64 = 2
comptime SMART_CONTEXT_REHYDRATE_ACTION_MINIMAL: Int64 = 3
comptime SMART_CONTEXT_REHYDRATE_STATUS_INVALID_INPUT: Int64 = 1
comptime SMART_CONTEXT_REHYDRATE_STATUS_ABI: Int64 = 2
comptime SMART_CONTEXT_REHYDRATE_STATUS_CAPACITY: Int64 = 3


@fieldwise_init
struct SmartContextRehydrateStringView(Copyable):
    var ptr: UInt64
    var len: UInt64


@fieldwise_init
struct SmartContextRehydrateOrderItem(Copyable):
    var id: SmartContextRehydrateStringView
    var token_cost: UInt64
    var required: Int64


def smart_context_rehydrate_view_valid(view: SmartContextRehydrateStringView) -> Bool:
    return view.len <= SMART_CONTEXT_REHYDRATE_MAX_IDENTIFIER_BYTES and (
        view.len == 0 or view.ptr != 0
    )


def smart_context_rehydrate_view_equal(
    left: SmartContextRehydrateStringView,
    right: SmartContextRehydrateStringView,
) -> Bool:
    if left.len != right.len:
        return False
    if left.len == 0:
        return True
    var left_ptr = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(left.ptr)
    )
    var right_ptr = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(right.ptr)
    )
    for index in range(Int64(left.len)):
        if left_ptr[unsafe_offset=index] != right_ptr[unsafe_offset=index]:
            return False
    return True


def smart_context_rehydrate_id_before(
    left: SmartContextRehydrateStringView,
    right: SmartContextRehydrateStringView,
) -> Bool:
    var common_length = left.len
    if right.len < common_length:
        common_length = right.len
    if common_length > 0:
        var left_ptr = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
            unsafe_from_address=Int(left.ptr)
        )
        var right_ptr = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
            unsafe_from_address=Int(right.ptr)
        )
        for index in range(Int64(common_length)):
            var left_byte = left_ptr[unsafe_offset=index]
            var right_byte = right_ptr[unsafe_offset=index]
            if left_byte != right_byte:
                return left_byte < right_byte
    return left.len < right.len


def smart_context_rehydrate_item_before(
    left_index: Int64,
    right_index: Int64,
    items: Pointer[mut=False, SmartContextRehydrateOrderItem, _],
) -> Bool:
    var left = items[unsafe_offset=left_index].copy()
    var right = items[unsafe_offset=right_index].copy()
    if left.required != right.required:
        return left.required > right.required
    if left.token_cost != right.token_cost:
        return left.token_cost < right.token_cost
    return smart_context_rehydrate_id_before(left.id, right.id)


def smart_context_rehydrate_is_available(
    id: SmartContextRehydrateStringView,
    available: Pointer[mut=False, SmartContextRehydrateStringView, _],
    available_count: Int64,
) -> Bool:
    # ponytail: scan at most 256 artifact IDs per reference; add a hash table if this bound grows.
    for index in range(available_count):
        if smart_context_rehydrate_view_equal(
            id, available[unsafe_offset=index].copy()
        ):
            return True
    return False


@export("prodex_smart_context_rehydrate_order_v1")
def prodex_smart_context_rehydrate_order_v1(
    abi_version: Int64,
    items_address: UInt,
    item_count: Int64,
    available_address: UInt,
    available_count: Int64,
    ordered_indices_address: UInt,
    indices_capacity: Int64,
    availability_tags_address: UInt,
    availability_capacity: Int64,
) abi("C") -> Int64:
    if abi_version != SMART_CONTEXT_REHYDRATE_ORDER_ABI_VERSION:
        return SMART_CONTEXT_REHYDRATE_STATUS_ABI
    if (
        item_count < 0
        or item_count > SMART_CONTEXT_REHYDRATE_MAX_COUNT
        or available_count < 0
        or available_count > SMART_CONTEXT_REHYDRATE_MAX_COUNT
    ):
        return SMART_CONTEXT_REHYDRATE_STATUS_INVALID_INPUT
    if indices_capacity < item_count or availability_capacity < item_count:
        return SMART_CONTEXT_REHYDRATE_STATUS_CAPACITY
    if (
        items_address == 0
        or available_address == 0
        or ordered_indices_address == 0
        or availability_tags_address == 0
    ):
        return SMART_CONTEXT_REHYDRATE_STATUS_INVALID_INPUT

    var items = Pointer[
        mut=False, SmartContextRehydrateOrderItem, ImmUntrackedOrigin
    ](unsafe_from_address=Int(items_address))
    var available = Pointer[
        mut=False, SmartContextRehydrateStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(available_address))
    var ordered_indices = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(ordered_indices_address)
    )
    var availability_tags = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(availability_tags_address)
    )

    for index in range(item_count):
        var item = items[unsafe_offset=index].copy()
        if (
            not smart_context_rehydrate_view_valid(item.id)
            or item.required < 0
            or item.required > 1
        ):
            return SMART_CONTEXT_REHYDRATE_STATUS_INVALID_INPUT
    for index in range(available_count):
        if not smart_context_rehydrate_view_valid(
            available[unsafe_offset=index].copy()
        ):
            return SMART_CONTEXT_REHYDRATE_STATUS_INVALID_INPUT

    for index in range(item_count):
        ordered_indices[unsafe_offset=index] = index

    # ponytail: stable insertion sort is bounded by 256 items; use merge sort if that limit grows.
    for index in range(Int64(1), item_count):
        var current = ordered_indices[unsafe_offset=index]
        var cursor = index
        while cursor > 0:
            var previous = ordered_indices[unsafe_offset=cursor - 1]
            if not smart_context_rehydrate_item_before(current, previous, items):
                break
            ordered_indices[unsafe_offset=cursor] = previous
            cursor -= 1
        ordered_indices[unsafe_offset=cursor] = current

    for index in range(item_count):
        var input_index = ordered_indices[unsafe_offset=index]
        var item = items[unsafe_offset=input_index].copy()
        availability_tags[unsafe_offset=index] = Int64(
            smart_context_rehydrate_is_available(item.id, available, available_count)
        )
    return 0


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
