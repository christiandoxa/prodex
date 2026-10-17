# Deterministic Gemini quota/rate-limit classification over Serde's borrowed
# JSON tree. The host handles JSON and SSE decoding, never duplicate policy.
from std.memory import Pointer
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_OBJECT, JSON_STRING, pj_valid,
    pj_field, pj_string_field, pj_kind, pj_text, pj_child, pj_next,
)
from rich_types import ProdexRichStringView
from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_matches_literal

comptime GEMINI_QUOTA_BODY_ABI: Int64 = 1

def gemini_quota_trimmed(view: ProdexRichStringView) -> ProdexRichStringView:
    var bounds = rich_trim_bounds(view)
    return ProdexRichStringView(view.ptr + UInt(bounds[0]), UInt(bounds[1] - bounds[0]))


def gemini_quota_terminal_code(view: ProdexRichStringView) -> Bool:
    var code = gemini_quota_trimmed(view)
    return (
        rich_view_matches_literal["quota_exhausted"](code, True)
        or rich_view_matches_literal["quota_exceeded"](code, True)
        or rich_view_matches_literal["resource_exhausted"](code, True)
        or rich_view_matches_literal["insufficient_g1_credits_balance"](code, True)
        or rich_view_matches_literal["insufficient_quota"](code, True)
    )


def gemini_quota_rate_code(view: ProdexRichStringView) -> Bool:
    var code = gemini_quota_trimmed(view)
    return (
        rich_view_matches_literal["rate_limit_exceeded"](code, True)
        or rich_view_matches_literal["rate_limit_exceeded_error"](code, True)
    )


def gemini_quota_message_code(view: ProdexRichStringView) -> Bool:
    var code = gemini_quota_trimmed(view)
    return (
        rich_view_matches_literal["resource_exhausted"](code, True)
        or rich_view_matches_literal["quota_exhausted"](code, True)
        or rich_view_matches_literal["quota_exceeded"](code, True)
        or gemini_quota_rate_code(view)
    )


def gemini_quota_contains(view: ProdexRichStringView, needle: StringSlice) -> Bool:
    var length = Int64(needle.byte_length())
    if view.len < UInt(length):
        return False
    var data = rich_view_ptr(view)
    var match_bytes = needle.unsafe_ptr()
    for start in range(Int64(view.len) - length + 1):
        var matched = True
        for index in range(length):
            if data[unsafe_offset=start + index] != match_bytes[unsafe_offset=index]:
                matched = False
                break
        if matched:
            return True
    return False


def gemini_quota_has_daily_limit(tree: ParsedJson, node: Int64) -> Bool:
    var quota = pj_field(tree, node, StringSlice("quotaId"))
    if quota < 0:
        quota = pj_field(tree, node, StringSlice("quota_limit"))
    if quota < 0:
        quota = pj_field(tree, node, StringSlice("quotaLimit"))
    if pj_kind(tree, quota) != JSON_STRING:
        return False
    var value = pj_text(tree, quota)
    return gemini_quota_contains(value, StringSlice("PerDay")) or gemini_quota_contains(value, StringSlice("Daily"))


def gemini_quota_message_field(tree: ParsedJson, node: Int64) -> Int64:
    var result = pj_string_field(tree, node, StringSlice("message"))
    if result < 0:
        result = pj_string_field(tree, node, StringSlice("detail"))
    if result < 0:
        result = pj_string_field(tree, node, StringSlice("error"))
    return result


# 3 outputs: terminal quota, rate limit, quota message string-node index;
# message index = -1 means default quota text, -2 means no quota code found.
@export("prodex_gemini_quota_body_plan_v1")
def prodex_gemini_quota_body_plan_v1(
    abi_version: Int64,
    nodes_address: UInt,
    nodes_count: Int64,
    raw_address: UInt,
    raw_length: Int64,
    previous_sibling_address: UInt,
    scratch_capacity: Int64,
    output_address: UInt,
    output_capacity: Int64,
) abi("C") -> Int64:
    if abi_version != GEMINI_QUOTA_BODY_ABI:
        return 4
    if (
        nodes_count <= 0 or nodes_count > 0x7FFFFFFFFFFFFFFF // 80
        or raw_length < 0
        or nodes_address == 0 or previous_sibling_address == 0
        or raw_address == 0 or output_address == 0
        or scratch_capacity < nodes_count or output_capacity < 3
    ):
        return 1
    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](
            unsafe_from_address=Int(nodes_address)
        ),
        nodes_count,
        ProdexRichStringView(raw_address, UInt(raw_length)),
    )
    if not pj_valid(tree):
        return 1
    var previous = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(previous_sibling_address)
    )
    for index in range(nodes_count):
        previous[unsafe_offset=index] = -1
        var child = pj_child(tree, index)
        var prior: Int64 = -1
        while child >= 0:
            previous[unsafe_offset=child] = prior
            prior = child
            child = pj_next(tree, child)

    var terminal = False
    var rate = False
    var message_index: Int64 = -2
    # Reproduce Rust BTreeMap+Vec stack DFS: each object's last child first.
    # A scratch previous-sibling index makes traversal linear, even for large
    # hostile error bodies.
    var node: Int64 = 0
    while node >= 0:
        if pj_kind(tree, node) == JSON_OBJECT:
            var terminal_here = False
            var rate_here = False
            var message_here = False
            for key in [StringSlice("status"), StringSlice("code"), StringSlice("reason")]:
                var field = pj_string_field(tree, node, key)
                if field >= 0:
                    var value = pj_text(tree, field)
                    terminal_here = terminal_here or gemini_quota_terminal_code(value)
                    rate_here = rate_here or gemini_quota_rate_code(value)
                    message_here = message_here or gemini_quota_message_code(value)
            terminal = terminal or terminal_here or gemini_quota_has_daily_limit(tree, node)
            rate = rate or rate_here
            if message_here and message_index == -2:
                message_index = gemini_quota_message_field(tree, node)
        var last = pj_child(tree, node)
        if last >= 0:
            while pj_next(tree, last) >= 0:
                last = pj_next(tree, last)
            node = last
        else:
            var current = node
            var prev: Int64 = -1
            while current >= 0:
                prev = previous[unsafe_offset=current]
                if prev >= 0:
                    break
                current = tree.nodes[unsafe_offset=current].parent
            node = prev

    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    result[unsafe_offset=0] = Int64(terminal)
    result[unsafe_offset=1] = Int64(rate)
    result[unsafe_offset=2] = message_index
    return 0


# The same compiled Gemini quota owner also extracts nested retry-delay
# fields and imposes rate/minute quota minimum delays. Float64 token parsing
# remains with Rust's standard f64 parser to retain exact accepted grammar
# and rounding, then Mojo selects the final maximum across all SSE frames.
def gemini_quota_has_minute_limit(tree: ParsedJson, node: Int64) -> Bool:
    var quota = pj_field(tree, node, StringSlice("quotaId"))
    if quota < 0:
        quota = pj_field(tree, node, StringSlice("quota_limit"))
    if quota < 0:
        quota = pj_field(tree, node, StringSlice("quotaLimit"))
    return (
        pj_kind(tree, quota) == JSON_STRING
        and gemini_quota_contains(pj_text(tree, quota), StringSlice("PerMinute"))
    )


@export("prodex_gemini_retry_candidates_v1")
def prodex_gemini_retry_candidates_v1(
    abi_version: Int64,
    nodes_address: UInt,
    nodes_count: Int64,
    raw_address: UInt,
    raw_length: Int64,
    candidate_address: UInt,
    candidate_capacity: Int64,
    written_address: UInt,
    floor_address: UInt,
) abi("C") -> Int64:
    if abi_version != GEMINI_QUOTA_BODY_ABI:
        return 4
    if (
        nodes_address == 0 or nodes_count <= 0
        or nodes_count > 0x7FFFFFFFFFFFFFFF // 80
        or raw_address == 0 or raw_length < 0
        or candidate_address == 0 or candidate_capacity < nodes_count * 2
        or written_address == 0 or floor_address == 0
    ):
        return 1
    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](
            unsafe_from_address=Int(nodes_address)
        ),
        nodes_count,
        ProdexRichStringView(raw_address, UInt(raw_length)),
    )
    if not pj_valid(tree):
        return 1
    var candidates = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(candidate_address)
    )
    var written: Int64 = 0
    var floor_ms: UInt64 = 0
    for index in range(nodes_count):
        var node = tree.nodes[unsafe_offset=index].copy()
        if node.kind == JSON_OBJECT:
            for key in [StringSlice("status"), StringSlice("code"), StringSlice("reason")]:
                var code = pj_string_field(tree, index, key)
                if code >= 0 and gemini_quota_rate_code(pj_text(tree, code)):
                    floor_ms = max(floor_ms, UInt64(10000))
            if gemini_quota_has_minute_limit(tree, index):
                floor_ms = max(floor_ms, UInt64(60000))
        if (
            node.kind == JSON_STRING
            and node.parent >= 0
            and pj_kind(tree, node.parent) == JSON_OBJECT
        ):
            var kind: Int64 = -1
            if rich_view_matches_literal["retryDelay"](node.key, False):
                kind = 0
            elif (
                rich_view_matches_literal["message"](node.key, False)
                or rich_view_matches_literal["detail"](node.key, False)
                or rich_view_matches_literal["error"](node.key, False)
            ):
                kind = 1
            if kind >= 0:
                candidates[unsafe_offset=written * 2] = index
                candidates[unsafe_offset=written * 2 + 1] = kind
                written += 1
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )[] = written
    Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(floor_address)
    )[] = floor_ms
    return 0


@export("prodex_gemini_retry_delay_max_v1")
def prodex_gemini_retry_delay_max_v1(
    abi_version: Int64,
    candidates_address: UInt,
    candidate_count: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != GEMINI_QUOTA_BODY_ABI:
        return 4
    if (
        candidate_count < 0 or (candidate_count > 0 and candidates_address == 0)
        or result_address == 0
    ):
        return 1
    var values = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(candidates_address)
    )
    var result: UInt64 = 0
    for index in range(candidate_count):
        result = max(result, values[unsafe_offset=index])
    Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )[] = result
    return 0
