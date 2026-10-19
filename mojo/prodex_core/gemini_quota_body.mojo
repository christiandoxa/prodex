# Deterministic Gemini quota/rate-limit classification over Serde's borrowed
# JSON tree. The host handles JSON and SSE decoding, never duplicate policy.
from std.memory import Pointer
from std.math import ceil, isfinite
from log_number import log_parse_u64_text
from rich_catalog import catalog_view_find_folded
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_OBJECT, JSON_STRING, pj_valid,
    pj_field, pj_string_field, pj_kind, pj_text, pj_child, pj_next,
)
from rich_types import ProdexRichStringView
from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_matches_literal, rich_view_prefix, rich_view_valid

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
    for index in range(nodes_count):
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

# Gemini normalized HTTP error compatibility is a single compiled decision:
# choose the error type/code, reject ambiguous 429 payloads and make retry/
# rotate decisions before the Rust host constructs any HTTP response.
comptime GEMINI_QUOTA_HTTP_ABI: Int64 = 1


def gemini_quota_retryable_http_status(status: Int64) -> Bool:
    return status == 403 or status == 429


def gemini_quota_copy_label(
    data: StringSlice,
    dest: Pointer[mut=True, UInt8, _],
    offset: Int64,
) -> Int64:
    var length = Int64(data.byte_length())
    var source = data.unsafe_ptr()
    for index in range(length):
        dest[unsafe_offset=offset + index] = source[unsafe_offset=index]
    return length


@export("prodex_gemini_quota_error_kind_v1")
def prodex_gemini_quota_error_kind_v1(
    abi_version: Int64,
    http_status: Int64,
    body_terminal: Int64,
    value_terminal: Int64,
    value_rate: Int64,
    output_address: UInt,
    output_capacity: Int64,
    metadata_address: UInt,
    metadata_capacity: Int64,
) abi("C") -> Int64:
    if abi_version != GEMINI_QUOTA_HTTP_ABI:
        return 4
    if (
        http_status < 0 or http_status > 65535
        or (body_terminal != 0 and body_terminal != 1)
        or (value_terminal != 0 and value_terminal != 1)
        or (value_rate != 0 and value_rate != 1)
        or output_address == 0 or output_capacity < 0
        or metadata_address == 0 or metadata_capacity < 3
    ):
        return 1
    var terminal = body_terminal == 1 or value_terminal == 1
    var rate = value_rate == 1
    var admit = http_status >= 400 and (
        http_status != 429 or terminal or rate
    )
    var error_type = StringSlice("api_error")
    var error_code = StringSlice("provider_error")
    if terminal:
        error_type = StringSlice("insufficient_quota")
        error_code = StringSlice("insufficient_quota")
    elif rate:
        error_type = StringSlice("rate_limit_error")
        error_code = StringSlice("rate_limit_exceeded")
    elif http_status == 400:
        error_type = StringSlice("invalid_request_error")
        error_code = StringSlice("bad_request")
    elif http_status == 401:
        error_type = StringSlice("authentication_error")
        error_code = StringSlice("invalid_authentication")
    elif http_status == 403:
        error_type = StringSlice("permission_error")
        error_code = StringSlice("permission_denied")
    elif http_status == 404:
        error_type = StringSlice("invalid_request_error")
        error_code = StringSlice("not_found")
    elif http_status >= 500 and http_status <= 599:
        error_type = StringSlice("server_error")
        error_code = StringSlice("provider_error")

    var type_len = Int64(error_type.byte_length())
    var code_len = Int64(error_code.byte_length())
    if type_len > output_capacity or code_len > output_capacity - type_len:
        return 3
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var meta = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(metadata_address)
    )
    _ = gemini_quota_copy_label(error_type, output, 0)
    _ = gemini_quota_copy_label(error_code, output, type_len)
    meta[unsafe_offset=0] = Int64(admit)
    meta[unsafe_offset=1] = type_len
    meta[unsafe_offset=2] = code_len
    return 0


@export("prodex_gemini_quota_retryable_http_v1")
def prodex_gemini_quota_retryable_http_v1(
    abi_version: Int64, http_status: Int64, result_address: UInt
) abi("C") -> Int64:
    if abi_version != GEMINI_QUOTA_HTTP_ABI:
        return 4
    if http_status < 0 or http_status > 65535 or result_address == 0:
        return 1
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )[] = Int64(gemini_quota_retryable_http_status(http_status))
    return 0


@export("prodex_gemini_quota_rotate_v1")
def prodex_gemini_quota_rotate_v1(
    abi_version: Int64,
    http_status: Int64,
    quota_blocked: Int64,
    hard_affinity: Int64,
    attempt_index: UInt64,
    attempt_count: UInt64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != GEMINI_QUOTA_HTTP_ABI:
        return 4
    if (
        http_status < 0 or http_status > 65535
        or (quota_blocked != 0 and quota_blocked != 1)
        or (hard_affinity != 0 and hard_affinity != 1)
        or result_address == 0
    ):
        return 1
    var rotate = (
        quota_blocked == 1
        and gemini_quota_retryable_http_status(http_status)
        and hard_affinity == 0
        and attempt_count > 0
        and attempt_index < attempt_count - 1
    )
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )[] = Int64(rotate)
    return 0


# Numeric acquisition stays in Rust: the pinned fallible Float64 text parser
# requires CompilerRT heap/error symbols absent from the native object ABI.
def gemini_retry_text_candidate(
    output: Pointer[mut=True, UInt64, MutUntrackedOrigin], slot: Int64,
    start: Int64, end: Int64, multiplier: UInt64,
):
    output[unsafe_offset=0] = 2
    output[unsafe_offset=2] = UInt64(slot + 1)
    output[unsafe_offset=3 + slot * 3] = UInt64(start)
    output[unsafe_offset=4 + slot * 3] = UInt64(end)
    output[unsafe_offset=5 + slot * 3] = multiplier


@export("prodex_gemini_retry_text_plan_v1")
def prodex_gemini_retry_text_plan_v1(
    version: Int64, operation: Int64, address: UInt, length: Int64,
    output_address: UInt, capacity: Int64,
) abi("C") -> Int64:
    if version != 1:
        return 4
    if operation < 0 or operation > 2 or length < 0 or output_address == 0 or output_address % 8 != 0:
        return 1
    if capacity < 9:
        return 3
    var view = ProdexRichStringView(address, UInt(length))
    if not rich_view_valid(view, length):
        return 1
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    for index in range(9):
        output[unsafe_offset=index] = 0
    var bounds = rich_trim_bounds(view)
    var start = bounds[0]
    var end = bounds[1]
    if operation == 2:
        var parsed = log_parse_u64_text(UInt64(address + UInt(start)), UInt64(end - start))
        if parsed[0] and parsed[1] > 0:
            output[unsafe_offset=0] = 1
            output[unsafe_offset=1] = 18446744073709551615 if parsed[1] > 18446744073709551 else parsed[1] * 1000
        return 0
    var source = rich_view_ptr(view)
    if operation == 0:
        var multiplier: UInt64 = 1000
        if end - start >= 2 and source[unsafe_offset=end - 2] == 109 and source[unsafe_offset=end - 1] == 115:
            end -= 2
            multiplier = 1
        elif end > start and source[unsafe_offset=end - 1] == 115:
            end -= 1
        else:
            return 0
        var numeric = rich_trim_bounds(ProdexRichStringView(address + UInt(start), UInt(end - start)))
        if numeric[0] < numeric[1]:
            gemini_retry_text_candidate(output, 0, start + numeric[0], start + numeric[1], multiplier)
        return 0
    var count: Int64 = 0
    for index in range(2):
        var marker = StringSlice("please retry in ")
        if index == 1:
            marker = StringSlice("suggested retry after ")
        var needle = ProdexRichStringView(UInt(Int(marker.unsafe_ptr())), UInt(marker.byte_length()))
        var offset = catalog_view_find_folded(view, needle)
        if offset < 0:
            continue
        var after = offset + Int64(needle.len)
        var token = rich_trim_bounds(ProdexRichStringView(address + UInt(after), UInt(length - after)))
        var number_start = after + token[0]
        var number_end = number_start
        while number_end < length:
            var byte = source[unsafe_offset=number_end]
            if byte != 46 and (byte < 48 or byte > 57):
                break
            number_end += 1
        if number_end == number_start:
            continue
        var suffix = ProdexRichStringView(address + UInt(number_end), UInt(length - number_end))
        var multiplier: UInt64 = 0
        if rich_view_prefix["ms"](suffix, True):
            multiplier = 1
        elif rich_view_prefix["s"](suffix, True):
            multiplier = 1000
        if multiplier != 0:
            gemini_retry_text_candidate(output, count, number_start, number_end, multiplier)
            count += 1
    return 0


@export("prodex_gemini_retry_text_finish_v1")
def prodex_gemini_retry_text_finish_v1(
    version: Int64, count: Int64, parsed: UInt64, first: Float64, second: Float64,
    first_multiplier: UInt64, second_multiplier: UInt64, output_address: UInt, capacity: Int64,
) abi("C") -> Int64:
    if version != 1:
        return 4
    if count < 1 or count > 2 or parsed >= (UInt64(1) << UInt64(count)) or output_address == 0 or output_address % 8 != 0:
        return 1
    if (first_multiplier != 1 and first_multiplier != 1000) or (count == 2 and second_multiplier != 1 and second_multiplier != 1000):
        return 1
    if capacity < 2:
        return 3
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = 0
    for index in range(count):
        var value = first if index == 0 else second
        if (parsed & (UInt64(1) << UInt64(index))) == 0 or not isfinite(value) or value <= 0.0:
            continue
        var multiplier = first_multiplier if index == 0 else second_multiplier
        var millis = ceil(value * Float64(multiplier))
        var delay = UInt64(18446744073709551615)
        if isfinite(millis) and millis < Float64(delay):
            delay = UInt64(millis)
        output[unsafe_offset=0] = 1
        output[unsafe_offset=1] = delay
        break
    return 0
