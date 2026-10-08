from std.memory import Pointer

from rich_text import rich_codepoint, rich_codepoint_width, rich_trim_bounds, rich_view_matches_literal, rich_view_valid
from rich_types import ProdexRichStringView, rich_view_ptr

comptime PRODEX_LOG_SNAPSHOT_ABI_VERSION: Int64 = 1
comptime PRODEX_LOG_SNAPSHOT_STATUS_OK: Int64 = 0
comptime PRODEX_LOG_SNAPSHOT_STATUS_INVALID: Int64 = 1
comptime PRODEX_LOG_SNAPSHOT_STATUS_ABI: Int64 = 4


def log_snapshot_flag_valid(value: Int64) -> Bool:
    return value == 0 or value == 1


@export("prodex_mojo_log_snapshot_order_v1")
def prodex_mojo_log_snapshot_order_v1(
    abi_version: Int64,
    transcript_present: Int64,
    upstream_present: Int64,
    token_usage_present: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_LOG_SNAPSHOT_ABI_VERSION:
        return PRODEX_LOG_SNAPSHOT_STATUS_ABI
    if (
        output_address == 0
        or not log_snapshot_flag_valid(transcript_present)
        or not log_snapshot_flag_valid(upstream_present)
        or not log_snapshot_flag_valid(token_usage_present)
    ):
        return PRODEX_LOG_SNAPSHOT_STATUS_INVALID

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var count: Int64 = 0
    if transcript_present == 1:
        output[unsafe_offset=count + 1] = 0
        count += 1
    if upstream_present == 1:
        output[unsafe_offset=count + 1] = 1
        count += 1
    if token_usage_present == 1:
        output[unsafe_offset=count + 1] = 2
        count += 1
    output[unsafe_offset=0] = count
    return PRODEX_LOG_SNAPSHOT_STATUS_OK


comptime PRODEX_LOG_LEVEL_ABI_VERSION: Int64 = 1
comptime PRODEX_LOG_LEVEL_MAX_BYTES: Int64 = 4096


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


comptime PRODEX_LOG_EVENT_NAME_ABI_VERSION: Int64 = 1
comptime PRODEX_LOG_EVENT_NAME_STATUS_OK: Int64 = 0
comptime PRODEX_LOG_EVENT_NAME_STATUS_INVALID: Int64 = 1
comptime PRODEX_LOG_EVENT_NAME_STATUS_CAPACITY: Int64 = 2
comptime PRODEX_LOG_EVENT_NAME_STATUS_ABI: Int64 = 4


def log_view_starts_literal(view: ProdexRichStringView, literal: StringSlice) -> Bool:
    var length = Int64(literal.byte_length())
    if view.ptr == 0 or length > Int64(view.len):
        return False
    var input = rich_view_ptr(view)
    var expected = literal.unsafe_ptr()
    for index in range(length):
        if input[unsafe_offset=index] != expected[unsafe_offset=index]:
            return False
    return True


def log_view_contains_literal(view: ProdexRichStringView, literal: StringSlice) -> Bool:
    var length = Int64(literal.byte_length())
    if view.ptr == 0 or length <= 0 or length > Int64(view.len):
        return False
    var input = rich_view_ptr(view)
    var expected = literal.unsafe_ptr()
    for start in range(Int64(view.len) - length + 1):
        var matched = True
        for offset in range(length):
            if input[unsafe_offset=start + offset] != expected[unsafe_offset=offset]:
                matched = False
                break
        if matched:
            return True
    return False


@export("prodex_mojo_log_event_name_v1")
def prodex_mojo_log_event_name_v1(
    abi_version: Int64,
    event_address: UInt,
    event_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_LOG_EVENT_NAME_ABI_VERSION:
        return PRODEX_LOG_EVENT_NAME_STATUS_ABI
    if (
        event_length < 0
        or output_capacity < 0
        or written_address == 0
        or (event_length > 0 and event_address == 0)
        or (output_capacity > 0 and output_address == 0)
    ):
        return PRODEX_LOG_EVENT_NAME_STATUS_INVALID
    var event = ProdexRichStringView(event_address, UInt(event_length))
    if not rich_view_valid(event, event_length):
        return PRODEX_LOG_EVENT_NAME_STATUS_INVALID

    var label = StringSlice("")
    if rich_view_matches_literal["request_captured"](event, False):
        label = StringSlice("received request")
    elif rich_view_matches_literal["route_decision"](event, False):
        label = StringSlice("route decided")
    elif rich_view_matches_literal["selection_plan"](event, False):
        label = StringSlice("route planned")
    elif rich_view_matches_literal["selection_pick"](event, False):
        label = StringSlice("profile picked")
    elif rich_view_matches_literal["selection_keep_affinity"](event, False):
        label = StringSlice("owner kept")
    elif rich_view_matches_literal["selection_keep_current"](event, False):
        label = StringSlice("current profile kept")
    elif rich_view_matches_literal["selection_skip_current"](event, False):
        label = StringSlice("profile skipped")
    elif rich_view_matches_literal["selection_skip_affinity"](event, False):
        label = StringSlice("affinity skipped")
    elif rich_view_matches_literal["selection_skip_sync_probe"](event, False):
        label = StringSlice("quota probe skipped")
    elif rich_view_matches_literal["local_selection_blocked"](event, False):
        label = StringSlice("route blocked")
    elif rich_view_matches_literal["profile_commit"](event, False):
        label = StringSlice("profile committed")
    elif rich_view_matches_literal["route_affinity_recompute"](event, False):
        label = StringSlice("affinity recomputed")
    elif rich_view_matches_literal["route_affinity_recompute_result"](event, False):
        label = StringSlice("affinity resolved")
    elif rich_view_matches_literal["previous_response_owner"](event, False):
        label = StringSlice("continuation owner")
    elif rich_view_matches_literal["previous_response_not_found"](event, False):
        label = StringSlice("continuation missing")
    elif rich_view_matches_literal["previous_response_negative_cache"](event, False):
        label = StringSlice("continuation cached missing")
    elif rich_view_matches_literal["previous_response_fresh_fallback"](event, False):
        label = StringSlice("fresh fallback")
    elif rich_view_matches_literal["previous_response_fresh_fallback_blocked"](event, False):
        label = StringSlice("fallback blocked")
    elif rich_view_matches_literal["previous_response_turn_state_rehydrated"](event, False):
        label = StringSlice("turn state restored")
    elif rich_view_matches_literal["session_rotation_release_affinity"](event, False):
        label = StringSlice("session affinity released")
    elif rich_view_matches_literal["binding_prompt_cache"](event, False):
        label = StringSlice("prompt cache bound")
    elif rich_view_matches_literal["upgrade"](event, False) or rich_view_matches_literal["upgraded"](event, False):
        label = StringSlice("request upgraded")
    elif rich_view_matches_literal["profile_quota_exhausted"](event, False) or rich_view_matches_literal["quota_exhausted"](event, False):
        label = StringSlice("quota exhausted")
    elif rich_view_matches_literal["quota_blocked"](event, False):
        label = StringSlice("quota blocked")
    elif rich_view_matches_literal["quota_critical_floor_before_send"](event, False):
        label = StringSlice("quota floor blocked")
    elif rich_view_matches_literal["profile_quota_quarantine"](event, False):
        label = StringSlice("quota quarantine")
    elif rich_view_matches_literal["profile_probe_refresh_start"](event, False):
        label = StringSlice("quota refresh started")
    elif rich_view_matches_literal["profile_probe_refresh_ok"](event, False):
        label = StringSlice("quota refreshed")
    elif rich_view_matches_literal["upstream_usage_limit_passthrough"](event, False):
        label = StringSlice("upstream limit passed through")
    elif rich_view_matches_literal["upstream_overload_passthrough"](event, False):
        label = StringSlice("upstream overload passed through")
    elif rich_view_matches_literal["profile_retry_backoff"](event, False):
        label = StringSlice("retry backoff")
    elif rich_view_matches_literal["compact_retryable_failure"](event, False):
        label = StringSlice("compaction retry")
    elif rich_view_matches_literal["compact_overload_conservative_retry"](event, False):
        label = StringSlice("compaction retry (overload)")
    elif rich_view_matches_literal["profile_transport_backoff"](event, False):
        label = StringSlice("transport backoff")
    elif rich_view_matches_literal["rotation_waiting_for_recovery"](event, False):
        label = StringSlice("waiting for recovery")
    elif rich_view_matches_literal["profile_circuit_open"](event, False):
        label = StringSlice("circuit open")
    elif rich_view_matches_literal["profile_circuit_half_open_probe"](event, False):
        label = StringSlice("circuit probe")
    elif rich_view_matches_literal["profile_transport_failure"](event, False):
        label = StringSlice("transport failed")
    elif rich_view_matches_literal["profile_health"](event, False):
        label = StringSlice("health penalty")
    elif rich_view_matches_literal["profile_bad_pairing"](event, False):
        label = StringSlice("affinity penalty")
    elif rich_view_matches_literal["upstream_start"](event, False) or rich_view_matches_literal["upstream_async_start"](event, False):
        label = StringSlice("upstream request")
    elif rich_view_matches_literal["upstream_response"](event, False) or rich_view_matches_literal["upstream_async_response"](event, False):
        label = StringSlice("upstream response")
    elif rich_view_matches_literal["upstream_connect_start"](event, False):
        label = StringSlice("upstream connecting")
    elif rich_view_matches_literal["upstream_connect_ok"](event, False):
        label = StringSlice("upstream connected")
    elif rich_view_matches_literal["upstream_connect_error"](event, False):
        label = StringSlice("upstream connect failed")
    elif rich_view_matches_literal["first_upstream_chunk"](event, False):
        label = StringSlice("first upstream chunk")
    elif rich_view_matches_literal["first_local_chunk"](event, False):
        label = StringSlice("first local chunk")
    elif rich_view_matches_literal["stream_complete"](event, False):
        label = StringSlice("stream complete")
    elif rich_view_matches_literal["buffered_response_complete"](event, False):
        label = StringSlice("response complete")
    elif rich_view_matches_literal["terminal_event"](event, False):
        label = StringSlice("terminal event")
    elif rich_view_matches_literal["runtime_proxy_queue_overloaded"](event, False):
        label = StringSlice("proxy queue full")
    elif rich_view_matches_literal["runtime_proxy_active_limit_reached"](event, False):
        label = StringSlice("proxy busy")
    elif rich_view_matches_literal["runtime_proxy_lane_limit_reached"](event, False):
        label = StringSlice("lane full")
    elif rich_view_matches_literal["profile_inflight_saturated"](event, False):
        label = StringSlice("profile busy")
    elif rich_view_matches_literal["smart_context_autopilot"](event, False):
        label = StringSlice("Smart Context")
    elif rich_view_matches_literal["smart_context_prepare_error"](event, False):
        label = StringSlice("Smart Context failed")
    elif rich_view_matches_literal["smart_context_prepare_fallback"](event, False):
        label = StringSlice("Smart Context fallback")
    elif rich_view_matches_literal["smart_context_disabled"](event, False):
        label = StringSlice("Smart Context disabled")
    elif rich_view_matches_literal["local_rewrite_request_detail"](event, False):
        label = StringSlice("provider request")
    elif rich_view_matches_literal["local_rewrite_provider_model_fallback"](event, False):
        label = StringSlice("model fallback")
    elif rich_view_matches_literal["local_rewrite_provider_auth_failure"](event, False):
        label = StringSlice("provider auth failed")
    elif rich_view_matches_literal["upstream_read_error"](event, False):
        label = StringSlice("upstream read failed")
    elif rich_view_matches_literal["upstream_send_error"](event, False):
        label = StringSlice("upstream send failed")
    elif rich_view_matches_literal["upstream_stream_error"](event, False):
        label = StringSlice("upstream stream failed")
    elif rich_view_matches_literal["upstream_close_before_completed"](event, False):
        label = StringSlice("upstream closed early")
    elif rich_view_matches_literal["upstream_connection_closed"](event, False):
        label = StringSlice("upstream disconnected")
    elif rich_view_matches_literal["stream_read_error"](event, False):
        label = StringSlice("stream read failed")
    elif rich_view_matches_literal["local_writer_error"](event, False):
        label = StringSlice("terminal write failed")
    elif rich_view_matches_literal["invalid_previous_response_id"](event, False):
        label = StringSlice("continuation invalid")
    elif rich_view_matches_literal["session_error"](event, False):
        label = StringSlice("session failed")
    elif rich_view_matches_literal["local_connection_closed"](event, False):
        label = StringSlice("local connection closed")
    elif rich_view_matches_literal["profile_probe_refresh_error"](event, False):
        label = StringSlice("quota refresh failed")
    elif rich_view_matches_literal["smart_context_token_calibration_save_error"](event, False):
        label = StringSlice("Smart Context calibration failed")
    elif log_view_contains_literal(event, StringSlice("compact")) or log_view_contains_literal(event, StringSlice("compaction")):
        label = StringSlice("compaction")
    elif log_view_contains_literal(event, StringSlice("mcp")) or log_view_starts_literal(event, StringSlice("expose_")):
        label = StringSlice("MCP")
    elif log_view_contains_literal(event, StringSlice("sub_agent")) or log_view_contains_literal(event, StringSlice("subagent")):
        label = StringSlice("sub-agent")

    var required = Int64(label.byte_length())
    if required == 0:
        required = event_length
    if output_capacity < required:
        return PRODEX_LOG_EVENT_NAME_STATUS_CAPACITY
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    if label.byte_length() > 0:
        var source = label.unsafe_ptr()
        for index in range(required):
            output[unsafe_offset=index] = source[unsafe_offset=index]
    else:
        var source = rich_view_ptr(event)
        for index in range(event_length):
            output[unsafe_offset=index] = UInt8(32) if source[unsafe_offset=index] == UInt8(95) else source[unsafe_offset=index]
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = required
    return PRODEX_LOG_EVENT_NAME_STATUS_OK


comptime PRODEX_TRANSCRIPT_ABI_VERSION: Int64 = 1
comptime PRODEX_TRANSCRIPT_TOOL_NAME_CHARS: Int64 = 96
comptime PRODEX_TRANSCRIPT_OPERATION_CHARS: Int64 = 192

comptime TRANSCRIPT_EVENT_UNKNOWN: Int64 = 0
comptime TRANSCRIPT_EVENT_PROTOCOL: Int64 = 1
comptime TRANSCRIPT_EVENT_STATUS_TERMINAL: Int64 = 2
comptime TRANSCRIPT_EVENT_STATUS_ERROR: Int64 = 3
comptime TRANSCRIPT_EVENT_USER: Int64 = 4
comptime TRANSCRIPT_EVENT_ASSISTANT: Int64 = 5
comptime TRANSCRIPT_EVENT_REASONING: Int64 = 6

comptime TRANSCRIPT_PROTOCOL_TOOL: Int64 = 0
comptime TRANSCRIPT_PROTOCOL_MCP: Int64 = 1
comptime TRANSCRIPT_PROTOCOL_AGENT: Int64 = 2

comptime TRANSCRIPT_ITEM_UNKNOWN: Int64 = 0
comptime TRANSCRIPT_ITEM_MESSAGE: Int64 = 1
comptime TRANSCRIPT_ITEM_FUNCTION_CALL: Int64 = 2
comptime TRANSCRIPT_ITEM_FUNCTION_OUTPUT: Int64 = 3
comptime TRANSCRIPT_ITEM_CUSTOM_CALL: Int64 = 4
comptime TRANSCRIPT_ITEM_CUSTOM_OUTPUT: Int64 = 5
comptime TRANSCRIPT_ITEM_SHELL_CALL: Int64 = 6
comptime TRANSCRIPT_ITEM_SHELL_OUTPUT: Int64 = 7
comptime TRANSCRIPT_ITEM_REASONING: Int64 = 8
comptime TRANSCRIPT_ITEM_PROTOCOL: Int64 = 9


def transcript_ascii_contains(
    view: ProdexRichStringView, needle: StringSlice
) -> Bool:
    var needle_length = Int64(needle.byte_length())
    if needle_length == 0 or needle_length > Int64(view.len):
        return False
    var ptr = rich_view_ptr(view)
    var expected = needle.unsafe_ptr()
    for start in range(Int64(view.len) - needle_length + 1):
        var matched = True
        for offset in range(needle_length):
            if ptr[unsafe_offset=start + offset] != expected[unsafe_offset=offset]:
                matched = False
                break
        if matched:
            return True
    return False


def transcript_view_equals[
    literal: StaticString
](view: ProdexRichStringView) -> Bool:
    return rich_view_matches_literal[literal](view, False)


def transcript_protocol_source(view: ProdexRichStringView) -> Int64:
    if transcript_ascii_contains(view, StringSlice("mcp")):
        return TRANSCRIPT_PROTOCOL_MCP
    if (
        transcript_ascii_contains(view, StringSlice("subagent"))
        or transcript_ascii_contains(view, StringSlice("sub_agent"))
    ):
        return TRANSCRIPT_PROTOCOL_AGENT
    return TRANSCRIPT_PROTOCOL_TOOL


def transcript_is_protocol_event(view: ProdexRichStringView) -> Bool:
    return (
        transcript_protocol_source(view) != TRANSCRIPT_PROTOCOL_TOOL
        or transcript_ascii_contains(view, StringSlice("tool_call"))
    )


def transcript_is_status_event(view: ProdexRichStringView) -> Bool:
    if (
        transcript_view_equals["task_started"](view)
        or transcript_view_equals["task_complete"](view)
        or transcript_view_equals["task_completed"](view)
        or transcript_view_equals["turn_started"](view)
        or transcript_view_equals["turn_complete"](view)
        or transcript_view_equals["turn_completed"](view)
        or transcript_view_equals["turn_aborted"](view)
        or transcript_view_equals["turn_cancelled"](view)
        or transcript_view_equals["turn_interrupted"](view)
        or transcript_view_equals["turn_failed"](view)
        or transcript_view_equals["command_execution_started"](view)
        or transcript_view_equals["command_execution_completed"](view)
        or transcript_view_equals["command_execution_finished"](view)
        or transcript_view_equals["command_execution_output"](view)
        or transcript_view_equals["exec_command_begin"](view)
        or transcript_view_equals["exec_command_end"](view)
        or transcript_view_equals["error"](view)
    ):
        return True
    return (
        transcript_ascii_contains(view, StringSlice("command"))
        and transcript_ascii_contains(view, StringSlice("status"))
    )


def transcript_status_is_error(
    event: ProdexRichStringView, status: ProdexRichStringView
) -> Bool:
    return (
        transcript_ascii_contains(event, StringSlice("fail"))
        or transcript_ascii_contains(event, StringSlice("abort"))
        or transcript_view_equals["error"](event)
        or transcript_ascii_contains(status, StringSlice("fail"))
        or transcript_ascii_contains(status, StringSlice("error"))
    )


@export("prodex_mojo_transcript_event_classify_v1")
def prodex_mojo_transcript_event_classify_v1(
    abi_version: Int64,
    event_address: UInt,
    event_length: Int64,
    status_address: UInt,
    status_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PRODEX_TRANSCRIPT_ABI_VERSION
        or event_length < 0
        or status_length < 0
        or output_address == 0
        or (event_length > 0 and event_address == 0)
        or (status_length > 0 and status_address == 0)
    ):
        return 1
    var event = ProdexRichStringView(event_address, UInt(event_length))
    var status = ProdexRichStringView(status_address, UInt(status_length))
    if (
        not rich_view_valid(event, event_length)
        or not rich_view_valid(status, status_length)
    ):
        return 2
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = TRANSCRIPT_EVENT_UNKNOWN
    output[unsafe_offset=1] = TRANSCRIPT_PROTOCOL_TOOL

    if transcript_is_protocol_event(event):
        output[unsafe_offset=0] = TRANSCRIPT_EVENT_PROTOCOL
        output[unsafe_offset=1] = transcript_protocol_source(event)
        return 0
    if transcript_is_status_event(event):
        output[unsafe_offset=0] = (
            TRANSCRIPT_EVENT_STATUS_ERROR
            if transcript_status_is_error(event, status)
            else TRANSCRIPT_EVENT_STATUS_TERMINAL
        )
        return 0
    if transcript_view_equals["user_message"](event):
        output[unsafe_offset=0] = TRANSCRIPT_EVENT_USER
    elif transcript_view_equals["agent_message"](event):
        output[unsafe_offset=0] = TRANSCRIPT_EVENT_ASSISTANT
    elif transcript_view_equals["agent_reasoning"](event):
        output[unsafe_offset=0] = TRANSCRIPT_EVENT_REASONING
    return 0


@export("prodex_mojo_transcript_item_classify_v1")
def prodex_mojo_transcript_item_classify_v1(
    abi_version: Int64,
    item_address: UInt,
    item_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PRODEX_TRANSCRIPT_ABI_VERSION
        or item_length < 0
        or output_address == 0
        or (item_length > 0 and item_address == 0)
    ):
        return 1
    var item = ProdexRichStringView(item_address, UInt(item_length))
    if not rich_view_valid(item, item_length):
        return 2
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = TRANSCRIPT_ITEM_UNKNOWN
    output[unsafe_offset=1] = TRANSCRIPT_PROTOCOL_TOOL
    if transcript_view_equals["message"](item):
        output[unsafe_offset=0] = TRANSCRIPT_ITEM_MESSAGE
    elif transcript_view_equals["function_call"](item):
        output[unsafe_offset=0] = TRANSCRIPT_ITEM_FUNCTION_CALL
    elif transcript_view_equals["function_call_output"](item):
        output[unsafe_offset=0] = TRANSCRIPT_ITEM_FUNCTION_OUTPUT
    elif transcript_view_equals["custom_tool_call"](item):
        output[unsafe_offset=0] = TRANSCRIPT_ITEM_CUSTOM_CALL
    elif transcript_view_equals["custom_tool_call_output"](item):
        output[unsafe_offset=0] = TRANSCRIPT_ITEM_CUSTOM_OUTPUT
    elif (
        transcript_view_equals["local_shell_call"](item)
        or transcript_view_equals["shell_call"](item)
    ):
        output[unsafe_offset=0] = TRANSCRIPT_ITEM_SHELL_CALL
    elif (
        transcript_view_equals["local_shell_call_output"](item)
        or transcript_view_equals["shell_call_output"](item)
    ):
        output[unsafe_offset=0] = TRANSCRIPT_ITEM_SHELL_OUTPUT
    elif transcript_view_equals["reasoning"](item):
        output[unsafe_offset=0] = TRANSCRIPT_ITEM_REASONING
    elif (
        transcript_protocol_source(item) != TRANSCRIPT_PROTOCOL_TOOL
        or transcript_view_equals["computer_call"](item)
        or transcript_view_equals["computer_call_output"](item)
        or transcript_view_equals["web_search_call"](item)
        or transcript_view_equals["file_search_call"](item)
        or transcript_view_equals["code_interpreter_call"](item)
    ):
        output[unsafe_offset=0] = TRANSCRIPT_ITEM_PROTOCOL
        output[unsafe_offset=1] = transcript_protocol_source(item)
    return 0


comptime TRANSCRIPT_OUTPUT_KIND_ASSISTANT: Int64 = 0
comptime TRANSCRIPT_OUTPUT_KIND_USER: Int64 = 1
comptime TRANSCRIPT_OUTPUT_KIND_TOOL: Int64 = 2
comptime TRANSCRIPT_OUTPUT_KIND_OTHER: Int64 = 3
comptime TRANSCRIPT_OUTPUT_STATUS_NONE: Int64 = 0
comptime TRANSCRIPT_OUTPUT_STATUS_STARTED: Int64 = 1
comptime TRANSCRIPT_OUTPUT_STATUS_COMPLETED: Int64 = 2

def transcript_output_starts_with(
    view: ProdexRichStringView, literal: StringSlice
) -> Bool:
    var length = Int64(literal.byte_length())
    if length > Int64(view.len):
        return False
    var source = rich_view_ptr(view)
    var expected = literal.unsafe_ptr()
    for index in range(length):
        if source[unsafe_offset=index] != expected[unsafe_offset=index]:
            return False
    return True

@export("prodex_mojo_transcript_output_event_plan_v1")
def prodex_mojo_transcript_output_event_plan_v1(
    abi_version: Int64,
    source_address: UInt,
    source_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PRODEX_TRANSCRIPT_ABI_VERSION
        or source_length < 0
        or output_address == 0
        or (source_length > 0 and source_address == 0)
    ):
        return 1
    var source = ProdexRichStringView(source_address, UInt(source_length))
    if not rich_view_valid(source, source_length):
        return 2
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = TRANSCRIPT_OUTPUT_KIND_OTHER
    output[unsafe_offset=2] = TRANSCRIPT_OUTPUT_STATUS_NONE
    output[unsafe_offset=3] = -1
    output[unsafe_offset=4] = -1
    if rich_view_matches_literal["assistant"](source, False):
        output[unsafe_offset=0] = 1
        output[unsafe_offset=1] = TRANSCRIPT_OUTPUT_KIND_ASSISTANT
    elif rich_view_matches_literal["user"](source, False):
        output[unsafe_offset=0] = 1
        output[unsafe_offset=1] = TRANSCRIPT_OUTPUT_KIND_USER
    elif transcript_output_starts_with(source, StringSlice("tool-call:")):
        output[unsafe_offset=0] = 1
        output[unsafe_offset=1] = TRANSCRIPT_OUTPUT_KIND_TOOL
        output[unsafe_offset=2] = TRANSCRIPT_OUTPUT_STATUS_STARTED
        output[unsafe_offset=3] = 10
        output[unsafe_offset=4] = source_length
    elif rich_view_matches_literal["tool-output"](source, False):
        output[unsafe_offset=0] = 1
        output[unsafe_offset=1] = TRANSCRIPT_OUTPUT_KIND_TOOL
        output[unsafe_offset=2] = TRANSCRIPT_OUTPUT_STATUS_COMPLETED
    elif (
        rich_view_matches_literal["mcp"](source, False)
        or rich_view_matches_literal["agent"](source, False)
        or rich_view_matches_literal["tool"](source, False)
        or rich_view_matches_literal["terminal"](source, False)
        or rich_view_matches_literal["error"](source, False)
    ):
        output[unsafe_offset=0] = 1
    return 0

comptime TRANSCRIPT_OUTPUT_MODE_TEXT: Int64 = 0
comptime TRANSCRIPT_OUTPUT_MODE_TIMESTAMP: Int64 = 1
comptime TRANSCRIPT_OUTPUT_MODE_NAME: Int64 = 2
comptime TRANSCRIPT_OUTPUT_TEXT_MAX_BYTES: Int64 = 8_192
comptime TRANSCRIPT_OUTPUT_TIMESTAMP_MAX_CHARS: Int64 = 128
comptime TRANSCRIPT_OUTPUT_NAME_MAX_CHARS: Int64 = 256
comptime TRANSCRIPT_OUTPUT_TEXT_MARKER = StringSlice(" …[text_truncated]")

def transcript_output_copy_range(
    source: ProdexRichStringView,
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    written: Pointer[mut=True, Int64, _],
):
    var source_ptr = rich_view_ptr(source)
    for index in range(start, end):
        output[unsafe_offset=written[]] = source_ptr[unsafe_offset=index]
        written[] += 1

def transcript_output_copy_literal(
    literal: StringSlice,
    output: Pointer[mut=True, UInt8, _],
    written: Pointer[mut=True, Int64, _],
):
    var source = literal.unsafe_ptr()
    var length = Int64(literal.byte_length())
    for index in range(length):
        output[unsafe_offset=written[]] = source[unsafe_offset=index]
        written[] += 1

@export("prodex_mojo_transcript_output_text_v1")
def prodex_mojo_transcript_output_text_v1(
    abi_version: Int64,
    mode: Int64,
    value_address: UInt,
    value_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PRODEX_TRANSCRIPT_ABI_VERSION
        or mode < TRANSCRIPT_OUTPUT_MODE_TEXT
        or mode > TRANSCRIPT_OUTPUT_MODE_NAME
        or value_length < 0
        or output_capacity < 0
        or written_address == 0
        or (value_length > 0 and value_address == 0)
        or (output_capacity > 0 and output_address == 0)
    ):
        return 1
    var value = ProdexRichStringView(value_address, UInt(value_length))
    if not rich_view_valid(value, value_length):
        return 2
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if mode == TRANSCRIPT_OUTPUT_MODE_TEXT:
        var marker_length = Int64(TRANSCRIPT_OUTPUT_TEXT_MARKER.byte_length())
        var required = value_length
        if required > TRANSCRIPT_OUTPUT_TEXT_MAX_BYTES:
            required = TRANSCRIPT_OUTPUT_TEXT_MAX_BYTES
        if output_capacity < required:
            return 3
        if value_length <= TRANSCRIPT_OUTPUT_TEXT_MAX_BYTES:
            transcript_output_copy_range(value, 0, value_length, output, written)
            return 0
        var budget = TRANSCRIPT_OUTPUT_TEXT_MAX_BYTES - marker_length
        var cursor: Int64 = 0
        var source = rich_view_ptr(value)
        while cursor < value_length:
            var width = rich_codepoint_width(source[unsafe_offset=cursor])
            if cursor + width > budget:
                break
            cursor += width
        transcript_output_copy_range(value, 0, cursor, output, written)
        transcript_output_copy_literal(TRANSCRIPT_OUTPUT_TEXT_MARKER, output, written)
        return 0
    var limit = (
        TRANSCRIPT_OUTPUT_TIMESTAMP_MAX_CHARS
        if mode == TRANSCRIPT_OUTPUT_MODE_TIMESTAMP
        else TRANSCRIPT_OUTPUT_NAME_MAX_CHARS
    )
    if output_capacity < value_length:
        return 3
    var cursor: Int64 = 0
    var count: Int64 = 0
    var source = rich_view_ptr(value)
    while cursor < value_length and count < limit:
        var width = rich_codepoint_width(source[unsafe_offset=cursor])
        cursor += width
        count += 1
    transcript_output_copy_range(value, 0, cursor, output, written)
    return 0


def transcript_codepoint_is_control(codepoint: Int64) -> Bool:
    return codepoint <= 31 or (codepoint >= 127 and codepoint <= 159)


@export("prodex_mojo_transcript_operation_span_v1")
def prodex_mojo_transcript_operation_span_v1(
    abi_version: Int64,
    value_address: UInt,
    value_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PRODEX_TRANSCRIPT_ABI_VERSION
        or value_length < 0
        or output_address == 0
        or (value_length > 0 and value_address == 0)
    ):
        return 1
    var view = ProdexRichStringView(value_address, UInt(value_length))
    if not rich_view_valid(view, value_length):
        return 2
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = 0

    var bounds = rich_trim_bounds(view)
    if bounds[0] >= bounds[1]:
        return 0

    var ptr = rich_view_ptr(view)
    var cursor: Int64 = 0
    var count: Int64 = 0
    var end: Int64 = 0
    while cursor < value_length:
        var width = rich_codepoint_width(ptr[unsafe_offset=cursor])
        var codepoint = rich_codepoint(ptr, cursor, width)
        if transcript_codepoint_is_control(codepoint):
            return 0
        if count < PRODEX_TRANSCRIPT_OPERATION_CHARS:
            end = cursor + width
        count += 1
        cursor += width
    output[unsafe_offset=0] = end
    output[unsafe_offset=1] = Int64(count > PRODEX_TRANSCRIPT_OPERATION_CHARS)
    return 0


@export("prodex_mojo_transcript_tool_name_v1")
def prodex_mojo_transcript_tool_name_v1(
    abi_version: Int64,
    value_address: UInt,
    value_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PRODEX_TRANSCRIPT_ABI_VERSION
        or value_length < 0
        or output_address == 0
        or output_capacity < PRODEX_TRANSCRIPT_TOOL_NAME_CHARS
        or written_address == 0
        or (value_length > 0 and value_address == 0)
    ):
        return 1
    var view = ProdexRichStringView(value_address, UInt(value_length))
    if not rich_view_valid(view, value_length):
        return 2
    var ptr = rich_view_ptr(view)
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    var cursor: Int64 = 0
    var count: Int64 = 0
    while cursor < value_length and count < PRODEX_TRANSCRIPT_TOOL_NAME_CHARS:
        var width = rich_codepoint_width(ptr[unsafe_offset=cursor])
        var codepoint = rich_codepoint(ptr, cursor, width)
        var emitted: UInt8 = 95
        if codepoint < 128:
            var byte = ptr[unsafe_offset=cursor]
            if (
                (byte >= 48 and byte <= 57)
                or (byte >= 65 and byte <= 90)
                or (byte >= 97 and byte <= 122)
                or byte == 95
                or byte == 45
                or byte == 46
                or byte == 58
            ):
                emitted = byte
        output[unsafe_offset=written[]] = emitted
        written[] += 1
        count += 1
        cursor += width
    if written[] == 0:
        var fallback = StringSlice("tool").unsafe_ptr()
        for index in range(4):
            output[unsafe_offset=index] = fallback[unsafe_offset=index]
        written[] = 4
    return 0


comptime PRODEX_UPSTREAM_PAYLOAD_ABI_VERSION: Int64 = 1
comptime UPSTREAM_BINARY_UNKNOWN: Int64 = 0
comptime UPSTREAM_BINARY_PNG: Int64 = 1
comptime UPSTREAM_BINARY_JPEG: Int64 = 2
comptime UPSTREAM_BINARY_GIF: Int64 = 3
comptime UPSTREAM_BINARY_PDF: Int64 = 4
comptime UPSTREAM_BINARY_ZIP: Int64 = 5
comptime UPSTREAM_BINARY_GZIP: Int64 = 6
comptime UPSTREAM_BINARY_ZSTD: Int64 = 7
comptime UPSTREAM_BINARY_WEBP: Int64 = 8


def upstream_payload_prefix(
    ptr: Pointer[mut=False, UInt8, _], length: Int64, literal: StringSlice
) -> Bool:
    var expected_length = Int64(literal.byte_length())
    if expected_length > length:
        return False
    var expected = literal.unsafe_ptr()
    for index in range(expected_length):
        if ptr[unsafe_offset=index] != expected[unsafe_offset=index]:
            return False
    return True


def upstream_payload_magic_kind(
    ptr: Pointer[mut=False, UInt8, _], length: Int64
) -> Int64:
    if (
        length >= 8
        and ptr[unsafe_offset=0] == 0x89
        and ptr[unsafe_offset=1] == 0x50
        and ptr[unsafe_offset=2] == 0x4E
        and ptr[unsafe_offset=3] == 0x47
        and ptr[unsafe_offset=4] == 0x0D
        and ptr[unsafe_offset=5] == 0x0A
        and ptr[unsafe_offset=6] == 0x1A
        and ptr[unsafe_offset=7] == 0x0A
    ):
        return UPSTREAM_BINARY_PNG
    if length >= 3 and ptr[unsafe_offset=0] == 0xFF and ptr[unsafe_offset=1] == 0xD8 and ptr[unsafe_offset=2] == 0xFF:
        return UPSTREAM_BINARY_JPEG
    if (
        upstream_payload_prefix(ptr, length, StringSlice("GIF87a"))
        or upstream_payload_prefix(ptr, length, StringSlice("GIF89a"))
    ):
        return UPSTREAM_BINARY_GIF
    if upstream_payload_prefix(ptr, length, StringSlice("%PDF-")):
        return UPSTREAM_BINARY_PDF
    if length >= 4 and ptr[unsafe_offset=0] == 0x50 and ptr[unsafe_offset=1] == 0x4B and ptr[unsafe_offset=2] == 0x03 and ptr[unsafe_offset=3] == 0x04:
        return UPSTREAM_BINARY_ZIP
    if length >= 2 and ptr[unsafe_offset=0] == 0x1F and ptr[unsafe_offset=1] == 0x8B:
        return UPSTREAM_BINARY_GZIP
    if (
        length >= 4
        and ptr[unsafe_offset=0] == 0x28
        and ptr[unsafe_offset=1] == 0xB5
        and ptr[unsafe_offset=2] == 0x2F
        and ptr[unsafe_offset=3] == 0xFD
    ):
        return UPSTREAM_BINARY_ZSTD
    if (
        length >= 12
        and upstream_payload_prefix(ptr, length, StringSlice("RIFF"))
        and ptr[unsafe_offset=8] == 0x57
        and ptr[unsafe_offset=9] == 0x45
        and ptr[unsafe_offset=10] == 0x42
        and ptr[unsafe_offset=11] == 0x50
    ):
        return UPSTREAM_BINARY_WEBP
    return UPSTREAM_BINARY_UNKNOWN


@export("prodex_mojo_upstream_payload_classify_v1")
def prodex_mojo_upstream_payload_classify_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PRODEX_UPSTREAM_PAYLOAD_ABI_VERSION
        or input_length < 0
        or output_address == 0
        or (input_length > 0 and input_address == 0)
    ):
        return 1

    var ptr = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = upstream_payload_magic_kind(ptr, input_length)

    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, input_length):
        return 0

    var cursor: Int64 = 0
    while cursor < input_length:
        var width = rich_codepoint_width(ptr[unsafe_offset=cursor])
        var codepoint = rich_codepoint(ptr, cursor, width)
        if (
            transcript_codepoint_is_control(codepoint)
            and codepoint != 9
            and codepoint != 10
            and codepoint != 13
        ):
            return 0
        cursor += width
    output[unsafe_offset=0] = 1
    return 0

comptime PREVIOUS_RESPONSE_LOG_ABI_VERSION: Int64 = 1
comptime PREVIOUS_RESPONSE_LOG_NOT_FOUND: Int64 = 0
comptime PREVIOUS_RESPONSE_LOG_RETRY_IMMEDIATE: Int64 = 1
comptime PREVIOUS_RESPONSE_LOG_STALE_CONTINUATION: Int64 = 2
comptime PREVIOUS_RESPONSE_LOG_FRESH_FALLBACK: Int64 = 3
comptime PREVIOUS_RESPONSE_LOG_AFFINITY_RELEASED: Int64 = 4

@fieldwise_init
struct PreviousResponseLogWriter(Copyable):
    var output: Pointer[mut=True, UInt8, MutUntrackedOrigin]
    var capacity: Int64
    var written: Int64


def previous_response_log_put_byte(
    writer: Pointer[mut=True, PreviousResponseLogWriter, _], byte: UInt8
) -> Bool:
    if writer[].written < 0 or writer[].written >= writer[].capacity:
        return False
    writer[].output[unsafe_offset=writer[].written] = byte
    writer[].written += 1
    return True


def previous_response_log_put_literal(
    writer: Pointer[mut=True, PreviousResponseLogWriter, _], value: StringSlice
) -> Bool:
    var source = value.unsafe_ptr()
    for index in range(Int64(value.byte_length())):
        if not previous_response_log_put_byte(
            writer, source[unsafe_offset=index]
        ):
            return False
    return True


def previous_response_log_put_view(
    writer: Pointer[mut=True, PreviousResponseLogWriter, _],
    value: ProdexRichStringView,
) -> Bool:
    var source = rich_view_ptr(value)
    for index in range(Int64(value.len)):
        if not previous_response_log_put_byte(
            writer, source[unsafe_offset=index]
        ):
            return False
    return True


def previous_response_log_put_u64(
    writer: Pointer[mut=True, PreviousResponseLogWriter, _], value: UInt64
) -> Bool:
    if value == 0:
        return previous_response_log_put_byte(writer, UInt8(48))
    var divisor: UInt64 = 1
    while value / divisor >= UInt64(10):
        divisor *= UInt64(10)
    var remaining = value
    while divisor > 0:
        if not previous_response_log_put_byte(
            writer, UInt8(remaining / divisor) + UInt8(48)
        ):
            return False
        remaining %= divisor
        divisor //= UInt64(10)
    return True


def previous_response_log_text(
    address: UInt, index: Int64
) -> ProdexRichStringView:
    var values = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(address))
    return values[unsafe_offset=index].copy()


def previous_response_log_validate_texts(address: UInt, count: Int64) -> Bool:
    if count != 6 or address == 0:
        return False
    var values = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(address))
    for index in range(count):
        var value = values[unsafe_offset=index].copy()
        if not rich_view_valid(value, Int64(value.len)):
            return False
    return True


def previous_response_log_put_suffix(
    writer: Pointer[mut=True, PreviousResponseLogWriter, _],
    text_address: UInt,
    presence: UInt64,
) -> Bool:
    if presence & UInt64(2) == 0:
        return True
    return (
        previous_response_log_put_literal(writer, StringSlice(" via="))
        and previous_response_log_put_view(
            writer, previous_response_log_text(text_address, 2)
        )
    )


def previous_response_log_put_not_found_prefix(
    writer: Pointer[mut=True, PreviousResponseLogWriter, _],
    request_id: UInt64,
    websocket_session: UInt64,
    text_address: UInt,
    presence: UInt64,
) -> Bool:
    if not (
        previous_response_log_put_literal(writer, StringSlice("request="))
        and previous_response_log_put_u64(writer, request_id)
        and previous_response_log_put_literal(writer, StringSlice(" transport="))
        and previous_response_log_put_view(
            writer, previous_response_log_text(text_address, 0)
        )
        and previous_response_log_put_literal(writer, StringSlice(" route="))
        and previous_response_log_put_view(
            writer, previous_response_log_text(text_address, 1)
        )
    ):
        return False
    if presence & UInt64(1) != 0:
        return (
            previous_response_log_put_literal(
                writer, StringSlice(" websocket_session=")
            )
            and previous_response_log_put_u64(writer, websocket_session)
        )
    return True


def previous_response_log_put_event_prefix(
    writer: Pointer[mut=True, PreviousResponseLogWriter, _],
    request_id: UInt64,
    websocket_session: UInt64,
    text_address: UInt,
    presence: UInt64,
) -> Bool:
    if not (
        previous_response_log_put_literal(writer, StringSlice("request="))
        and previous_response_log_put_u64(writer, request_id)
    ):
        return False
    if presence & UInt64(1) != 0:
        return (
            previous_response_log_put_literal(
                writer, StringSlice(" websocket_session=")
            )
            and previous_response_log_put_u64(writer, websocket_session)
        )
    return (
        previous_response_log_put_literal(writer, StringSlice(" transport="))
        and previous_response_log_put_view(
            writer, previous_response_log_text(text_address, 0)
        )
    )


def previous_response_log_render(
    writer: Pointer[mut=True, PreviousResponseLogWriter, _],
    operation: Int64,
    request_id: UInt64,
    websocket_session: UInt64,
    retry_index: UInt64,
    text_address: UInt,
    presence: UInt64,
) -> Bool:
    var profile = previous_response_log_text(text_address, 3)
    if operation == PREVIOUS_RESPONSE_LOG_NOT_FOUND:
        return (
            previous_response_log_put_not_found_prefix(
                writer,
                request_id,
                websocket_session,
                text_address,
                presence,
            )
            and previous_response_log_put_literal(
                writer, StringSlice(" previous_response_not_found profile=")
            )
            and previous_response_log_put_view(writer, profile)
            and previous_response_log_put_literal(
                writer, StringSlice(" retry_index=")
            )
            and previous_response_log_put_u64(writer, retry_index)
            and previous_response_log_put_literal(
                writer, StringSlice(" replay_turn_state=")
            )
            and previous_response_log_put_view(
                writer, previous_response_log_text(text_address, 4)
            )
            and previous_response_log_put_suffix(
                writer, text_address, presence
            )
        )
    if operation == PREVIOUS_RESPONSE_LOG_RETRY_IMMEDIATE:
        return (
            previous_response_log_put_event_prefix(
                writer,
                request_id,
                websocket_session,
                text_address,
                presence,
            )
            and previous_response_log_put_literal(
                writer,
                StringSlice(" previous_response_retry_immediate profile="),
            )
            and previous_response_log_put_view(writer, profile)
            and previous_response_log_put_literal(
                writer, StringSlice(" delay_ms=")
            )
            and previous_response_log_put_view(
                writer, previous_response_log_text(text_address, 4)
            )
            and previous_response_log_put_literal(
                writer, StringSlice(" reason=")
            )
            and previous_response_log_put_view(
                writer, previous_response_log_text(text_address, 5)
            )
            and previous_response_log_put_suffix(
                writer, text_address, presence
            )
        )
    if operation == PREVIOUS_RESPONSE_LOG_STALE_CONTINUATION:
        return (
            previous_response_log_put_event_prefix(
                writer,
                request_id,
                websocket_session,
                text_address,
                presence,
            )
            and previous_response_log_put_literal(
                writer,
                StringSlice(
                    " stale_continuation reason=previous_response_not_found_locked_affinity profile="
                ),
            )
            and previous_response_log_put_view(writer, profile)
            and previous_response_log_put_suffix(
                writer, text_address, presence
            )
        )
    if operation == PREVIOUS_RESPONSE_LOG_FRESH_FALLBACK:
        if not previous_response_log_put_event_prefix(
            writer,
            request_id,
            websocket_session,
            text_address,
            presence,
        ):
            return False
        if presence & UInt64(4) != 0:
            if not previous_response_log_put_literal(
                writer,
                StringSlice(" previous_response_fresh_fallback_blocked"),
            ):
                return False
        elif not previous_response_log_put_literal(
            writer, StringSlice(" previous_response_fresh_fallback")
        ):
            return False
        return (
            previous_response_log_put_literal(
                writer,
                StringSlice(
                    " reason=previous_response_not_found request_shape="
                ),
            )
            and previous_response_log_put_view(
                writer, previous_response_log_text(text_address, 4)
            )
            and previous_response_log_put_literal(
                writer, StringSlice(" outcome=")
            )
            and previous_response_log_put_view(
                writer, previous_response_log_text(text_address, 5)
            )
            and previous_response_log_put_literal(
                writer, StringSlice(" profile=")
            )
            and previous_response_log_put_view(writer, profile)
            and previous_response_log_put_suffix(
                writer, text_address, presence
            )
        )
    return (
        previous_response_log_put_event_prefix(
            writer,
            request_id,
            websocket_session,
            text_address,
            presence,
        )
        and previous_response_log_put_literal(
            writer, StringSlice(" previous_response_affinity_released profile=")
        )
        and previous_response_log_put_view(writer, profile)
        and previous_response_log_put_suffix(writer, text_address, presence)
    )


@export("prodex_mojo_previous_response_log_render_v1")
def prodex_mojo_previous_response_log_render_v1(
    abi_version: Int64,
    operation: Int64,
    request_id: UInt64,
    websocket_session: UInt64,
    retry_index: UInt64,
    text_address: UInt,
    text_count: Int64,
    presence: UInt64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PREVIOUS_RESPONSE_LOG_ABI_VERSION:
        return 4
    if (
        operation < PREVIOUS_RESPONSE_LOG_NOT_FOUND
        or operation > PREVIOUS_RESPONSE_LOG_AFFINITY_RELEASED
        or presence > UInt64(7)
        or output_address == 0
        or output_capacity < 0
        or written_address == 0
        or not previous_response_log_validate_texts(text_address, text_count)
    ):
        return 1
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var writer = PreviousResponseLogWriter(output, output_capacity, 0)
    var ok = previous_response_log_render(
        Pointer(to=writer),
        operation,
        request_id,
        websocket_session,
        retry_index,
        text_address,
        presence,
    )
    written[] = writer.written
    if not ok:
        return 2
    return 0


comptime ROUTE_AFFINITY_LOG_ABI_VERSION: Int64 = 1
comptime ROUTE_AFFINITY_LOG_PREFIX: Int64 = 0
comptime ROUTE_AFFINITY_LOG_RECOMPUTE: Int64 = 1
comptime ROUTE_AFFINITY_LOG_RESULT: Int64 = 2
comptime ROUTE_AFFINITY_LOG_FOLLOWUP_OWNER: Int64 = 3
comptime ROUTE_AFFINITY_LOG_SESSION_OWNER: Int64 = 4

comptime CHAIN_LOG_ABI_VERSION: Int64 = 1
comptime CHAIN_LOG_RETRIED_OWNER: Int64 = 0
comptime CHAIN_LOG_DEAD_UPSTREAM: Int64 = 1


def runtime_log_validate_texts(
    address: UInt, count: Int64, expected: Int64
) -> Bool:
    if count != expected or address == 0:
        return False
    var values = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(address))
    for index in range(count):
        var value = values[unsafe_offset=index].copy()
        if not rich_view_valid(value, Int64(value.len)):
            return False
    return True


def runtime_log_put_bool(
    writer: Pointer[mut=True, PreviousResponseLogWriter, _], value: Bool
) -> Bool:
    return previous_response_log_put_literal(
        writer, StringSlice("true") if value else StringSlice("false")
    )


def route_affinity_log_put_prefix(
    writer: Pointer[mut=True, PreviousResponseLogWriter, _],
    request_id: UInt64,
    websocket_session: UInt64,
    presence: UInt64,
) -> Bool:
    if not (
        previous_response_log_put_literal(writer, StringSlice("request="))
        and previous_response_log_put_u64(writer, request_id)
    ):
        return False
    if presence & UInt64(1) != 0:
        return (
            previous_response_log_put_literal(
                writer, StringSlice(" websocket_session=")
            )
            and previous_response_log_put_u64(writer, websocket_session)
        )
    return previous_response_log_put_literal(
        writer, StringSlice(" transport=http")
    )


def route_affinity_log_render(
    writer: Pointer[mut=True, PreviousResponseLogWriter, _],
    operation: Int64,
    request_id: UInt64,
    websocket_session: UInt64,
    text_address: UInt,
    presence: UInt64,
) -> Bool:
    if not route_affinity_log_put_prefix(
        writer, request_id, websocket_session, presence
    ):
        return False
    if operation == ROUTE_AFFINITY_LOG_PREFIX:
        return True

    if operation == ROUTE_AFFINITY_LOG_RECOMPUTE:
        return (
            previous_response_log_put_literal(
                writer, StringSlice(" route_affinity_recompute reason=")
            )
            and previous_response_log_put_view(
                writer, previous_response_log_text(text_address, 0)
            )
            and previous_response_log_put_literal(
                writer, StringSlice(" previous_response_id_present=")
            )
            and runtime_log_put_bool(writer, presence & UInt64(2) != 0)
            and previous_response_log_put_literal(
                writer, StringSlice(" request_turn_state_present=")
            )
            and runtime_log_put_bool(writer, presence & UInt64(4) != 0)
            and previous_response_log_put_literal(
                writer, StringSlice(" request_session_id_present=")
            )
            and runtime_log_put_bool(writer, presence & UInt64(8) != 0)
            and previous_response_log_put_literal(
                writer, StringSlice(" explicit_session_id_present=")
            )
            and runtime_log_put_bool(writer, presence & UInt64(16) != 0)
        )

    if operation == ROUTE_AFFINITY_LOG_RESULT:
        if not (
            previous_response_log_put_literal(
                writer, StringSlice(" route_affinity_recompute_result reason=")
            )
            and previous_response_log_put_view(
                writer, previous_response_log_text(text_address, 0)
            )
            and previous_response_log_put_literal(
                writer, StringSlice(" previous_response_id_present=")
            )
            and runtime_log_put_bool(writer, presence & UInt64(2) != 0)
            and previous_response_log_put_literal(
                writer, StringSlice(" request_turn_state_present=")
            )
            and runtime_log_put_bool(writer, presence & UInt64(4) != 0)
            and previous_response_log_put_literal(
                writer, StringSlice(" request_session_id_present=")
            )
            and runtime_log_put_bool(writer, presence & UInt64(8) != 0)
            and previous_response_log_put_literal(
                writer, StringSlice(" explicit_session_id_present=")
            )
            and runtime_log_put_bool(writer, presence & UInt64(16) != 0)
        ):
            return False
        for index in range(Int64(5)):
            var label = StringSlice(" bound_session_profile=")
            if index == 1:
                label = StringSlice(" compact_followup_profile=")
            elif index == 2:
                label = StringSlice(" compact_session_profile=")
            elif index == 3:
                label = StringSlice(" session_profile=")
            elif index == 4:
                label = StringSlice(" pinned_profile=")
            if not (
                previous_response_log_put_literal(writer, label)
                and previous_response_log_put_view(
                    writer,
                    previous_response_log_text(text_address, index + 1),
                )
            ):
                return False
        return True

    if operation == ROUTE_AFFINITY_LOG_FOLLOWUP_OWNER:
        if presence & UInt64(32) == 0:
            writer[].written = 0
            return True
        return (
            previous_response_log_put_literal(
                writer, StringSlice(" compact_followup_owner profile=")
            )
            and previous_response_log_put_view(
                writer, previous_response_log_text(text_address, 6)
            )
            and previous_response_log_put_literal(
                writer, StringSlice(" source=")
            )
            and previous_response_log_put_view(
                writer, previous_response_log_text(text_address, 7)
            )
        )

    if presence & UInt64(64) == 0:
        writer[].written = 0
        return True
    return (
        previous_response_log_put_literal(
            writer, StringSlice(" compact_followup_owner profile=")
        )
        and previous_response_log_put_view(
            writer, previous_response_log_text(text_address, 8)
        )
        and previous_response_log_put_literal(
            writer, StringSlice(" source=session_id")
        )
    )


@export("prodex_mojo_route_affinity_log_render_v1")
def prodex_mojo_route_affinity_log_render_v1(
    abi_version: Int64,
    operation: Int64,
    request_id: UInt64,
    websocket_session: UInt64,
    text_address: UInt,
    text_count: Int64,
    presence: UInt64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != ROUTE_AFFINITY_LOG_ABI_VERSION:
        return 4
    if (
        operation < ROUTE_AFFINITY_LOG_PREFIX
        or operation > ROUTE_AFFINITY_LOG_SESSION_OWNER
        or presence > UInt64(127)
        or output_address == 0
        or output_capacity < 0
        or written_address == 0
        or not runtime_log_validate_texts(text_address, text_count, 9)
    ):
        return 1
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var writer = PreviousResponseLogWriter(output, output_capacity, 0)
    var ok = route_affinity_log_render(
        Pointer(to=writer),
        operation,
        request_id,
        websocket_session,
        text_address,
        presence,
    )
    written[] = writer.written
    return 0 if ok else 2


@export("prodex_mojo_route_affinity_owner_logs_v1")
def prodex_mojo_route_affinity_owner_logs_v1(
    abi_version: Int64,
    request_id: UInt64,
    websocket_session: UInt64,
    text_address: UInt,
    text_count: Int64,
    presence: UInt64,
    followup_output_address: UInt,
    output_capacity: Int64,
    followup_written_address: UInt,
    session_output_address: UInt,
    session_written_address: UInt,
) abi("C") -> Int64:
    if abi_version != ROUTE_AFFINITY_LOG_ABI_VERSION:
        return 4
    if (
        presence > UInt64(127)
        or followup_output_address == 0
        or session_output_address == 0
        or output_capacity < 0
        or followup_written_address == 0
        or session_written_address == 0
        or not runtime_log_validate_texts(text_address, text_count, 9)
    ):
        return 1
    var followup_output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(followup_output_address)
    )
    var followup_written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(followup_written_address)
    )
    var followup_writer = PreviousResponseLogWriter(
        followup_output, output_capacity, 0
    )
    var followup_ok = route_affinity_log_render(
        Pointer(to=followup_writer),
        ROUTE_AFFINITY_LOG_FOLLOWUP_OWNER,
        request_id,
        websocket_session,
        text_address,
        presence,
    )
    followup_written[] = followup_writer.written

    var session_output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(session_output_address)
    )
    var session_written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(session_written_address)
    )
    var session_writer = PreviousResponseLogWriter(
        session_output, output_capacity, 0
    )
    var session_ok = route_affinity_log_render(
        Pointer(to=session_writer),
        ROUTE_AFFINITY_LOG_SESSION_OWNER,
        request_id,
        websocket_session,
        text_address,
        presence,
    )
    session_written[] = session_writer.written
    return 0 if followup_ok and session_ok else 2


def chain_log_put_optional(
    writer: Pointer[mut=True, PreviousResponseLogWriter, _],
    text_address: UInt,
    index: Int64,
    present: Bool,
) -> Bool:
    if not present:
        return previous_response_log_put_byte(writer, UInt8(45))
    return previous_response_log_put_view(
        writer, previous_response_log_text(text_address, index)
    )


def chain_log_render(
    writer: Pointer[mut=True, PreviousResponseLogWriter, _],
    operation: Int64,
    request_id: UInt64,
    websocket_session: UInt64,
    text_address: UInt,
    presence: UInt64,
) -> Bool:
    if not (
        previous_response_log_put_literal(writer, StringSlice("request="))
        and previous_response_log_put_u64(writer, request_id)
        and previous_response_log_put_literal(writer, StringSlice(" transport="))
        and previous_response_log_put_view(
            writer, previous_response_log_text(text_address, 0)
        )
        and previous_response_log_put_literal(writer, StringSlice(" route="))
        and previous_response_log_put_view(
            writer, previous_response_log_text(text_address, 1)
        )
        and previous_response_log_put_literal(
            writer, StringSlice(" websocket_session=")
        )
    ):
        return False
    if presence & UInt64(1) != 0:
        if not previous_response_log_put_u64(writer, websocket_session):
            return False
    elif not previous_response_log_put_byte(writer, UInt8(45)):
        return False

    if operation == CHAIN_LOG_RETRIED_OWNER:
        return (
            previous_response_log_put_literal(
                writer, StringSlice(" chain_retried_owner profile=")
            )
            and previous_response_log_put_view(
                writer, previous_response_log_text(text_address, 2)
            )
            and previous_response_log_put_literal(
                writer, StringSlice(" previous_response_id=")
            )
            and chain_log_put_optional(
                writer, text_address, 3, presence & UInt64(2) != 0
            )
            and previous_response_log_put_literal(
                writer, StringSlice(" delay_ms=")
            )
            and previous_response_log_put_view(
                writer, previous_response_log_text(text_address, 6)
            )
            and previous_response_log_put_literal(
                writer, StringSlice(" reason=")
            )
            and previous_response_log_put_view(
                writer, previous_response_log_text(text_address, 4)
            )
            and previous_response_log_put_literal(
                writer, StringSlice(" via=")
            )
            and chain_log_put_optional(
                writer, text_address, 5, presence & UInt64(4) != 0
            )
        )
    return (
        previous_response_log_put_literal(
            writer, StringSlice(" chain_dead_upstream_confirmed profile=")
        )
        and previous_response_log_put_view(
            writer, previous_response_log_text(text_address, 2)
        )
        and previous_response_log_put_literal(
            writer, StringSlice(" previous_response_id=")
        )
        and chain_log_put_optional(
            writer, text_address, 3, presence & UInt64(2) != 0
        )
        and previous_response_log_put_literal(
            writer, StringSlice(" reason=")
        )
        and previous_response_log_put_view(
            writer, previous_response_log_text(text_address, 4)
        )
        and previous_response_log_put_literal(
            writer, StringSlice(" via=")
        )
        and chain_log_put_optional(
            writer, text_address, 5, presence & UInt64(4) != 0
        )
        and previous_response_log_put_literal(
            writer, StringSlice(" event=")
        )
        and chain_log_put_optional(
            writer, text_address, 6, presence & UInt64(8) != 0
        )
    )


@export("prodex_mojo_chain_log_render_v1")
def prodex_mojo_chain_log_render_v1(
    abi_version: Int64,
    operation: Int64,
    request_id: UInt64,
    websocket_session: UInt64,
    text_address: UInt,
    text_count: Int64,
    presence: UInt64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != CHAIN_LOG_ABI_VERSION:
        return 4
    if (
        operation < CHAIN_LOG_RETRIED_OWNER
        or operation > CHAIN_LOG_DEAD_UPSTREAM
        or presence > UInt64(15)
        or output_address == 0
        or output_capacity < 0
        or written_address == 0
        or not runtime_log_validate_texts(text_address, text_count, 7)
    ):
        return 1
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var writer = PreviousResponseLogWriter(output, output_capacity, 0)
    var ok = chain_log_render(
        Pointer(to=writer),
        operation,
        request_id,
        websocket_session,
        text_address,
        presence,
    )
    written[] = writer.written
    return 0 if ok else 2


comptime PRODEX_STRUCTURED_LOG_ABI_VERSION: Int64 = 1
comptime PRODEX_STRUCTURED_LOG_STATUS_OK: Int64 = 0
comptime PRODEX_STRUCTURED_LOG_STATUS_INVALID: Int64 = 1
comptime PRODEX_STRUCTURED_LOG_STATUS_CAPACITY: Int64 = 2
comptime PRODEX_STRUCTURED_LOG_STATUS_ABI: Int64 = 4


@fieldwise_init
struct StructuredLogWriter(Copyable):
    var output: Pointer[mut=True, UInt8, MutUntrackedOrigin]
    var capacity: Int64
    var written: Int64


def structured_log_put_byte(
    writer: Pointer[mut=True, StructuredLogWriter, _], value: UInt8
) -> Bool:
    if writer[].written < 0 or writer[].written >= writer[].capacity:
        return False
    writer[].output[unsafe_offset=writer[].written] = value
    writer[].written += 1
    return True


def structured_log_put_range(
    writer: Pointer[mut=True, StructuredLogWriter, _],
    input: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
) -> Bool:
    for index in range(start, end):
        if not structured_log_put_byte(writer, input[unsafe_offset=index]):
            return False
    return True


def structured_log_put_literal(
    writer: Pointer[mut=True, StructuredLogWriter, _], value: StringSlice
) -> Bool:
    var input = value.unsafe_ptr()
    return structured_log_put_range(
        writer, input, 0, Int64(value.byte_length())
    )


def structured_log_key_skip(view: ProdexRichStringView) -> Bool:
    var input = rich_view_ptr(view)
    for index in range(Int64(view.len)):
        var value = input[unsafe_offset=index]
        if (
            value == UInt8(61)
            or value == UInt8(32)
            or (value >= UInt8(9) and value <= UInt8(13))
        ):
            return True
    return False


def structured_log_key_known_safe(view: ProdexRichStringView) -> Bool:
    return (
        rich_view_matches_literal["affinity"](view, False)
        or rich_view_matches_literal["acceptance_state"](view, False)
        or rich_view_matches_literal["balance"](view, False)
        or rich_view_matches_literal["cold_start_jobs"](view, False)
        or rich_view_matches_literal["excluded_count"](view, False)
        or rich_view_matches_literal["effective_model"](view, False)
        or rich_view_matches_literal["eligible_profiles_remaining"](view, False)
        or rich_view_matches_literal["fallback"](view, False)
        or rich_view_matches_literal["failure_class"](view, False)
        or rich_view_matches_literal["health"](view, False)
        or rich_view_matches_literal["inflight"](view, False)
        or rich_view_matches_literal["mode"](view, False)
        or rich_view_matches_literal["order"](view, False)
        or rich_view_matches_literal["outcome"](view, False)
        or rich_view_matches_literal["performance"](view, False)
        or rich_view_matches_literal["pressure_mode"](view, False)
        or rich_view_matches_literal["profile"](view, False)
        or rich_view_matches_literal["profile_hash"](view, False)
        or rich_view_matches_literal["prompt_cache_bound"](view, False)
        or rich_view_matches_literal["ready"](view, False)
        or rich_view_matches_literal["reason"](view, False)
        or rich_view_matches_literal["reports"](view, False)
        or rich_view_matches_literal["recovery_generation"](view, False)
        or rich_view_matches_literal["recovery_outcome"](view, False)
        or rich_view_matches_literal["requeue_reason"](view, False)
        or rich_view_matches_literal["request"](view, False)
        or rich_view_matches_literal["response_id"](view, False)
        or rich_view_matches_literal["route"](view, False)
        or rich_view_matches_literal["retry_layer"](view, False)
        or rich_view_matches_literal["schema_version"](view, False)
        or rich_view_matches_literal["side_effect_state"](view, False)
        or rich_view_matches_literal["signaled"](view, False)
        or rich_view_matches_literal["soft_limit"](view, False)
        or rich_view_matches_literal["sync_probe_jobs"](view, False)
        or rich_view_matches_literal["payload_b64"](view, False)
        or rich_view_matches_literal["stream"](view, False)
        or rich_view_matches_literal["stream_committed"](view, False)
        or rich_view_matches_literal["trace"](view, False)
        or rich_view_matches_literal["transport"](view, False)
        or rich_view_matches_literal["useful"](view, False)
        or rich_view_matches_literal["wait_ms"](view, False)
        or rich_view_matches_literal["waited_ms"](view, False)
        or rich_view_matches_literal["last_prompt_requeued"](view, False)
        or rich_view_matches_literal["requested_model"](view, False)
    )


def structured_log_key_free_form(view: ProdexRichStringView) -> Bool:
    return (
        rich_view_matches_literal["error"](view, True)
        or rich_view_matches_literal["message"](view, True)
        or rich_view_matches_literal["detail"](view, True)
        or rich_view_matches_literal["body"](view, True)
        or rich_view_matches_literal["response"](view, True)
        or rich_view_matches_literal["stderr"](view, True)
        or rich_view_matches_literal["panic"](view, True)
    )


def structured_log_view_suffix(
    view: ProdexRichStringView, suffix: StringSlice
) -> Bool:
    var suffix_length = Int64(suffix.byte_length())
    if suffix_length > Int64(view.len):
        return False
    var input = rich_view_ptr(view)
    var expected = suffix.unsafe_ptr()
    var start = Int64(view.len) - suffix_length
    for index in range(suffix_length):
        if input[unsafe_offset=start + index] != expected[unsafe_offset=index]:
            return False
    return True


def structured_log_key_location(view: ProdexRichStringView) -> Bool:
    return (
        rich_view_matches_literal["path"](view, True)
        or structured_log_view_suffix(view, StringSlice("_path"))
        or rich_view_matches_literal["url"](view, True)
        or structured_log_view_suffix(view, StringSlice("_url"))
        or rich_view_matches_literal["endpoint"](view, True)
        or structured_log_view_suffix(view, StringSlice("_endpoint"))
    )


def structured_log_value_stable_code(view: ProdexRichStringView) -> Bool:
    if view.len == 0 or view.len > 128:
        return False
    var input = rich_view_ptr(view)
    if view.len >= 3:
        for index in range(Int64(view.len) - 2):
            if (
                input[unsafe_offset=index] == UInt8(115)
                and input[unsafe_offset=index + 1] == UInt8(107)
                and (
                    input[unsafe_offset=index + 2] == UInt8(45)
                    or input[unsafe_offset=index + 2] == UInt8(95)
                )
            ):
                return False
    for index in range(Int64(view.len)):
        var value = input[unsafe_offset=index]
        if (
            (value >= UInt8(97) and value <= UInt8(122))
            or (value >= UInt8(48) and value <= UInt8(57))
            or value == UInt8(95)
            or value == UInt8(45)
            or value == UInt8(46)
            or value == UInt8(58)
        ):
            continue
        return False
    return True


@export("prodex_mojo_structured_log_field_policy_v1")
def prodex_mojo_structured_log_field_policy_v1(
    abi_version: Int64,
    key_address: UInt,
    key_length: Int64,
    value_address: UInt,
    value_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PRODEX_STRUCTURED_LOG_ABI_VERSION
        or key_length < 0
        or value_length < 0
        or output_address == 0
        or (key_length > 0 and key_address == 0)
        or (value_length > 0 and value_address == 0)
    ):
        return PRODEX_STRUCTURED_LOG_STATUS_INVALID
    var key = ProdexRichStringView(key_address, UInt(key_length))
    var value = ProdexRichStringView(value_address, UInt(value_length))
    if (
        not rich_view_valid(key, key_length)
        or not rich_view_valid(value, value_length)
    ):
        return PRODEX_STRUCTURED_LOG_STATUS_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = 1 if structured_log_key_skip(key) else 0
    output[unsafe_offset=1] = 1 if structured_log_key_known_safe(key) else 0
    output[unsafe_offset=2] = 1 if structured_log_key_free_form(key) else 0
    output[unsafe_offset=3] = 1 if structured_log_value_stable_code(value) else 0
    output[unsafe_offset=4] = 1 if structured_log_key_location(key) else 0
    return PRODEX_STRUCTURED_LOG_STATUS_OK


@export("prodex_mojo_structured_log_sanitize_v1")
def prodex_mojo_structured_log_sanitize_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
    quote_required_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PRODEX_STRUCTURED_LOG_ABI_VERSION
        or input_length < 0
        or output_capacity < 0
        or written_address == 0
        or quote_required_address == 0
        or (input_length > 0 and input_address == 0)
        or (output_capacity > 0 and output_address == 0)
    ):
        return PRODEX_STRUCTURED_LOG_STATUS_INVALID
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, input_length):
        return PRODEX_STRUCTURED_LOG_STATUS_INVALID
    var writer = StructuredLogWriter(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](
            unsafe_from_address=Int(output_address)
        ),
        output_capacity,
        0,
    )
    var quote_required = input_length == 0
    var input = rich_view_ptr(view)
    var index: Int64 = 0
    while index < input_length:
        var value = input[unsafe_offset=index]
        var output_value = value
        if (
            value <= UInt8(31)
            or value == UInt8(127)
        ):
            output_value = UInt8(32)
        elif (
            value == UInt8(194)
            and index + 1 < input_length
            and input[unsafe_offset=index + 1] >= UInt8(128)
            and input[unsafe_offset=index + 1] <= UInt8(159)
        ):
            output_value = UInt8(32)
            index += 1
        if not structured_log_put_byte(Pointer(to=writer), output_value):
            return PRODEX_STRUCTURED_LOG_STATUS_CAPACITY
        if (
            output_value == UInt8(34)
            or output_value == UInt8(92)
            or output_value == UInt8(32)
            or (output_value >= UInt8(9) and output_value <= UInt8(13))
        ):
            quote_required = True
        index += 1
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var quote_output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(quote_required_address)
    )
    written[] = writer.written
    quote_output[] = 1 if quote_required else 0
    return PRODEX_STRUCTURED_LOG_STATUS_OK


@export("prodex_mojo_structured_log_location_strip_v1")
def prodex_mojo_structured_log_location_strip_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PRODEX_STRUCTURED_LOG_ABI_VERSION
        or input_length < 0
        or output_capacity < 0
        or written_address == 0
        or (input_length > 0 and input_address == 0)
        or (output_capacity > 0 and output_address == 0)
    ):
        return PRODEX_STRUCTURED_LOG_STATUS_INVALID
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, input_length):
        return PRODEX_STRUCTURED_LOG_STATUS_INVALID
    var input = rich_view_ptr(view)
    var end = input_length
    for index in range(input_length):
        var value = input[unsafe_offset=index]
        if value == UInt8(63) or value == UInt8(35):
            end = index
            break
    var scheme_end: Int64 = -1
    if end >= 3:
        for index in range(end - 2):
            if (
                input[unsafe_offset=index] == UInt8(58)
                and input[unsafe_offset=index + 1] == UInt8(47)
                and input[unsafe_offset=index + 2] == UInt8(47)
            ):
                scheme_end = index
                break
    var last_at: Int64 = -1
    if scheme_end >= 0:
        for index in range(scheme_end + 3, end):
            if input[unsafe_offset=index] == UInt8(64):
                last_at = index
    var writer = StructuredLogWriter(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](
            unsafe_from_address=Int(output_address)
        ),
        output_capacity,
        0,
    )
    var ok = True
    if scheme_end < 0 or last_at < 0:
        ok = structured_log_put_range(Pointer(to=writer), input, 0, end)
    else:
        ok = (
            structured_log_put_range(Pointer(to=writer), input, 0, scheme_end)
            and structured_log_put_literal(
                Pointer(to=writer), StringSlice("://<redacted>@")
            )
            and structured_log_put_range(
                Pointer(to=writer), input, last_at + 1, end
            )
        )
    if not ok:
        return PRODEX_STRUCTURED_LOG_STATUS_CAPACITY
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = writer.written
    return PRODEX_STRUCTURED_LOG_STATUS_OK


comptime PRODEX_LOG_LOAD_ABI_VERSION: Int64 = 1
comptime PRODEX_LOG_LOAD_UPDATE: Int64 = 0
comptime PRODEX_LOG_LOAD_SUMMARY: Int64 = 1
comptime PRODEX_LOG_LOAD_MAX_RUNS: UInt64 = UInt64(256)
comptime PRODEX_LOG_LOAD_MAX_EVENT_NAME_BYTES: UInt64 = UInt64(256)
comptime PRODEX_LOG_LOAD_MAX_KEY_BYTES: UInt64 = UInt64(16384)
comptime PRODEX_LOG_LOAD_MAX_RUN_ID_BYTES: UInt64 = UInt64(256)
comptime PRODEX_LOG_LOAD_WINDOW_NS: UInt64 = UInt64(5000000000)
comptime PRODEX_LOG_LOAD_UINT64_MAX: UInt64 = UInt64(0xFFFF_FFFF_FFFF_FFFF)


def log_load_text_equal(
    left_address: UInt,
    left_length: UInt64,
    right_address: UInt,
    right_length: UInt64,
) -> Bool:
    if left_length != right_length:
        return False
    if left_length == 0:
        return True
    if left_address == 0 or right_address == 0:
        return False
    var left = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(left_address)
    )
    var right = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(right_address)
    )
    for index in range(Int(left_length)):
        if left[unsafe_offset=index] != right[unsafe_offset=index]:
            return False
    return True


def log_load_matches_literal(
    address: UInt, length: UInt64, literal: StringSlice
) -> Bool:
    if length != UInt64(literal.byte_length()):
        return False
    if length == 0:
        return True
    if address == 0:
        return False
    var input = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    var expected = literal.unsafe_ptr()
    for index in range(Int(length)):
        if input[unsafe_offset=index] != expected[unsafe_offset=index]:
            return False
    return True


def log_load_update_impl(
    input_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
) -> Int64:
    if input_address == 0 or output_address == 0:
        return 1
    if output_capacity < 5:
        return 2
    var input = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_address)
    )
    var event_address = UInt(input[unsafe_offset=0])
    var event_length = input[unsafe_offset=1]
    var previous_key_address = UInt(input[unsafe_offset=2])
    var previous_key_length = input[unsafe_offset=3]
    var observation_key_address = UInt(input[unsafe_offset=4])
    var observation_key_length = input[unsafe_offset=5]
    var elapsed_ns = input[unsafe_offset=6]
    var occurrences = input[unsafe_offset=7]
    var previous_overflow = input[unsafe_offset=8]
    var run_id_address = UInt(input[unsafe_offset=9])
    var run_id_length = input[unsafe_offset=10]
    var run_id_present = input[unsafe_offset=11]
    var run_views_address = UInt(input[unsafe_offset=12])
    var run_count = input[unsafe_offset=13]
    var previous_present = input[unsafe_offset=14]
    if (
        event_length > PRODEX_LOG_LOAD_MAX_EVENT_NAME_BYTES
        or observation_key_length > PRODEX_LOG_LOAD_MAX_KEY_BYTES
        or (event_length > 0 and event_address == 0)
        or (observation_key_length > 0 and observation_key_address == 0)
        or previous_overflow > 1
        or run_id_present > 1
        or previous_present > 1
        or run_count > PRODEX_LOG_LOAD_MAX_RUNS
        or (run_id_length > PRODEX_LOG_LOAD_MAX_RUN_ID_BYTES)
        or (run_id_length > 0 and run_id_address == 0)
        or (run_id_present == 0 and run_id_length != 0)
        or (run_count > 0 and run_views_address == 0)
    ):
        return 1
    if previous_present == 0:
        if (
            previous_key_address != 0
            or previous_key_length != 0
            or occurrences != 0
            or previous_overflow != 0
            or run_count != 0
        ):
            return 1
    elif (
        occurrences == 0
        or previous_key_length > PRODEX_LOG_LOAD_MAX_KEY_BYTES
        or (previous_key_length > 0 and previous_key_address == 0)
        or previous_overflow == 1 and run_count != PRODEX_LOG_LOAD_MAX_RUNS
    ):
        return 1

    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var routine = (
        log_load_matches_literal(
            event_address, event_length, StringSlice("profile_inflight_saturated")
        )
        or log_load_matches_literal(
            event_address,
            event_length,
            StringSlice("runtime_proxy_active_limit_reached"),
        )
        or log_load_matches_literal(
            event_address,
            event_length,
            StringSlice("runtime_proxy_lane_limit_reached"),
        )
    )
    if routine:
        output[unsafe_offset=0] = UInt64(1)
        output[unsafe_offset=1] = UInt64(0)
        output[unsafe_offset=2] = UInt64(0)
        output[unsafe_offset=3] = UInt64(0)
        output[unsafe_offset=4] = UInt64(0)
        return 0

    var coalesce = (
        previous_present == 1
        and elapsed_ns <= PRODEX_LOG_LOAD_WINDOW_NS
        and log_load_text_equal(
            previous_key_address,
            previous_key_length,
            observation_key_address,
            observation_key_length,
        )
    )
    var next_occurrences = UInt64(1)
    var next_overflow = UInt64(0)
    var append_run = UInt64(0)
    var tracked_runs = UInt64(0)
    if coalesce:
        next_occurrences = (
            occurrences
            if occurrences == PRODEX_LOG_LOAD_UINT64_MAX
            else occurrences + UInt64(1)
        )
        next_overflow = previous_overflow
        tracked_runs = run_count
    if run_id_present == 1:
        var run_seen = False
        if coalesce:
            var run_views = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
                unsafe_from_address=Int(run_views_address)
            )
            for index in range(Int(run_count)):
                var base = index * 2
                var current_address = UInt(run_views[unsafe_offset=base])
                var current_length = run_views[unsafe_offset=base + 1]
                if current_length > PRODEX_LOG_LOAD_MAX_RUN_ID_BYTES or (
                    current_length > 0 and current_address == 0
                ):
                    return 1
                if log_load_text_equal(
                    run_id_address,
                    run_id_length,
                    current_address,
                    current_length,
                ):
                    run_seen = True
                    break
        if not run_seen:
            if tracked_runs >= PRODEX_LOG_LOAD_MAX_RUNS:
                next_overflow = UInt64(1)
            else:
                append_run = UInt64(1)
    output[unsafe_offset=0] = UInt64(0)
    output[unsafe_offset=1] = UInt64(coalesce)
    output[unsafe_offset=2] = next_occurrences
    output[unsafe_offset=3] = append_run
    output[unsafe_offset=4] = next_overflow
    return 0


def log_load_decimal_length(value: UInt64) -> Int64:
    var length: Int64 = 1
    var remaining = value
    while remaining >= UInt64(10):
        remaining = remaining / UInt64(10)
        length += 1
    return length


def log_load_write_literal(
    output: Pointer[mut=True, UInt8, MutUntrackedOrigin],
    position: Int64,
    literal: StringSlice,
) -> Int64:
    var source = literal.unsafe_ptr()
    var length = Int64(literal.byte_length())
    for index in range(length):
        output[unsafe_offset=position + index] = source[unsafe_offset=index]
    return position + length


def log_load_write_decimal(
    output: Pointer[mut=True, UInt8, MutUntrackedOrigin],
    position: Int64,
    value: UInt64,
) -> Int64:
    var divisor = UInt64(1)
    while divisor <= value / UInt64(10):
        divisor *= UInt64(10)
    var written = position
    while divisor > 0:
        var digit = (value / divisor) % UInt64(10)
        output[unsafe_offset=written] = UInt8(digit) + UInt8(48)
        written += 1
        divisor = divisor / UInt64(10)
    return written


def log_load_summary_impl(
    input_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) -> Int64:
    if input_address == 0 or written_address == 0 or output_capacity < 0:
        return 1
    if output_capacity > 0 and output_address == 0:
        return 1
    var input = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_address)
    )
    var occurrences = input[unsafe_offset=0]
    var unique_runs = input[unsafe_offset=1]
    var overflow = input[unsafe_offset=2]
    if (
        unique_runs > PRODEX_LOG_LOAD_MAX_RUNS
        or overflow > 1
        or overflow == 1 and unique_runs != PRODEX_LOG_LOAD_MAX_RUNS
    ):
        return 1
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    var prefix = StringSlice(" · ×")
    var separator = StringSlice(" · ")
    var suffix = StringSlice(" runs")
    var needed = (
        Int64(prefix.byte_length())
        + log_load_decimal_length(occurrences)
        + Int64(separator.byte_length())
        + log_load_decimal_length(unique_runs)
        + Int64(suffix.byte_length())
        + Int64(overflow)
    )
    if output_capacity < needed:
        return 2
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var position = log_load_write_literal(output, 0, prefix)
    position = log_load_write_decimal(output, position, occurrences)
    position = log_load_write_literal(output, position, separator)
    position = log_load_write_decimal(output, position, unique_runs)
    if overflow == 1:
        output[unsafe_offset=position] = UInt8(43)
        position += 1
    position = log_load_write_literal(output, position, suffix)
    written[] = position
    return 0


@export("prodex_mojo_log_load_semantics_v1")
def prodex_mojo_log_load_semantics_v1(
    abi_version: Int64,
    operation: Int64,
    input_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_LOG_LOAD_ABI_VERSION:
        return 4
    if operation == PRODEX_LOG_LOAD_UPDATE:
        if written_address != 0:
            return 1
        return log_load_update_impl(input_address, output_address, output_capacity)
    if operation == PRODEX_LOG_LOAD_SUMMARY:
        return log_load_summary_impl(
            input_address, output_address, output_capacity, written_address
        )
    return 1
