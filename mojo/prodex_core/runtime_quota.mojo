from std.memory import Pointer

def prodex_runtime_quota_pressure_band_for_status(
    remaining_percent: Int64,
    has_value: Int64,
    thin_threshold: Int64,
    critical_threshold: Int64,
) -> Int64:
    if has_value == 0:
        return 0
    if remaining_percent == 0:
        return 3
    if remaining_percent <= critical_threshold:
        return 2
    if remaining_percent <= thin_threshold:
        return 1
    return 0

@export("prodex_runtime_quota_pressure_band_for_route")
def prodex_runtime_quota_pressure_band_for_route(
    five_hour_remaining_percent: Int64,
    five_hour_has_value: Int64,
    weekly_remaining_percent: Int64,
    weekly_has_value: Int64,
    route_kind: Int64,
) abi("C") -> Int64:
    if five_hour_has_value == 0 and weekly_has_value == 0:
        return 4
    if (five_hour_has_value != 0 and five_hour_remaining_percent == 0) or (
        weekly_has_value != 0 and weekly_remaining_percent == 0
    ):
        return 3

    var thin_weekly: Int64 = 10
    var thin_five_hour: Int64 = 5
    var critical_weekly: Int64 = 5
    var critical_five_hour: Int64 = 3
    if route_kind == 0 or route_kind == 2:
        thin_weekly = 20
        thin_five_hour = 10
        critical_weekly = 10
        critical_five_hour = 5

    var weekly_band = prodex_runtime_quota_pressure_band_for_status(
        weekly_remaining_percent,
        weekly_has_value,
        thin_weekly,
        critical_weekly,
    )
    var five_hour_band = prodex_runtime_quota_pressure_band_for_status(
        five_hour_remaining_percent,
        five_hour_has_value,
        thin_five_hour,
        critical_five_hour,
    )
    if weekly_band > five_hour_band:
        return weekly_band
    return five_hour_band

@export("prodex_runtime_quota_profile_score_batch")
def prodex_runtime_quota_profile_score_batch(
    weekly_pressure: Pointer[mut=False, Int64, _],
    five_hour_pressure: Pointer[mut=False, Int64, _],
    scale_bps: Pointer[mut=False, Int64, _],
    weekly_remaining: Pointer[mut=False, Int64, _],
    five_hour_remaining: Pointer[mut=False, Int64, _],
    reserve_bias: Pointer[mut=False, Int64, _],
    weekly_weight: Pointer[mut=False, Int64, _],
    total_pressure: Pointer[mut=True, Int64, _],
    scaled_weekly_pressure: Pointer[mut=True, Int64, _],
    scaled_five_hour_pressure: Pointer[mut=True, Int64, _],
    reserve_floor: Pointer[mut=True, Int64, _],
    count: Int64,
) abi("C") -> Int64:
    if count < 0 or count > 64:
        return 1

    for index in range(count):
        var weekly_value = weekly_pressure[unsafe_offset=index]
        var five_hour_value = five_hour_pressure[unsafe_offset=index]
        var scale_value = scale_bps[unsafe_offset=index]
        var weekly_remaining_value = weekly_remaining[unsafe_offset=index]
        var five_hour_remaining_value = five_hour_remaining[unsafe_offset=index]
        var reserve_bias_value = reserve_bias[unsafe_offset=index]
        var weekly_weight_value = weekly_weight[unsafe_offset=index]
        if weekly_value < 0 or five_hour_value < 0 or scale_value < 0 or reserve_bias_value < 0 or weekly_weight_value < 0:
            return 2
        if weekly_remaining_value < 0 or weekly_remaining_value > 100 or five_hour_remaining_value < 0 or five_hour_remaining_value > 100:
            return 2

        var weekly_scaled = runtime_quota_scale_pressure(weekly_value, scale_value)
        var five_hour_scaled = runtime_quota_scale_pressure(five_hour_value, scale_value)
        var weighted_weekly = runtime_quota_saturating_mul(weekly_scaled, weekly_weight_value)
        var total = runtime_quota_saturating_add(reserve_bias_value, weighted_weekly)
        total = runtime_quota_saturating_add(total, five_hour_scaled)
        total_pressure[unsafe_offset=index] = total
        scaled_weekly_pressure[unsafe_offset=index] = weekly_scaled
        scaled_five_hour_pressure[unsafe_offset=index] = five_hour_scaled
        if weekly_remaining_value < five_hour_remaining_value:
            reserve_floor[unsafe_offset=index] = weekly_remaining_value
        else:
            reserve_floor[unsafe_offset=index] = five_hour_remaining_value

    return 0

comptime RUNTIME_PROFILE_ORDER_FIELD_COUNT: Int64 = 15
comptime RUNTIME_PROFILE_ORDER_MAX_COUNT: Int64 = 256

def runtime_profile_order_field(
    fields: Pointer[mut=False, Int64, _],
    index: Int64,
    field: Int64,
) -> Int64:
    return fields[unsafe_offset=(index * RUNTIME_PROFILE_ORDER_FIELD_COUNT) + field]

def runtime_profile_order_less(
    fields: Pointer[mut=False, Int64, _],
    left: Int64,
    right: Int64,
) -> Bool:
    for field in range(7):
        var left_value = runtime_profile_order_field(fields, left, Int64(field))
        var right_value = runtime_profile_order_field(fields, right, Int64(field))
        if left_value != right_value:
            return left_value < right_value

    for field in range(7, 10):
        var left_value = runtime_profile_order_field(fields, left, Int64(field))
        var right_value = runtime_profile_order_field(fields, right, Int64(field))
        if left_value != right_value:
            return left_value > right_value

    for field in range(10, 15):
        var left_value = runtime_profile_order_field(fields, left, Int64(field))
        var right_value = runtime_profile_order_field(fields, right, Int64(field))
        if left_value != right_value:
            return left_value < right_value

    return left < right

@export("prodex_runtime_quota_profile_order_batch")
def prodex_runtime_quota_profile_order_batch(
    fields: Pointer[mut=False, Int64, _],
    ordered_indices: Pointer[mut=True, Int64, _],
    ordered_count: Pointer[mut=True, Int64, _],
    count: Int64,
) abi("C") -> Int64:
    if count < 0 or count > RUNTIME_PROFILE_ORDER_MAX_COUNT:
        return 1
    for index in range(count):
        var near_optimal = runtime_profile_order_field(fields, index, 1)
        var recently_used = runtime_profile_order_field(fields, index, 2)
        var preferred = runtime_profile_order_field(fields, index, 13)
        if near_optimal < 0 or near_optimal > 1 or recently_used < 0 or recently_used > 1 or preferred < 0 or preferred > 1:
            return 2

    var sorted_count: Int64 = 0
    for index in range(count):
        var position = sorted_count
        while position > 0 and runtime_profile_order_less(
            fields,
            index,
            ordered_indices[unsafe_offset=position - 1],
        ):
            ordered_indices[unsafe_offset=position] = ordered_indices[unsafe_offset=position - 1]
            position -= 1
        ordered_indices[unsafe_offset=position] = index
        sorted_count += 1

    ordered_count[unsafe_offset=0] = sorted_count
    return 0

comptime INT64_MAX: Int64 = 9223372036854775807

def runtime_quota_saturating_add(left: Int64, right: Int64) -> Int64:
    if right > 0 and left > INT64_MAX - right:
        return INT64_MAX
    return left + right

def runtime_quota_saturating_mul(left: Int64, right: Int64) -> Int64:
    if left <= 0 or right <= 0:
        return 0
    if left > INT64_MAX / right:
        return INT64_MAX
    return left * right

def runtime_quota_scale_pressure(pressure: Int64, scale_bps: Int64) -> Int64:
    if pressure == INT64_MAX:
        return INT64_MAX
    var scale = scale_bps
    if scale < 0:
        scale = 0
    if pressure < 0:
        return pressure
    if scale == 0 or pressure == 0:
        return 0
    if pressure > INT64_MAX / scale:
        return INT64_MAX
    return (pressure * scale) / 10_000

comptime RUNTIME_CANDIDATE_PLAN_FIELD_COUNT: Int64 = 22
comptime RUNTIME_CANDIDATE_PLAN_MAX_COUNT: Int64 = 256

def runtime_candidate_field(fields: Pointer[mut=False, Int64, _], index: Int64, field: Int64) -> Int64:
    return fields[unsafe_offset=(index * RUNTIME_CANDIDATE_PLAN_FIELD_COUNT) + field]

def runtime_candidate_source_sort_key(route_kind: Int64, source: Int64) -> Int64:
    if route_kind == 0 or route_kind == 2:
        return source
    return 0

def runtime_candidate_ready_less(
    fields: Pointer[mut=False, Int64, _],
    left: Int64,
    right: Int64,
    route_kind: Int64,
) -> Bool:
    var left_value = runtime_candidate_field(fields, left, 1)
    var right_value = runtime_candidate_field(fields, right, 1)
    if left_value != right_value:
        return left_value < right_value

    # quota_sort_key = (band, a, b, c, Reverse(d), Reverse(e), Reverse(f), g, h)
    left_value = runtime_candidate_field(fields, left, 2)
    right_value = runtime_candidate_field(fields, right, 2)
    if left_value != right_value:
        return left_value < right_value
    left_value = runtime_candidate_field(fields, left, 3)
    right_value = runtime_candidate_field(fields, right, 3)
    if left_value != right_value:
        return left_value < right_value
    left_value = runtime_candidate_field(fields, left, 4)
    right_value = runtime_candidate_field(fields, right, 4)
    if left_value != right_value:
        return left_value < right_value
    left_value = runtime_candidate_field(fields, left, 5)
    right_value = runtime_candidate_field(fields, right, 5)
    if left_value != right_value:
        return left_value > right_value
    left_value = runtime_candidate_field(fields, left, 6)
    right_value = runtime_candidate_field(fields, right, 6)
    if left_value != right_value:
        return left_value > right_value
    left_value = runtime_candidate_field(fields, left, 7)
    right_value = runtime_candidate_field(fields, right, 7)
    if left_value != right_value:
        return left_value > right_value
    left_value = runtime_candidate_field(fields, left, 8)
    right_value = runtime_candidate_field(fields, right, 8)
    if left_value != right_value:
        return left_value < right_value
    left_value = runtime_candidate_field(fields, left, 9)
    right_value = runtime_candidate_field(fields, right, 9)
    if left_value != right_value:
        return left_value < right_value

    left_value = runtime_candidate_source_sort_key(
        route_kind,
        runtime_candidate_field(fields, left, 11),
    )
    right_value = runtime_candidate_source_sort_key(
        route_kind,
        runtime_candidate_field(fields, right, 11),
    )
    if left_value != right_value:
        return left_value < right_value
    left_value = runtime_candidate_field(fields, left, 12)
    right_value = runtime_candidate_field(fields, right, 12)
    if left_value != right_value:
        return left_value < right_value
    left_value = runtime_candidate_field(fields, left, 13)
    right_value = runtime_candidate_field(fields, right, 13)
    if left_value != right_value:
        return left_value < right_value
    left_value = runtime_candidate_field(fields, left, 14)
    right_value = runtime_candidate_field(fields, right, 14)
    if left_value != right_value:
        return left_value < right_value
    left_value = runtime_candidate_field(fields, left, 15)
    right_value = runtime_candidate_field(fields, right, 15)
    if left_value != right_value:
        return left_value < right_value
    left_value = runtime_candidate_field(fields, left, 16)
    right_value = runtime_candidate_field(fields, right, 16)
    if left_value != right_value:
        return left_value < right_value
    left_value = runtime_candidate_field(fields, left, 17)
    right_value = runtime_candidate_field(fields, right, 17)
    if left_value != right_value:
        return left_value < right_value
    return left < right

def runtime_candidate_less(
    fields: Pointer[mut=False, Int64, _],
    left: Int64,
    right: Int64,
    route_kind: Int64,
    fallback: Bool,
) -> Bool:
    if fallback:
        var left_value = runtime_candidate_field(fields, left, 18)
        var right_value = runtime_candidate_field(fields, right, 18)
        if left_value != right_value:
            return left_value < right_value
        left_value = runtime_candidate_field(fields, left, 19)
        right_value = runtime_candidate_field(fields, right, 19)
        if left_value != right_value:
            return left_value < right_value
        left_value = runtime_candidate_field(fields, left, 20)
        right_value = runtime_candidate_field(fields, right, 20)
        if left_value != right_value:
            return left_value < right_value
        left_value = runtime_candidate_field(fields, left, 21)
        right_value = runtime_candidate_field(fields, right, 21)
        if left_value != right_value:
            return left_value < right_value
    return runtime_candidate_ready_less(fields, left, right, route_kind)

comptime OPTIMISTIC_CANDIDATE_KEEP: Int64 = 0
comptime OPTIMISTIC_CANDIDATE_AUTH_FAILURE: Int64 = 1
comptime OPTIMISTIC_CANDIDATE_SELECTION_BACKOFF: Int64 = 2
comptime OPTIMISTIC_CANDIDATE_ROUTE_CIRCUIT: Int64 = 3
comptime OPTIMISTIC_CANDIDATE_HEALTH: Int64 = 4
comptime OPTIMISTIC_CANDIDATE_PERFORMANCE: Int64 = 5
comptime OPTIMISTIC_CANDIDATE_QUOTA_PROBE: Int64 = 6
comptime OPTIMISTIC_CANDIDATE_STALE_PERSISTED_QUOTA: Int64 = 7
comptime OPTIMISTIC_CANDIDATE_QUOTA_THIN: Int64 = 8
comptime OPTIMISTIC_CANDIDATE_QUOTA_CRITICAL: Int64 = 9
comptime OPTIMISTIC_CANDIDATE_QUOTA_EXHAUSTED: Int64 = 10
comptime OPTIMISTIC_CANDIDATE_QUOTA_UNKNOWN: Int64 = 11
comptime OPTIMISTIC_CANDIDATE_INFLIGHT: Int64 = 12
comptime OPTIMISTIC_CANDIDATE_INCOMPATIBLE: Int64 = 13
comptime OPTIMISTIC_CANDIDATE_PROMPT_CACHE: Int64 = 14

@export("prodex_runtime_optimistic_current_candidate_decision")
def prodex_runtime_optimistic_current_candidate_decision(
    route_kind: Int64,
    auth_failure_active: Int64,
    in_selection_backoff: Int64,
    circuit_open: Int64,
    health_score: Int64,
    performance_score: Int64,
    current_profile_quota_compatible: Int64,
    has_alternative_quota_compatible_profile: Int64,
    quota_band: Int64,
    quota_source_present: Int64,
    quota_source: Int64,
    inflight_count: Int64,
    inflight_soft_limit: Int64,
    prompt_cache_present: Int64,
    prompt_cache_owner_matches: Int64,
) abi("C") -> Int64:
    if route_kind < 0 or route_kind > 3:
        return -1
    if auth_failure_active < 0 or auth_failure_active > 1 or in_selection_backoff < 0 or in_selection_backoff > 1 or circuit_open < 0 or circuit_open > 1:
        return -1
    if current_profile_quota_compatible < 0 or current_profile_quota_compatible > 1 or has_alternative_quota_compatible_profile < 0 or has_alternative_quota_compatible_profile > 1:
        return -1
    if quota_band < 0 or quota_band > 4 or quota_source_present < 0 or quota_source_present > 1 or quota_source < 0 or quota_source > 1:
        return -1
    if inflight_count < 0 or inflight_soft_limit < 0 or prompt_cache_present < 0 or prompt_cache_present > 1 or prompt_cache_owner_matches < 0 or prompt_cache_owner_matches > 1:
        return -1

    if auth_failure_active == 1:
        return OPTIMISTIC_CANDIDATE_AUTH_FAILURE
    if in_selection_backoff == 1:
        return OPTIMISTIC_CANDIDATE_SELECTION_BACKOFF
    if circuit_open == 1:
        return OPTIMISTIC_CANDIDATE_ROUTE_CIRCUIT
    if health_score > 0:
        return OPTIMISTIC_CANDIDATE_HEALTH
    if performance_score > 0:
        return OPTIMISTIC_CANDIDATE_PERFORMANCE

    if has_alternative_quota_compatible_profile == 1 and quota_source_present == 0:
        return OPTIMISTIC_CANDIDATE_QUOTA_PROBE
    if has_alternative_quota_compatible_profile == 1 and (route_kind == 0 or route_kind == 2) and quota_source != 0:
        if quota_source == 1:
            return OPTIMISTIC_CANDIDATE_STALE_PERSISTED_QUOTA
        return OPTIMISTIC_CANDIDATE_QUOTA_PROBE

    if quota_band > 0 and not (quota_band == 4 and has_alternative_quota_compatible_profile == 0):
        if quota_band == 1:
            return OPTIMISTIC_CANDIDATE_QUOTA_THIN
        if quota_band == 2:
            return OPTIMISTIC_CANDIDATE_QUOTA_CRITICAL
        if quota_band == 3:
            return OPTIMISTIC_CANDIDATE_QUOTA_EXHAUSTED
        return OPTIMISTIC_CANDIDATE_QUOTA_UNKNOWN
    if inflight_count >= inflight_soft_limit:
        return OPTIMISTIC_CANDIDATE_INFLIGHT
    if current_profile_quota_compatible == 0:
        return OPTIMISTIC_CANDIDATE_INCOMPATIBLE
    if prompt_cache_present == 1 and (route_kind == 0 or route_kind == 2) and has_alternative_quota_compatible_profile == 1 and prompt_cache_owner_matches == 0:
        return OPTIMISTIC_CANDIDATE_PROMPT_CACHE
    return OPTIMISTIC_CANDIDATE_KEEP

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
    if tier < SMART_CONTEXT_REHYDRATE_MINIMAL_TIER or tier > SMART_CONTEXT_REHYDRATE_EXACT_TIER:
        return 2

    var used: UInt64 = 0
    for index in range(count):
        var required_value = required[unsafe_offset=index]
        var available_value = available[unsafe_offset=index]
        if (required_value != 0 and required_value != 1) or (available_value != 0 and available_value != 1):
            return 2
        var cost = token_costs[unsafe_offset=index]
        if available_value == 0:
            action_tags[unsafe_offset=index] = SMART_CONTEXT_REHYDRATE_ACTION_MISSING
        elif tier == SMART_CONTEXT_REHYDRATE_MINIMAL_TIER and required_value == 0:
            action_tags[unsafe_offset=index] = SMART_CONTEXT_REHYDRATE_ACTION_MINIMAL
        elif cost > token_budget - used:
            action_tags[unsafe_offset=index] = SMART_CONTEXT_REHYDRATE_ACTION_BUDGET
        else:
            used += cost
            action_tags[unsafe_offset=index] = SMART_CONTEXT_REHYDRATE_ACTION_REHYDRATE
    used_tokens[unsafe_offset=0] = used
    return 0

comptime RUNTIME_TUNING_INT64_MAX: Int64 = 9223372036854775807

def runtime_tuning_saturating_mul(left: Int64, right: Int64) -> Int64:
    if left <= 0 or right <= 0:
        return 0
    if left > RUNTIME_TUNING_INT64_MAX / right:
        return RUNTIME_TUNING_INT64_MAX
    return left * right

def runtime_tuning_clamp(value: Int64, minimum: Int64, maximum: Int64) -> Int64:
    if value < minimum:
        return minimum
    if value > maximum:
        return maximum
    return value

@export("prodex_runtime_tuning_defaults")
def prodex_runtime_tuning_defaults(
    parallelism: Int64,
    worker_count: Pointer[mut=True, Int64, _],
    long_lived_worker_count: Pointer[mut=True, Int64, _],
    probe_refresh_worker_count: Pointer[mut=True, Int64, _],
    async_worker_count: Pointer[mut=True, Int64, _],
    log_queue_capacity: Pointer[mut=True, Int64, _],
    websocket_connect_worker_count: Pointer[mut=True, Int64, _],
    websocket_dns_worker_count: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    if parallelism < 0:
        return 1
    worker_count[unsafe_offset=0] = runtime_tuning_clamp(parallelism, 4, 12)
    long_lived_worker_count[unsafe_offset=0] = runtime_tuning_clamp(
        runtime_tuning_saturating_mul(parallelism, 2),
        8,
        24,
    )
    probe_refresh_worker_count[unsafe_offset=0] = runtime_tuning_clamp(parallelism, 2, 4)
    async_worker_count[unsafe_offset=0] = runtime_tuning_clamp(parallelism, 2, 4)
    log_queue_capacity[unsafe_offset=0] = runtime_tuning_clamp(
        runtime_tuning_saturating_mul(parallelism, 256),
        1_024,
        8_192,
    )
    websocket_connect_worker_count[unsafe_offset=0] = runtime_tuning_clamp(parallelism, 4, 16)
    websocket_dns_worker_count[unsafe_offset=0] = runtime_tuning_clamp(parallelism, 2, 8)
    return 0

@export("prodex_runtime_candidate_plan_batch")
def prodex_runtime_candidate_plan_batch(
    fields: Pointer[mut=False, Int64, _],
    ready_indices: Pointer[mut=True, Int64, _],
    ready_count: Pointer[mut=True, Int64, _],
    fallback_indices: Pointer[mut=True, Int64, _],
    fallback_count: Pointer[mut=True, Int64, _],
    count: Int64,
    route_kind: Int64,
) abi("C") -> Int64:
    if count < 0 or count > RUNTIME_CANDIDATE_PLAN_MAX_COUNT:
        return 1
    if route_kind < 0 or route_kind > 3:
        return 2

    for index in range(count):
        if runtime_candidate_field(fields, index, 0) < 0 or runtime_candidate_field(fields, index, 0) > 1:
            return 2
        if runtime_candidate_field(fields, index, 11) < 0 or runtime_candidate_field(fields, index, 11) > 1:
            return 2
        if runtime_candidate_field(fields, index, 14) < 0 or runtime_candidate_field(fields, index, 14) > 1:
            return 2
        if runtime_candidate_field(fields, index, 1) < 0 or runtime_candidate_field(fields, index, 12) < 0 or runtime_candidate_field(fields, index, 13) < 0:
            return 2
        if runtime_candidate_field(fields, index, 16) < 0 or runtime_candidate_field(fields, index, 18) < 0:
            return 2

    var ready_len: Int64 = 0
    for index in range(count):
        if runtime_candidate_field(fields, index, 0) == 0:
            ready_indices[unsafe_offset=ready_len] = index
            ready_len += 1
    ready_count[unsafe_offset=0] = ready_len

    var fallback_len: Int64 = 0
    for index in range(count):
        fallback_indices[unsafe_offset=fallback_len] = index
        fallback_len += 1
    fallback_count[unsafe_offset=0] = fallback_len

    # ponytail: bounded O(n²) selection keeps the ABI allocation-free; replace with
    # a verified stable sort only if the runtime pool exceeds 256 candidates.
    for position in range(ready_len):
        var best = position
        for offset in range(position + 1, ready_len):
            var candidate = ready_indices[unsafe_offset=offset]
            var current = ready_indices[unsafe_offset=best]
            if runtime_candidate_less(fields, candidate, current, route_kind, False):
                best = offset
        if best != position:
            var selected = ready_indices[unsafe_offset=best]
            ready_indices[unsafe_offset=best] = ready_indices[unsafe_offset=position]
            ready_indices[unsafe_offset=position] = selected

    for position in range(fallback_len):
        var best = position
        for offset in range(position + 1, fallback_len):
            var candidate = fallback_indices[unsafe_offset=offset]
            var current = fallback_indices[unsafe_offset=best]
            if runtime_candidate_less(fields, candidate, current, route_kind, True):
                best = offset
        if best != position:
            var selected = fallback_indices[unsafe_offset=best]
            fallback_indices[unsafe_offset=best] = fallback_indices[unsafe_offset=position]
            fallback_indices[unsafe_offset=position] = selected
    return 0
