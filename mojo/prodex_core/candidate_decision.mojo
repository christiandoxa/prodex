from std.memory import Pointer
from rich_text import rich_view_valid, rich_views_equal
from rich_types import ProdexRichStringView, rich_view_ptr
from runtime_math import INT64_MAX, INT64_MIN, runtime_quota_saturating_add

comptime RUNTIME_CANDIDATE_PLAN_FIELD_COUNT: Int64 = 24
comptime RUNTIME_CANDIDATE_PLAN_MAX_COUNT: Int64 = 256
comptime RUNTIME_CANDIDATE_DECISION_FIELD_COUNT: Int64 = 5
comptime RICH_MAX_IDENTIFIER_BYTES: Int64 = 4_096
comptime UINT64_MAX: UInt64 = 18446744073709551615

comptime RUNTIME_CANDIDATE_AVAILABILITY_READY: Int64 = 0
comptime RUNTIME_CANDIDATE_AVAILABILITY_QUOTA_EXHAUSTED: Int64 = 1
comptime RUNTIME_CANDIDATE_AVAILABILITY_TRANSIENT_BACKOFF: Int64 = 2
comptime RUNTIME_CANDIDATE_AVAILABILITY_AUTH_INVALID: Int64 = 3
comptime RUNTIME_CANDIDATE_AVAILABILITY_UNKNOWN: Int64 = 4

comptime RUNTIME_CANDIDATE_SKIP_NONE: Int64 = 0
comptime RUNTIME_CANDIDATE_SKIP_AUTH_FAILURE: Int64 = 1
comptime RUNTIME_CANDIDATE_SKIP_SELECTION_BACKOFF: Int64 = 2
comptime RUNTIME_CANDIDATE_SKIP_QUOTA_EXHAUSTED: Int64 = 3
comptime RUNTIME_CANDIDATE_SKIP_INFLIGHT: Int64 = 5
comptime RUNTIME_CANDIDATE_SKIP_EXCLUDED: Int64 = 6


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


def runtime_prompt_cache_hash_static[literal: StaticString](hash: UInt64) -> UInt64:
    var value = hash
    var pointer = literal.unsafe_ptr()
    for index in range(literal.byte_length()):
        value = (value ^ UInt64(pointer[unsafe_offset=index])) * 1099511628211
    return value


def runtime_prompt_cache_hash_view(hash: UInt64, view: ProdexRichStringView) -> UInt64:
    var value = hash
    var pointer = rich_view_ptr(view)
    for index in range(Int64(view.len)):
        value = (value ^ UInt64(pointer[unsafe_offset=index])) * 1099511628211
    return value


def runtime_prompt_cache_hash_byte(hash: UInt64, byte: UInt64) -> UInt64:
    return (hash ^ byte) * 1099511628211


@export("prodex_runtime_prompt_cache_affinity_batch_v1")
def prodex_runtime_prompt_cache_affinity_batch_v1(
    profile_views_address: UInt,
    key_view_address: UInt,
    key_present: Int64,
    owner_view_address: UInt,
    owner_present: Int64,
    priorities_address: UInt,
    scores_address: UInt,
    count: Int64,
) abi("C") -> Int64:
    if count < 0 or count > RUNTIME_CANDIDATE_PLAN_MAX_COUNT:
        return 1
    if key_present < 0 or key_present > 1 or owner_present < 0 or owner_present > 1:
        return 1
    if count == 0:
        return 0
    if profile_views_address == 0 or priorities_address == 0 or scores_address == 0:
        return 1
    if key_present == 1 and key_view_address == 0:
        return 1
    if owner_present == 1 and owner_view_address == 0:
        return 1
    var profiles = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(profile_views_address))
    var priorities = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(priorities_address))
    var scores = Pointer[
        mut=True, UInt64, MutUntrackedOrigin
    ](unsafe_from_address=Int(scores_address))
    var key = ProdexRichStringView(0, 0)
    if key_present == 1:
        key = Pointer[
            mut=False, ProdexRichStringView, ImmUntrackedOrigin
        ](unsafe_from_address=Int(key_view_address))[].copy()
        if not rich_view_valid(key, RICH_MAX_IDENTIFIER_BYTES):
            return 2
    var owner = ProdexRichStringView(0, 0)
    if owner_present == 1:
        owner = Pointer[
            mut=False, ProdexRichStringView, ImmUntrackedOrigin
        ](unsafe_from_address=Int(owner_view_address))[].copy()
        if not rich_view_valid(owner, RICH_MAX_IDENTIFIER_BYTES):
            return 2
    for index in range(count):
        var profile = profiles[unsafe_offset=index].copy()
        if not rich_view_valid(profile, RICH_MAX_IDENTIFIER_BYTES):
            return 2
        priorities[unsafe_offset=index] = 0
        scores[unsafe_offset=index] = 0
        if key_present == 0:
            continue
        if owner_present == 1 and rich_views_equal(profile, owner):
            continue
        if owner_present == 1:
            priorities[unsafe_offset=index] = 1
        var hash: UInt64 = 1469598103934665603
        hash = runtime_prompt_cache_hash_static["prodex-prompt-cache-affinity-v1"](hash)
        hash = runtime_prompt_cache_hash_byte(hash, 0)
        hash = runtime_prompt_cache_hash_view(hash, key)
        hash = runtime_prompt_cache_hash_byte(hash, 0)
        hash = runtime_prompt_cache_hash_view(hash, profile)
        scores[unsafe_offset=index] = UINT64_MAX - hash
    return 0


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

    if quota_band == 3:
        return OPTIMISTIC_CANDIDATE_QUOTA_EXHAUSTED
    if quota_band == 4 and has_alternative_quota_compatible_profile == 1:
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


comptime SOFT_AFFINITY_POLICY_ALLOWED: Int64 = 0
comptime SOFT_AFFINITY_POLICY_QUOTA_WINDOWS_UNAVAILABLE: Int64 = 1
comptime SOFT_AFFINITY_POLICY_QUOTA_EXHAUSTED_BEFORE_SEND: Int64 = 2
comptime SOFT_AFFINITY_POLICY_QUOTA_EXHAUSTED: Int64 = 3
comptime SOFT_AFFINITY_POLICY_QUOTA_HEALTHY: Int64 = 4
comptime SOFT_AFFINITY_POLICY_QUOTA_THIN: Int64 = 5
comptime SOFT_AFFINITY_POLICY_QUOTA_CRITICAL: Int64 = 6
comptime SOFT_AFFINITY_POLICY_QUOTA_UNKNOWN: Int64 = 7


def runtime_soft_affinity_policy_band_reason(band: Int64) -> Int64:
    if band == 0:
        return SOFT_AFFINITY_POLICY_QUOTA_HEALTHY
    if band == 1:
        return SOFT_AFFINITY_POLICY_QUOTA_THIN
    if band == 2:
        return SOFT_AFFINITY_POLICY_QUOTA_CRITICAL
    if band == 3:
        return SOFT_AFFINITY_POLICY_QUOTA_EXHAUSTED
    return SOFT_AFFINITY_POLICY_QUOTA_UNKNOWN


@export("prodex_runtime_soft_affinity_policy_v1")
def prodex_runtime_soft_affinity_policy_v1(
    affinity_kind: Int64,
    route_kind: Int64,
    five_hour_status: Int64,
    weekly_status: Int64,
    quota_band: Int64,
    quota_source_present: Int64,
    current_profile_matches_candidate: Int64,
    has_route_eligible_quota_fallback: Int64,
) abi("C") -> Int64:
    if (
        affinity_kind < 0
        or affinity_kind > 3
        or route_kind < 0
        or route_kind > 3
        or five_hour_status < 0
        or five_hour_status > 4
        or weekly_status < 0
        or weekly_status > 4
        or quota_band < 0
        or quota_band > 4
        or quota_source_present < 0
        or quota_source_present > 1
        or current_profile_matches_candidate < 0
        or current_profile_matches_candidate > 1
        or has_route_eligible_quota_fallback < 0
        or has_route_eligible_quota_fallback > 1
    ):
        return -1

    var summary_allows = (
        quota_source_present == 1
        and five_hour_status <= 2
        and weekly_status <= 2
    )
    var precommit_guard = five_hour_status == 3
    var allowed = False
    if affinity_kind == 0:
        allowed = summary_allows or (
            (route_kind == 0 or route_kind == 2) and quota_source_present == 0
        )
    elif affinity_kind == 1 or affinity_kind == 2:
        allowed = quota_band <= 2 and not precommit_guard
    else:
        allowed = summary_allows or (
            route_kind == 1 and quota_source_present == 0
        ) or (
            route_kind == 2
            and quota_source_present == 0
            and current_profile_matches_candidate == 1
            and has_route_eligible_quota_fallback == 0
        )
    if allowed:
        return SOFT_AFFINITY_POLICY_ALLOWED

    if affinity_kind == 1 or affinity_kind == 2:
        return runtime_soft_affinity_policy_band_reason(quota_band)
    if quota_source_present == 0 or five_hour_status == 4 or weekly_status == 4:
        return SOFT_AFFINITY_POLICY_QUOTA_WINDOWS_UNAVAILABLE
    if precommit_guard:
        return SOFT_AFFINITY_POLICY_QUOTA_EXHAUSTED_BEFORE_SEND
    if five_hour_status == 3 or weekly_status == 3:
        return SOFT_AFFINITY_POLICY_QUOTA_EXHAUSTED
    return runtime_soft_affinity_policy_band_reason(quota_band)


@export("prodex_runtime_candidate_plan_batch")
def prodex_runtime_candidate_plan_batch(
    fields: Pointer[mut=False, Int64, _],
    excluded: Pointer[mut=False, Int64, _],
    decision_tags: Pointer[mut=True, Int64, _],
    ready_indices: Pointer[mut=True, Int64, _],
    ready_count: Pointer[mut=True, Int64, _],
    fallback_indices: Pointer[mut=True, Int64, _],
    fallback_count: Pointer[mut=True, Int64, _],
    count: Int64,
    route_kind: Int64,
    inflight_soft_limit: Int64,
    responses_critical_floor_percent: Int64,
) abi("C") -> Int64:
    if count < 0 or count > RUNTIME_CANDIDATE_PLAN_MAX_COUNT:
        return 1
    if route_kind < 0 or route_kind > 3:
        return 2
    if inflight_soft_limit < 0 or responses_critical_floor_percent < 0:
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
        if (
            runtime_candidate_field(fields, index, 22) < 0
            or runtime_candidate_field(fields, index, 22) > 1
            or runtime_candidate_field(fields, index, 23) < 0
            or runtime_candidate_field(fields, index, 23) > 4
            or excluded[unsafe_offset=index] < 0
            or excluded[unsafe_offset=index] > 1
        ):
            return 2

        var eligible: Int64 = 1 - excluded[unsafe_offset=index]
        var availability: Int64 = RUNTIME_CANDIDATE_AVAILABILITY_READY
        var quota_guard_reason: Int64 = RUNTIME_CANDIDATE_SKIP_NONE
        var ready_skip_reason: Int64 = RUNTIME_CANDIDATE_SKIP_NONE
        var fallback_skip_reason: Int64 = RUNTIME_CANDIDATE_SKIP_NONE
        if eligible == 0:
            availability = RUNTIME_CANDIDATE_AVAILABILITY_UNKNOWN
            ready_skip_reason = RUNTIME_CANDIDATE_SKIP_EXCLUDED
            fallback_skip_reason = RUNTIME_CANDIDATE_SKIP_EXCLUDED
        elif runtime_candidate_field(fields, index, 22) == 1:
            availability = RUNTIME_CANDIDATE_AVAILABILITY_AUTH_INVALID
            ready_skip_reason = RUNTIME_CANDIDATE_SKIP_AUTH_FAILURE
            fallback_skip_reason = RUNTIME_CANDIDATE_SKIP_AUTH_FAILURE
        elif runtime_candidate_field(fields, index, 23) == 3:
            availability = RUNTIME_CANDIDATE_AVAILABILITY_QUOTA_EXHAUSTED
            quota_guard_reason = RUNTIME_CANDIDATE_SKIP_QUOTA_EXHAUSTED
            ready_skip_reason = RUNTIME_CANDIDATE_SKIP_QUOTA_EXHAUSTED
            fallback_skip_reason = RUNTIME_CANDIDATE_SKIP_QUOTA_EXHAUSTED
        elif runtime_candidate_field(fields, index, 0) == 1:
            availability = RUNTIME_CANDIDATE_AVAILABILITY_TRANSIENT_BACKOFF
            ready_skip_reason = RUNTIME_CANDIDATE_SKIP_SELECTION_BACKOFF
        elif runtime_candidate_field(fields, index, 23) == 4:
            availability = RUNTIME_CANDIDATE_AVAILABILITY_UNKNOWN
        if (
            eligible == 1
            and ready_skip_reason == RUNTIME_CANDIDATE_SKIP_NONE
            and runtime_candidate_field(fields, index, 12) >= inflight_soft_limit
        ):
            ready_skip_reason = RUNTIME_CANDIDATE_SKIP_INFLIGHT
        var decision_offset = index * RUNTIME_CANDIDATE_DECISION_FIELD_COUNT
        decision_tags[unsafe_offset=decision_offset] = eligible
        decision_tags[unsafe_offset=decision_offset + 1] = availability
        decision_tags[unsafe_offset=decision_offset + 2] = quota_guard_reason
        decision_tags[unsafe_offset=decision_offset + 3] = ready_skip_reason
        decision_tags[unsafe_offset=decision_offset + 4] = fallback_skip_reason

    var ready_len: Int64 = 0
    for index in range(count):
        if (
            excluded[unsafe_offset=index] == 0
            and runtime_candidate_field(fields, index, 0) == 0
        ):
            ready_indices[unsafe_offset=ready_len] = index
            ready_len += 1
    ready_count[unsafe_offset=0] = ready_len

    var fallback_len: Int64 = 0
    for index in range(count):
        if excluded[unsafe_offset=index] == 0:
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


comptime RUNTIME_ADAPTIVE_QUALITY_FIELD_COUNT: Int64 = 9
comptime RUNTIME_ADAPTIVE_ROUTING_MAX_COUNT: Int64 = 256
comptime ADAPTIVE_PLAN_REASON_INSUFFICIENT_SAMPLES: Int64 = 0
comptime ADAPTIVE_PLAN_REASON_SHADOW_ONLY: Int64 = 1
comptime ADAPTIVE_PLAN_REASON_ADAPTIVE_ENABLED: Int64 = 2
comptime ADAPTIVE_PLAN_REASON_SHADOW_EXPLORATION: Int64 = 3
comptime ADAPTIVE_PLAN_REASON_ADAPTIVE_EXPLORATION: Int64 = 4


def runtime_adaptive_saturating_add(left: UInt64, right: UInt64) -> UInt64:
    if left > UINT64_MAX - right:
        return UINT64_MAX
    return left + right


def runtime_adaptive_saturating_mul(left: UInt64, right: UInt64) -> UInt64:
    if left == 0 or right == 0:
        return 0
    if left > UINT64_MAX / right:
        return UINT64_MAX
    return left * right


def runtime_adaptive_quality_score(
    samples: UInt64,
    task_completed: UInt64,
    corrective_user_messages: UInt64,
    additional_turns: UInt64,
    previous_response_not_found: UInt64,
    invalid_tool_call_continuation: UInt64,
    errors: UInt64,
    token_savings: UInt64,
    latency_ms_total: UInt64,
) -> Int64:
    var positive = runtime_adaptive_saturating_mul(task_completed, 1_000)
    var savings = token_savings / 100
    if savings > 2_000:
        savings = 2_000
    positive = runtime_adaptive_saturating_add(positive, savings)

    var negative = runtime_adaptive_saturating_mul(
        corrective_user_messages, 1_200
    )
    negative = runtime_adaptive_saturating_add(
        negative, runtime_adaptive_saturating_mul(additional_turns, 200)
    )
    negative = runtime_adaptive_saturating_add(
        negative,
        runtime_adaptive_saturating_mul(previous_response_not_found, 2_000),
    )
    negative = runtime_adaptive_saturating_add(
        negative,
        runtime_adaptive_saturating_mul(invalid_tool_call_continuation, 2_000),
    )
    negative = runtime_adaptive_saturating_add(
        negative, runtime_adaptive_saturating_mul(errors, 1_500)
    )
    var latency = latency_ms_total / 1_000
    if latency > 2_000:
        latency = 2_000
    negative = runtime_adaptive_saturating_add(negative, latency)

    if positive >= negative:
        var difference = positive - negative
        if difference > UInt64(INT64_MAX):
            return INT64_MAX
        return Int64(difference)
    var difference = negative - positive
    if difference >= UInt64(9223372036854775808):
        return INT64_MIN
    return -Int64(difference)


def runtime_adaptive_quality_field(
    fields: Pointer[mut=False, UInt64, _], index: Int64, field: Int64
) -> UInt64:
    return fields[
        unsafe_offset=(index * RUNTIME_ADAPTIVE_QUALITY_FIELD_COUNT) + field
    ]


def runtime_adaptive_seed(value: UInt64) -> UInt64:
    var mixed = value + UInt64(0x9e3779b97f4a7c15)
    mixed = (mixed ^ (mixed >> 30)) * UInt64(0xbf58476d1ce4e5b9)
    mixed = (mixed ^ (mixed >> 27)) * UInt64(0x94d049bb133111eb)
    return mixed ^ (mixed >> 31)


@export("prodex_runtime_gateway_adaptive_plan_v1")
def prodex_runtime_gateway_adaptive_plan_v1(
    quality_fields: Pointer[mut=False, UInt64, _],
    window_present: Pointer[mut=False, Int64, _],
    recommended_index: Pointer[mut=True, Int64, _],
    quality_score_bps: Pointer[mut=True, Int64, _],
    quality_score_present: Pointer[mut=True, Int64, _],
    reason: Pointer[mut=True, Int64, _],
    count: Int64,
    actual_index: Int64,
    shadow_mode: Int64,
    min_samples: UInt64,
    exploration_rate_bps: Int64,
    diagnostic_seed: UInt64,
) abi("C") -> Int64:
    if (
        count < 0
        or count > RUNTIME_ADAPTIVE_ROUTING_MAX_COUNT
        or actual_index < -1
        or actual_index >= count
        or shadow_mode < 0
        or shadow_mode > 1
        or exploration_rate_bps < 0
    ):
        return 1

    recommended_index[unsafe_offset=0] = -1
    quality_score_bps[unsafe_offset=0] = 0
    quality_score_present[unsafe_offset=0] = 0
    reason[unsafe_offset=0] = ADAPTIVE_PLAN_REASON_INSUFFICIENT_SAMPLES

    if count > 0 and exploration_rate_bps > 0:
        if (
            runtime_adaptive_seed(diagnostic_seed) % 10_000
            < UInt64(exploration_rate_bps)
        ):
            var selected = Int64(
                runtime_adaptive_seed(
                    diagnostic_seed ^ UInt64(0x9e3779b97f4a7c15)
                ) % UInt64(count)
            )
            if count > 1 and selected == actual_index:
                selected = (selected + 1) % count
            recommended_index[unsafe_offset=0] = selected
            if shadow_mode == 1:
                reason[unsafe_offset=0] = ADAPTIVE_PLAN_REASON_SHADOW_EXPLORATION
            else:
                reason[unsafe_offset=0] = ADAPTIVE_PLAN_REASON_ADAPTIVE_EXPLORATION
            return 0

    var best_index: Int64 = -1
    var best_score = INT64_MIN
    for index in range(count):
        var present = window_present[unsafe_offset=index]
        if present < 0 or present > 1:
            return 2
        if present == 1 and runtime_adaptive_quality_field(
            quality_fields, index, 0
        ) >= min_samples:
            var score = runtime_adaptive_quality_score(
                runtime_adaptive_quality_field(quality_fields, index, 0),
                runtime_adaptive_quality_field(quality_fields, index, 1),
                runtime_adaptive_quality_field(quality_fields, index, 2),
                runtime_adaptive_quality_field(quality_fields, index, 3),
                runtime_adaptive_quality_field(quality_fields, index, 4),
                runtime_adaptive_quality_field(quality_fields, index, 5),
                runtime_adaptive_quality_field(quality_fields, index, 6),
                runtime_adaptive_quality_field(quality_fields, index, 7),
                runtime_adaptive_quality_field(quality_fields, index, 8),
            )
            if best_index == -1 or score > best_score:
                best_index = index
                best_score = score

    if best_index == -1:
        return 0
    recommended_index[unsafe_offset=0] = best_index
    quality_score_bps[unsafe_offset=0] = best_score
    quality_score_present[unsafe_offset=0] = 1
    if shadow_mode == 1:
        reason[unsafe_offset=0] = ADAPTIVE_PLAN_REASON_SHADOW_ONLY
    else:
        reason[unsafe_offset=0] = ADAPTIVE_PLAN_REASON_ADAPTIVE_ENABLED
    return 0
