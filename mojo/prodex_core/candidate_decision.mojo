from std.memory import Pointer
from runtime_math import INT64_MAX, runtime_quota_saturating_add

comptime RUNTIME_CANDIDATE_PLAN_FIELD_COUNT: Int64 = 22
comptime RUNTIME_CANDIDATE_PLAN_MAX_COUNT: Int64 = 256


def runtime_candidate_field(
    fields: Pointer[mut=False, Int64, _], index: Int64, field: Int64
) -> Int64:
    return fields[
        unsafe_offset=(index * RUNTIME_CANDIDATE_PLAN_FIELD_COUNT) + field
    ]


def runtime_candidate_source_sort_key(
    route_kind: Int64, source: Int64
) -> Int64:
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
    if (
        auth_failure_active < 0
        or auth_failure_active > 1
        or in_selection_backoff < 0
        or in_selection_backoff > 1
        or circuit_open < 0
        or circuit_open > 1
    ):
        return -1
    if (
        current_profile_quota_compatible < 0
        or current_profile_quota_compatible > 1
        or has_alternative_quota_compatible_profile < 0
        or has_alternative_quota_compatible_profile > 1
    ):
        return -1
    if (
        quota_band < 0
        or quota_band > 4
        or quota_source_present < 0
        or quota_source_present > 1
        or quota_source < 0
        or quota_source > 1
    ):
        return -1
    if (
        inflight_count < 0
        or inflight_soft_limit < 0
        or prompt_cache_present < 0
        or prompt_cache_present > 1
        or prompt_cache_owner_matches < 0
        or prompt_cache_owner_matches > 1
    ):
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

    if (
        has_alternative_quota_compatible_profile == 1
        and quota_source_present == 0
    ):
        return OPTIMISTIC_CANDIDATE_QUOTA_PROBE
    if (
        has_alternative_quota_compatible_profile == 1
        and (route_kind == 0 or route_kind == 2)
        and quota_source != 0
    ):
        if quota_source == 1:
            return OPTIMISTIC_CANDIDATE_STALE_PERSISTED_QUOTA
        return OPTIMISTIC_CANDIDATE_QUOTA_PROBE

    if quota_band > 0 and not (
        quota_band == 4 and has_alternative_quota_compatible_profile == 0
    ):
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
    if (
        prompt_cache_present == 1
        and (route_kind == 0 or route_kind == 2)
        and has_alternative_quota_compatible_profile == 1
        and prompt_cache_owner_matches == 0
    ):
        return OPTIMISTIC_CANDIDATE_PROMPT_CACHE
    return OPTIMISTIC_CANDIDATE_KEEP


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
        if (
            runtime_candidate_field(fields, index, 0) < 0
            or runtime_candidate_field(fields, index, 0) > 1
        ):
            return 2
        if (
            runtime_candidate_field(fields, index, 11) < 0
            or runtime_candidate_field(fields, index, 11) > 1
        ):
            return 2
        if (
            runtime_candidate_field(fields, index, 14) < 0
            or runtime_candidate_field(fields, index, 14) > 1
        ):
            return 2
        if (
            runtime_candidate_field(fields, index, 1) < 0
            or runtime_candidate_field(fields, index, 12) < 0
            or runtime_candidate_field(fields, index, 13) < 0
        ):
            return 2
        if (
            runtime_candidate_field(fields, index, 16) < 0
            or runtime_candidate_field(fields, index, 18) < 0
        ):
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
            if runtime_candidate_less(
                fields, candidate, current, route_kind, False
            ):
                best = offset
        if best != position:
            var selected = ready_indices[unsafe_offset=best]
            ready_indices[unsafe_offset=best] = ready_indices[
                unsafe_offset=position
            ]
            ready_indices[unsafe_offset=position] = selected

    for position in range(fallback_len):
        var best = position
        for offset in range(position + 1, fallback_len):
            var candidate = fallback_indices[unsafe_offset=offset]
            var current = fallback_indices[unsafe_offset=best]
            if runtime_candidate_less(
                fields, candidate, current, route_kind, True
            ):
                best = offset
        if best != position:
            var selected = fallback_indices[unsafe_offset=best]
            fallback_indices[unsafe_offset=best] = fallback_indices[
                unsafe_offset=position
            ]
            fallback_indices[unsafe_offset=position] = selected
    return 0
