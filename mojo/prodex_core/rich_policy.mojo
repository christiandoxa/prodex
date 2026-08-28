from std.memory import Pointer

from rich_text import (
    rich_copy_trimmed,
    rich_valid_identifier,
    rich_view_matches_literal,
    rich_view_valid,
    rich_views_equal,
)
from rich_types import (
    PolicyRule,
    ProdexRichPolicyInput,
    ProdexRichPolicyModel,
    ProdexRichPolicyResult,
    ProdexRichPolicyRouteInput,
    ProdexRichPolicyRouteResult,
    ProdexRichSlice,
    ProdexRichStringView,
)


comptime PRODEX_RICH_ABI_VERSION: Int64 = 6
comptime RICH_MAX_RECORDS: Int64 = 256
comptime RICH_MAX_IDENTIFIER_BYTES: Int64 = 4_096
comptime RICH_STATUS_OK: Int64 = 0
comptime RICH_STATUS_INVALID: Int64 = 1
comptime RICH_STATUS_UTF8: Int64 = 2
comptime RICH_STATUS_CAPACITY: Int64 = 3
comptime RICH_STATUS_ABI: Int64 = 4
comptime RICH_ISSUE_EMPTY: Int64 = 1
comptime RICH_ISSUE_WHITESPACE: Int64 = 2
comptime RICH_ISSUE_STRATEGY: Int64 = 3
comptime RICH_ISSUE_MODEL: Int64 = 4
comptime RICH_ISSUE_INVALID_UTF8: Int64 = 6
comptime RICH_FIELD_ALIAS: Int64 = 1
comptime RICH_FIELD_MODELS: Int64 = 2
comptime RICH_FIELD_STRATEGY: Int64 = 3
comptime RICH_FIELD_METRIC: Int64 = 4
comptime RICH_MAX_ROUTE_MODELS: Int64 = 256
comptime ROUTE_FALLBACK: Int64 = 0
comptime ROUTE_ROUND_ROBIN: Int64 = 1
comptime ROUTE_FIRST: Int64 = 2
comptime ROUTE_LEAST_BUSY: Int64 = 3
comptime ROUTE_LOWEST_COST: Int64 = 4
comptime ROUTE_LOWEST_LATENCY: Int64 = 5
comptime ROUTE_RPM: Int64 = 6
comptime ROUTE_TPM: Int64 = 7
comptime UINT64_MAX: UInt64 = 18446744073709551615
comptime UNKNOWN_HEADROOM: UInt64 = 9223372036854775807


def policy_issue(
    result: Pointer[mut=True, ProdexRichPolicyResult, _],
    kind: Int64,
    field: Int64,
    index: Int64,
    offset: Int64,
    length: Int64,
) -> None:
    result[].issue_kind = kind
    result[].issue_field = field
    result[].issue_index = index
    result[].issue_offset = offset
    result[].issue_length = length


def policy_strategy_valid(view: ProdexRichStringView) -> Bool:
    return rich_view_matches_literal["fallback"](view, True) or rich_view_matches_literal["ordered-fallback"](view, True) or rich_view_matches_literal["ordered_fallback"](view, True) or rich_view_matches_literal["round-robin"](view, True) or rich_view_matches_literal["round_robin"](view, True) or rich_view_matches_literal["rr"](view, True) or rich_view_matches_literal["first"](view, True) or rich_view_matches_literal["first-available"](view, True) or rich_view_matches_literal["first_available"](view, True) or rich_view_matches_literal["ordered"](view, True) or rich_view_matches_literal["least-busy"](view, True) or rich_view_matches_literal["least_busy"](view, True) or rich_view_matches_literal["least-busy-model"](view, True) or rich_view_matches_literal["least_busy_model"](view, True) or rich_view_matches_literal["lowest-cost"](view, True) or rich_view_matches_literal["lowest_cost"](view, True) or rich_view_matches_literal["cost"](view, True) or rich_view_matches_literal["cost-optimized"](view, True) or rich_view_matches_literal["cost_optimized"](view, True) or rich_view_matches_literal["lowest-latency"](view, True) or rich_view_matches_literal["lowest_latency"](view, True) or rich_view_matches_literal["latency"](view, True) or rich_view_matches_literal["latency-optimized"](view, True) or rich_view_matches_literal["latency_optimized"](view, True) or rich_view_matches_literal["rpm"](view, True) or rich_view_matches_literal["rpm-headroom"](view, True) or rich_view_matches_literal["rpm_headroom"](view, True) or rich_view_matches_literal["tpm"](view, True) or rich_view_matches_literal["tpm-headroom"](view, True) or rich_view_matches_literal["tpm_headroom"](view, True)


def policy_route_strategy(view: ProdexRichStringView) -> Int64:
    if rich_view_matches_literal["fallback"](view, True) or rich_view_matches_literal["ordered-fallback"](view, True) or rich_view_matches_literal["ordered_fallback"](view, True):
        return ROUTE_FALLBACK
    if rich_view_matches_literal["round-robin"](view, True) or rich_view_matches_literal["round_robin"](view, True) or rich_view_matches_literal["rr"](view, True):
        return ROUTE_ROUND_ROBIN
    if rich_view_matches_literal["first"](view, True) or rich_view_matches_literal["first-available"](view, True) or rich_view_matches_literal["first_available"](view, True) or rich_view_matches_literal["ordered"](view, True):
        return ROUTE_FIRST
    if rich_view_matches_literal["least-busy"](view, True) or rich_view_matches_literal["least_busy"](view, True) or rich_view_matches_literal["least-busy-model"](view, True) or rich_view_matches_literal["least_busy_model"](view, True):
        return ROUTE_LEAST_BUSY
    if rich_view_matches_literal["lowest-cost"](view, True) or rich_view_matches_literal["lowest_cost"](view, True) or rich_view_matches_literal["cost"](view, True) or rich_view_matches_literal["cost-optimized"](view, True) or rich_view_matches_literal["cost_optimized"](view, True):
        return ROUTE_LOWEST_COST
    if rich_view_matches_literal["lowest-latency"](view, True) or rich_view_matches_literal["lowest_latency"](view, True) or rich_view_matches_literal["latency"](view, True) or rich_view_matches_literal["latency-optimized"](view, True) or rich_view_matches_literal["latency_optimized"](view, True):
        return ROUTE_LOWEST_LATENCY
    if rich_view_matches_literal["rpm"](view, True) or rich_view_matches_literal["rpm-headroom"](view, True) or rich_view_matches_literal["rpm_headroom"](view, True):
        return ROUTE_RPM
    if rich_view_matches_literal["tpm"](view, True) or rich_view_matches_literal["tpm-headroom"](view, True) or rich_view_matches_literal["tpm_headroom"](view, True):
        return ROUTE_TPM
    return -1


def policy_route_saturating_add(left: UInt64, right: UInt64) -> UInt64:
    if right > UINT64_MAX - left:
        return UINT64_MAX
    return left + right


def policy_route_latency(input: ProdexRichPolicyRouteInput) -> UInt64:
    if input.state_latency_present == 1:
        return input.state_latency
    if input.policy_latency_present == 1:
        return input.policy_latency
    return UINT64_MAX


def policy_route_cost(input: ProdexRichPolicyRouteInput) -> UInt64:
    if input.input_cost_present == 1 and input.output_cost_present == 1:
        return policy_route_saturating_add(input.input_cost, input.output_cost)
    if input.input_cost_present == 1:
        return input.input_cost
    if input.output_cost_present == 1:
        return input.output_cost
    return UINT64_MAX


def policy_route_rpm_headroom(input: ProdexRichPolicyRouteInput) -> UInt64:
    if input.rpm_limit_present == 0:
        return UNKNOWN_HEADROOM
    if input.rpm_used >= input.rpm_limit:
        return 0
    return input.rpm_limit - input.rpm_used


def policy_route_tpm_headroom(
    input: ProdexRichPolicyRouteInput, estimated_tokens: UInt64
) -> UInt64:
    if input.tpm_limit_present == 0:
        return UNKNOWN_HEADROOM
    var used = policy_route_saturating_add(input.tpm_used, estimated_tokens)
    if used >= input.tpm_limit:
        return 0
    return input.tpm_limit - used


def policy_route_better(
    strategy: Int64,
    candidate: ProdexRichPolicyRouteInput,
    current: ProdexRichPolicyRouteInput,
    estimated_tokens: UInt64,
) -> Bool:
    if strategy == ROUTE_LEAST_BUSY:
        return candidate.in_flight < current.in_flight
    if strategy == ROUTE_LOWEST_COST:
        return policy_route_cost(candidate) < policy_route_cost(current)
    if strategy == ROUTE_LOWEST_LATENCY:
        return policy_route_latency(candidate) < policy_route_latency(current)
    if strategy == ROUTE_RPM:
        return policy_route_rpm_headroom(candidate) > policy_route_rpm_headroom(current)
    if strategy == ROUTE_TPM:
        return policy_route_tpm_headroom(candidate, estimated_tokens) > policy_route_tpm_headroom(current, estimated_tokens)
    return False


@export("prodex_mojo_rich_policy_route_v1")
def prodex_mojo_rich_policy_route_v1(
    abi_version: Int64,
    strategy_address: UInt,
    request_id: UInt64,
    estimated_tokens: UInt64,
    inputs_address: UInt,
    input_count: Int64,
    ordered_indices_address: UInt,
    ordered_capacity: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if result_address == 0:
        return RICH_STATUS_INVALID
    var result = Pointer[
        mut=True, ProdexRichPolicyRouteResult, MutUntrackedOrigin
    ](unsafe_from_address=Int(result_address))
    result[].abi_version = PRODEX_RICH_ABI_VERSION
    result[].selected_index = -1
    result[].ordered_written = 0
    result[].required_ordered = 0
    result[].issue_kind = 0
    result[].issue_index = -1
    result[].issue_offset = -1
    result[].issue_length = 0
    if abi_version != PRODEX_RICH_ABI_VERSION:
        result[].issue_kind = RICH_STATUS_ABI
        return RICH_STATUS_ABI
    if strategy_address == 0 or input_count < 0 or input_count > RICH_MAX_ROUTE_MODELS or ordered_capacity < 0 or ordered_capacity < input_count or ordered_indices_address == 0:
        return RICH_STATUS_INVALID
    var strategy = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(strategy_address))[].copy()
    if not rich_view_valid(strategy, RICH_MAX_IDENTIFIER_BYTES) or not rich_valid_identifier(strategy):
        result[].issue_kind = RICH_ISSUE_WHITESPACE
        return RICH_STATUS_OK
    var route_strategy = policy_route_strategy(strategy)
    if route_strategy < 0:
        result[].issue_kind = RICH_ISSUE_STRATEGY
        return RICH_STATUS_OK
    if input_count > 0 and inputs_address == 0:
        return RICH_STATUS_INVALID
    var inputs = Pointer[
        mut=False, ProdexRichPolicyRouteInput, ImmUntrackedOrigin
    ](unsafe_from_address=Int(inputs_address))
    var ordered = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(ordered_indices_address)
    )
    for index in range(input_count):
        var input = inputs[unsafe_offset=index].copy()
        if not rich_view_valid(input.model, RICH_MAX_IDENTIFIER_BYTES) or not rich_valid_identifier(input.model):
            result[].issue_kind = RICH_ISSUE_MODEL
            result[].issue_index = index
            return RICH_STATUS_OK
        if input.input_cost_present < 0 or input.input_cost_present > 1 or input.output_cost_present < 0 or input.output_cost_present > 1 or input.policy_latency_present < 0 or input.policy_latency_present > 1 or input.state_latency_present < 0 or input.state_latency_present > 1 or input.rpm_limit_present < 0 or input.rpm_limit_present > 1 or input.tpm_limit_present < 0 or input.tpm_limit_present > 1:
            return RICH_STATUS_INVALID
    if input_count == 0:
        return RICH_STATUS_OK
    var selected: Int64 = 0
    if route_strategy == ROUTE_ROUND_ROBIN:
        if request_id == 0:
            selected = 0
        else:
            selected = Int64((request_id - 1) % UInt64(input_count))
    elif route_strategy == ROUTE_LEAST_BUSY or route_strategy == ROUTE_LOWEST_COST or route_strategy == ROUTE_LOWEST_LATENCY or route_strategy == ROUTE_RPM or route_strategy == ROUTE_TPM:
        for index in range(Int64(1), input_count):
            if policy_route_better(route_strategy, inputs[unsafe_offset=index], inputs[unsafe_offset=selected], estimated_tokens):
                selected = index
    result[].selected_index = selected
    if route_strategy == ROUTE_FALLBACK:
        for index in range(input_count):
            ordered[unsafe_offset=index] = index
        result[].ordered_written = input_count
    else:
        ordered[unsafe_offset=0] = selected
        result[].ordered_written = 1
    result[].required_ordered = result[].ordered_written
    return RICH_STATUS_OK


@export("prodex_mojo_rich_policy_alias_v2")
def prodex_mojo_rich_policy_alias_v2(
    abi_version: Int64,
    input_address: UInt,
    output_models_address: UInt,
    model_capacity: Int64,
    output_address: UInt,
    output_capacity: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if result_address == 0:
        return RICH_STATUS_INVALID
    var result_ptr = Pointer[mut=True, ProdexRichPolicyResult, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result_ptr[].abi_version = PRODEX_RICH_ABI_VERSION
    result_ptr[].models_written = 0
    result_ptr[].required_models = 0
    result_ptr[].output_written = 0
    result_ptr[].required_output = 0
    result_ptr[].issue_kind = 0
    result_ptr[].issue_field = 0
    result_ptr[].issue_index = -1
    result_ptr[].issue_offset = -1
    result_ptr[].issue_length = 0
    if abi_version != PRODEX_RICH_ABI_VERSION:
        policy_issue(result_ptr, RICH_STATUS_ABI, 0, -1, -1, 0)
        return RICH_STATUS_ABI
    if input_address == 0:
        return RICH_STATUS_INVALID
    var input_ptr = Pointer[
        mut=False, ProdexRichPolicyInput, ImmUntrackedOrigin
    ](unsafe_from_address=Int(input_address))
    var input = input_ptr[].copy()
    result_ptr[].required_models = input.model_count
    if input.model_count < 0 or input.model_count > RICH_MAX_RECORDS or input.metric_count < 0 or input.metric_count > RICH_MAX_RECORDS or model_capacity < input.model_count:
        return RICH_STATUS_INVALID
    if output_models_address == 0 or output_address == 0:
        return RICH_STATUS_INVALID
    if not rich_view_valid(input.alias_view, RICH_MAX_IDENTIFIER_BYTES) or not rich_view_valid(input.strategy, RICH_MAX_IDENTIFIER_BYTES):
        policy_issue(result_ptr, RICH_ISSUE_INVALID_UTF8, RICH_FIELD_ALIAS, -1, 0, 0)
        return RICH_STATUS_UTF8
    if input.model_count > 0:
        if not input.models:
            return RICH_STATUS_INVALID
        var models = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
            unsafe_from_address=Int(input.models)
        )
        for index in range(input.model_count):
            if not rich_view_valid(models[unsafe_offset=index], RICH_MAX_IDENTIFIER_BYTES):
                policy_issue(result_ptr, RICH_ISSUE_INVALID_UTF8, RICH_FIELD_MODELS, index, 0, Int64(models[unsafe_offset=index].len))
                return RICH_STATUS_UTF8
            result_ptr[].required_output += Int64(models[unsafe_offset=index].len)
    if input.metric_count > 0:
        if not input.metrics:
            return RICH_STATUS_INVALID
        var metrics = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
            unsafe_from_address=Int(input.metrics)
        )
        for index in range(input.metric_count):
            if not rich_view_valid(metrics[unsafe_offset=index], RICH_MAX_IDENTIFIER_BYTES):
                policy_issue(result_ptr, RICH_ISSUE_INVALID_UTF8, RICH_FIELD_METRIC, index, 0, Int64(metrics[unsafe_offset=index].len))
                return RICH_STATUS_UTF8
            if not rich_valid_identifier(metrics[unsafe_offset=index]):
                policy_issue(result_ptr, RICH_ISSUE_EMPTY, RICH_FIELD_METRIC, index, 0, Int64(metrics[unsafe_offset=index].len))
                return RICH_STATUS_OK
    if output_capacity < result_ptr[].required_output:
        return RICH_STATUS_CAPACITY
    if not rich_valid_identifier(input.alias_view):
        if input.alias_view.len == 0:
            policy_issue(result_ptr, RICH_ISSUE_EMPTY, RICH_FIELD_ALIAS, -1, 0, 0)
        else:
            policy_issue(result_ptr, RICH_ISSUE_WHITESPACE, RICH_FIELD_ALIAS, -1, 0, Int64(input.alias_view.len))
        return RICH_STATUS_OK
    if input.model_count == 0:
        policy_issue(result_ptr, RICH_ISSUE_EMPTY, RICH_FIELD_MODELS, -1, 0, 0)
        return RICH_STATUS_OK
    if input.strategy.len > 0:
        if not rich_valid_identifier(input.strategy):
            policy_issue(result_ptr, RICH_ISSUE_WHITESPACE, RICH_FIELD_STRATEGY, -1, 0, Int64(input.strategy.len))
            return RICH_STATUS_OK
        if not policy_strategy_valid(input.strategy):
            policy_issue(result_ptr, RICH_ISSUE_STRATEGY, RICH_FIELD_STRATEGY, -1, 0, Int64(input.strategy.len))
            return RICH_STATUS_OK
    if input.metric_count > 0:
        var metrics = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
            unsafe_from_address=Int(input.metrics)
        )
        var models = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
            unsafe_from_address=Int(input.models)
        )
        for metric_index in range(input.metric_count):
            var matched = False
            for model_index in range(input.model_count):
                if rich_views_equal(metrics[unsafe_offset=metric_index], models[unsafe_offset=model_index]):
                    matched = True
                    break
            if not matched:
                policy_issue(result_ptr, RICH_ISSUE_MODEL, RICH_FIELD_METRIC, metric_index, 0, Int64(metrics[unsafe_offset=metric_index].len))
                return RICH_STATUS_OK
    var written: Int64 = 0
    var output_models = Pointer[mut=True, ProdexRichPolicyModel, MutUntrackedOrigin](
        unsafe_from_address=Int(output_models_address)
    )
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var models = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(input.models)
    )
    for index in range(input.model_count):
        if not rich_valid_identifier(models[unsafe_offset=index]):
            policy_issue(result_ptr, RICH_ISSUE_EMPTY, RICH_FIELD_MODELS, index, 0, Int64(models[unsafe_offset=index].len))
            return RICH_STATUS_OK
        var slice = rich_copy_trimmed(models[unsafe_offset=index], output, output_capacity, Pointer(to=written), True)
        if slice.len < 0:
            return RICH_STATUS_CAPACITY
        var rule = PolicyRule(ProdexRichSlice(0, 0), slice.copy(), -1)
        if input.metric_count > 0:
            var metrics = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
                unsafe_from_address=Int(input.metrics)
            )
            for metric_index in range(input.metric_count):
                if rich_views_equal(metrics[unsafe_offset=metric_index], models[unsafe_offset=index]):
                    rule.metric_match = metric_index
                    break
        output_models[unsafe_offset=index].model = rule.model.copy()
        output_models[unsafe_offset=index].model_index = index
        output_models[unsafe_offset=index].metric_match = rule.metric_match
    result_ptr[].models_written = input.model_count
    result_ptr[].output_written = written
    return RICH_STATUS_OK
