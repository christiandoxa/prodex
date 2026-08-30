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
    ProdexGatewayBillingSummaryBucket,
    ProdexGatewayBillingSummaryInput,
    ProdexGatewayBillingSummaryResult,
    rich_view_ptr,
)


comptime PRODEX_RICH_ABI_VERSION: Int64 = 6
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
    var ptr = rich_view_ptr(view)
    for index in range(Int64(view.len)):
        hash = (hash ^ UInt64(ptr[unsafe_offset=index])) * 1099511628211
    var slot = Int64(hash % UInt64(slot_count))
    for _ in range(slot_count):
        var existing = slots[unsafe_offset=slot]
        if existing < 0:
            return False
        var other = available[unsafe_offset=existing].copy()
        if other.len == view.len:
            var other_ptr = rich_view_ptr(other)
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
        var ptr = rich_view_ptr(item)
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
        var slice = rich_copy_range(rich_view_ptr(item.id), 0, Int64(item.id.len), output, output_capacity, Pointer(to=written), False)
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


comptime RICH_MAX_BILLING_INPUTS: Int64 = 100_000
comptime RICH_BILLING_CATEGORIES: Int64 = 9
comptime RICH_BILLING_STATUS_MAX: Int64 = 65_535
comptime UINT64_MAX: UInt64 = 18446744073709551615
comptime RICH_STATUS_ABI: Int64 = 4


def billing_saturating_add(left: UInt64, right: UInt64) -> UInt64:
    if left > UINT64_MAX - right:
        return UINT64_MAX
    return left + right


def billing_flag_valid(value: Int64) -> Bool:
    return value == 0 or value == 1


def billing_bucket_apply(
    output: Pointer[mut=True, ProdexGatewayBillingSummaryBucket, _],
    bucket_index: Int64,
    input: ProdexGatewayBillingSummaryInput,
) -> None:
    var bucket_output = output + bucket_index
    var bucket = bucket_output[].copy()
    bucket.requests = billing_saturating_add(bucket.requests, 1)
    if input.response_status_present == 0:
        bucket.unreconciled_requests = billing_saturating_add(
            bucket.unreconciled_requests, 1
        )
    elif input.response_status >= 200 and input.response_status < 300:
        bucket.successful_requests = billing_saturating_add(
            bucket.successful_requests, 1
        )
    else:
        bucket.failed_requests = billing_saturating_add(bucket.failed_requests, 1)
    bucket.input_tokens = billing_saturating_add(bucket.input_tokens, input.input_tokens)
    bucket.output_tokens = billing_saturating_add(bucket.output_tokens, input.output_tokens)
    bucket.response_bytes = billing_saturating_add(bucket.response_bytes, input.response_bytes)
    bucket.estimated_cost_microusd = billing_saturating_add(
        bucket.estimated_cost_microusd, input.estimated_cost_microusd
    )
    bucket.final_cost_microusd = billing_saturating_add(
        bucket.final_cost_microusd, input.final_cost_microusd
    )
    if bucket.first_created_at_present == 0:
        bucket.first_created_at_epoch = input.created_at_epoch
        bucket.first_created_at_present = 1
        bucket.last_created_at_epoch = input.created_at_epoch
    else:
        if input.created_at_epoch < bucket.first_created_at_epoch:
            bucket.first_created_at_epoch = input.created_at_epoch
        if input.created_at_epoch > bucket.last_created_at_epoch:
            bucket.last_created_at_epoch = input.created_at_epoch
    if input.reconciled_at_present == 1:
        if bucket.last_reconciled_at_present == 0:
            bucket.last_reconciled_at_epoch = input.reconciled_at_epoch
            bucket.last_reconciled_at_present = 1
        else:
            if input.reconciled_at_epoch > bucket.last_reconciled_at_epoch:
                bucket.last_reconciled_at_epoch = input.reconciled_at_epoch
    bucket_output[] = bucket.copy()


def billing_bucket_reference_valid(bucket_index: Int64, bucket_count: Int64) -> Bool:
    return bucket_index >= -1 and bucket_index < bucket_count


def billing_bucket_apply_if_present(
    output: Pointer[mut=True, ProdexGatewayBillingSummaryBucket, _],
    bucket_index: Int64,
    bucket_count: Int64,
    input: ProdexGatewayBillingSummaryInput,
) -> Bool:
    if not billing_bucket_reference_valid(bucket_index, bucket_count):
        return False
    if bucket_index >= 0:
        billing_bucket_apply(output, bucket_index, input)
    return True


@export("prodex_mojo_rich_gateway_billing_summary_v1")
def prodex_mojo_rich_gateway_billing_summary_v1(
    abi_version: Int64,
    inputs_address: UInt,
    input_count: Int64,
    outputs_address: UInt,
    bucket_count: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if result_address == 0:
        return RICH_STATUS_INVALID
    var result = Pointer[
        mut=True, ProdexGatewayBillingSummaryResult, MutUntrackedOrigin
    ](unsafe_from_address=Int(result_address))
    result[].abi_version = PRODEX_RICH_ABI_VERSION
    result[].buckets_written = 0
    result[].required_buckets = bucket_count
    result[].issue_kind = 0
    if abi_version != PRODEX_RICH_ABI_VERSION:
        result[].issue_kind = RICH_STATUS_ABI
        return RICH_STATUS_ABI
    if (
        input_count < 0
        or input_count > RICH_MAX_BILLING_INPUTS
        or bucket_count < 1
        or bucket_count > input_count * RICH_BILLING_CATEGORIES + 1
        or outputs_address == 0
    ):
        return RICH_STATUS_INVALID
    if input_count > 0 and inputs_address == 0:
        return RICH_STATUS_INVALID
    var inputs = Pointer[
        mut=False, ProdexGatewayBillingSummaryInput, ImmUntrackedOrigin
    ](unsafe_from_address=Int(inputs_address))
    var outputs = Pointer[
        mut=True, ProdexGatewayBillingSummaryBucket, MutUntrackedOrigin
    ](unsafe_from_address=Int(outputs_address))
    for bucket_index in range(bucket_count):
        var bucket_output = outputs + bucket_index
        var bucket = bucket_output[].copy()
        bucket.requests = 0
        bucket.successful_requests = 0
        bucket.failed_requests = 0
        bucket.unreconciled_requests = 0
        bucket.input_tokens = 0
        bucket.output_tokens = 0
        bucket.response_bytes = 0
        bucket.estimated_cost_microusd = 0
        bucket.final_cost_microusd = 0
        bucket.first_created_at_epoch = 0
        bucket.first_created_at_present = 0
        bucket.last_created_at_epoch = 0
        bucket.last_reconciled_at_epoch = 0
        bucket.last_reconciled_at_present = 0
        bucket_output[] = bucket.copy()
    for input_index in range(input_count):
        var input = inputs[unsafe_offset=input_index].copy()
        if not billing_flag_valid(input.response_status_present) or not billing_flag_valid(
            input.reconciled_at_present
        ):
            return RICH_STATUS_INVALID
        if input.response_status_present == 1 and (
            input.response_status < 0 or input.response_status > RICH_BILLING_STATUS_MAX
        ):
            return RICH_STATUS_INVALID
        if not billing_bucket_apply_if_present(
            outputs, input.bucket_ids[0], bucket_count, input
        ) or not billing_bucket_apply_if_present(
            outputs, input.bucket_ids[1], bucket_count, input
        ) or not billing_bucket_apply_if_present(
            outputs, input.bucket_ids[2], bucket_count, input
        ) or not billing_bucket_apply_if_present(
            outputs, input.bucket_ids[3], bucket_count, input
        ) or not billing_bucket_apply_if_present(
            outputs, input.bucket_ids[4], bucket_count, input
        ) or not billing_bucket_apply_if_present(
            outputs, input.bucket_ids[5], bucket_count, input
        ) or not billing_bucket_apply_if_present(
            outputs, input.bucket_ids[6], bucket_count, input
        ) or not billing_bucket_apply_if_present(
            outputs, input.bucket_ids[7], bucket_count, input
        ) or not billing_bucket_apply_if_present(
            outputs, input.bucket_ids[8], bucket_count, input
        ):
            return RICH_STATUS_INVALID
    result[].buckets_written = bucket_count
    return RICH_STATUS_OK
