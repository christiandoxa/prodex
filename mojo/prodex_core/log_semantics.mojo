from std.memory import Pointer

from rich_text import rich_view_matches_literal, rich_view_prefix, rich_view_valid
from rich_types import ProdexRichStringView, rich_view_ptr

comptime PRODEX_LOG_ABI_VERSION: Int64 = 4
comptime LOG_CATEGORY_NONE: Int64 = 0
comptime LOG_CATEGORY_ROUTE: Int64 = 1
comptime LOG_CATEGORY_QUOTA: Int64 = 2
comptime LOG_CATEGORY_BACKOFF: Int64 = 3
comptime LOG_CATEGORY_HTTP: Int64 = 4
comptime LOG_CATEGORY_WS: Int64 = 5
comptime LOG_CATEGORY_STREAM: Int64 = 6
comptime LOG_CATEGORY_SMART: Int64 = 7
comptime LOG_CATEGORY_COMPACT: Int64 = 8
comptime LOG_CATEGORY_ERROR: Int64 = 9
comptime LOG_CATEGORY_HOOK: Int64 = 10
comptime LOG_CATEGORY_REQUEST: Int64 = 11
comptime LOG_CATEGORY_MODEL: Int64 = 12
comptime LOG_CATEGORY_MCP: Int64 = 13
comptime LOG_CATEGORY_AGENT: Int64 = 14
comptime LOG_CATEGORY_TOOL: Int64 = 15
comptime LOG_CATEGORY_RETRY: Int64 = 16
comptime LOG_CATEGORY_HEALTH: Int64 = 17
comptime LOG_CATEGORY_UPSTREAM: Int64 = 18
comptime LOG_CATEGORY_RESPONSE: Int64 = 19
comptime LOG_CATEGORY_TERMINAL: Int64 = 20
comptime LOG_CATEGORY_LOAD: Int64 = 21
comptime PRODEX_LOG_LEVEL_ABI_VERSION: Int64 = 1
comptime PRODEX_LOG_LEVEL_MAX_BYTES: Int64 = 4096


def log_view_contains[literal: StaticString](
    view: ProdexRichStringView,
) -> Bool:
    var needle: Int64 = Int64(literal.byte_length())
    if needle == 0:
        return True
    if view.len < UInt(needle) or view.ptr == 0:
        return False
    var ptr = rich_view_ptr(view)
    for start in range(Int64(view.len) - needle + 1):
        var matched = True
        for index in range(needle):
            if ptr[unsafe_offset=start + index] != literal.unsafe_ptr()[unsafe_offset=index]:
                matched = False
                break
        if matched:
            return True
    return False


def log_view_part_matches(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, part: StringSlice
) -> Bool:
    var right = part.unsafe_ptr()
    for index in range(Int64(part.byte_length())):
        if ptr[unsafe_offset=start + index] != right[unsafe_offset=index]:
            return False
    return True


def log_view_starts_sequence2(
    view: ProdexRichStringView, first: StringSlice, second: StringSlice
) -> Bool:
    var total = Int64(first.byte_length()) + Int64(second.byte_length())
    if total > Int64(view.len) or view.ptr == 0:
        return False
    var ptr = rich_view_ptr(view)
    return log_view_part_matches(ptr, 0, first) and log_view_part_matches(
        ptr, Int64(first.byte_length()), second
    )


def log_view_starts_sequence3(
    view: ProdexRichStringView,
    first: StringSlice,
    second: StringSlice,
    third: StringSlice,
) -> Bool:
    var first_length = Int64(first.byte_length())
    var second_length = Int64(second.byte_length())
    var total = first_length + second_length + Int64(third.byte_length())
    if total > Int64(view.len) or view.ptr == 0:
        return False
    var ptr = rich_view_ptr(view)
    return log_view_part_matches(ptr, 0, first) and log_view_part_matches(
        ptr, first_length, second
    ) and log_view_part_matches(ptr, first_length + second_length, third)


def log_view_contains_sequence3(
    view: ProdexRichStringView,
    first: StringSlice,
    second: StringSlice,
    third: StringSlice,
) -> Bool:
    var first_length = Int64(first.byte_length())
    var second_length = Int64(second.byte_length())
    var third_length = Int64(third.byte_length())
    var total = first_length + second_length + third_length
    if total <= 0 or total > Int64(view.len) or view.ptr == 0:
        return False
    var ptr = rich_view_ptr(view)
    for start in range(Int64(view.len) - total + 1):
        if log_view_part_matches(ptr, start, first) and log_view_part_matches(
            ptr, start + first_length, second
        ) and log_view_part_matches(
            ptr, start + first_length + second_length, third
        ):
            return True
    return False


def log_view_contains_sequence5(
    view: ProdexRichStringView,
    first: StringSlice,
    second: StringSlice,
    third: StringSlice,
    fourth: StringSlice,
    fifth: StringSlice,
) -> Bool:
    var first_length = Int64(first.byte_length())
    var second_length = Int64(second.byte_length())
    var third_length = Int64(third.byte_length())
    var fourth_length = Int64(fourth.byte_length())
    var total = (
        first_length
        + second_length
        + third_length
        + fourth_length
        + Int64(fifth.byte_length())
    )
    if total <= 0 or total > Int64(view.len) or view.ptr == 0:
        return False
    var ptr = rich_view_ptr(view)
    for start in range(Int64(view.len) - total + 1):
        if not log_view_part_matches(ptr, start, first):
            continue
        var second_start = start + first_length
        if not log_view_part_matches(ptr, second_start, second):
            continue
        var third_start = second_start + second_length
        if not log_view_part_matches(ptr, third_start, third):
            continue
        var fourth_start = third_start + third_length
        if not log_view_part_matches(ptr, fourth_start, fourth):
            continue
        if log_view_part_matches(ptr, fourth_start + fourth_length, fifth):
            return True
    return False


def log_view_contains_json_level(
    view: ProdexRichStringView, field: StringSlice, level: StringSlice
) -> Bool:
    return log_view_contains_sequence5(
        view,
        StringSlice("\""),
        field,
        StringSlice("\":\""),
        level,
        StringSlice("\""),
    ) or log_view_contains_sequence5(
        view,
        StringSlice("\""),
        field,
        StringSlice("\": \""),
        level,
        StringSlice("\""),
    )


def log_view_contains_kv_level(
    view: ProdexRichStringView, field: StringSlice, level: StringSlice
) -> Bool:
    return log_view_contains_sequence3(view, field, StringSlice("="), level) or log_view_contains_sequence3(
        view, field, StringSlice(": "), level
    )


def log_view_level_matches(view: ProdexRichStringView, level_id: Int64) -> Bool:
    if level_id < 1 or level_id > 7:
        return False
    var level = StringSlice("fatal")
    if level_id == 2:
        level = StringSlice("error")
    elif level_id == 3:
        level = StringSlice("warn")
    elif level_id == 4:
        level = StringSlice("warning")
    elif level_id == 5:
        level = StringSlice("info")
    elif level_id == 6:
        level = StringSlice("debug")
    elif level_id == 7:
        level = StringSlice("trace")

    for field in [StringSlice("level"), StringSlice("severity"), StringSlice("status")]:
        if log_view_contains_json_level(view, field, level) or log_view_contains_kv_level(
            view, field, level
        ):
            return True
    return log_view_starts_sequence2(view, level, StringSlice(" ")) or log_view_starts_sequence2(
        view, level, StringSlice(":")
    ) or log_view_starts_sequence2(view, level, StringSlice("\t")) or log_view_starts_sequence3(
        view, StringSlice("["), level, StringSlice("]")
    ) or log_view_contains_sequence3(
        view, StringSlice(" "), level, StringSlice(" ")
    ) or log_view_contains_sequence3(
        view, StringSlice(" "), level, StringSlice(":")
    ) or log_view_contains_sequence3(
        view, StringSlice(" ["), level, StringSlice("]")
    ) or log_view_contains_sequence3(
        view, StringSlice(" "), level, StringSlice("\t")
    )


def set_category(
    category: Pointer[mut=True, Int64, _],
    severity: Pointer[mut=True, Int64, _],
    value: Int64,
    level: Int64,
) -> None:
    category[] = value
    severity[] = level


@export("prodex_mojo_log_classify_v3")
def prodex_mojo_log_classify_v3(
    abi_version: Int64,
    event_address: UInt,
    category_address: UInt,
    severity_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_LOG_ABI_VERSION or event_address == 0 or category_address == 0 or severity_address == 0:
        return 1
    var event_ptr = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(event_address))
    var event = event_ptr[].copy()
    if not rich_view_valid(event, 128):
        return 2

    var category = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(category_address)
    )
    var severity = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(severity_address)
    )
    set_category(category, severity, LOG_CATEGORY_NONE, 0)
    if rich_view_matches_literal["request_captured"](event, False) or rich_view_matches_literal["compat_request_surface"](event, False):
        set_category(category, severity, LOG_CATEGORY_REQUEST, 1)
    elif (
        rich_view_matches_literal["selection_pick"](event, False)
        or rich_view_matches_literal["selection_skip_current"](event, False)
        or rich_view_matches_literal["profile_commit"](event, False)
    ):
        set_category(category, severity, LOG_CATEGORY_ROUTE, 1)
    elif (
        rich_view_matches_literal["profile_quota_exhausted"](event, False)
        or rich_view_matches_literal["quota_exhausted"](event, False)
    ):
        set_category(category, severity, LOG_CATEGORY_QUOTA, 2)
    elif (
        rich_view_matches_literal["profile_retry_backoff"](event, False)
        or rich_view_matches_literal["profile_transport_backoff"](event, False)
        or rich_view_matches_literal["rotation_waiting_for_recovery"](event, False)
        or rich_view_matches_literal["profile_circuit_open"](event, False)
    ):
        set_category(category, severity, LOG_CATEGORY_BACKOFF, 2)
    elif (
        rich_view_matches_literal["upstream_start"](event, False)
        or rich_view_matches_literal["upstream_response"](event, False)
        or rich_view_matches_literal["upstream_async_start"](event, False)
        or rich_view_matches_literal["upstream_async_response"](event, False)
        or rich_view_matches_literal["upstream_connect_start"](event, False)
        or rich_view_matches_literal["upstream_connect_ok"](event, False)
        or rich_view_matches_literal["upstream_connect_error"](event, False)
    ):
        set_category(category, severity, LOG_CATEGORY_HTTP, 1)
    elif (
        rich_view_matches_literal["first_upstream_chunk"](event, False)
        or rich_view_matches_literal["first_local_chunk"](event, False)
    ):
        set_category(category, severity, LOG_CATEGORY_STREAM, 1)
    elif (
        rich_view_matches_literal["smart_context_autopilot"](event, False)
        or rich_view_matches_literal["smart_context_prepare_error"](event, False)
        or rich_view_matches_literal["smart_context_prepare_fallback"](event, False)
    ):
        set_category(category, severity, LOG_CATEGORY_SMART, 1)
    elif (
        rich_view_matches_literal["profile_probe_refresh_error"](event, False)
        or rich_view_matches_literal["upstream_read_error"](event, False)
        or rich_view_matches_literal["upstream_close_before_completed"](event, False)
        or rich_view_matches_literal["invalid_previous_response_id"](event, False)
        or rich_view_matches_literal["session_error"](event, False)
    ):
        set_category(category, severity, LOG_CATEGORY_ERROR, 3)
    elif (
        rich_view_matches_literal["hook_started"](event, False)
        or rich_view_matches_literal["hook_completed"](event, False)
    ):
        set_category(category, severity, LOG_CATEGORY_HOOK, 1)
    elif (
        rich_view_matches_literal["context_compacted"](event, False)
        or rich_view_matches_literal["compact_started"](event, False)
        or rich_view_matches_literal["compact_completed"](event, False)
    ):
        set_category(category, severity, LOG_CATEGORY_COMPACT, 1)
    elif rich_view_matches_literal["compat_request_surface"](event, False):
        set_category(category, severity, LOG_CATEGORY_REQUEST, 1)
    elif (
        rich_view_matches_literal["profile_auth_recovered"](event, False)
        or rich_view_matches_literal["local_rewrite_request_detail"](event, False)
        or rich_view_matches_literal["local_rewrite_provider_model_fallback"](event, False)
    ):
        set_category(category, severity, LOG_CATEGORY_MODEL, 1)
    elif (
        log_view_contains["mcp"](event)
        or rich_view_prefix["expose_"](event, False)
    ):
        set_category(category, severity, LOG_CATEGORY_MCP, 1)
    elif log_view_contains["sub_agent"](event) or log_view_contains["subagent"](event):
        set_category(category, severity, LOG_CATEGORY_AGENT, 1)
    elif rich_view_matches_literal["local_rewrite_gemini_builtin_tool_fallback"](event, False):
        set_category(category, severity, LOG_CATEGORY_TOOL, 1)
    elif log_view_contains["retry"](event) or log_view_contains["fresh_fallback"](event):
        set_category(category, severity, LOG_CATEGORY_RETRY, 2)
    elif (
        rich_view_matches_literal["profile_transport_failure"](event, False)
        or rich_view_matches_literal["profile_health"](event, False)
        or rich_view_matches_literal["profile_bad_pairing"](event, False)
    ):
        set_category(category, severity, LOG_CATEGORY_HEALTH, 1)
    elif log_view_contains["upstream"](event):
        set_category(category, severity, LOG_CATEGORY_UPSTREAM, 1)
    elif rich_view_matches_literal["buffered_response_complete"](event, False):
        set_category(category, severity, LOG_CATEGORY_RESPONSE, 1)
    elif rich_view_matches_literal["terminal_event"](event, False):
        set_category(category, severity, LOG_CATEGORY_TERMINAL, 2)
    elif (
        log_view_contains["queue_overloaded"](event)
        or log_view_contains["limit_reached"](event)
        or rich_view_matches_literal["profile_inflight_saturated"](event, False)
    ):
        set_category(category, severity, LOG_CATEGORY_LOAD, 2)
    return 0


@export("prodex_mojo_log_level_classify_v1")
def prodex_mojo_log_level_classify_v1(
    abi_version: Int64, event_address: UInt, level_address: UInt
) abi("C") -> Int64:
    if (
        abi_version != PRODEX_LOG_LEVEL_ABI_VERSION
        or event_address == 0
        or level_address == 0
    ):
        return 1
    var event_ptr = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(event_address))
    var event = event_ptr[].copy()
    if not rich_view_valid(event, PRODEX_LOG_LEVEL_MAX_BYTES):
        return 2

    var level = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(level_address)
    )
    level[] = 0
    var candidate: Int64 = 1
    while candidate <= 7:
        if log_view_level_matches(event, candidate):
            if candidate >= 4:
                level[] = candidate - 1
            else:
                level[] = candidate
            return 0
        candidate += 1
    return 0
