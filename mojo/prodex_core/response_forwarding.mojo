
from std.memory import Pointer
from rich_text import rich_trim_bounds, rich_utf8_valid
from rich_types import ProdexRichStringView


comptime RESPONSE_FORWARDING_SKIP_HEADER: Int64 = 0
comptime RESPONSE_FORWARDING_CONTENT_TYPE_SSE: Int64 = 1
comptime RESPONSE_FORWARDING_USAGE_EVENT_LOGGABLE: Int64 = 2
comptime RESPONSE_FORWARDING_GENERATION_START: Int64 = 3
comptime RESPONSE_FORWARDING_WEBSOCKET_TERMINAL_RESET: Int64 = 4
comptime RESPONSE_FORWARDING_RECORD_RESPONSE_IDS: Int64 = 5
comptime RESPONSE_FORWARDING_COMMITTED_PREVIOUS_RESPONSE_NOT_FOUND: Int64 = 6
comptime RESPONSE_FORWARDING_GENERATION_START_ONCE: Int64 = 7
comptime RESPONSE_FORWARDING_USAGE_EVENT_LIVE: Int64 = 8
comptime RESPONSE_FORWARDING_PRECOMMIT_ATTEMPT: Int64 = 9
comptime RESPONSE_FORWARDING_TAP_PLAN: Int64 = 10

comptime RESPONSE_ATTEMPT_SUCCESS: Int64 = 0
comptime RESPONSE_ATTEMPT_AUTH_FAILED: Int64 = 1
comptime RESPONSE_ATTEMPT_QUOTA_BLOCKED: Int64 = 2
comptime RESPONSE_ATTEMPT_RATE_LIMITED: Int64 = 3
comptime RESPONSE_ATTEMPT_OVERLOADED: Int64 = 4
comptime RESPONSE_ATTEMPT_PREVIOUS_RESPONSE_NOT_FOUND: Int64 = 5
comptime RESPONSE_ATTEMPT_AUTH_FAILURE_NOTICE: Int64 = 8

comptime RESPONSE_USAGE_PROGRESS_ABI_VERSION: Int64 = 1
comptime RESPONSE_USAGE_PROGRESS_IGNORE: Int64 = 0
comptime RESPONSE_USAGE_PROGRESS_SUPPRESS: Int64 = 1
comptime RESPONSE_USAGE_PROGRESS_LOG: Int64 = 2
comptime RESPONSE_USAGE_PROGRESS_INTERVAL_MS: UInt64 = 250


def response_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def response_text_ptr(address: UInt) -> Pointer[mut=False, UInt8, ImmUntrackedOrigin]:
    return Pointer[mut=False, UInt8, ImmUntrackedOrigin](unsafe_from_address=Int(address))


def response_equals(
    address: UInt,
    length: Int64,
    literal: StringSlice,
) -> Bool:
    var expected = Int64(literal.byte_length())
    if length != expected:
        return False
    if length == 0:
        return True
    if address == 0:
        return False
    var source = response_text_ptr(address)
    var target = literal.unsafe_ptr()
    for index in range(length):
        if source[unsafe_offset=index] != target[unsafe_offset=index]:
            return False
    return True


def response_equals_ci_range(
    address: UInt,
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    var expected = Int64(literal.byte_length())
    if end - start != expected:
        return False
    if expected == 0:
        return True
    if address == 0:
        return False
    var source = response_text_ptr(address)
    var target = literal.unsafe_ptr()
    for offset in range(expected):
        if response_ascii_lower(source[unsafe_offset=start + offset]) != target[unsafe_offset=offset]:
            return False
    return True


def response_contains_ci(
    address: UInt,
    length: Int64,
    literal: StringSlice,
) -> Bool:
    var expected = Int64(literal.byte_length())
    if expected == 0:
        return True
    if address == 0 or length < expected:
        return False
    var source = response_text_ptr(address)
    var target = literal.unsafe_ptr()
    var start: Int64 = 0
    while start + expected <= length:
        var matched = True
        for offset in range(expected):
            if response_ascii_lower(source[unsafe_offset=start + offset]) != target[unsafe_offset=offset]:
                matched = False
                break
        if matched:
            return True
        start += 1
    return False


def response_ends_with(
    address: UInt,
    length: Int64,
    literal: StringSlice,
) -> Bool:
    var expected = Int64(literal.byte_length())
    if address == 0 or length < expected:
        return False
    var source = response_text_ptr(address)
    var target = literal.unsafe_ptr()
    var start = length - expected
    for offset in range(expected):
        if source[unsafe_offset=start + offset] != target[unsafe_offset=offset]:
            return False
    return True


def response_generation_start(address: UInt, length: Int64) -> Bool:
    return (
        response_equals(address, length, StringSlice("response.output_text.delta"))
        or response_equals(address, length, StringSlice("response.refusal.delta"))
        or response_equals(
            address, length, StringSlice("response.reasoning_summary_text.delta")
        )
        or response_equals(address, length, StringSlice("response.reasoning_text.delta"))
        or response_equals(
            address, length, StringSlice("response.function_call_arguments.delta")
        )
        or response_equals(
            address, length, StringSlice("response.mcp_call_arguments.delta")
        )
        or response_equals(
            address, length, StringSlice("response.custom_tool_call_input.delta")
        )
    )


def response_precommit_attempt_plan(numeric: UInt64) -> Int64:
    # Rust packs status, the existing Mojo error class/action tags, and the
    # caller-owned response observations into this scalar. Mojo owns only the
    # deterministic outcome precedence; Rust retains response/effect handling.
    var status = numeric & 0xffff
    var class_tag = (numeric >> 16) & 0x7
    var action_tag = (numeric >> 19) & 0x3
    var retryable_previous = ((numeric >> 21) & 1) == 1
    var token_invalidated = ((numeric >> 22) & 1) == 1
    var committed = ((numeric >> 23) & 1) == 1
    if class_tag > 5 or action_tag > 2:
        return -1
    if committed:
        return RESPONSE_ATTEMPT_SUCCESS
    if status == 401:
        return RESPONSE_ATTEMPT_AUTH_FAILED
    if action_tag == 1 and class_tag == 1:
        return RESPONSE_ATTEMPT_QUOTA_BLOCKED
    if action_tag == 2 and class_tag == 2:
        return RESPONSE_ATTEMPT_RATE_LIMITED
    if action_tag == 2 or (action_tag == 1 and class_tag == 3):
        return RESPONSE_ATTEMPT_OVERLOADED
    if retryable_previous:
        return RESPONSE_ATTEMPT_PREVIOUS_RESPONSE_NOT_FOUND
    if token_invalidated:
        return RESPONSE_ATTEMPT_AUTH_FAILURE_NOTICE
    return RESPONSE_ATTEMPT_SUCCESS


@export("prodex_runtime_response_forwarding_classify_v1")
def prodex_runtime_response_forwarding_classify_v1(
    operation: Int64,
    address: UInt,
    length: Int64,
    present: Int64,
    _numeric: UInt64,
) abi("C") -> Int64:
    if (
        operation < RESPONSE_FORWARDING_SKIP_HEADER
        or operation > RESPONSE_FORWARDING_TAP_PLAN
        or present < 0
        or present > 1
        or length < 0
        or (present == 1 and length > 0 and address == 0)
    ):
        return -1

    if operation == RESPONSE_FORWARDING_SKIP_HEADER:
        if present == 0:
            return 0
        if length > 0 and not rich_utf8_valid(response_text_ptr(address), length):
            return -1
        var bounds = rich_trim_bounds(ProdexRichStringView(address, UInt(length)))
        var start = bounds[0]
        var end = bounds[1]
        if (
            response_equals_ci_range(address, start, end, StringSlice("connection"))
            or response_equals_ci_range(address, start, end, StringSlice("content-length"))
            or response_equals_ci_range(address, start, end, StringSlice("keep-alive"))
            or response_equals_ci_range(
                address, start, end, StringSlice("proxy-authenticate")
            )
            or response_equals_ci_range(
                address, start, end, StringSlice("proxy-authorization")
            )
            or response_equals_ci_range(address, start, end, StringSlice("te"))
            or response_equals_ci_range(address, start, end, StringSlice("trailer"))
            or response_equals_ci_range(
                address, start, end, StringSlice("transfer-encoding")
            )
            or response_equals_ci_range(address, start, end, StringSlice("upgrade"))
        ):
            return 1
        return 0

    if operation == RESPONSE_FORWARDING_CONTENT_TYPE_SSE:
        if present == 0:
            return 0
        return Int64(
            response_contains_ci(address, length, StringSlice("text/event-stream"))
        )

    if operation == RESPONSE_FORWARDING_USAGE_EVENT_LOGGABLE:
        if present == 0:
            return 1
        return Int64(
            response_equals(address, length, StringSlice("response.completed"))
            or response_equals(address, length, StringSlice("response.failed"))
            or response_ends_with(address, length, StringSlice(".completed"))
        )

    if operation == RESPONSE_FORWARDING_GENERATION_START:
        return Int64(present == 1 and response_generation_start(address, length))

    if operation == RESPONSE_FORWARDING_WEBSOCKET_TERMINAL_RESET:
        var realtime = _numeric & 1
        return Int64(
            realtime == 0
            and present == 1
            and (
                response_equals(address, length, StringSlice("error"))
                or response_equals(address, length, StringSlice("response.failed"))
                or response_equals(address, length, StringSlice("response.incomplete"))
            )
        )

    if operation == RESPONSE_FORWARDING_RECORD_RESPONSE_IDS:
        var precommit_hold = _numeric & 1
        return Int64(precommit_hold == 0)

    if operation == RESPONSE_FORWARDING_COMMITTED_PREVIOUS_RESPONSE_NOT_FOUND:
        var committed = _numeric & 1
        var previous_response_not_found = (_numeric >> 1) & 1
        return Int64(committed == 1 and previous_response_not_found == 1)

    if operation == RESPONSE_FORWARDING_TAP_PLAN:
        # Bits: generation-start, completed, live-usage, loggable-usage.
        var generation = present == 1 and response_generation_start(address, length)
        var completed = present == 1 and response_equals(address, length, StringSlice("response.completed"))
        var live = generation and _numeric > 0
        var loggable = present == 0 or completed or response_equals(address, length, StringSlice("response.failed")) or response_ends_with(address, length, StringSlice(".completed"))
        return Int64(generation) | (Int64(completed) << 1) | (Int64(live) << 2) | (Int64(loggable) << 3)

    if operation == RESPONSE_FORWARDING_GENERATION_START_ONCE:
        var already_started = _numeric & 1
        return Int64(
            already_started == 0
            and present == 1
            and response_generation_start(address, length)
        )

    if operation == RESPONSE_FORWARDING_PRECOMMIT_ATTEMPT:
        if present != 0:
            return -1
        return response_precommit_attempt_plan(_numeric)

    return Int64(
        present == 1
        and _numeric > 0
        and response_generation_start(address, length)
    )


@export("prodex_runtime_token_usage_progress_plan_v1")
def prodex_runtime_token_usage_progress_plan_v1(
    abi_version: Int64,
    output_tokens: UInt64,
    last_output_present: Int64,
    last_output_tokens: UInt64,
    last_log_present: Int64,
    elapsed_since_last_log_ms: UInt64,
) abi("C") -> Int64:
    if (
        abi_version != RESPONSE_USAGE_PROGRESS_ABI_VERSION
        or (last_output_present != 0 and last_output_present != 1)
        or (last_log_present != 0 and last_log_present != 1)
    ):
        return -1
    if (
        output_tokens == 0
        or (
            last_output_present == 1
            and output_tokens <= last_output_tokens
        )
    ):
        return RESPONSE_USAGE_PROGRESS_IGNORE
    if (
        last_log_present == 1
        and elapsed_since_last_log_ms < RESPONSE_USAGE_PROGRESS_INTERVAL_MS
    ):
        return RESPONSE_USAGE_PROGRESS_SUPPRESS
    return RESPONSE_USAGE_PROGRESS_LOG
