from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_matches_literal, rich_view_valid

# Native Anthropic SSE event routing and stop-reason semantics. The Rust
# transport owns JSON acquisition and response materialization, not routing.
comptime ANTHROPIC_STREAM_POLICY_ABI: Int64 = 1

comptime ACTION_IGNORE: Int64 = 0
comptime ACTION_MESSAGE_START: Int64 = 1
comptime ACTION_START_TOOL_USE: Int64 = 2
comptime ACTION_START_WEB_SEARCH: Int64 = 3
comptime ACTION_WEB_SEARCH_RESULT: Int64 = 4
comptime ACTION_MISSING_BLOCK: Int64 = 5
comptime ACTION_MISSING_BLOCK_TYPE: Int64 = 6
comptime ACTION_DELTA_TEXT: Int64 = 7
comptime ACTION_DELTA_ARGUMENTS: Int64 = 8
comptime ACTION_DELTA_THINKING: Int64 = 9
comptime ACTION_DELTA_SEARCH_INPUT: Int64 = 10
comptime ACTION_MISSING_DELTA_TYPE: Int64 = 11
comptime ACTION_MESSAGE_DELTA: Int64 = 12
comptime ACTION_MESSAGE_STOP: Int64 = 13
comptime ACTION_ERROR: Int64 = 14
comptime ACTION_BLOCK_STOP: Int64 = 15


@export("prodex_anthropic_stream_action_v1")
def prodex_anthropic_stream_action_v1(
    abi_version: Int64,
    event_address: UInt,
    event_length: Int64,
    block_address: UInt,
    block_length: Int64,
    delta_address: UInt,
    delta_length: Int64,
    name_address: UInt,
    name_length: Int64,
    block_present: Int64,
    block_type_present: Int64,
    delta_type_present: Int64,
    ignored_block: Int64,
    active_web_search: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != ANTHROPIC_STREAM_POLICY_ABI:
        return 4
    if (
        event_length < 0 or block_length < 0 or delta_length < 0
        or name_length < 0
        or (event_length > 0 and event_address == 0)
        or (block_length > 0 and block_address == 0)
        or (delta_length > 0 and delta_address == 0)
        or (name_length > 0 and name_address == 0)
        or result_address == 0
        or (block_present != 0 and block_present != 1)
        or (block_type_present != 0 and block_type_present != 1)
        or (delta_type_present != 0 and delta_type_present != 1)
        or (ignored_block != 0 and ignored_block != 1)
        or (active_web_search != 0 and active_web_search != 1)
    ):
        return 1
    var event = ProdexRichStringView(event_address, UInt(event_length))
    var block = ProdexRichStringView(block_address, UInt(block_length))
    var delta = ProdexRichStringView(delta_address, UInt(delta_length))
    var name = ProdexRichStringView(name_address, UInt(name_length))
    if (
        not rich_view_valid(event, 0x7FFFFFFFFFFFFFFF)
        or not rich_view_valid(block, 0x7FFFFFFFFFFFFFFF)
        or not rich_view_valid(delta, 0x7FFFFFFFFFFFFFFF)
        or not rich_view_valid(name, 0x7FFFFFFFFFFFFFFF)
    ):
        return 1
    var action = ACTION_IGNORE
    if rich_view_matches_literal["message_start"](event, False):
        action = ACTION_MESSAGE_START
    elif rich_view_matches_literal["content_block_start"](event, False):
        if block_present == 0:
            action = ACTION_MISSING_BLOCK
        elif block_type_present == 0:
            action = ACTION_MISSING_BLOCK_TYPE
        elif rich_view_matches_literal["tool_use"](block, False):
            action = ACTION_START_TOOL_USE
        elif (
            rich_view_matches_literal["server_tool_use"](block, False)
            and rich_view_matches_literal["web_search"](name, False)
        ):
            action = ACTION_START_WEB_SEARCH
        elif rich_view_matches_literal["web_search_tool_result"](block, False):
            action = ACTION_WEB_SEARCH_RESULT
    elif rich_view_matches_literal["content_block_delta"](event, False):
        if ignored_block == 1:
            action = ACTION_IGNORE
        elif active_web_search == 1:
            if rich_view_matches_literal["input_json_delta"](delta, False):
                action = ACTION_DELTA_SEARCH_INPUT
        elif delta_type_present == 0:
            action = ACTION_MISSING_DELTA_TYPE
        elif rich_view_matches_literal["text_delta"](delta, False):
            action = ACTION_DELTA_TEXT
        elif rich_view_matches_literal["input_json_delta"](delta, False):
            action = ACTION_DELTA_ARGUMENTS
        elif rich_view_matches_literal["thinking_delta"](delta, False):
            action = ACTION_DELTA_THINKING
    elif rich_view_matches_literal["message_delta"](event, False):
        action = ACTION_MESSAGE_DELTA
    elif rich_view_matches_literal["message_stop"](event, False):
        action = ACTION_MESSAGE_STOP
    elif rich_view_matches_literal["error"](event, False):
        action = ACTION_ERROR
    elif rich_view_matches_literal["content_block_stop"](event, False):
        action = ACTION_BLOCK_STOP
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )[] = action
    return 0


@export("prodex_anthropic_stream_finish_reason_v1")
def prodex_anthropic_stream_finish_reason_v1(
    abi_version: Int64,
    reason_present: Int64,
    reason_address: UInt,
    reason_length: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != ANTHROPIC_STREAM_POLICY_ABI:
        return 4
    if (
        reason_length < 0
        or (reason_length > 0 and reason_address == 0)
        or (reason_present != 0 and reason_present != 1)
        or result_address == 0
    ):
        return 1
    var reason = ProdexRichStringView(reason_address, UInt(reason_length))
    if not rich_view_valid(reason, 0x7FFFFFFFFFFFFFFF):
        return 1
    var result: Int64 = 0
    if reason_present == 1:
        result = 1
        if rich_view_matches_literal["tool_use"](reason, False):
            result = 2
        elif rich_view_matches_literal["max_tokens"](reason, False):
            result = 3
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )[] = result
    return 0


# Web search lifecycle: result IDs select the first matching indexed tool,
# even when already completed (the host still parses repeated result content).
# End-of-message completion includes all unfinished tools in BTreeMap order.
def anthropic_stream_same_id(
    left: ProdexRichStringView, right: ProdexRichStringView
) -> Bool:
    if left.len != right.len:
        return False
    var a = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(left.ptr)
    )
    var b = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(right.ptr)
    )
    for index in range(Int64(left.len)):
        if a[unsafe_offset=index] != b[unsafe_offset=index]:
            return False
    return True


@export("prodex_anthropic_web_search_completion_v1")
def prodex_anthropic_web_search_completion_v1(
    abi_version: Int64,
    id_views_address: UInt,
    state_address: UInt,
    candidate_count: Int64,
    target_present: Int64,
    target_address: UInt,
    target_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != ANTHROPIC_STREAM_POLICY_ABI:
        return 4
    if (
        candidate_count < 0 or candidate_count > 0x7FFFFFFFFFFFFFFF // 2
        or (candidate_count > 0 and (id_views_address == 0 or state_address == 0))
        or (target_present != 0 and target_present != 1)
        or target_length < 0 or (target_length > 0 and target_address == 0)
        or output_address == 0 or output_capacity < candidate_count * 2
        or written_address == 0
    ):
        return 1
    var views = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(id_views_address)
    )
    var states = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(state_address)
    )
    var target = ProdexRichStringView(target_address, UInt(target_length))
    if not rich_view_valid(target, 0x7FFFFFFFFFFFFFFF):
        return 1
    # Validate all strings and state bits before any caller-owned output.
    for i in range(candidate_count):
        var address = UInt(views[unsafe_offset=2 * i])
        var length = UInt(views[unsafe_offset=2 * i + 1])
        if length > 0x7FFFFFFFFFFFFFFF or (length > 0 and address == 0):
            return 1
        var id = ProdexRichStringView(address, length)
        if not rich_view_valid(id, 0x7FFFFFFFFFFFFFFF):
            return 1
        var state = states[unsafe_offset=i]
        if state != 0 and state != 1:
            return 1
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var count: Int64 = 0
    for i in range(candidate_count):
        var id = ProdexRichStringView(
            UInt(views[unsafe_offset=2 * i]),
            UInt(views[unsafe_offset=2 * i + 1]),
        )
        if target_present == 1:
            if anthropic_stream_same_id(id, target):
                output[unsafe_offset=0] = UInt64(i)
                output[unsafe_offset=1] = UInt64(states[unsafe_offset=i] == 0)
                count = 1
                break
        elif states[unsafe_offset=i] == 0:
            output[unsafe_offset=2 * count] = UInt64(i)
            output[unsafe_offset=2 * count + 1] = 1
            count += 1
    written[] = count
    return 0


# First strict "data: " line from an Anthropic SSE event. Rust keeps the
# String::from_utf8_lossy transport decode and Serde JSON parse; compiled
# Mojo owns line scanning, one-CR removal, and first-match selection.
@export("prodex_anthropic_stream_data_line_v1")
def prodex_anthropic_stream_data_line_v1(
    abi_version: Int64, event_address: UInt, event_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != ANTHROPIC_STREAM_POLICY_ABI:
        return 4
    if (
        event_length < 0 or (event_length > 0 and event_address == 0)
        or output_address == 0 or output_address % 8 != 0
    ):
        return 1
    var view = ProdexRichStringView(event_address, UInt(event_length))
    if not rich_view_valid(view, 0x7FFFFFFFFFFFFFFF):
        return 1
    var bytes = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(event_address)
    )
    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var cursor = Int64(0)
    var selected = False
    var selected_start = Int64(0)
    var selected_end = Int64(0)
    while cursor < event_length:
        var line_start = cursor
        while cursor < event_length and bytes[unsafe_offset=cursor] != 10:
            cursor += 1
        var line_end = cursor
        if cursor < event_length:
            cursor += 1
        # Rust str::lines removes one final carriage return per line,
        # including the last line without a trailing LF.
        if line_end > line_start and bytes[unsafe_offset=line_end - 1] == 13:
            line_end -= 1
        if line_end - line_start >= 6 and (
            bytes[unsafe_offset=line_start] == 100
            and bytes[unsafe_offset=line_start + 1] == 97
            and bytes[unsafe_offset=line_start + 2] == 116
            and bytes[unsafe_offset=line_start + 3] == 97
            and bytes[unsafe_offset=line_start + 4] == 58
            and bytes[unsafe_offset=line_start + 5] == 32
        ):
            selected = True
            selected_start = line_start + 6
            selected_end = line_end
            break
    result[unsafe_offset=0] = Int64(selected)
    result[unsafe_offset=1] = selected_start
    result[unsafe_offset=2] = selected_end
    return 0
