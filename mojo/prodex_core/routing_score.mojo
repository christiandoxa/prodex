from std.memory import Pointer

comptime ROUTING_SCORE_SCALE: Int64 = 10_000
comptime SCORE_COMPONENT_COUNT: Int64 = 7

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
