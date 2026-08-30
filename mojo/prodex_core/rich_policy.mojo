from std.memory import Pointer

from rich_text import (
    rich_copy_trimmed,
    rich_trim_bounds,
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
    rich_view_ptr,
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


comptime APPLICATION_OBLIGATION_ABI_VERSION: Int64 = 1
comptime APPLICATION_OBLIGATION_MAX_COUNT: Int64 = 256
comptime APPLICATION_OBLIGATION_MAX_VIOLATIONS: Int64 = 32
comptime APPLICATION_OBLIGATION_MAX_TEXT_BYTES: Int64 = 4_096

comptime APPLICATION_OBLIGATION_MASK_FINDING: Int64 = 0
comptime APPLICATION_OBLIGATION_DISABLE_TOOLS: Int64 = 1
comptime APPLICATION_OBLIGATION_ALLOW_TOOL: Int64 = 2
comptime APPLICATION_OBLIGATION_ALLOW_MODEL: Int64 = 3
comptime APPLICATION_OBLIGATION_ALLOW_MODALITY: Int64 = 4
comptime APPLICATION_OBLIGATION_MAX_INPUT_TOKENS: Int64 = 5
comptime APPLICATION_OBLIGATION_MAX_OUTPUT_TOKENS: Int64 = 6
comptime APPLICATION_OBLIGATION_MAX_CONTEXT_TOKENS: Int64 = 7
comptime APPLICATION_OBLIGATION_REQUIRE_RESPONSE_INSPECTION: Int64 = 8
comptime APPLICATION_OBLIGATION_SESSION_IDLE_TIMEOUT: Int64 = 9
comptime APPLICATION_OBLIGATION_SESSION_ABSOLUTE_TIMEOUT: Int64 = 10
comptime APPLICATION_OBLIGATION_MIN_AUTHENTICATION_STRENGTH: Int64 = 11
comptime APPLICATION_OBLIGATION_REQUIRE_REAUTHENTICATION: Int64 = 12
comptime APPLICATION_OBLIGATION_REQUIRE_MFA: Int64 = 13
comptime APPLICATION_OBLIGATION_REQUIRE_HUMAN_APPROVAL: Int64 = 14
comptime APPLICATION_OBLIGATION_OTHER: Int64 = 15

comptime APPLICATION_OBLIGATION_POLICY_DENIED: Int64 = 0
comptime APPLICATION_OBLIGATION_APPROVAL_REQUIRED: Int64 = 1
comptime APPLICATION_OBLIGATION_REQUIRED_MASK_MISSING: Int64 = 2
comptime APPLICATION_OBLIGATION_TOOLS_DISABLED: Int64 = 3
comptime APPLICATION_OBLIGATION_TOOL_NOT_ALLOWED: Int64 = 4
comptime APPLICATION_OBLIGATION_TOOL_METADATA_UNSUPPORTED: Int64 = 5
comptime APPLICATION_OBLIGATION_MODEL_NOT_ALLOWED: Int64 = 6
comptime APPLICATION_OBLIGATION_MODALITY_NOT_ALLOWED: Int64 = 7
comptime APPLICATION_OBLIGATION_INPUT_TOKEN_LIMIT_EXCEEDED: Int64 = 8
comptime APPLICATION_OBLIGATION_OUTPUT_TOKEN_LIMIT_EXCEEDED: Int64 = 9
comptime APPLICATION_OBLIGATION_CONTEXT_TOKEN_LIMIT_EXCEEDED: Int64 = 10
comptime APPLICATION_OBLIGATION_RESPONSE_INSPECTION_UNSUPPORTED: Int64 = 11
comptime APPLICATION_OBLIGATION_RESPONSE_INSPECTION_INCOMPLETE: Int64 = 12
comptime APPLICATION_OBLIGATION_SESSION_REVOKED: Int64 = 13
comptime APPLICATION_OBLIGATION_SESSION_IDLE_TIMEOUT_VIOLATION: Int64 = 14
comptime APPLICATION_OBLIGATION_SESSION_ABSOLUTE_TIMEOUT_VIOLATION: Int64 = 15
comptime APPLICATION_OBLIGATION_AUTHENTICATION_STRENGTH_REQUIRED: Int64 = 16
comptime APPLICATION_OBLIGATION_REAUTHENTICATION_REQUIRED: Int64 = 17
comptime APPLICATION_OBLIGATION_MFA_REQUIRED: Int64 = 18


@fieldwise_init
struct ApplicationObligationResult(Copyable):
    var abi_version: Int64
    var mask_count: Int64
    var disable_tools: Int64
    var maximum_input_present: Int64
    var maximum_input_tokens: UInt64
    var maximum_context_present: Int64
    var maximum_context_tokens: UInt64
    var maximum_output_present: Int64
    var maximum_output_tokens: UInt64
    var enforce: Int64
    var inspection_required: Int64
    var require_full_inspection: Int64
    var violation_count: Int64
    var disposition: Int64


def application_obligation_add_unique(
    values: Pointer[mut=True, Int64, _],
    count: Pointer[mut=True, Int64, _],
    capacity: Int64,
    value: Int64,
) -> Bool:
    for index in range(count[]):
        if values[unsafe_offset=index] == value:
            return True
    if count[] >= capacity:
        return False
    values[unsafe_offset=count[]] = value
    count[] += 1
    return True


# ponytail: bounded selection sort keeps output deterministic for 32 tags.
def application_obligation_sort(
    values: Pointer[mut=True, Int64, _], count: Int64
) -> None:
    for position in range(count):
        var best = position
        for index in range(position + 1, count):
            if values[unsafe_offset=index] < values[unsafe_offset=best]:
                best = index
        if best != position:
            var saved = values[unsafe_offset=position]
            values[unsafe_offset=position] = values[unsafe_offset=best]
            values[unsafe_offset=best] = saved


def application_obligation_selector_matches(
    selector: ProdexRichStringView, requested: ProdexRichStringView
) -> Bool:
    return rich_view_matches_literal["*"](selector, False) or rich_views_equal(
        selector, requested
    )


@export("prodex_mojo_rich_application_obligation_plan_v1")
def prodex_mojo_rich_application_obligation_plan_v1(
    abi_version: Int64,
    mode: Int64,
    effect: Int64,
    obligation_kinds_address: UInt,
    obligation_values_address: UInt,
    obligation_selectors_address: UInt,
    obligation_count: Int64,
    classification: Int64,
    inspection_coverage: Int64,
    detected_findings_mask: Int64,
    masked_findings_mask: Int64,
    requested_capabilities_mask: Int64,
    requested_model_address: UInt,
    requested_model_present: Int64,
    requested_tools_address: UInt,
    requested_tools_present: Int64,
    requested_tools_count: Int64,
    requested_modalities_mask: Int64,
    estimated_input_tokens: UInt64,
    estimated_context_tokens: UInt64,
    requested_output_present: Int64,
    requested_output_tokens: UInt64,
    session_age_seconds: UInt64,
    session_idle_seconds: UInt64,
    session_revoked: Int64,
    session_mfa_satisfied: Int64,
    authentication_strength: Int64,
    environment_mfa_satisfied: Int64,
    reauthentication_satisfied: Int64,
    response_transport: Int64,
    response_inspection_coverage: Int64,
    output_masks_address: UInt,
    output_mask_capacity: Int64,
    output_violations_address: UInt,
    output_violation_capacity: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if result_address == 0:
        return RICH_STATUS_INVALID
    var result = Pointer[
        mut=True, ApplicationObligationResult, MutUntrackedOrigin
    ](unsafe_from_address=Int(result_address))
    result[].abi_version = APPLICATION_OBLIGATION_ABI_VERSION
    result[].mask_count = 0
    result[].disable_tools = 0
    result[].maximum_input_present = 0
    result[].maximum_input_tokens = 0
    result[].maximum_context_present = 0
    result[].maximum_context_tokens = 0
    result[].maximum_output_present = 0
    result[].maximum_output_tokens = 0
    result[].enforce = 0
    result[].inspection_required = 0
    result[].require_full_inspection = 0
    result[].violation_count = 0
    result[].disposition = 0

    if abi_version != APPLICATION_OBLIGATION_ABI_VERSION:
        return RICH_STATUS_ABI
    if (
        mode < 0
        or mode > 2
        or effect < 0
        or effect > 2
        or obligation_count < 0
        or obligation_count > APPLICATION_OBLIGATION_MAX_COUNT
        or classification < 0
        or classification > 3
        or inspection_coverage < 0
        or inspection_coverage > 2
        or detected_findings_mask < 0
        or detected_findings_mask > 4095
        or masked_findings_mask < 0
        or masked_findings_mask > 4095
        or requested_capabilities_mask < 0
        or requested_capabilities_mask > 127
        or requested_modalities_mask < 0
        or requested_modalities_mask > 31
        or requested_tools_count < 0
        or requested_tools_count > APPLICATION_OBLIGATION_MAX_COUNT
        or response_transport < 0
        or response_transport > 2
        or response_inspection_coverage < 0
        or response_inspection_coverage > 2
        or output_mask_capacity < obligation_count
        or output_mask_capacity > APPLICATION_OBLIGATION_MAX_COUNT
        or output_violation_capacity < 19
        or output_violation_capacity > APPLICATION_OBLIGATION_MAX_VIOLATIONS
        or output_masks_address == 0
        or output_violations_address == 0
    ):
        return RICH_STATUS_INVALID
    if requested_model_present < 0 or requested_model_present > 1:
        return RICH_STATUS_INVALID
    if requested_tools_present < 0 or requested_tools_present > 1:
        return RICH_STATUS_INVALID
    if requested_output_present < 0 or requested_output_present > 1:
        return RICH_STATUS_INVALID
    if (
        session_revoked < 0
        or session_revoked > 1
        or session_mfa_satisfied < 0
        or session_mfa_satisfied > 1
        or authentication_strength < 0
        or authentication_strength > 255
        or environment_mfa_satisfied < 0
        or environment_mfa_satisfied > 1
        or reauthentication_satisfied < 0
        or reauthentication_satisfied > 1
    ):
        return RICH_STATUS_INVALID
    if obligation_count > 0 and (
        obligation_kinds_address == 0
        or obligation_values_address == 0
        or obligation_selectors_address == 0
    ):
        return RICH_STATUS_INVALID
    if requested_model_present == 1 and requested_model_address == 0:
        return RICH_STATUS_INVALID
    if requested_tools_count > 0 and requested_tools_address == 0:
        return RICH_STATUS_INVALID

    var kinds = Pointer[
        mut=False, Int64, ImmUntrackedOrigin
    ](unsafe_from_address=Int(obligation_kinds_address))
    var values = Pointer[
        mut=False, UInt64, ImmUntrackedOrigin
    ](unsafe_from_address=Int(obligation_values_address))
    var selectors = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(obligation_selectors_address))
    var output_masks = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(output_masks_address))
    var output_violations = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(output_violations_address))

    var requested_model = ProdexRichStringView(0, 0)
    if requested_model_present == 1:
        requested_model = Pointer[
            mut=False, ProdexRichStringView, ImmUntrackedOrigin
        ](unsafe_from_address=Int(requested_model_address))[].copy()
        if not rich_view_valid(requested_model, APPLICATION_OBLIGATION_MAX_TEXT_BYTES):
            return RICH_STATUS_UTF8
    var requested_tools = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(requested_tools_address))
    if requested_tools_count > 0:
        for index in range(requested_tools_count):
            if not rich_view_valid(
                requested_tools[unsafe_offset=index],
                APPLICATION_OBLIGATION_MAX_TEXT_BYTES,
            ):
                return RICH_STATUS_UTF8
    for index in range(obligation_count):
        if not rich_view_valid(
            selectors[unsafe_offset=index], APPLICATION_OBLIGATION_MAX_TEXT_BYTES
        ):
            return RICH_STATUS_UTF8
        var kind = kinds[unsafe_offset=index]
        if kind < APPLICATION_OBLIGATION_MASK_FINDING or kind > APPLICATION_OBLIGATION_OTHER:
            return RICH_STATUS_INVALID
        var value = values[unsafe_offset=index]
        if kind == APPLICATION_OBLIGATION_MASK_FINDING and value > 11:
            return RICH_STATUS_INVALID
        if kind == APPLICATION_OBLIGATION_ALLOW_MODALITY and value > 4:
            return RICH_STATUS_INVALID

    var mask_count: Int64 = 0
    var violation_count: Int64 = 0
    var disable_tools: Int64 = 0
    var allowed_tool_count: Int64 = 0
    var allowed_model_count: Int64 = 0
    var allowed_modalities_mask: Int64 = 0
    var maximum_input_present: Int64 = 0
    var maximum_input_tokens: UInt64 = 0
    var maximum_context_present: Int64 = 0
    var maximum_context_tokens: UInt64 = 0
    var maximum_output_present: Int64 = 0
    var maximum_output_tokens: UInt64 = 0

    if effect == 2 and not application_obligation_add_unique(
        output_violations,
        Pointer(to=violation_count),
        output_violation_capacity,
        APPLICATION_OBLIGATION_POLICY_DENIED,
    ):
        return RICH_STATUS_CAPACITY
    elif effect == 1 and not application_obligation_add_unique(
        output_violations,
        Pointer(to=violation_count),
        output_violation_capacity,
        APPLICATION_OBLIGATION_APPROVAL_REQUIRED,
    ):
        return RICH_STATUS_CAPACITY
    if session_revoked == 1 and not application_obligation_add_unique(
        output_violations,
        Pointer(to=violation_count),
        output_violation_capacity,
        APPLICATION_OBLIGATION_SESSION_REVOKED,
    ):
        return RICH_STATUS_CAPACITY

    for index in range(obligation_count):
        var kind = kinds[unsafe_offset=index]
        var value = values[unsafe_offset=index]
        if kind == APPLICATION_OBLIGATION_MASK_FINDING:
            if not application_obligation_add_unique(
                output_masks,
                Pointer(to=mask_count),
                output_mask_capacity,
                Int64(value),
            ):
                return RICH_STATUS_CAPACITY
            if (
                detected_findings_mask & (Int64(1) << Int64(value)) != 0
                and masked_findings_mask & (Int64(1) << Int64(value)) == 0
                and not application_obligation_add_unique(
                    output_violations,
                    Pointer(to=violation_count),
                    output_violation_capacity,
                    APPLICATION_OBLIGATION_REQUIRED_MASK_MISSING,
                )
            ):
                return RICH_STATUS_CAPACITY
        elif kind == APPLICATION_OBLIGATION_DISABLE_TOOLS:
            disable_tools = 1
            if requested_capabilities_mask & 4 != 0 and not application_obligation_add_unique(
                output_violations,
                Pointer(to=violation_count),
                output_violation_capacity,
                APPLICATION_OBLIGATION_TOOLS_DISABLED,
            ):
                return RICH_STATUS_CAPACITY
        elif kind == APPLICATION_OBLIGATION_ALLOW_TOOL:
            allowed_tool_count += 1
        elif kind == APPLICATION_OBLIGATION_ALLOW_MODEL:
            allowed_model_count += 1
        elif kind == APPLICATION_OBLIGATION_ALLOW_MODALITY:
            allowed_modalities_mask |= Int64(1) << Int64(value)
        elif kind == APPLICATION_OBLIGATION_MAX_INPUT_TOKENS:
            if maximum_input_present == 0 or value < maximum_input_tokens:
                maximum_input_present = 1
                maximum_input_tokens = value
            if estimated_input_tokens > value and not application_obligation_add_unique(
                output_violations,
                Pointer(to=violation_count),
                output_violation_capacity,
                APPLICATION_OBLIGATION_INPUT_TOKEN_LIMIT_EXCEEDED,
            ):
                return RICH_STATUS_CAPACITY
        elif kind == APPLICATION_OBLIGATION_MAX_OUTPUT_TOKENS:
            if maximum_output_present == 0 or value < maximum_output_tokens:
                maximum_output_present = 1
                maximum_output_tokens = value
            if requested_output_present == 1 and requested_output_tokens > value and not application_obligation_add_unique(
                output_violations,
                Pointer(to=violation_count),
                output_violation_capacity,
                APPLICATION_OBLIGATION_OUTPUT_TOKEN_LIMIT_EXCEEDED,
            ):
                return RICH_STATUS_CAPACITY
        elif kind == APPLICATION_OBLIGATION_MAX_CONTEXT_TOKENS:
            if maximum_context_present == 0 or value < maximum_context_tokens:
                maximum_context_present = 1
                maximum_context_tokens = value
            if estimated_context_tokens > value and not application_obligation_add_unique(
                output_violations,
                Pointer(to=violation_count),
                output_violation_capacity,
                APPLICATION_OBLIGATION_CONTEXT_TOKEN_LIMIT_EXCEEDED,
            ):
                return RICH_STATUS_CAPACITY
        elif kind == APPLICATION_OBLIGATION_REQUIRE_RESPONSE_INSPECTION:
            result[].inspection_required = 1
            if response_inspection_coverage == 2 and not application_obligation_add_unique(
                output_violations,
                Pointer(to=violation_count),
                output_violation_capacity,
                APPLICATION_OBLIGATION_RESPONSE_INSPECTION_UNSUPPORTED,
            ):
                return RICH_STATUS_CAPACITY
            if mode == 2 and response_inspection_coverage != 0 and not application_obligation_add_unique(
                output_violations,
                Pointer(to=violation_count),
                output_violation_capacity,
                APPLICATION_OBLIGATION_RESPONSE_INSPECTION_INCOMPLETE,
            ):
                return RICH_STATUS_CAPACITY
        elif kind == APPLICATION_OBLIGATION_SESSION_IDLE_TIMEOUT:
            if session_idle_seconds > value and not application_obligation_add_unique(
                output_violations,
                Pointer(to=violation_count),
                output_violation_capacity,
                APPLICATION_OBLIGATION_SESSION_IDLE_TIMEOUT_VIOLATION,
            ):
                return RICH_STATUS_CAPACITY
        elif kind == APPLICATION_OBLIGATION_SESSION_ABSOLUTE_TIMEOUT:
            if session_age_seconds > value and not application_obligation_add_unique(
                output_violations,
                Pointer(to=violation_count),
                output_violation_capacity,
                APPLICATION_OBLIGATION_SESSION_ABSOLUTE_TIMEOUT_VIOLATION,
            ):
                return RICH_STATUS_CAPACITY
        elif kind == APPLICATION_OBLIGATION_MIN_AUTHENTICATION_STRENGTH:
            if UInt64(authentication_strength) < value and not application_obligation_add_unique(
                output_violations,
                Pointer(to=violation_count),
                output_violation_capacity,
                APPLICATION_OBLIGATION_AUTHENTICATION_STRENGTH_REQUIRED,
            ):
                return RICH_STATUS_CAPACITY
        elif kind == APPLICATION_OBLIGATION_REQUIRE_REAUTHENTICATION:
            if reauthentication_satisfied == 0 and not application_obligation_add_unique(
                output_violations,
                Pointer(to=violation_count),
                output_violation_capacity,
                APPLICATION_OBLIGATION_REAUTHENTICATION_REQUIRED,
            ):
                return RICH_STATUS_CAPACITY
        elif kind == APPLICATION_OBLIGATION_REQUIRE_MFA:
            if (
                session_mfa_satisfied == 0
                or environment_mfa_satisfied == 0
            ) and not application_obligation_add_unique(
                output_violations,
                Pointer(to=violation_count),
                output_violation_capacity,
                APPLICATION_OBLIGATION_MFA_REQUIRED,
            ):
                return RICH_STATUS_CAPACITY
        elif kind == APPLICATION_OBLIGATION_REQUIRE_HUMAN_APPROVAL:
            if not application_obligation_add_unique(
                output_violations,
                Pointer(to=violation_count),
                output_violation_capacity,
                APPLICATION_OBLIGATION_APPROVAL_REQUIRED,
            ):
                return RICH_STATUS_CAPACITY

    if allowed_tool_count > 0 and requested_capabilities_mask & 4 != 0:
        if requested_tools_present == 0:
            if not application_obligation_add_unique(
                output_violations,
                Pointer(to=violation_count),
                output_violation_capacity,
                APPLICATION_OBLIGATION_TOOL_METADATA_UNSUPPORTED,
            ):
                return RICH_STATUS_CAPACITY
        else:
            for requested_index in range(requested_tools_count):
                var matched = False
                var requested = requested_tools[unsafe_offset=requested_index].copy()
                for obligation_index in range(obligation_count):
                    if kinds[unsafe_offset=obligation_index] == APPLICATION_OBLIGATION_ALLOW_TOOL and application_obligation_selector_matches(
                        selectors[unsafe_offset=obligation_index], requested
                    ):
                        matched = True
                        break
                if not matched and not application_obligation_add_unique(
                    output_violations,
                    Pointer(to=violation_count),
                    output_violation_capacity,
                    APPLICATION_OBLIGATION_TOOL_NOT_ALLOWED,
                ):
                    return RICH_STATUS_CAPACITY

    if allowed_model_count > 0:
        var matched = False
        if requested_model_present == 1:
            for obligation_index in range(obligation_count):
                if kinds[unsafe_offset=obligation_index] == APPLICATION_OBLIGATION_ALLOW_MODEL and application_obligation_selector_matches(
                    selectors[unsafe_offset=obligation_index], requested_model
                ):
                    matched = True
                    break
        if not matched and not application_obligation_add_unique(
            output_violations,
            Pointer(to=violation_count),
            output_violation_capacity,
            APPLICATION_OBLIGATION_MODEL_NOT_ALLOWED,
        ):
            return RICH_STATUS_CAPACITY

    if allowed_modalities_mask != 0 and requested_modalities_mask & (
        31 ^ allowed_modalities_mask
    ) != 0 and not application_obligation_add_unique(
        output_violations,
        Pointer(to=violation_count),
        output_violation_capacity,
        APPLICATION_OBLIGATION_MODALITY_NOT_ALLOWED,
    ):
        return RICH_STATUS_CAPACITY

    application_obligation_sort(output_masks, mask_count)
    application_obligation_sort(output_violations, violation_count)
    result[].mask_count = mask_count
    result[].disable_tools = disable_tools
    result[].maximum_input_present = maximum_input_present
    result[].maximum_input_tokens = maximum_input_tokens
    result[].maximum_context_present = maximum_context_present
    result[].maximum_context_tokens = maximum_context_tokens
    result[].maximum_output_present = maximum_output_present
    result[].maximum_output_tokens = maximum_output_tokens
    result[].enforce = 1 if mode == 1 or mode == 2 else 0
    result[].require_full_inspection = 1 if result[].inspection_required == 1 and mode == 2 else 0
    result[].violation_count = violation_count
    result[].disposition = 1 if result[].enforce == 1 and violation_count > 0 else 0
    return RICH_STATUS_OK


comptime APPLICATION_METADATA_ABI_VERSION: Int64 = 1
comptime APPLICATION_METADATA_MAX_HEADERS: Int64 = 64
comptime APPLICATION_METADATA_MAX_TEXT_BYTES: Int64 = 4_096

@fieldwise_init
struct ApplicationRequestMetadataResult(Copyable):
    var abi_version: Int64
    var observed_header_count: Int64
    var headers_truncated: Int64
    var trace_context_present: Int64
    var credential_present: Int64
    var affinity_present: Int64
    var codex_metadata_present: Int64
    var user_agent_present: Int64


def application_metadata_header_matches(
    view: ProdexRichStringView, literal: StringSlice
) -> Bool:
    var bounds = rich_trim_bounds(view)
    var length = bounds[1] - bounds[0]
    if length != Int64(literal.byte_length()):
        return False
    var right = literal.unsafe_ptr()
    var left = rich_view_ptr(view)
    for index in range(length):
        var value = left[unsafe_offset=bounds[0] + index]
        if value >= 65 and value <= 90:
            value += 32
        if value != right[unsafe_offset=index]:
            return False
    return True


@export("prodex_mojo_rich_application_request_metadata_v1")
def prodex_mojo_rich_application_request_metadata_v1(
    abi_version: Int64,
    header_names_address: UInt,
    header_count: Int64,
    total_header_count: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if result_address == 0:
        return RICH_STATUS_INVALID
    var result = Pointer[
        mut=True, ApplicationRequestMetadataResult, MutUntrackedOrigin
    ](unsafe_from_address=Int(result_address))
    result[].abi_version = APPLICATION_METADATA_ABI_VERSION
    result[].observed_header_count = 0
    result[].headers_truncated = 0
    result[].trace_context_present = 0
    result[].credential_present = 0
    result[].affinity_present = 0
    result[].codex_metadata_present = 0
    result[].user_agent_present = 0
    if (
        abi_version != APPLICATION_METADATA_ABI_VERSION
        or header_count < 0
        or header_count > APPLICATION_METADATA_MAX_HEADERS
        or total_header_count < header_count
    ):
        return RICH_STATUS_INVALID
    if header_count > 0 and header_names_address == 0:
        return RICH_STATUS_INVALID
    var header_names = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(header_names_address))
    for index in range(header_count):
        if not rich_view_valid(
            header_names[unsafe_offset=index], APPLICATION_METADATA_MAX_TEXT_BYTES
        ):
            return RICH_STATUS_UTF8
    result[].observed_header_count = header_count
    result[].headers_truncated = 1 if total_header_count > header_count else 0
    for index in range(header_count):
        var name = header_names[unsafe_offset=index].copy()
        if application_metadata_header_matches(name, StringSlice("traceparent")):
            result[].trace_context_present = 1
        if application_metadata_header_matches(name, StringSlice("authorization")) or application_metadata_header_matches(name, StringSlice("chatgpt-account-id")):
            result[].credential_present = 1
        if application_metadata_header_matches(name, StringSlice("session_id")) or application_metadata_header_matches(name, StringSlice("x-codex-turn-state")):
            result[].affinity_present = 1
        if application_metadata_header_matches(name, StringSlice("x-openai-subagent")) or application_metadata_header_matches(name, StringSlice("x-codex-turn-metadata")) or application_metadata_header_matches(name, StringSlice("x-codex-beta-features")):
            result[].codex_metadata_present = 1
        if application_metadata_header_matches(name, StringSlice("user-agent")):
            result[].user_agent_present = 1
    return RICH_STATUS_OK
