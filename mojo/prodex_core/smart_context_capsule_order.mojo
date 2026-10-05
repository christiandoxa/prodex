from std.memory import Pointer


comptime SMART_CONTEXT_CAPSULE_ORDER_ABI_VERSION: Int64 = 1
comptime SMART_CONTEXT_CAPSULE_ORDER_MAX_COUNT: Int64 = 65_537
comptime SMART_CONTEXT_CAPSULE_ORDER_STATUS_OK: Int64 = 0
comptime SMART_CONTEXT_CAPSULE_ORDER_STATUS_INVALID: Int64 = 1
comptime SMART_CONTEXT_CAPSULE_ORDER_STATUS_ABI: Int64 = 4


def smart_context_capsule_id_compare(
    id_addresses: Pointer[mut=False, UInt64, _],
    id_lengths: Pointer[mut=False, Int64, _],
    left: Int64,
    right: Int64,
) -> Int64:
    var left_address = id_addresses[unsafe_offset=left]
    var right_address = id_addresses[unsafe_offset=right]
    var left_length = id_lengths[unsafe_offset=left]
    var right_length = id_lengths[unsafe_offset=right]
    var common_length = left_length
    if right_length < common_length:
        common_length = right_length
    if common_length > 0:
        var left_id = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
            unsafe_from_address=Int(left_address)
        )
        var right_id = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
            unsafe_from_address=Int(right_address)
        )
        for offset in range(common_length):
            var left_byte = left_id[unsafe_offset=offset]
            var right_byte = right_id[unsafe_offset=offset]
            if left_byte < right_byte:
                return -1
            if left_byte > right_byte:
                return 1
    if left_length < right_length:
        return -1
    if left_length > right_length:
        return 1
    return 0


def smart_context_capsule_order_before(
    id_addresses: Pointer[mut=False, UInt64, _],
    id_lengths: Pointer[mut=False, Int64, _],
    relevances: Pointer[mut=False, Float32, _],
    token_costs: Pointer[mut=False, UInt64, _],
    required: Pointer[mut=False, Int64, _],
    left: Int64,
    right: Int64,
) -> Bool:
    var left_required = required[unsafe_offset=left]
    var right_required = required[unsafe_offset=right]
    if left_required != right_required:
        return left_required == 1

    if left_required == 1:
        return (
            smart_context_capsule_id_compare(
                id_addresses, id_lengths, left, right
            )
            < 0
        )

    var left_relevance = relevances[unsafe_offset=left]
    var right_relevance = relevances[unsafe_offset=right]
    # Float comparisons both return false for NaN, matching partial_cmp == None.
    if left_relevance > right_relevance:
        return True
    if left_relevance < right_relevance:
        return False

    var left_cost = token_costs[unsafe_offset=left]
    var right_cost = token_costs[unsafe_offset=right]
    if left_cost < right_cost:
        return True
    if left_cost > right_cost:
        return False
    return (
        smart_context_capsule_id_compare(id_addresses, id_lengths, left, right)
        < 0
    )


@export("prodex_mojo_smart_context_capsule_order_v1")
def prodex_mojo_smart_context_capsule_order_v1(
    abi_version: Int64,
    id_addresses_address: UInt,
    id_lengths_address: UInt,
    relevances_address: UInt,
    token_costs_address: UInt,
    required_address: UInt,
    permutation_address: UInt,
    scratch_address: UInt,
    count: Int64,
) abi("C") -> Int64:
    if abi_version != SMART_CONTEXT_CAPSULE_ORDER_ABI_VERSION:
        return SMART_CONTEXT_CAPSULE_ORDER_STATUS_ABI
    if (
        count < 0
        or count > SMART_CONTEXT_CAPSULE_ORDER_MAX_COUNT
        or permutation_address == 0
        or scratch_address == 0
    ):
        return SMART_CONTEXT_CAPSULE_ORDER_STATUS_INVALID
    if count > 0 and (
        id_addresses_address == 0
        or id_lengths_address == 0
        or relevances_address == 0
        or token_costs_address == 0
        or required_address == 0
    ):
        return SMART_CONTEXT_CAPSULE_ORDER_STATUS_INVALID
    var id_addresses = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(id_addresses_address)
    )
    var id_lengths = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(id_lengths_address)
    )
    var relevances = Pointer[mut=False, Float32, ImmUntrackedOrigin](
        unsafe_from_address=Int(relevances_address)
    )
    var token_costs = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(token_costs_address)
    )
    var required = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(required_address)
    )
    var permutation = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(permutation_address)
    )
    var scratch = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(scratch_address)
    )

    for index in range(count):
        var address = id_addresses[unsafe_offset=index]
        var length = id_lengths[unsafe_offset=index]
        var required_value = required[unsafe_offset=index]
        if (
            length < 0
            or (length > 0 and address == 0)
            or (required_value != 0 and required_value != 1)
        ):
            return SMART_CONTEXT_CAPSULE_ORDER_STATUS_INVALID

    for index in range(count):
        permutation[unsafe_offset=index] = index

    var width: Int64 = 1
    while width < count:
        var run_start: Int64 = 0
        while run_start < count:
            var middle = run_start + width
            if middle > count:
                middle = count
            var run_end = middle + width
            if run_end > count:
                run_end = count
            var left = run_start
            var right = middle
            var output = run_start
            while left < middle and right < run_end:
                if smart_context_capsule_order_before(
                    id_addresses,
                    id_lengths,
                    relevances,
                    token_costs,
                    required,
                    permutation[unsafe_offset=right],
                    permutation[unsafe_offset=left],
                ):
                    scratch[unsafe_offset=output] = permutation[
                        unsafe_offset=right
                    ]
                    right += 1
                else:
                    scratch[unsafe_offset=output] = permutation[
                        unsafe_offset=left
                    ]
                    left += 1
                output += 1
            while left < middle:
                scratch[unsafe_offset=output] = permutation[unsafe_offset=left]
                left += 1
                output += 1
            while right < run_end:
                scratch[unsafe_offset=output] = permutation[unsafe_offset=right]
                right += 1
                output += 1
            run_start = run_end
        for index in range(count):
            permutation[unsafe_offset=index] = scratch[unsafe_offset=index]
        width *= 2

    return SMART_CONTEXT_CAPSULE_ORDER_STATUS_OK
