from std.memory import Pointer

comptime ROUTING_SCORE_SCALE: Int64 = 10_000
comptime SCORE_COMPONENT_COUNT: Int64 = 7
comptime ROUTING_REASON_ELIGIBLE: Int64 = 0
comptime ROUTING_REASON_HARD_REJECTED: Int64 = 1
comptime ROUTING_REASON_CAPABILITY_MISSING: Int64 = 2
comptime PRODEX_MOJO_ABI_VERSION: Int64 = 1

@export("prodex_mojo_abi_version")
def prodex_mojo_abi_version() abi("C") -> Int64:
    return PRODEX_MOJO_ABI_VERSION

def routing_score_valid(value: Int64) -> Bool:
    return value >= 0 and value <= ROUTING_SCORE_SCALE

def routing_score_inverse(value: Int64) -> Int64:
    return ROUTING_SCORE_SCALE - value

def routing_score_weighted_total(
    health: Int64,
    available_capacity: Int64,
    cost: Int64,
    latency: Int64,
    risk: Int64,
    priority: Int64,
    affinity: Int64,
    health_weight: Int64,
    load_weight: Int64,
    cost_weight: Int64,
    latency_weight: Int64,
    risk_weight: Int64,
    priority_weight: Int64,
    affinity_weight: Int64,
) -> Int64:
    return health * health_weight + available_capacity * load_weight + cost * cost_weight + latency * latency_weight + risk * risk_weight + priority * priority_weight + affinity * affinity_weight

@export("prodex_routing_score_batch")
def prodex_routing_score_batch(
    health: Pointer[mut=False, Int64, _],
    load: Pointer[mut=False, Int64, _],
    quota_headroom: Pointer[mut=False, Int64, _],
    quota_present: Pointer[mut=False, Int64, _],
    cost: Pointer[mut=False, Int64, _],
    latency: Pointer[mut=False, Int64, _],
    risk: Pointer[mut=False, Int64, _],
    priority: Pointer[mut=False, Int64, _],
    affinity: Pointer[mut=False, Int64, _],
    normalized_values: Pointer[mut=True, Int64, _],
    weighted_totals: Pointer[mut=True, Int64, _],
    scores: Pointer[mut=True, Int64, _],
    count: Int64,
    health_weight: Int64,
    load_weight: Int64,
    cost_weight: Int64,
    latency_weight: Int64,
    risk_weight: Int64,
    priority_weight: Int64,
    affinity_weight: Int64,
) abi("C") -> Int64:
    if count < 0 or count > 64:
        return 1

    var weight_total = health_weight + load_weight + cost_weight + latency_weight + risk_weight + priority_weight + affinity_weight
    if weight_total <= 0 or weight_total > ROUTING_SCORE_SCALE:
        return 2
    if not routing_score_valid(health_weight) or not routing_score_valid(load_weight) or not routing_score_valid(cost_weight) or not routing_score_valid(latency_weight) or not routing_score_valid(risk_weight) or not routing_score_valid(priority_weight) or not routing_score_valid(affinity_weight):
        return 2

    for index in range(count):
        var health_value = health[unsafe_offset=index]
        var load_value = load[unsafe_offset=index]
        var quota_value = quota_headroom[unsafe_offset=index]
        var quota_has_value = quota_present[unsafe_offset=index]
        var cost_value = cost[unsafe_offset=index]
        var latency_value = latency[unsafe_offset=index]
        var risk_value = risk[unsafe_offset=index]
        var priority_value = priority[unsafe_offset=index]
        var affinity_value = affinity[unsafe_offset=index]
        if not routing_score_valid(health_value) or not routing_score_valid(load_value) or not routing_score_valid(cost_value) or not routing_score_valid(latency_value) or not routing_score_valid(risk_value) or not routing_score_valid(priority_value):
            return 3
        if quota_has_value != 0 and quota_has_value != 1:
            return 3
        if quota_has_value == 1 and not routing_score_valid(quota_value):
            return 3
        if affinity_value != 0 and affinity_value != 1:
            return 3

        var available_capacity = routing_score_inverse(load_value)
        if quota_has_value == 1 and quota_value < available_capacity:
            available_capacity = quota_value
        if affinity_value == 1:
            affinity_value = ROUTING_SCORE_SCALE

        var base = index * SCORE_COMPONENT_COUNT
        normalized_values[unsafe_offset=base] = health_value
        normalized_values[unsafe_offset=base + 1] = available_capacity
        normalized_values[unsafe_offset=base + 2] = routing_score_inverse(cost_value)
        normalized_values[unsafe_offset=base + 3] = routing_score_inverse(latency_value)
        normalized_values[unsafe_offset=base + 4] = routing_score_inverse(risk_value)
        normalized_values[unsafe_offset=base + 5] = priority_value
        normalized_values[unsafe_offset=base + 6] = affinity_value

        var weighted_total = routing_score_weighted_total(
            health_value,
            available_capacity,
            routing_score_inverse(cost_value),
            routing_score_inverse(latency_value),
            routing_score_inverse(risk_value),
            priority_value,
            affinity_value,
            health_weight,
            load_weight,
            cost_weight,
            latency_weight,
            risk_weight,
            priority_weight,
            affinity_weight,
        )
        weighted_totals[unsafe_offset=index] = weighted_total
        scores[unsafe_offset=index] = weighted_total / weight_total

    return 0

@export("prodex_routing_plan_batch")
def prodex_routing_plan_batch(
    hard_eligible: Pointer[mut=False, Int64, _],
    capability_masks: Pointer[mut=False, Int64, _],
    provider_order: Pointer[mut=False, Int64, _],
    health: Pointer[mut=False, Int64, _],
    load: Pointer[mut=False, Int64, _],
    quota_headroom: Pointer[mut=False, Int64, _],
    quota_present: Pointer[mut=False, Int64, _],
    cost: Pointer[mut=False, Int64, _],
    latency: Pointer[mut=False, Int64, _],
    risk: Pointer[mut=False, Int64, _],
    priority: Pointer[mut=False, Int64, _],
    affinity: Pointer[mut=False, Int64, _],
    eligible: Pointer[mut=True, Int64, _],
    reason_tags: Pointer[mut=True, Int64, _],
    normalized_values: Pointer[mut=True, Int64, _],
    weighted_totals: Pointer[mut=True, Int64, _],
    scores: Pointer[mut=True, Int64, _],
    ordered_indices: Pointer[mut=True, Int64, _],
    out_ordered_count: Pointer[mut=True, Int64, _],
    count: Int64,
    required_capability_mask: Int64,
    health_weight: Int64,
    load_weight: Int64,
    cost_weight: Int64,
    latency_weight: Int64,
    risk_weight: Int64,
    priority_weight: Int64,
    affinity_weight: Int64,
) abi("C") -> Int64:
    if required_capability_mask < 0 or required_capability_mask > 255:
        return 4

    var status = prodex_routing_score_batch(
        health,
        load,
        quota_headroom,
        quota_present,
        cost,
        latency,
        risk,
        priority,
        affinity,
        normalized_values,
        weighted_totals,
        scores,
        count,
        health_weight,
        load_weight,
        cost_weight,
        latency_weight,
        risk_weight,
        priority_weight,
        affinity_weight,
    )
    if status != 0:
        return status

    var ordered_count: Int64 = 0
    for index in range(count):
        var hard_value = hard_eligible[unsafe_offset=index]
        var capability_value = capability_masks[unsafe_offset=index]
        if (hard_value != 0 and hard_value != 1) or capability_value < 0 or capability_value > 255:
            return 4

        if hard_value == 0:
            eligible[unsafe_offset=index] = 0
            reason_tags[unsafe_offset=index] = ROUTING_REASON_HARD_REJECTED
        elif (capability_value & required_capability_mask) != required_capability_mask:
            eligible[unsafe_offset=index] = 0
            reason_tags[unsafe_offset=index] = ROUTING_REASON_CAPABILITY_MISSING
        else:
            eligible[unsafe_offset=index] = 1
            reason_tags[unsafe_offset=index] = ROUTING_REASON_ELIGIBLE
            ordered_indices[unsafe_offset=ordered_count] = index
            ordered_count += 1

    for position in range(ordered_count):
        var best_position = position
        var best_index = ordered_indices[unsafe_offset=best_position]
        for candidate_position in range(position + 1, ordered_count):
            var candidate_index = ordered_indices[unsafe_offset=candidate_position]
            var candidate_is_better = False
            if affinity[unsafe_offset=candidate_index] > affinity[unsafe_offset=best_index]:
                candidate_is_better = True
            elif affinity[unsafe_offset=candidate_index] == affinity[unsafe_offset=best_index]:
                if scores[unsafe_offset=candidate_index] > scores[unsafe_offset=best_index]:
                    candidate_is_better = True
                elif scores[unsafe_offset=candidate_index] == scores[unsafe_offset=best_index]:
                    if provider_order[unsafe_offset=candidate_index] < provider_order[unsafe_offset=best_index]:
                        candidate_is_better = True
                    elif provider_order[unsafe_offset=candidate_index] == provider_order[unsafe_offset=best_index] and candidate_index < best_index:
                        candidate_is_better = True
            if candidate_is_better:
                best_position = candidate_position
                best_index = candidate_index

        if best_position != position:
            var selected_index = ordered_indices[unsafe_offset=position]
            ordered_indices[unsafe_offset=position] = ordered_indices[unsafe_offset=best_position]
            ordered_indices[unsafe_offset=best_position] = selected_index

    out_ordered_count[unsafe_offset=0] = ordered_count
    return 0

@export("prodex_capability_match_batch")
def prodex_capability_match_batch(
    well_formed: Pointer[mut=False, Int64, _],
    capability_masks: Pointer[mut=False, Int64, _],
    compatible: Pointer[mut=True, Int64, _],
    reason_tags: Pointer[mut=True, Int64, _],
    out_first_compatible: Pointer[mut=True, Int64, _],
    out_first_incompatible: Pointer[mut=True, Int64, _],
    count: Int64,
    required_capability_mask: Int64,
) abi("C") -> Int64:
    if count < 0 or count > 64 or required_capability_mask < 0 or required_capability_mask > 127:
        return 5

    var first_compatible: Int64 = -1
    var first_incompatible: Int64 = -1
    for index in range(count):
        var well_formed_value = well_formed[unsafe_offset=index]
        var capability_value = capability_masks[unsafe_offset=index]
        if (well_formed_value != 0 and well_formed_value != 1) or capability_value < 0 or capability_value > 127:
            return 5
        if well_formed_value == 0:
            compatible[unsafe_offset=index] = 0
            reason_tags[unsafe_offset=index] = 0
        elif (capability_value & required_capability_mask) == required_capability_mask:
            compatible[unsafe_offset=index] = 1
            reason_tags[unsafe_offset=index] = 1
            if first_compatible < 0:
                first_compatible = index
        else:
            compatible[unsafe_offset=index] = 0
            reason_tags[unsafe_offset=index] = 2
            if first_incompatible < 0:
                first_incompatible = index

    out_first_compatible[unsafe_offset=0] = first_compatible
    out_first_incompatible[unsafe_offset=0] = first_incompatible
    return 0
