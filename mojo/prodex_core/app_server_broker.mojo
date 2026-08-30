from std.memory import Pointer

from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView


comptime APP_SERVER_BROKER_ABI_VERSION: Int64 = 1
comptime APP_SERVER_BROKER_METHOD_MAX_BYTES: Int64 = 1_048_576

comptime FRAME_KIND_INVALID: Int64 = 0
comptime FRAME_KIND_REQUEST: Int64 = 2
comptime FRAME_KIND_NOTIFICATION: Int64 = 3
comptime FRAME_KIND_RESPONSE: Int64 = 4

comptime METHOD_KIND_ABSENT: Int64 = 0
comptime METHOD_KIND_LIFECYCLE: Int64 = 1
comptime METHOD_KIND_OTHER: Int64 = 2

comptime STAGE_NONE: Int64 = 0
comptime STAGE_INITIALIZE_REQUEST: Int64 = 1
comptime STAGE_INITIALIZED_NOTIFICATION: Int64 = 2
comptime STAGE_THREAD_START_REQUEST: Int64 = 3
comptime STAGE_THREAD_STARTED_NOTIFICATION: Int64 = 4
comptime STAGE_THREAD_RESUME_REQUEST: Int64 = 5
comptime STAGE_THREAD_FORK_REQUEST: Int64 = 6
comptime STAGE_THREAD_QUEUE_REQUEST: Int64 = 7
comptime STAGE_THREAD_QUEUE_CHANGED_NOTIFICATION: Int64 = 8
comptime STAGE_THREAD_REVERT_REQUEST: Int64 = 9
comptime STAGE_THREAD_REVERTED_NOTIFICATION: Int64 = 10
comptime STAGE_TURN_START_REQUEST: Int64 = 11
comptime STAGE_TURN_STARTED_NOTIFICATION: Int64 = 12
comptime STAGE_TURN_COMPLETED_NOTIFICATION: Int64 = 13
comptime STAGE_TURN_INTERRUPT_REQUEST: Int64 = 14

comptime SCHEMA_NONE: Int64 = 0
comptime SCHEMA_THREAD_START_PARAMS: Int64 = 1
comptime SCHEMA_THREAD_STARTED_NOTIFICATION: Int64 = 2
comptime SCHEMA_THREAD_RESUME_PARAMS: Int64 = 3
comptime SCHEMA_THREAD_FORK_PARAMS: Int64 = 4
comptime SCHEMA_TURN_START_PARAMS: Int64 = 5
comptime SCHEMA_TURN_STARTED_NOTIFICATION: Int64 = 6
comptime SCHEMA_TURN_COMPLETED_NOTIFICATION: Int64 = 7
comptime SCHEMA_TURN_INTERRUPT_PARAMS: Int64 = 8

comptime RESPONSE_SCHEMA_NONE: Int64 = 0
comptime RESPONSE_SCHEMA_THREAD_START: Int64 = 1
comptime RESPONSE_SCHEMA_THREAD_RESUME: Int64 = 2
comptime RESPONSE_SCHEMA_THREAD_FORK: Int64 = 3
comptime RESPONSE_SCHEMA_TURN_START: Int64 = 4
comptime RESPONSE_SCHEMA_TURN_INTERRUPT: Int64 = 5

comptime AFFINITY_SESSION: Int64 = 1
comptime AFFINITY_THREAD: Int64 = 2
comptime AFFINITY_TURN: Int64 = 3

comptime DECISION_FRESH: Int64 = 0
comptime DECISION_SESSION: Int64 = 1
comptime DECISION_THREAD: Int64 = 2
comptime DECISION_TURN: Int64 = 3

comptime REASON_NONE: Int64 = 0
comptime REASON_NON_JSONRPC_VERSION: Int64 = 1
comptime REASON_NON_SCALAR_ID: Int64 = 7
comptime REASON_NON_CONTAINER_PARAMS: Int64 = 8
comptime REASON_NON_OBJECT_ERROR: Int64 = 9
comptime REASON_NON_INTEGER_ERROR_CODE: Int64 = 10
comptime REASON_NON_STRING_ERROR_MESSAGE: Int64 = 11
comptime REASON_NON_STRING_METHOD: Int64 = 12
comptime REASON_INVALID_METHOD_NAME: Int64 = 13
comptime REASON_RESULT_WITH_ERROR: Int64 = 14
comptime REASON_MISSING_RESPONSE_ID: Int64 = 15
comptime REASON_METHOD_WITH_RESULT_OR_ERROR: Int64 = 16
comptime REASON_MISSING_METHOD_AND_RESPONSE_PAYLOAD: Int64 = 17

comptime VALIDATION_MISSING_THREAD_ID: Int64 = 1
comptime VALIDATION_MISSING_THREAD_OBJECT_ID: Int64 = 2
comptime VALIDATION_MISSING_THREAD_CONTEXT: Int64 = 3
comptime VALIDATION_MISSING_THREAD_STATUS: Int64 = 4
comptime VALIDATION_INVALID_THREAD_STATUS: Int64 = 5
comptime VALIDATION_MISSING_TURN_INPUT: Int64 = 6
comptime VALIDATION_MISSING_TURN_ITEMS: Int64 = 7
comptime VALIDATION_INVALID_TURN_STATUS: Int64 = 8
comptime VALIDATION_MISSING_TURN_STATUS: Int64 = 9
comptime VALIDATION_RESPONSE_MISSING_THREAD_ID: Int64 = 10
comptime VALIDATION_RESPONSE_MISSING_THREAD_STATUS: Int64 = 11
comptime VALIDATION_RESPONSE_INVALID_THREAD_STATUS: Int64 = 12
comptime VALIDATION_RESPONSE_MISSING_THREAD_CONTEXT: Int64 = 13
comptime VALIDATION_RESPONSE_INVALID_THREAD_CONTEXT: Int64 = 14
comptime VALIDATION_RESPONSE_MISSING_THREAD_OBJECT_CONTEXT: Int64 = 15
comptime VALIDATION_RESPONSE_MISSING_TURN_ID: Int64 = 16
comptime VALIDATION_RESPONSE_MISSING_TURN_ITEMS: Int64 = 17
comptime VALIDATION_RESPONSE_MISSING_TURN_STATUS: Int64 = 18
comptime VALIDATION_RESPONSE_INVALID_TURN_STATUS: Int64 = 19


def app_server_view_matches[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64, lowercase: Bool
) -> Bool:
    var length = Int64(literal.byte_length())
    if start < 0 or end < start or end - start != length:
        return False
    var expected = literal.unsafe_ptr()
    for index in range(length):
        var value = ptr[unsafe_offset=start + index]
        if lowercase and value >= 65 and value <= 90:
            value += 32
        if value != expected[unsafe_offset=index]:
            return False
    return True


def app_server_view_starts[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64, lowercase: Bool
) -> Bool:
    var length = Int64(literal.byte_length())
    if start < 0 or end < start or end - start < length:
        return False
    var expected = literal.unsafe_ptr()
    for index in range(length):
        var value = ptr[unsafe_offset=start + index]
        if lowercase and value >= 65 and value <= 90:
            value += 32
        if value != expected[unsafe_offset=index]:
            return False
    return True


def app_server_method_is_lifecycle(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return app_server_view_matches["initialize"](ptr, start, end, True) or app_server_view_matches[
        "initialized"
    ](ptr, start, end, True) or app_server_view_matches["notifications/initialized"](
        ptr, start, end, True
    ) or app_server_view_matches["thread/start"](ptr, start, end, True) or app_server_view_matches[
        "thread/started"
    ](ptr, start, end, True) or app_server_view_matches["thread/resume"](
        ptr, start, end, True
    ) or app_server_view_matches["thread/fork"](ptr, start, end, True) or app_server_view_matches[
        "thread/queue/add"
    ](ptr, start, end, True) or app_server_view_matches["thread/queue/list"](
        ptr, start, end, True
    ) or app_server_view_matches["thread/queue/update"](ptr, start, end, True) or app_server_view_matches[
        "thread/queue/delete"
    ](ptr, start, end, True) or app_server_view_matches["thread/queue/reorder"](
        ptr, start, end, True
    ) or app_server_view_matches["thread/queue/start"](ptr, start, end, True) or app_server_view_matches[
        "thread/queue/changed"
    ](ptr, start, end, True) or app_server_view_matches["thread/revert"](
        ptr, start, end, True
    ) or app_server_view_matches["thread/reverted"](ptr, start, end, True) or app_server_view_matches[
        "turn/start"
    ](ptr, start, end, True) or app_server_view_matches["turn/started"](ptr, start, end, True) or app_server_view_matches[
        "turn/completed"
    ](ptr, start, end, True) or app_server_view_matches["turn/interrupt"](
        ptr, start, end, True
    ) or app_server_view_matches["turn/cancel"](ptr, start, end, True)


def app_server_method_stage(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64, frame_kind: Int64
) -> Int64:
    if frame_kind == FRAME_KIND_REQUEST:
        if app_server_view_matches["initialize"](ptr, start, end, True):
            return STAGE_INITIALIZE_REQUEST
        if app_server_view_matches["thread/start"](ptr, start, end, True):
            return STAGE_THREAD_START_REQUEST
        if app_server_view_matches["thread/resume"](ptr, start, end, True):
            return STAGE_THREAD_RESUME_REQUEST
        if app_server_view_matches["thread/fork"](ptr, start, end, True):
            return STAGE_THREAD_FORK_REQUEST
        if (
            app_server_view_matches["thread/queue/add"](ptr, start, end, True)
            or app_server_view_matches["thread/queue/list"](ptr, start, end, True)
            or app_server_view_matches["thread/queue/update"](ptr, start, end, True)
            or app_server_view_matches["thread/queue/delete"](ptr, start, end, True)
            or app_server_view_matches["thread/queue/reorder"](ptr, start, end, True)
            or app_server_view_matches["thread/queue/start"](ptr, start, end, True)
        ):
            return STAGE_THREAD_QUEUE_REQUEST
        if app_server_view_matches["thread/revert"](ptr, start, end, True):
            return STAGE_THREAD_REVERT_REQUEST
        if app_server_view_matches["turn/start"](ptr, start, end, True):
            return STAGE_TURN_START_REQUEST
        if (
            app_server_view_matches["turn/interrupt"](ptr, start, end, True)
            or app_server_view_matches["turn/cancel"](ptr, start, end, True)
        ):
            return STAGE_TURN_INTERRUPT_REQUEST
    elif frame_kind == FRAME_KIND_NOTIFICATION:
        if (
            app_server_view_matches["notifications/initialized"](ptr, start, end, True)
            or app_server_view_matches["initialized"](ptr, start, end, True)
        ):
            return STAGE_INITIALIZED_NOTIFICATION
        if app_server_view_matches["thread/started"](ptr, start, end, True):
            return STAGE_THREAD_STARTED_NOTIFICATION
        if app_server_view_matches["thread/queue/changed"](ptr, start, end, True):
            return STAGE_THREAD_QUEUE_CHANGED_NOTIFICATION
        if app_server_view_matches["thread/reverted"](ptr, start, end, True):
            return STAGE_THREAD_REVERTED_NOTIFICATION
        if app_server_view_matches["turn/started"](ptr, start, end, True):
            return STAGE_TURN_STARTED_NOTIFICATION
        if app_server_view_matches["turn/completed"](ptr, start, end, True):
            return STAGE_TURN_COMPLETED_NOTIFICATION
    return STAGE_NONE


def app_server_schema_for_stage(stage: Int64) -> Int64:
    if stage == STAGE_THREAD_START_REQUEST:
        return SCHEMA_THREAD_START_PARAMS
    if stage == STAGE_THREAD_STARTED_NOTIFICATION:
        return SCHEMA_THREAD_STARTED_NOTIFICATION
    if stage == STAGE_THREAD_RESUME_REQUEST:
        return SCHEMA_THREAD_RESUME_PARAMS
    if stage == STAGE_THREAD_FORK_REQUEST:
        return SCHEMA_THREAD_FORK_PARAMS
    if stage == STAGE_TURN_START_REQUEST:
        return SCHEMA_TURN_START_PARAMS
    if stage == STAGE_TURN_STARTED_NOTIFICATION:
        return SCHEMA_TURN_STARTED_NOTIFICATION
    if stage == STAGE_TURN_COMPLETED_NOTIFICATION:
        return SCHEMA_TURN_COMPLETED_NOTIFICATION
    if stage == STAGE_TURN_INTERRUPT_REQUEST:
        return SCHEMA_TURN_INTERRUPT_PARAMS
    return SCHEMA_NONE


def app_server_method_from_address(method_address: UInt) -> ProdexRichStringView:
    var method_ptr = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(method_address))
    return method_ptr[].copy()


def app_server_method_plan(
    frame_kind: Int64,
    method_kind: Int64,
    method_address: UInt,
    output: Pointer[mut=True, Int64, _],
) -> Int64:
    output[unsafe_offset=0] = METHOD_KIND_ABSENT
    output[unsafe_offset=1] = STAGE_NONE
    output[unsafe_offset=2] = SCHEMA_NONE
    if method_kind == METHOD_KIND_ABSENT:
        return 0
    if method_kind != 1 or method_address == 0:
        return 1
    var method = app_server_method_from_address(method_address)
    if not rich_view_valid(method, APP_SERVER_BROKER_METHOD_MAX_BYTES):
        return 2
    var bounds = rich_trim_bounds(method)
    var ptr = rich_view_ptr(method)
    output[unsafe_offset=0] = METHOD_KIND_OTHER
    if bounds[0] < bounds[1] and app_server_method_is_lifecycle(ptr, bounds[0], bounds[1]):
        output[unsafe_offset=0] = METHOD_KIND_LIFECYCLE
    var stage = app_server_method_stage(ptr, bounds[0], bounds[1], frame_kind)
    output[unsafe_offset=1] = stage
    output[unsafe_offset=2] = app_server_schema_for_stage(stage)
    return 0


def app_server_broker_method_v1_impl(
    abi_version: Int64,
    frame_kind: Int64,
    method_kind: Int64,
    method_address: UInt,
    output_address: UInt,
) abi("C") -> Int64:
    if output_address == 0:
        return 1
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    if abi_version != APP_SERVER_BROKER_ABI_VERSION:
        return 4
    if (
        frame_kind < FRAME_KIND_INVALID
        or frame_kind > FRAME_KIND_RESPONSE
        or method_kind < METHOD_KIND_ABSENT
        or method_kind > METHOD_KIND_OTHER
    ):
        return 1
    return app_server_method_plan(frame_kind, method_kind, method_address, output)


def app_server_broker_response_schema_v1_impl(
    abi_version: Int64, request_stage_address: UInt, output_address: UInt
) abi("C") -> Int64:
    if output_address == 0 or request_stage_address == 0:
        return 1
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = RESPONSE_SCHEMA_NONE
    if abi_version != APP_SERVER_BROKER_ABI_VERSION:
        return 4
    var stage = app_server_method_from_address(request_stage_address)
    if not rich_view_valid(stage, APP_SERVER_BROKER_METHOD_MAX_BYTES):
        return 2
    var bounds = InlineArray[Int64, 2](fill=0)
    bounds[1] = Int64(stage.len)
    var ptr = rich_view_ptr(stage)
    if app_server_view_matches["thread_start_request"](ptr, bounds[0], bounds[1], False):
        output[] = RESPONSE_SCHEMA_THREAD_START
    elif app_server_view_matches["thread_resume_request"](ptr, bounds[0], bounds[1], False):
        output[] = RESPONSE_SCHEMA_THREAD_RESUME
    elif app_server_view_matches["thread_fork_request"](ptr, bounds[0], bounds[1], False):
        output[] = RESPONSE_SCHEMA_THREAD_FORK
    elif app_server_view_matches["turn_start_request"](ptr, bounds[0], bounds[1], False):
        output[] = RESPONSE_SCHEMA_TURN_START
    elif app_server_view_matches["turn_interrupt_request"](ptr, bounds[0], bounds[1], False):
        output[] = RESPONSE_SCHEMA_TURN_INTERRUPT
    return 0


def app_server_broker_wire_v1_impl(
    abi_version: Int64,
    jsonrpc_state: Int64,
    id_kind: Int64,
    params_kind: Int64,
    error_kind: Int64,
    error_code_kind: Int64,
    error_message_kind: Int64,
    method_kind: Int64,
    method_address: UInt,
    has_result: Int64,
    has_error: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if output_address == 0:
        return 1
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = FRAME_KIND_INVALID
    output[unsafe_offset=2] = REASON_NONE
    if abi_version != APP_SERVER_BROKER_ABI_VERSION:
        return 4
    if (
        jsonrpc_state < 0
        or jsonrpc_state > 2
        or id_kind < 0
        or id_kind > 2
        or params_kind < 0
        or params_kind > 2
        or error_kind < 0
        or error_kind > 2
        or error_code_kind < 0
        or error_code_kind > 2
        or error_message_kind < 0
        or error_message_kind > 2
        or method_kind < 0
        or method_kind > 2
        or has_result < 0
        or has_result > 1
        or has_error < 0
        or has_error > 1
    ):
        return 1
    output[unsafe_offset=0] = Int64(jsonrpc_state != 2)
    if jsonrpc_state == 2:
        output[unsafe_offset=2] = REASON_NON_JSONRPC_VERSION
        return 0
    if id_kind == 2:
        output[unsafe_offset=2] = REASON_NON_SCALAR_ID
        return 0
    if params_kind == 2:
        output[unsafe_offset=2] = REASON_NON_CONTAINER_PARAMS
        return 0
    if error_kind == 2:
        output[unsafe_offset=2] = REASON_NON_OBJECT_ERROR
        return 0
    if error_kind == 1 and error_code_kind == 2:
        output[unsafe_offset=2] = REASON_NON_INTEGER_ERROR_CODE
        return 0
    if error_kind == 1 and error_message_kind == 2:
        output[unsafe_offset=2] = REASON_NON_STRING_ERROR_MESSAGE
        return 0
    if method_kind == 2:
        output[unsafe_offset=2] = REASON_NON_STRING_METHOD
        return 0
    if method_kind == 1:
        if method_address == 0:
            return 1
        var method = app_server_method_from_address(method_address)
        if not rich_view_valid(method, APP_SERVER_BROKER_METHOD_MAX_BYTES):
            return 2
        var bounds = rich_trim_bounds(method)
        var ptr = rich_view_ptr(method)
        if bounds[0] == bounds[1] or app_server_view_starts["rpc."](ptr, bounds[0], bounds[1], False):
            output[unsafe_offset=2] = REASON_INVALID_METHOD_NAME
            return 0
    var has_method = method_kind == 1
    var has_id = id_kind != 0
    var has_response_payload = has_result == 1 or has_error == 1
    if has_result == 1 and has_error == 1:
        output[unsafe_offset=2] = REASON_RESULT_WITH_ERROR
        return 0
    if has_method and has_response_payload:
        output[unsafe_offset=2] = REASON_METHOD_WITH_RESULT_OR_ERROR
        return 0
    if not has_method and has_response_payload and not has_id:
        output[unsafe_offset=2] = REASON_MISSING_RESPONSE_ID
        return 0
    if not has_method and not has_response_payload:
        output[unsafe_offset=2] = REASON_MISSING_METHOD_AND_RESPONSE_PAYLOAD
        return 0
    if has_method and has_id:
        output[unsafe_offset=1] = FRAME_KIND_REQUEST
    elif has_method:
        output[unsafe_offset=1] = FRAME_KIND_NOTIFICATION
    else:
        output[unsafe_offset=1] = FRAME_KIND_RESPONSE
    return 0


def app_server_method_is_thread_affinity(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return app_server_view_matches["thread/archive"](ptr, start, end, True) or app_server_view_matches[
        "thread/delete"
    ](ptr, start, end, True) or app_server_view_matches["thread/unarchive"](ptr, start, end, True) or app_server_view_matches[
        "thread/read"
    ](ptr, start, end, True) or app_server_view_matches["thread/rollback"](ptr, start, end, True) or app_server_view_matches[
        "thread/compact/start"
    ](ptr, start, end, True) or app_server_view_matches["thread/settings/update"](
        ptr, start, end, True
    ) or app_server_view_matches["thread/metadata/update"](ptr, start, end, True) or app_server_view_matches[
        "thread/section/move"
    ](ptr, start, end, True) or app_server_view_matches["thread/memorymode/set"](
        ptr, start, end, True
    ) or app_server_view_matches["thread/archived"](ptr, start, end, True) or app_server_view_matches[
        "thread/deleted"
    ](ptr, start, end, True) or app_server_view_matches["thread/unarchived"](ptr, start, end, True) or app_server_view_matches[
        "thread/closed"
    ](ptr, start, end, True) or app_server_view_matches["turn/steer"](ptr, start, end, True)


def app_server_affinity_push(
    output: Pointer[mut=True, Int64, _], count: Int64, kind: Int64, present: Int64
) -> Int64:
    if present == 1:
        output[unsafe_offset=1 + count] = kind
        return count + 1
    return count


def app_server_broker_affinity_v1_impl(
    abi_version: Int64,
    frame_kind: Int64,
    lifecycle_stage: Int64,
    method_address: UInt,
    session_present: Int64,
    thread_present: Int64,
    turn_present: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if output_address == 0:
        return 1
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(9):
        output[unsafe_offset=index] = 0
    if abi_version != APP_SERVER_BROKER_ABI_VERSION:
        return 4
    if (
        frame_kind < FRAME_KIND_INVALID
        or frame_kind > FRAME_KIND_RESPONSE
        or lifecycle_stage < STAGE_NONE
        or lifecycle_stage > STAGE_TURN_INTERRUPT_REQUEST
        or session_present < 0
        or session_present > 1
        or thread_present < 0
        or thread_present > 1
        or turn_present < 0
        or turn_present > 1
    ):
        return 1
    var count: Int64 = 0
    if lifecycle_stage == STAGE_INITIALIZE_REQUEST or lifecycle_stage == STAGE_INITIALIZED_NOTIFICATION:
        count = app_server_affinity_push(output, count, AFFINITY_SESSION, session_present)
    elif lifecycle_stage == STAGE_THREAD_START_REQUEST or lifecycle_stage == STAGE_THREAD_STARTED_NOTIFICATION or lifecycle_stage == STAGE_THREAD_RESUME_REQUEST or lifecycle_stage == STAGE_THREAD_FORK_REQUEST or lifecycle_stage == STAGE_THREAD_QUEUE_REQUEST or lifecycle_stage == STAGE_THREAD_QUEUE_CHANGED_NOTIFICATION or lifecycle_stage == STAGE_THREAD_REVERT_REQUEST or lifecycle_stage == STAGE_THREAD_REVERTED_NOTIFICATION:
        count = app_server_affinity_push(output, count, AFFINITY_THREAD, thread_present)
        count = app_server_affinity_push(output, count, AFFINITY_SESSION, session_present)
    elif lifecycle_stage == STAGE_TURN_START_REQUEST or lifecycle_stage == STAGE_TURN_STARTED_NOTIFICATION or lifecycle_stage == STAGE_TURN_COMPLETED_NOTIFICATION or lifecycle_stage == STAGE_TURN_INTERRUPT_REQUEST:
        count = app_server_affinity_push(output, count, AFFINITY_TURN, turn_present)
        count = app_server_affinity_push(output, count, AFFINITY_THREAD, thread_present)
        count = app_server_affinity_push(output, count, AFFINITY_SESSION, session_present)
    else:
        var thread_method = False
        if (
            method_address != 0
            and (frame_kind == FRAME_KIND_REQUEST or frame_kind == FRAME_KIND_NOTIFICATION)
        ):
            var method = app_server_method_from_address(method_address)
            if not rich_view_valid(method, APP_SERVER_BROKER_METHOD_MAX_BYTES):
                return 2
            var bounds = rich_trim_bounds(method)
            thread_method = bounds[0] < bounds[1] and app_server_method_is_thread_affinity(
                rich_view_ptr(method), bounds[0], bounds[1]
            )
        if thread_method or frame_kind == FRAME_KIND_RESPONSE:
            count = app_server_affinity_push(output, count, AFFINITY_TURN, turn_present)
            count = app_server_affinity_push(output, count, AFFINITY_THREAD, thread_present)
            count = app_server_affinity_push(output, count, AFFINITY_SESSION, session_present)
    output[unsafe_offset=0] = count
    var decision = DECISION_FRESH
    if count > 0:
        var primary = output[unsafe_offset=1]
        if primary == AFFINITY_SESSION:
            decision = DECISION_SESSION
        elif primary == AFFINITY_THREAD:
            decision = DECISION_THREAD
        elif primary == AFFINITY_TURN:
            decision = DECISION_TURN
    output[unsafe_offset=4] = decision
    output[unsafe_offset=5] = decision
    output[unsafe_offset=6] = decision
    var committed = lifecycle_stage == STAGE_TURN_STARTED_NOTIFICATION or lifecycle_stage == STAGE_TURN_COMPLETED_NOTIFICATION or lifecycle_stage == STAGE_TURN_INTERRUPT_REQUEST or frame_kind == FRAME_KIND_RESPONSE and turn_present == 1
    output[unsafe_offset=7] = Int64(committed)
    output[unsafe_offset=8] = Int64(not (decision == DECISION_FRESH and not committed))
    return 0


def app_server_status_kind(status_address: UInt, active_flags_valid: Int64) -> Int64:
    if status_address == 0:
        return 0
    var status = app_server_method_from_address(status_address)
    if not rich_view_valid(status, APP_SERVER_BROKER_METHOD_MAX_BYTES):
        return 0
    var bounds = rich_trim_bounds(status)
    if bounds[0] == bounds[1]:
        return 0
    var ptr = rich_view_ptr(status)
    var active = app_server_view_matches["active"](ptr, bounds[0], bounds[1], False)
    if not (
        active
        or app_server_view_matches["notLoaded"](ptr, bounds[0], bounds[1], False)
        or app_server_view_matches["idle"](ptr, bounds[0], bounds[1], False)
        or app_server_view_matches["systemError"](ptr, bounds[0], bounds[1], False)
    ):
        return 2
    if active and active_flags_valid != 1:
        return 2
    return 1


def app_server_turn_status_kind(status_address: UInt) -> Int64:
    if status_address == 0:
        return 0
    var status = app_server_method_from_address(status_address)
    if not rich_view_valid(status, APP_SERVER_BROKER_METHOD_MAX_BYTES):
        return 0
    var bounds = rich_trim_bounds(status)
    if bounds[0] == bounds[1]:
        return 0
    var ptr = rich_view_ptr(status)
    if (
        app_server_view_matches["completed"](ptr, bounds[0], bounds[1], False)
        or app_server_view_matches["interrupted"](ptr, bounds[0], bounds[1], False)
        or app_server_view_matches["failed"](ptr, bounds[0], bounds[1], False)
        or app_server_view_matches["inProgress"](ptr, bounds[0], bounds[1], False)
    ):
        return 1
    return 2


def app_server_broker_validation_v1_impl(
    abi_version: Int64,
    response: Int64,
    stage_address: UInt,
    thread_id_present: Int64,
    thread_object_id_present: Int64,
    thread_status_address: UInt,
    thread_active_flags_valid: Int64,
    thread_object_context: Int64,
    response_thread_context: Int64,
    response_thread_context_valid: Int64,
    response_thread_object_context: Int64,
    turn_input: Int64,
    turn_id_present: Int64,
    turn_status_address: UInt,
    turn_items: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if output_address == 0 or stage_address == 0:
        return 1
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = 0
    if abi_version != APP_SERVER_BROKER_ABI_VERSION:
        return 4
    if (
        response < 0
        or response > 1
        or thread_id_present < 0
        or thread_id_present > 1
        or thread_object_id_present < 0
        or thread_object_id_present > 1
        or thread_active_flags_valid < 0
        or thread_active_flags_valid > 1
        or thread_object_context < 0
        or thread_object_context > 1
        or response_thread_context < 0
        or response_thread_context > 1
        or response_thread_context_valid < 0
        or response_thread_context_valid > 1
        or response_thread_object_context < 0
        or response_thread_object_context > 1
        or turn_input < 0
        or turn_input > 1
        or turn_id_present < 0
        or turn_id_present > 1
        or turn_items < 0
        or turn_items > 1
    ):
        return 1
    var stage = app_server_method_from_address(stage_address)
    if not rich_view_valid(stage, APP_SERVER_BROKER_METHOD_MAX_BYTES):
        return 2
    var thread_status = app_server_status_kind(thread_status_address, thread_active_flags_valid)
    var turn_status = app_server_turn_status_kind(turn_status_address)
    var bounds = rich_trim_bounds(stage)
    var ptr = rich_view_ptr(stage)
    if response == 1:
        if (
            app_server_view_matches["thread_start_request"](ptr, bounds[0], bounds[1], False)
            or app_server_view_matches["thread_resume_request"](ptr, bounds[0], bounds[1], False)
            or app_server_view_matches["thread_fork_request"](ptr, bounds[0], bounds[1], False)
        ):
            if thread_id_present == 0:
                output[] = VALIDATION_RESPONSE_MISSING_THREAD_ID
            elif thread_status == 0:
                output[] = VALIDATION_RESPONSE_MISSING_THREAD_STATUS
            elif thread_status == 2:
                output[] = VALIDATION_RESPONSE_INVALID_THREAD_STATUS
            elif response_thread_context == 0:
                output[] = VALIDATION_RESPONSE_MISSING_THREAD_CONTEXT
            elif response_thread_context_valid == 0:
                output[] = VALIDATION_RESPONSE_INVALID_THREAD_CONTEXT
            elif response_thread_object_context == 0:
                output[] = VALIDATION_RESPONSE_MISSING_THREAD_OBJECT_CONTEXT
        elif app_server_view_matches["turn_start_request"](ptr, bounds[0], bounds[1], False):
            if turn_id_present == 0:
                output[] = VALIDATION_RESPONSE_MISSING_TURN_ID
            elif turn_status == 0:
                output[] = VALIDATION_RESPONSE_MISSING_TURN_STATUS
            elif turn_status == 2:
                output[] = VALIDATION_RESPONSE_INVALID_TURN_STATUS
            elif turn_items == 0:
                output[] = VALIDATION_RESPONSE_MISSING_TURN_ITEMS
    else:
        if (
            app_server_view_matches["thread_started_notification"](ptr, bounds[0], bounds[1], False)
            or app_server_view_matches["thread_resume_request"](ptr, bounds[0], bounds[1], False)
            or app_server_view_matches["thread_fork_request"](ptr, bounds[0], bounds[1], False)
            or app_server_view_matches["turn_start_request"](ptr, bounds[0], bounds[1], False)
        ) and thread_id_present == 0:
            output[] = VALIDATION_MISSING_THREAD_ID
        elif app_server_view_matches["thread_started_notification"](ptr, bounds[0], bounds[1], False):
            if thread_object_id_present == 0:
                output[] = VALIDATION_MISSING_THREAD_OBJECT_ID
            elif thread_status == 0:
                output[] = VALIDATION_MISSING_THREAD_STATUS
            elif thread_status == 2:
                output[] = VALIDATION_INVALID_THREAD_STATUS
            elif thread_object_context == 0:
                output[] = VALIDATION_MISSING_THREAD_CONTEXT
        elif app_server_view_matches["turn_start_request"](ptr, bounds[0], bounds[1], False):
            if turn_input == 0:
                output[] = VALIDATION_MISSING_TURN_INPUT
        elif (
            app_server_view_matches["turn_started_notification"](ptr, bounds[0], bounds[1], False)
            or app_server_view_matches["turn_completed_notification"](ptr, bounds[0], bounds[1], False)
        ):
            if turn_status == 0:
                output[] = VALIDATION_MISSING_TURN_STATUS
            elif turn_status == 2:
                output[] = VALIDATION_INVALID_TURN_STATUS
            elif turn_items == 0:
                output[] = VALIDATION_MISSING_TURN_ITEMS
    return 0
