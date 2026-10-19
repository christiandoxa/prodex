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
