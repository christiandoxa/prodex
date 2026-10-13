from std.memory import Pointer

from launch_args_common import (
    LaunchArgView, launch_arg, launch_is, launch_prefix, launch_rust_space, launch_view,
)
from rich_text import rich_codepoint, rich_codepoint_width, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime LAUNCH_CONFIG_ABI_VERSION: Int64 = 1
comptime CONFIG_ORIGINAL: Int64 = 0
comptime CONFIG_SEPARATE: Int64 = 1
comptime CONFIG_LONG_INLINE: Int64 = 2
comptime CONFIG_SHORT_INLINE: Int64 = 3


def launch_config_key(arg: LaunchArgView, start: Int64) -> ProdexRichStringView:
    if arg.valid_utf8 == 0:
        return ProdexRichStringView(0, 0)
    var view = launch_view(arg)
    var ptr = rich_view_ptr(view)
    var end = start
    while end < Int64(view.len) and ptr[unsafe_offset=end] != 61:
        end += 1
    if end == Int64(view.len):
        return ProdexRichStringView(0, 0)
    var left = start
    var last_nonspace = start
    var cursor = start
    var leading = True
    while cursor < end:
        var width = rich_codepoint_width(ptr[unsafe_offset=cursor])
        var space = launch_rust_space(rich_codepoint(ptr, cursor, width))
        cursor += width
        if leading and space:
            left = cursor
        else:
            leading = False
        if not space:
            last_nonspace = cursor
    if last_nonspace <= left:
        return ProdexRichStringView(0, 0)
    return ProdexRichStringView(view.ptr + UInt(left), UInt(last_nonspace - left))


def launch_config_match(key: ProdexRichStringView, keys_address: UInt64, key_count: Int64) -> Int64:
    if key.len == 0:
        return -1
    var keys = Pointer[mut=False, LaunchArgView, MutUntrackedOrigin](unsafe_from_address=Int(keys_address))
    var left = rich_view_ptr(key)
    for index in range(key_count):
        var candidate = keys[unsafe_offset=index].copy()
        if candidate.length != UInt64(key.len):
            continue
        var right = rich_view_ptr(launch_view(candidate))
        var equal = True
        for offset in range(Int64(key.len)):
            if left[unsafe_offset=offset] != right[unsafe_offset=offset]:
                equal = False
                break
        if equal:
            return index
    return -1


@export("prodex_mojo_launch_config_v1")
def prodex_mojo_launch_config_v1(
    abi_version: Int64,
    arguments_address: UInt64, argument_count: Int64,
    keys_address: UInt64, key_count: Int64,
    output_address: UInt64, output_count: Int64,
    replaced_address: UInt64, replaced_count: Int64,
) abi("C") -> Int64:
    if abi_version != LAUNCH_CONFIG_ABI_VERSION:
        return 4
    if (
        argument_count < 0 or key_count < 0
        or argument_count > 0x7FFFFFFFFFFFFFFF // 24
        or key_count > 0x7FFFFFFFFFFFFFFF // 24
        or (argument_count > 0 and arguments_address == 0)
        or (key_count > 0 and keys_address == 0)
    ):
        return 1
    if output_count != argument_count or replaced_count != key_count:
        return 3
    if (output_count > 0 and output_address == 0) or (replaced_count > 0 and replaced_address == 0):
        return 1
    for table in range(2):
        var address = arguments_address if table == 0 else keys_address
        var count = argument_count if table == 0 else key_count
        var records = Pointer[mut=False, LaunchArgView, MutUntrackedOrigin](unsafe_from_address=Int(address))
        for index in range(count):
            var value = records[unsafe_offset=index].copy()
            if value.valid_utf8 == 0 and table == 0:
                if value.address != 0 or value.length != 0:
                    return 1
            elif value.valid_utf8 == 1:
                if not rich_view_valid(launch_view(value), 0x7FFFFFFFFFFFFFFF):
                    return 2
            else:
                return 1
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var replaced = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(replaced_address))
    for index in range(key_count):
        replaced[unsafe_offset=index] = 0
    for index in range(argument_count):
        output[unsafe_offset=index * 2] = CONFIG_ORIGINAL
        output[unsafe_offset=index * 2 + 1] = -1
    var index: Int64 = 0
    while index < argument_count:
        var arg = launch_arg(arguments_address, 0, index)
        if launch_is["--"](arg):
            break
        var kind = CONFIG_ORIGINAL
        var start: Int64 = 0
        if (launch_is["-c"](arg) or launch_is["--config"](arg)) and index + 1 < argument_count:
            kind = CONFIG_SEPARATE
            index += 1
            arg = launch_arg(arguments_address, 0, index)
        elif launch_prefix["--config="](arg):
            kind = CONFIG_LONG_INLINE
            start = 9
        elif launch_prefix["-c"](arg) and arg.length > 2:
            kind = CONFIG_SHORT_INLINE
            start = 2
        if kind != CONFIG_ORIGINAL:
            var matched = launch_config_match(launch_config_key(arg, start), keys_address, key_count)
            if matched >= 0:
                output[unsafe_offset=index * 2] = kind
                output[unsafe_offset=index * 2 + 1] = matched
                replaced[unsafe_offset=matched] = 1
        index += 1
    return 0


# Native recovery is an idle-thread operation, not a frontend relaunch.
# 0 ignore, 1 clear stale recovery, 2 arm a terminal recoverable failure.
@export("prodex_launch_recovery_observation_v1")
def prodex_launch_recovery_observation_v1(
    abi_version: Int64,
    kind: Int64,
    session_matches: Int64,
    recoverable: Int64,
) abi("C") -> Int64:
    if abi_version != 1:
        return -3
    if kind < 0 or kind > 2 or (session_matches != 0 and session_matches != 1) or (
        recoverable != 0 and recoverable != 1
    ):
        return -2
    if session_matches == 0 or kind == 0:
        return 0
    if kind == 1:
        return 1
    return 2 if recoverable == 1 else 1


# Workflow recovery classes: none=0, usageLimit=1, rateLimit=2,
# overload=3, auth=4, transport=5. The Rust adapter owns JSON envelope
# projection; this operation owns the class decision for the bounded error tag.
comptime PRODEX_WORKFLOW_CLASS_ABI_VERSION: Int64 = 1
comptime PRODEX_WORKFLOW_CLASS_STATUS_OK: Int64 = 0
comptime PRODEX_WORKFLOW_CLASS_STATUS_INVALID: Int64 = 1
comptime PRODEX_WORKFLOW_CLASS_STATUS_ABI: Int64 = 4
comptime PRODEX_WORKFLOW_CLASS_NONE: Int64 = 0
comptime PRODEX_WORKFLOW_CLASS_USAGE: Int64 = 1
comptime PRODEX_WORKFLOW_CLASS_RATE: Int64 = 2
comptime PRODEX_WORKFLOW_CLASS_OVERLOAD: Int64 = 3
comptime PRODEX_WORKFLOW_CLASS_AUTH: Int64 = 4
comptime PRODEX_WORKFLOW_CLASS_TRANSPORT: Int64 = 5


def launch_workflow_text_equals(address: UInt, length: Int64, literal: StringSlice) -> Bool:
    var expected = Int64(literal.byte_length())
    if address == 0 or length != expected or length < 0 or length > 256:
        return False
    var input = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    var target = literal.unsafe_ptr()
    for index in range(expected):
        if input[unsafe_offset=index] != target[unsafe_offset=index]:
            return False
    return True


def launch_workflow_transient_status(status: Int64) -> Bool:
    return status == -1 or status == 401 or status == 429 or status == 500 or status == 502 or status == 503 or status == 504 or status == 529


@export("prodex_launch_workflow_recovery_class_v1")
def prodex_launch_workflow_recovery_class_v1(
    abi_version: Int64,
    name_address: UInt,
    name_length: Int64,
    status: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_WORKFLOW_CLASS_ABI_VERSION:
        return PRODEX_WORKFLOW_CLASS_STATUS_ABI
    if name_length < 0 or name_length > 256 or (name_length > 0 and name_address == 0) or output_address == 0:
        return PRODEX_WORKFLOW_CLASS_STATUS_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = PRODEX_WORKFLOW_CLASS_NONE
    if launch_workflow_text_equals(name_address, name_length, StringSlice("usage_limit_exceeded")) or launch_workflow_text_equals(name_address, name_length, StringSlice("usageLimitExceeded")):
        output[] = PRODEX_WORKFLOW_CLASS_USAGE
    elif launch_workflow_text_equals(name_address, name_length, StringSlice("rate_limit_exceeded")) or launch_workflow_text_equals(name_address, name_length, StringSlice("rateLimitExceeded")):
        output[] = PRODEX_WORKFLOW_CLASS_RATE
    elif launch_workflow_text_equals(name_address, name_length, StringSlice("server_overloaded")) or launch_workflow_text_equals(name_address, name_length, StringSlice("serverOverloaded")):
        output[] = PRODEX_WORKFLOW_CLASS_OVERLOAD
    elif launch_workflow_text_equals(name_address, name_length, StringSlice("unauthorized")):
        output[] = PRODEX_WORKFLOW_CLASS_AUTH
    elif launch_workflow_transient_status(status):
        if launch_workflow_text_equals(name_address, name_length, StringSlice("internal_server_error")) or launch_workflow_text_equals(name_address, name_length, StringSlice("internalServerError")) or launch_workflow_text_equals(name_address, name_length, StringSlice("http_connection_failed")) or launch_workflow_text_equals(name_address, name_length, StringSlice("httpConnectionFailed")) or launch_workflow_text_equals(name_address, name_length, StringSlice("response_stream_connection_failed")) or launch_workflow_text_equals(name_address, name_length, StringSlice("responseStreamConnectionFailed")) or launch_workflow_text_equals(name_address, name_length, StringSlice("response_stream_disconnected")) or launch_workflow_text_equals(name_address, name_length, StringSlice("responseStreamDisconnected")) or launch_workflow_text_equals(name_address, name_length, StringSlice("response_too_many_failed_attempts")) or launch_workflow_text_equals(name_address, name_length, StringSlice("responseTooManyFailedAttempts")):
            output[] = PRODEX_WORKFLOW_CLASS_TRANSPORT
    return PRODEX_WORKFLOW_CLASS_STATUS_OK


# Native RequestTimeout is serialized as codex_error_info=other. Restrict the
# compatibility projection to exact native error messages in a failed event;
# arbitrary text mentioning timeouts or explicit auth/client errors must not
# authorize a retry or account change.
@export("prodex_launch_workflow_recovery_class_v2")
def prodex_launch_workflow_recovery_class_v2(
    abi_version: Int64,
    name_address: UInt,
    name_length: Int64,
    status: Int64,
    message_address: UInt,
    message_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != 2:
        return PRODEX_WORKFLOW_CLASS_STATUS_ABI
    if message_length < 0 or (message_length > 0 and message_address == 0):
        return PRODEX_WORKFLOW_CLASS_STATUS_INVALID
    var result = prodex_launch_workflow_recovery_class_v1(
        PRODEX_WORKFLOW_CLASS_ABI_VERSION,
        name_address, name_length, status, output_address,
    )
    if result != PRODEX_WORKFLOW_CLASS_STATUS_OK:
        return result
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    if output[] != PRODEX_WORKFLOW_CLASS_NONE:
        return result
    if (
        (status == -1 or status == 408 or status == 504)
        and launch_workflow_text_equals(name_address, name_length, StringSlice("other"))
        and (
            launch_workflow_text_equals(
                message_address, message_length, StringSlice("request timed out")
            )
            or launch_workflow_text_equals(
                message_address, message_length,
                StringSlice("Error running remote compact task: request timed out")
            )
        )
    ):
        output[] = PRODEX_WORKFLOW_CLASS_TRANSPORT
    return PRODEX_WORKFLOW_CLASS_STATUS_OK


# Workflow evidence states: acceptance ambiguous=0, accepted=1, committed=2,
# side effect=3; stream unknown=-1, false=0, true=1; side effect unknown=0,
# none=1, observed=2. Rust supplies only the already parsed event kind.
comptime PRODEX_WORKFLOW_EVIDENCE_ABI_VERSION: Int64 = 1
comptime PRODEX_WORKFLOW_EVIDENCE_STATUS_OK: Int64 = 0
comptime PRODEX_WORKFLOW_EVIDENCE_STATUS_INVALID: Int64 = 1
comptime PRODEX_WORKFLOW_EVIDENCE_STATUS_ABI: Int64 = 4


@export("prodex_launch_workflow_evidence_v1")
def prodex_launch_workflow_evidence_v1(
    abi_version: Int64,
    observation: Int64,
    acceptance_state: Int64,
    stream_state: Int64,
    side_effect_state: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_WORKFLOW_EVIDENCE_ABI_VERSION:
        return PRODEX_WORKFLOW_EVIDENCE_STATUS_ABI
    if observation < 0 or observation > 4 or acceptance_state < 0 or acceptance_state > 3 or stream_state < -1 or stream_state > 1 or side_effect_state < 0 or side_effect_state > 2 or output_address == 0:
        return PRODEX_WORKFLOW_EVIDENCE_STATUS_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = acceptance_state
    output[unsafe_offset=1] = stream_state
    output[unsafe_offset=2] = side_effect_state
    if observation == 1:
        output[unsafe_offset=0] = 0
        output[unsafe_offset=1] = -1
        output[unsafe_offset=2] = 0
    elif observation == 2:
        output[unsafe_offset=0] = 1
        output[unsafe_offset=1] = 0
        output[unsafe_offset=2] = 1
    elif observation == 3:
        output[unsafe_offset=0] = 2
        output[unsafe_offset=1] = 1
    elif observation == 4:
        output[unsafe_offset=0] = 3
        output[unsafe_offset=1] = 1
        output[unsafe_offset=2] = 2
    return PRODEX_WORKFLOW_EVIDENCE_STATUS_OK


# Goal status: absent=0, active=1, blocked=2, usageLimited=3,
# paused=4, budgetLimited=5, complete=6, unknown=7.
# Thread status: unknown=0, idle=1, systemError=2, active=3, notLoaded=4.
# Native SystemError is quiescent: Codex's running work and approval/input
# waits take precedence and are reported as Active. Never recover Active.
# Action: defer=0, stop=1, resume goal=2, continue non-goal turn=3.
@export("prodex_launch_in_place_recovery_v2")
def prodex_launch_in_place_recovery_v2(
    abi_version: Int64,
    verified: Int64,
    thread_status: Int64,
    expected_goal: Int64,
    goal_status: Int64,
    budget_available: Int64,
) abi("C") -> Int64:
    if abi_version != 2:
        return -3
    for flag in [verified, expected_goal, budget_available]:
        if flag != 0 and flag != 1:
            return -2
    if goal_status < 0 or goal_status > 7 or thread_status < 0 or thread_status > 4:
        return -2
    if verified == 0 or (thread_status != 1 and thread_status != 2):
        return 0
    if goal_status == 0:
        return 1 if expected_goal == 1 else 3
    if goal_status >= 4 or budget_available == 0:
        return 1
    return 2


# SessionStart identifies the root session shared by its agent tree. Legacy
# notify identifies the concrete completing thread and inherits the parent's
# hook command. It must not redirect an established root recovery monitor.
# Actions: 0=preserve existing marker/cursor; 1=install a new root marker.
@export("prodex_launch_session_marker_update_v1")
def prodex_launch_session_marker_update_v1(
    abi_version: Int64,
    canonical_session: Int64,
    existing_marker: Int64,
    same_session: Int64,
) abi("C") -> Int64:
    if abi_version != 1:
        return -3
    if (
        (canonical_session != 0 and canonical_session != 1)
        or (existing_marker != 0 and existing_marker != 1)
        or (same_session != 0 and same_session != 1)
        or (existing_marker == 0 and same_session == 1)
    ):
        return -2
    # Keeping the cursor is equally important: repeated notify/compact hooks
    # must not seek beyond an unobserved terminal capacity error.
    if existing_marker == 1 and (same_session == 1 or canonical_session == 0):
        return 0
    return 1
