from std.memory import Pointer

from rich_text import (
    rich_copy_range,
    rich_required_hash_capacity,
    rich_view_valid,
)
from rich_types import (
    ContextItem,
    ContextPlan,
    ProdexRichPlanAction,
    ProdexRichPlanItem,
    ProdexRichPlanResult,
    ProdexRichStringView,
)


comptime PRODEX_RICH_ABI_VERSION: Int64 = 3
comptime RICH_MAX_PLAN_ITEMS: Int64 = 256
comptime RICH_MAX_IDENTIFIER_BYTES: Int64 = 4_096
comptime RICH_STATUS_OK: Int64 = 0
comptime RICH_STATUS_INVALID: Int64 = 1
comptime RICH_STATUS_CAPACITY: Int64 = 3


def plan_available_contains(
    view: ProdexRichStringView,
    available: Pointer[mut=False, ProdexRichStringView, _],
    slots: Pointer[mut=True, Int64, _],
    slot_count: Int64,
) -> Bool:
    var hash: UInt64 = UInt64(view.len) * 1099511628211
    var ptr = view.ptr.unsafe_value()
    for index in range(Int64(view.len)):
        hash = (hash ^ UInt64(ptr[unsafe_offset=index])) * 1099511628211
    var slot = Int64(hash % UInt64(slot_count))
    for _ in range(slot_count):
        var existing = slots[unsafe_offset=slot]
        if existing < 0:
            return False
        var other = available[unsafe_offset=existing].copy()
        if other.len == view.len:
            var other_ptr = other.ptr.unsafe_value()
            var equal = True
            for index in range(Int64(view.len)):
                if ptr[unsafe_offset=index] != other_ptr[unsafe_offset=index]:
                    equal = False
                    break
            if equal:
                return True
        slot += 1
        if slot == slot_count:
            slot = 0
    return False


@export("prodex_mojo_rich_context_plan_v2")
def prodex_mojo_rich_context_plan_v2(
    abi_version: Int64,
    items_address: UInt,
    item_count: Int64,
    available_address: UInt,
    available_count: Int64,
    token_budget: Int64,
    tier: Int64,
    output_actions_address: UInt,
    action_capacity: Int64,
    output_address: UInt,
    output_capacity: Int64,
    hash_slots_address: UInt,
    hash_capacity: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if result_address == 0:
        return RICH_STATUS_INVALID
    var result_ptr = Pointer[mut=True, ProdexRichPlanResult, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result_ptr[].abi_version = PRODEX_RICH_ABI_VERSION
    result_ptr[].actions_written = 0
    result_ptr[].required_actions = item_count
    result_ptr[].output_written = 0
    result_ptr[].required_output = 0
    result_ptr[].used_tokens = 0
    result_ptr[].issue_kind = 0
    result_ptr[].issue_offset = -1
    result_ptr[].issue_length = 0
    if abi_version != PRODEX_RICH_ABI_VERSION or item_count < 0 or item_count > RICH_MAX_PLAN_ITEMS or available_count < 0 or available_count > RICH_MAX_PLAN_ITEMS or action_capacity < item_count or token_budget < 0 or tier < 0 or tier > 3:
        return RICH_STATUS_INVALID
    if items_address == 0 or available_address == 0 or output_actions_address == 0 or output_address == 0 or hash_slots_address == 0:
        return RICH_STATUS_INVALID
    var items = Pointer[mut=False, ProdexRichPlanItem, ImmUntrackedOrigin](
        unsafe_from_address=Int(items_address)
    )
    var available = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(available_address)
    )
    var output_actions = Pointer[mut=True, ProdexRichPlanAction, MutUntrackedOrigin](
        unsafe_from_address=Int(output_actions_address)
    )
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var hash_slots = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(hash_slots_address)
    )
    var required_output: Int64 = 0
    for index in range(item_count):
        var item = items[unsafe_offset=index].copy()
        if not rich_view_valid(item.id, RICH_MAX_IDENTIFIER_BYTES) or item.token_cost < 0 or item.required != 0 and item.required != 1:
            return RICH_STATUS_INVALID
        required_output += Int64(item.id.len)
    for index in range(available_count):
        if not rich_view_valid(available[unsafe_offset=index], RICH_MAX_IDENTIFIER_BYTES):
            return RICH_STATUS_INVALID
    result_ptr[].required_output = required_output
    var required_hash = rich_required_hash_capacity(available_count)
    if output_capacity < required_output or hash_capacity < required_hash:
        return RICH_STATUS_CAPACITY
    for index in range(hash_capacity):
        hash_slots[unsafe_offset=index] = -1
    for index in range(available_count):
        var item = available[unsafe_offset=index].copy()
        var ptr = item.ptr.unsafe_value()
        var hash: UInt64 = UInt64(item.len) * 1099511628211
        for byte_index in range(Int64(item.len)):
            hash = (hash ^ UInt64(ptr[unsafe_offset=byte_index])) * 1099511628211
        var slot = Int64(hash % UInt64(hash_capacity))
        for _ in range(hash_capacity):
            if hash_slots[unsafe_offset=slot] < 0:
                hash_slots[unsafe_offset=slot] = index
                break
            slot += 1
            if slot == hash_capacity:
                slot = 0
    var written: Int64 = 0
    var plan = ContextPlan(0, 0)
    for index in range(item_count):
        var source_item = items[unsafe_offset=index].copy()
        var item = ContextItem(source_item.id.copy(), source_item.token_cost, source_item.required, 0)
        item.available = Int64(plan_available_contains(item.id, available, hash_slots, hash_capacity))
        var action: Int64
        var reason: Int64
        if item.available == 0:
            action = 0
            reason = 1
        elif tier == 0 and item.required == 0:
            action = 0
            reason = 3
        elif plan.used_tokens > token_budget - item.token_cost:
            action = 0
            reason = 2
        else:
            action = 1
            reason = 0
            plan.used_tokens += item.token_cost
        var slice = rich_copy_range(item.id.ptr.unsafe_value(), 0, Int64(item.id.len), output, output_capacity, Pointer(to=written), False)
        if slice.len < 0:
            return RICH_STATUS_CAPACITY
        output_actions[unsafe_offset=index].id = slice.copy()
        output_actions[unsafe_offset=index].action = action
        output_actions[unsafe_offset=index].reason = reason
        output_actions[unsafe_offset=index].token_cost = item.token_cost
        output_actions[unsafe_offset=index].input_index = index
        plan.action_count += 1
    result_ptr[].actions_written = plan.action_count
    result_ptr[].output_written = written
    result_ptr[].used_tokens = plan.used_tokens
    return RICH_STATUS_OK
