from std.memory import Pointer

from rich_text import (
    rich_capability_mask,
    rich_copy_trimmed,
    rich_hash_pair,
    rich_required_hash_capacity,
    rich_slice_equal_folded,
    rich_slice_matches_literal,
    rich_view_valid,
)
from rich_types import (
    ProdexRichRouteInput,
    ProdexRichRouteRecord,
    ProdexRichRouteResult,
    ProdexRichSlice,
    ProdexRichStringView,
    NormalizedIdentifier,
    RouteCandidate,
    RouteScore,
)


comptime PRODEX_RICH_ABI_VERSION: Int64 = 2
comptime RICH_MAX_RECORDS: Int64 = 256
comptime RICH_MAX_IDENTIFIER_BYTES: Int64 = 4_096
comptime RICH_MAX_CAPABILITIES_BYTES: Int64 = 2_048
comptime RICH_STATUS_OK: Int64 = 0
comptime RICH_STATUS_INVALID: Int64 = 1
comptime RICH_STATUS_UTF8: Int64 = 2
comptime RICH_STATUS_CAPACITY: Int64 = 3
comptime RICH_STATUS_ABI: Int64 = 4
comptime RICH_ISSUE_INVALID_UTF8: Int64 = 6
comptime RICH_ROUTE_REASON_ELIGIBLE: Int64 = 0
comptime RICH_ROUTE_REASON_HARD_REJECTED: Int64 = 1
comptime RICH_ROUTE_REASON_CAPABILITY_MISSING: Int64 = 2
comptime RICH_ROUTE_REASON_DUPLICATE: Int64 = 3
comptime RICH_ROUTE_SCORE_SCALE: Int64 = 10_000


def route_issue(
    result: Pointer[mut=True, ProdexRichRouteResult, _],
    kind: Int64,
    offset: Int64,
    length: Int64,
) -> None:
    result[].issue_kind = kind
    result[].issue_offset = offset
    result[].issue_length = length


def route_provider_order(
    output: Pointer[mut=True, UInt8, _], provider: ProdexRichSlice
) -> Int64:
    if rich_slice_matches_literal["openai"](output, provider, False):
        return 0
    if rich_slice_matches_literal["anthropic"](output, provider, False):
        return 1
    if rich_slice_matches_literal["copilot"](output, provider, False):
        return 2
    if rich_slice_matches_literal["deepseek"](output, provider, False):
        return 3
    if rich_slice_matches_literal["gemini"](output, provider, False):
        return 4
    if rich_slice_matches_literal["kiro"](output, provider, False):
        return 5
    if rich_slice_matches_literal["local"](output, provider, False):
        return 6
    return 1_000


def route_score(
    input: ProdexRichRouteInput, weights: InlineArray[Int64, 7]
) -> RouteScore:
    var available = RICH_ROUTE_SCORE_SCALE - input.load
    if input.quota_present == 1 and input.quota_headroom < available:
        available = input.quota_headroom
    var affinity = input.affinity * RICH_ROUTE_SCORE_SCALE
    var components = InlineArray[Int64, 7](fill=0)
    components[0] = input.health
    components[1] = available
    components[2] = RICH_ROUTE_SCORE_SCALE - input.cost
    components[3] = RICH_ROUTE_SCORE_SCALE - input.latency
    components[4] = RICH_ROUTE_SCORE_SCALE - input.risk
    components[5] = input.priority
    components[6] = affinity
    var total = (
        input.health * weights[0]
        + available * weights[1]
        + (RICH_ROUTE_SCORE_SCALE - input.cost) * weights[2]
        + (RICH_ROUTE_SCORE_SCALE - input.latency) * weights[3]
        + (RICH_ROUTE_SCORE_SCALE - input.risk) * weights[4]
        + input.priority * weights[5]
        + affinity * weights[6]
    )
    var weight_total = (
        weights[0]
        + weights[1]
        + weights[2]
        + weights[3]
        + weights[4]
        + weights[5]
        + weights[6]
    )
    return RouteScore(components^, total, total / weight_total)


def route_values_valid(input: ProdexRichRouteInput) -> Bool:
    return (
        (input.hard_eligible == 0 or input.hard_eligible == 1)
        and (input.quota_present == 0 or input.quota_present == 1)
        and (input.affinity == 0 or input.affinity == 1)
        and input.health >= 0
        and input.health <= RICH_ROUTE_SCORE_SCALE
        and input.load >= 0
        and input.load <= RICH_ROUTE_SCORE_SCALE
        and input.cost >= 0
        and input.cost <= RICH_ROUTE_SCORE_SCALE
        and input.latency >= 0
        and input.latency <= RICH_ROUTE_SCORE_SCALE
        and input.risk >= 0
        and input.risk <= RICH_ROUTE_SCORE_SCALE
        and input.priority >= 0
        and input.priority <= RICH_ROUTE_SCORE_SCALE
        and (
            input.quota_present == 0
            or input.quota_headroom >= 0
            and input.quota_headroom <= RICH_ROUTE_SCORE_SCALE
        )
    )


def route_better(
    records: Pointer[mut=True, ProdexRichRouteRecord, _], left: Int64, right: Int64
) -> Bool:
    var a = records[unsafe_offset=left].copy()
    var b = records[unsafe_offset=right].copy()
    if a.affinity != b.affinity:
        return a.affinity > b.affinity
    if a.score != b.score:
        return a.score > b.score
    if a.provider_order != b.provider_order:
        return a.provider_order < b.provider_order
    return a.input_index < b.input_index


@export("prodex_mojo_rich_route_plan_v2")
def prodex_mojo_rich_route_plan_v2(
    abi_version: Int64,
    inputs_opt: Optional[Pointer[mut=False, ProdexRichRouteInput, ImmUntrackedOrigin]],
    input_count: Int64,
    required_capabilities: ProdexRichStringView,
    output_records_opt: Optional[Pointer[mut=True, ProdexRichRouteRecord, MutUntrackedOrigin]],
    record_capacity: Int64,
    ordered_indices_opt: Optional[Pointer[mut=True, Int64, MutUntrackedOrigin]],
    ordered_capacity: Int64,
    output_opt: Optional[Pointer[mut=True, UInt8, MutUntrackedOrigin]],
    output_capacity: Int64,
    hash_slots_opt: Optional[Pointer[mut=True, Int64, MutUntrackedOrigin]],
    hash_capacity: Int64,
    health_weight: Int64,
    load_weight: Int64,
    cost_weight: Int64,
    latency_weight: Int64,
    risk_weight: Int64,
    priority_weight: Int64,
    affinity_weight: Int64,
    result_opt: Optional[Pointer[mut=True, ProdexRichRouteResult, MutUntrackedOrigin]],
) abi("C") -> Int64:
    if not result_opt:
        return RICH_STATUS_INVALID
    var result = result_opt.unsafe_value()
    result[].abi_version = PRODEX_RICH_ABI_VERSION
    result[].candidates_written = 0
    result[].required_candidates = 0
    result[].ordered_written = 0
    result[].selected_index = -1
    result[].output_written = 0
    result[].required_output = 0
    result[].issue_kind = 0
    result[].issue_offset = -1
    result[].issue_length = 0
    if abi_version != PRODEX_RICH_ABI_VERSION:
        route_issue(result, RICH_STATUS_ABI, -1, 0)
        return RICH_STATUS_ABI
    if input_count < 0 or input_count > RICH_MAX_RECORDS or record_capacity < input_count or ordered_capacity < input_count:
        result[].required_candidates = input_count
        return RICH_STATUS_INVALID
    if not rich_view_valid(required_capabilities, RICH_MAX_CAPABILITIES_BYTES):
        route_issue(result, RICH_ISSUE_INVALID_UTF8, 0, Int64(required_capabilities.len))
        return RICH_STATUS_UTF8
    var weights = InlineArray[Int64, 7](fill=0)
    weights[0] = health_weight
    weights[1] = load_weight
    weights[2] = cost_weight
    weights[3] = latency_weight
    weights[4] = risk_weight
    weights[5] = priority_weight
    weights[6] = affinity_weight
    if health_weight < 0 or health_weight > RICH_ROUTE_SCORE_SCALE or load_weight < 0 or load_weight > RICH_ROUTE_SCORE_SCALE or cost_weight < 0 or cost_weight > RICH_ROUTE_SCORE_SCALE or latency_weight < 0 or latency_weight > RICH_ROUTE_SCORE_SCALE or risk_weight < 0 or risk_weight > RICH_ROUTE_SCORE_SCALE or priority_weight < 0 or priority_weight > RICH_ROUTE_SCORE_SCALE or affinity_weight < 0 or affinity_weight > RICH_ROUTE_SCORE_SCALE:
        return RICH_STATUS_INVALID
    var weight_total = health_weight + load_weight + cost_weight + latency_weight + risk_weight + priority_weight + affinity_weight
    if weight_total <= 0 or weight_total > RICH_ROUTE_SCORE_SCALE:
        return RICH_STATUS_INVALID
    if input_count == 0:
        return RICH_STATUS_OK
    if not inputs_opt or not output_records_opt or not ordered_indices_opt or not output_opt or not hash_slots_opt:
        return RICH_STATUS_INVALID
    var inputs = inputs_opt.unsafe_value()
    var output_records = output_records_opt.unsafe_value()
    var ordered_indices = ordered_indices_opt.unsafe_value()
    var output = output_opt.unsafe_value()
    var hash_slots = hash_slots_opt.unsafe_value()
    var required_output: Int64 = 0
    for index in range(input_count):
        var item = inputs[unsafe_offset=index].copy()
        if not rich_view_valid(item.provider, RICH_MAX_IDENTIFIER_BYTES) or not rich_view_valid(item.model, RICH_MAX_IDENTIFIER_BYTES) or not rich_view_valid(item.capabilities, RICH_MAX_CAPABILITIES_BYTES) or not route_values_valid(item):
            return RICH_STATUS_INVALID
        required_output += Int64(item.provider.len) + Int64(item.model.len)
    result[].required_output = required_output
    var required_hash = rich_required_hash_capacity(input_count)
    if output_capacity < required_output or hash_capacity < required_hash:
        return RICH_STATUS_CAPACITY
    for index in range(hash_capacity):
        hash_slots[unsafe_offset=index] = -1
    var required_mask = rich_capability_mask(required_capabilities)
    var written: Int64 = 0
    var record_count: Int64 = 0
    var ordered_count: Int64 = 0
    for input_index in range(input_count):
        var input = inputs[unsafe_offset=input_index].copy()
        var before = written
        var provider_slice = rich_copy_trimmed(input.provider, output, output_capacity, Pointer(to=written), True)
        var model_slice = rich_copy_trimmed(input.model, output, output_capacity, Pointer(to=written), True)
        if provider_slice.len < 0 or model_slice.len < 0:
            return RICH_STATUS_CAPACITY
        var score = route_score(input, weights)
        var candidate = RouteCandidate(
            NormalizedIdentifier(input.provider.copy(), provider_slice.copy()),
            NormalizedIdentifier(input.model.copy(), model_slice.copy()),
            rich_capability_mask(input.capabilities),
            0,
            score.score,
            score.components.copy(),
            score.weighted_total,
            input_index,
        )
        var hash = rich_hash_pair(output, provider_slice, model_slice)
        var slot = Int64(hash % UInt64(hash_capacity))
        var duplicate: Int64 = -1
        for _ in range(hash_capacity):
            var existing = hash_slots[unsafe_offset=slot]
            if existing < 0:
                hash_slots[unsafe_offset=slot] = record_count
                break
            var existing_record = output_records[unsafe_offset=existing].copy()
            if rich_slice_equal_folded(output, provider_slice, existing_record.provider) and rich_slice_equal_folded(output, model_slice, existing_record.model):
                duplicate = existing
                break
            slot += 1
            if slot == hash_capacity:
                slot = 0
        var record = ProdexRichRouteRecord(
            provider_slice.copy(),
            model_slice.copy(),
            candidate.capability_mask,
            0,
            RICH_ROUTE_REASON_HARD_REJECTED,
            candidate.score,
            candidate.components.copy(),
            candidate.weighted_total,
            input_index,
            duplicate,
            route_provider_order(output, provider_slice),
            input.affinity,
        )
        if duplicate >= 0:
            record.provider = output_records[unsafe_offset=duplicate].provider.copy()
            record.model = output_records[unsafe_offset=duplicate].model.copy()
            record.reason = RICH_ROUTE_REASON_DUPLICATE
            written = before
        elif input.hard_eligible == 1:
            if (candidate.capability_mask & required_mask) == required_mask:
                record.eligible = 1
                record.reason = RICH_ROUTE_REASON_ELIGIBLE
                ordered_indices[unsafe_offset=ordered_count] = record_count
                ordered_count += 1
            else:
                record.reason = RICH_ROUTE_REASON_CAPABILITY_MISSING
        output_records[unsafe_offset=record_count] = record.copy()
        record_count += 1
    for position in range(ordered_count):
        var best = position
        for candidate_position in range(position + 1, ordered_count):
            var candidate_index = ordered_indices[unsafe_offset=candidate_position]
            var best_index = ordered_indices[unsafe_offset=best]
            if route_better(output_records, candidate_index, best_index):
                best = candidate_position
        if best != position:
            var saved = ordered_indices[unsafe_offset=position]
            ordered_indices[unsafe_offset=position] = ordered_indices[unsafe_offset=best]
            ordered_indices[unsafe_offset=best] = saved
    result[].candidates_written = record_count
    result[].required_candidates = input_count
    result[].ordered_written = ordered_count
    if ordered_count > 0:
        result[].selected_index = ordered_indices[unsafe_offset=0]
    result[].output_written = written
    return RICH_STATUS_OK
