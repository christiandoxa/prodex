
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
comptime RESPONSE_FORWARDING_RESPONSES_STREAM: Int64 = 11

comptime RESPONSE_FORWARDING_HEADER_FORWARD: Int64 = 0
comptime RESPONSE_FORWARDING_HEADER_SKIP: Int64 = 1
comptime RESPONSE_FORWARDING_HEADER_CONNECTION_TOKEN: Int64 = 2
comptime RESPONSE_FORWARDING_HEADER_CONNECTION: Int64 = 3

comptime RESPONSE_FORWARDING_ATTEMPT_SUCCESS: Int64 = 0
comptime RESPONSE_FORWARDING_ATTEMPT_AUTH_FAILED: Int64 = 1
comptime RESPONSE_FORWARDING_ATTEMPT_PROFILE_UNAVAILABLE: Int64 = 2
comptime RESPONSE_FORWARDING_ATTEMPT_QUOTA_RETRY: Int64 = 3
comptime RESPONSE_FORWARDING_ATTEMPT_RATE_LIMITED: Int64 = 4
comptime RESPONSE_FORWARDING_ATTEMPT_OVERLOADED: Int64 = 5
comptime RESPONSE_FORWARDING_ATTEMPT_PREVIOUS_RESPONSE_NOT_FOUND: Int64 = 6
comptime RESPONSE_FORWARDING_ATTEMPT_AUTH_FAILURE_NOTICE: Int64 = 7

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


def response_is_hop_header(address: UInt, length: Int64) -> Bool:
    if length > 0 and not rich_utf8_valid(response_text_ptr(address), length):
        return False
    var bounds = rich_trim_bounds(ProdexRichStringView(address, UInt(length)))
    var start = bounds[0]
    var end = bounds[1]
    return (
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
    )


def response_is_connection_header(address: UInt, length: Int64) -> Bool:
    if length > 0 and not rich_utf8_valid(response_text_ptr(address), length):
        return False
    var bounds = rich_trim_bounds(ProdexRichStringView(address, UInt(length)))
    return response_equals_ci_range(
        address,
        bounds[0],
        bounds[1],
        StringSlice("connection"),
    )


def response_is_tchar(value: UInt8) -> Bool:
    return (
        (value >= 48 and value <= 57)
        or (value >= 65 and value <= 90)
        or (value >= 97 and value <= 122)
        or value == 33
        or value == 35
        or value == 36
        or value == 37
        or value == 38
        or value == 39
        or value == 42
        or value == 43
        or value == 45
        or value == 46
        or value == 94
        or value == 95
        or value == 96
        or value == 124
        or value == 126
    )


def response_ranges_equal_ci(
    left_address: UInt,
    left_start: Int64,
    left_end: Int64,
    right_address: UInt,
    right_start: Int64,
    right_end: Int64,
) -> Bool:
    if left_end - left_start != right_end - right_start:
        return False
    var left = response_text_ptr(left_address)
    var right = response_text_ptr(right_address)
    for offset in range(left_end - left_start):
        if response_ascii_lower(left[unsafe_offset=left_start + offset]) != response_ascii_lower(
            right[unsafe_offset=right_start + offset]
        ):
            return False
    return True


def response_connection_value_matches_name(
    name_address: UInt,
    name_length: Int64,
    value_address: UInt,
    value_length: Int64,
) -> Bool:
    if value_length == 0 or not rich_utf8_valid(response_text_ptr(value_address), value_length):
        return False
    var value = response_text_ptr(value_address)
    var name_bounds = rich_trim_bounds(ProdexRichStringView(name_address, UInt(name_length)))
    var token_start: Int64 = 0
    while token_start <= value_length:
        var token_end = token_start
        while token_end < value_length and value[unsafe_offset=token_end] != 44:
            token_end += 1
        var bounds = rich_trim_bounds(
            ProdexRichStringView(
                value_address + UInt(token_start),
                UInt(token_end - token_start),
            )
        )
        var start = token_start + bounds[0]
        var end = token_start + bounds[1]
        var valid = start < end
        for offset in range(start, end):
            if not response_is_tchar(value[unsafe_offset=offset]):
                valid = False
                break
        if valid and response_ranges_equal_ci(
            name_address,
            name_bounds[0],
            name_bounds[1],
            value_address,
            start,
            end,
        ):
            return True
        if token_end == value_length:
            break
        token_start = token_end + 1
    return False


@export("prodex_runtime_response_forwarding_header_v1")
def prodex_runtime_response_forwarding_header_v1(
    name_address: UInt,
    name_length: Int64,
    value_address: UInt,
    value_length: Int64,
    value_present: Int64,
) abi("C") -> Int64:
    if (
        name_length < 0
        or value_length < 0
        or value_present < 0
        or value_present > 1
        or (name_length > 0 and name_address == 0)
        or (value_present == 1 and value_length > 0 and value_address == 0)
    ):
        return -1
    if name_length > 0 and not rich_utf8_valid(response_text_ptr(name_address), name_length):
        return -1
    if response_is_connection_header(name_address, name_length):
        return RESPONSE_FORWARDING_HEADER_CONNECTION
    if response_is_hop_header(name_address, name_length):
        return RESPONSE_FORWARDING_HEADER_SKIP
    if value_present == 1 and value_length > 0:
        if not rich_utf8_valid(response_text_ptr(value_address), value_length):
            return RESPONSE_FORWARDING_HEADER_FORWARD
        if response_connection_value_matches_name(
            name_address,
            name_length,
            value_address,
            value_length,
        ):
            return RESPONSE_FORWARDING_HEADER_CONNECTION_TOKEN
    return RESPONSE_FORWARDING_HEADER_FORWARD


@export("prodex_runtime_response_forwarding_content_type_v1")
def prodex_runtime_response_forwarding_content_type_v1(
    name_address: UInt,
    name_length: Int64,
    value_address: UInt,
    value_length: Int64,
) abi("C") -> Int64:
    if (
        name_length < 0
        or value_length < 0
        or (name_length > 0 and name_address == 0)
        or (value_length > 0 and value_address == 0)
    ):
        return -1
    if name_length > 0 and not rich_utf8_valid(response_text_ptr(name_address), name_length):
        return -1
    if value_length == 0 or not rich_utf8_valid(response_text_ptr(value_address), value_length):
        return 0
    var name_bounds = rich_trim_bounds(ProdexRichStringView(name_address, UInt(name_length)))
    var value_bounds = rich_trim_bounds(ProdexRichStringView(value_address, UInt(value_length)))
    return Int64(
        response_equals_ci_range(
            name_address,
            name_bounds[0],
            name_bounds[1],
            StringSlice("content-type"),
        )
        and value_bounds[0] < value_bounds[1]
    )


@export("prodex_runtime_response_forwarding_attempt_v1")
def prodex_runtime_response_forwarding_attempt_v1(
    status: Int64,
    class_tag: Int64,
    action_tag: Int64,
    retryable_previous: Int64,
    token_invalidated: Int64,
    committed: Int64,
) abi("C") -> Int64:
    return response_forwarding_attempt_plan(
        status,
        class_tag,
        action_tag,
        retryable_previous,
        token_invalidated,
        committed,
    )


def response_forwarding_attempt_plan(
    status: Int64,
    class_tag: Int64,
    action_tag: Int64,
    retryable_previous: Int64,
    token_invalidated: Int64,
    committed: Int64,
) -> Int64:
    if (
        status < 0
        or status > 65535
        or class_tag < 0
        or class_tag > 5
        or action_tag < 0
        or action_tag > 2
        or retryable_previous < 0
        or retryable_previous > 1
        or token_invalidated < 0
        or token_invalidated > 1
        or committed < 0
        or committed > 1
    ):
        return -1
    # Once output is committed the upstream response is transparent and no
    # recovery action may rotate or retry it.
    if committed == 1:
        return RESPONSE_FORWARDING_ATTEMPT_SUCCESS
    # A stale continuation is terminal for standard and compact forwarding.
    if retryable_previous == 1:
        return RESPONSE_FORWARDING_ATTEMPT_PREVIOUS_RESPONSE_NOT_FOUND
    if status == 401:
        return RESPONSE_FORWARDING_ATTEMPT_AUTH_FAILED
    if class_tag == 3 and action_tag == 1:
        return RESPONSE_FORWARDING_ATTEMPT_PROFILE_UNAVAILABLE
    if class_tag == 1 and action_tag == 1:
        return RESPONSE_FORWARDING_ATTEMPT_QUOTA_RETRY
    if class_tag == 2 and action_tag == 2:
        return RESPONSE_FORWARDING_ATTEMPT_RATE_LIMITED
    if (class_tag == 4 or class_tag == 5) and action_tag == 2:
        return RESPONSE_FORWARDING_ATTEMPT_OVERLOADED
    if token_invalidated == 1:
        return RESPONSE_FORWARDING_ATTEMPT_AUTH_FAILURE_NOTICE
    return RESPONSE_FORWARDING_ATTEMPT_SUCCESS


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
        or operation > RESPONSE_FORWARDING_RESPONSES_STREAM
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
        return Int64(response_is_hop_header(address, length))

    if operation == RESPONSE_FORWARDING_RESPONSES_STREAM:
        if _numeric > 1:
            return -1
        # The HTTP fallback client still requests SSE. An omitted or blank MIME
        # header must not bypass precommit inspection and commit a failure as 200.
        if present == 0 or length == 0:
            return Int64(_numeric)
        if not rich_utf8_valid(response_text_ptr(address), length):
            return -1
        var bounds = rich_trim_bounds(ProdexRichStringView(address, UInt(length)))
        if bounds[0] == bounds[1]:
            return Int64(_numeric)
        return Int64(
            response_contains_ci(address, length, StringSlice("text/event-stream"))
        )

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


@export("prodex_runtime_bound_overload_retry_v1")
def prodex_runtime_bound_overload_retry_v1(
    abi_version: Int64,
    hard_affinity: Int64,
    previous_response_present: Int64,
    committed: Int64,
    retries: UInt64,
    elapsed_ms: UInt64,
    retry_after_present: Int64,
    retry_after_ms: UInt64,
    jitter_key: UInt64,
) abi("C") -> Int64:
    if abi_version != 1:
        return -4
    if (
        (hard_affinity != 0 and hard_affinity != 1)
        or (previous_response_present != 0 and previous_response_present != 1)
        or (committed != 0 and committed != 1)
        or (retry_after_present != 0 and retry_after_present != 1)
    ):
        return -2
    # Preserve the owner of a live continuation. Never re-send committed output,
    # and never retry indefinitely when the provider remains at capacity.
    # Previous-response-ID repair retains its established full-history signaling.
    # This additional retry path is for the HTTP client's turn-state-only samples.
    if hard_affinity == 0 or previous_response_present == 1 or committed == 1 or retries >= 5 or elapsed_ms >= 60_000:
        return -1
    var delay_ms = UInt64(250) * (UInt64(1) << retries) + jitter_key % UInt64(251)
    if retry_after_present == 1 and retry_after_ms > delay_ms:
        delay_ms = retry_after_ms
    # Do not truncate Retry-After to manufacture an early retry at the deadline.
    if delay_ms >= UInt64(60_000) - elapsed_ms:
        return -1
    return Int64(delay_ms)
