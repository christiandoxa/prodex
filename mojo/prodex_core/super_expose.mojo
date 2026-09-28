from std.collections import Array

from std.memory import Pointer

from rich_text import (
    rich_codepoint,
    rich_codepoint_width,
    rich_trim_bounds,
    rich_view_matches_literal,
    rich_view_ptr,
    rich_view_valid,
)
from rich_types import ProdexRichStringView

comptime SUPER_EXPOSE_ABI_VERSION: Int64 = 1
comptime SUPER_EXPOSE_MAX_NAME_BYTES: Int64 = 128

comptime SUPER_EXPOSE_METHOD_UNKNOWN: Int64 = 0
comptime SUPER_EXPOSE_METHOD_INITIALIZE: Int64 = 1
comptime SUPER_EXPOSE_METHOD_PING: Int64 = 2
comptime SUPER_EXPOSE_METHOD_TOOLS_LIST: Int64 = 3
comptime SUPER_EXPOSE_METHOD_TOOLS_CALL: Int64 = 4
comptime SUPER_EXPOSE_METHOD_NOTIFICATION: Int64 = 5
comptime SUPER_EXPOSE_METHOD_SERVER_DISCOVER: Int64 = 6

comptime SUPER_EXPOSE_TOOL_UNKNOWN: Int64 = 0
comptime SUPER_EXPOSE_TOOL_START: Int64 = 1
comptime SUPER_EXPOSE_TOOL_STATUS: Int64 = 2
comptime SUPER_EXPOSE_TOOL_RESULT: Int64 = 3
comptime SUPER_EXPOSE_TOOL_CANCEL: Int64 = 4
comptime SUPER_EXPOSE_TOOL_LIST: Int64 = 5
comptime SUPER_EXPOSE_TOOL_EXEC: Int64 = 6
comptime SUPER_EXPOSE_TOOL_EVENTS: Int64 = 7
comptime SUPER_EXPOSE_TOOL_SESSION_PROMPT_WRITE: Int64 = 8
comptime SUPER_EXPOSE_TOOL_SESSION_PREEMPT: Int64 = 9
comptime SUPER_EXPOSE_TOOL_SESSION_OUTPUT_READ: Int64 = 10

def super_expose_method(view: ProdexRichStringView) -> Int64:
    if rich_view_matches_literal["server/discover"](view, False):
        return SUPER_EXPOSE_METHOD_SERVER_DISCOVER
    if rich_view_matches_literal["initialize"](view, False):
        return SUPER_EXPOSE_METHOD_INITIALIZE
    if rich_view_matches_literal["ping"](view, False):
        return SUPER_EXPOSE_METHOD_PING
    if rich_view_matches_literal["tools/list"](view, False):
        return SUPER_EXPOSE_METHOD_TOOLS_LIST
    if rich_view_matches_literal["tools/call"](view, False):
        return SUPER_EXPOSE_METHOD_TOOLS_CALL
    if (
        rich_view_matches_literal["notifications/initialized"](view, False)
        or rich_view_matches_literal["notifications/cancelled"](view, False)
    ):
        return SUPER_EXPOSE_METHOD_NOTIFICATION
    return SUPER_EXPOSE_METHOD_UNKNOWN

def super_expose_tool(view: ProdexRichStringView) -> Int64:
    if rich_view_matches_literal["prodex_super_start"](view, False):
        return SUPER_EXPOSE_TOOL_START
    if rich_view_matches_literal["prodex_super_status"](view, False):
        return SUPER_EXPOSE_TOOL_STATUS
    if rich_view_matches_literal["prodex_super_result"](view, False):
        return SUPER_EXPOSE_TOOL_RESULT
    if rich_view_matches_literal["prodex_super_cancel"](view, False):
        return SUPER_EXPOSE_TOOL_CANCEL
    if rich_view_matches_literal["prodex_super_list"](view, False):
        return SUPER_EXPOSE_TOOL_LIST
    if rich_view_matches_literal["prodex_super_exec"](view, False):
        return SUPER_EXPOSE_TOOL_EXEC
    if rich_view_matches_literal["prodex_super_events"](view, False):
        return SUPER_EXPOSE_TOOL_EVENTS
    if rich_view_matches_literal["prodex_session_prompt_write"](view, False):
        return SUPER_EXPOSE_TOOL_SESSION_PROMPT_WRITE
    if rich_view_matches_literal["prodex_session_preempt"](view, False):
        return SUPER_EXPOSE_TOOL_SESSION_PREEMPT
    if rich_view_matches_literal["prodex_session_output_read"](view, False):
        return SUPER_EXPOSE_TOOL_SESSION_OUTPUT_READ
    return SUPER_EXPOSE_TOOL_UNKNOWN

@export("prodex_mojo_super_expose_route_v1")
def prodex_mojo_super_expose_route_v1(
    abi_version: Int64,
    method_address: UInt,
    method_length: Int64,
    tool_address: UInt,
    tool_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != SUPER_EXPOSE_ABI_VERSION:
        return 4
    if (
        method_length < 0
        or method_length > SUPER_EXPOSE_MAX_NAME_BYTES
        or tool_length < 0
        or tool_length > SUPER_EXPOSE_MAX_NAME_BYTES
        or output_address == 0
        or (method_length > 0 and method_address == 0)
        or (tool_length > 0 and tool_address == 0)
    ):
        return 1
    var method = ProdexRichStringView(method_address, UInt(method_length))
    var tool = ProdexRichStringView(tool_address, UInt(tool_length))
    if (
        not rich_view_valid(method, SUPER_EXPOSE_MAX_NAME_BYTES)
        or not rich_view_valid(tool, SUPER_EXPOSE_MAX_NAME_BYTES)
    ):
        return 2
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = super_expose_method(method)
    output[unsafe_offset=1] = super_expose_tool(tool)
    return 0

comptime SUPER_EXPOSE_MODE_FULL: Int64 = 0
comptime SUPER_EXPOSE_MODE_EXEC: Int64 = 1

@export("prodex_mojo_super_expose_tool_allowed_v1")
def prodex_mojo_super_expose_tool_allowed_v1(
    abi_version: Int64,
    mode: Int64,
    tool_address: UInt,
    tool_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != SUPER_EXPOSE_ABI_VERSION:
        return 4
    if (
        mode < SUPER_EXPOSE_MODE_FULL
        or mode > SUPER_EXPOSE_MODE_EXEC
        or tool_length < 0
        or tool_length > SUPER_EXPOSE_MAX_NAME_BYTES
        or output_address == 0
        or (tool_length > 0 and tool_address == 0)
    ):
        return 1
    var tool = ProdexRichStringView(tool_address, UInt(tool_length))
    if not rich_view_valid(tool, SUPER_EXPOSE_MAX_NAME_BYTES):
        return 2
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var tool_kind = super_expose_tool(tool)
    if mode == SUPER_EXPOSE_MODE_EXEC:
        output[] = 1 if tool_kind == SUPER_EXPOSE_TOOL_EXEC else 0
    else:
        output[] = 1 if tool_kind != SUPER_EXPOSE_TOOL_UNKNOWN else 0
    return 0


comptime SUPER_EXPOSE_TUNNEL_ID_BYTES: Int64 = 39
comptime SUPER_EXPOSE_TUNNEL_PREFIX_BYTES: Int64 = 7

def super_expose_tunnel_id_valid(view: ProdexRichStringView) -> Bool:
    if Int64(view.len) != SUPER_EXPOSE_TUNNEL_ID_BYTES or view.ptr == 0:
        return False
    var ptr = rich_view_ptr(view)
    var prefix = StringSlice("tunnel_").unsafe_ptr()
    for index in range(SUPER_EXPOSE_TUNNEL_PREFIX_BYTES):
        if ptr[unsafe_offset=index] != prefix[unsafe_offset=index]:
            return False
    for index in range(SUPER_EXPOSE_TUNNEL_PREFIX_BYTES, SUPER_EXPOSE_TUNNEL_ID_BYTES):
        var byte = ptr[unsafe_offset=index]
        var lowercase = byte >= 97 and byte <= 122
        var digit = byte >= 48 and byte <= 57
        if not lowercase and not digit:
            return False
    return True

@export("prodex_mojo_super_expose_tunnel_id_valid_v1")
def prodex_mojo_super_expose_tunnel_id_valid_v1(
    abi_version: Int64,
    value_address: UInt,
    value_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != SUPER_EXPOSE_ABI_VERSION:
        return 4
    if (
        value_length < 0
        or value_length > SUPER_EXPOSE_MAX_NAME_BYTES
        or output_address == 0
        or (value_length > 0 and value_address == 0)
    ):
        return 1
    var value = ProdexRichStringView(value_address, UInt(value_length))
    if not rich_view_valid(value, value_length):
        return 2
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = 1 if super_expose_tunnel_id_valid(value) else 0
    return 0



def super_expose_ascii_trim_bounds(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Array[Int64, 2]:
    var left = start
    var right = end
    var ptr = rich_view_ptr(view)
    while left < right:
        var byte = ptr[unsafe_offset=left]
        if byte == 9 or byte == 10 or byte == 13 or byte == 32:
            left += 1
        else:
            break
    while right > left:
        var byte = ptr[unsafe_offset=right - 1]
        if byte == 9 or byte == 10 or byte == 13 or byte == 32:
            right -= 1
        else:
            break
    var bounds = Array[Int64, 2](fill=0)
    bounds[0] = left
    bounds[1] = right
    return bounds^

def super_expose_range_matches_literal(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    var length = end - start
    if length != Int64(literal.byte_length()) or start < 0:
        return False
    var ptr = rich_view_ptr(view)
    var expected = literal.unsafe_ptr()
    for index in range(length):
        if ptr[unsafe_offset=start + index] != expected[unsafe_offset=index]:
            return False
    return True

def super_expose_ascii_digit(byte: UInt8) -> Bool:
    return byte >= 48 and byte <= 57

def super_expose_ascii_hex(byte: UInt8) -> Bool:
    return (
        (byte >= 48 and byte <= 57)
        or (byte >= 97 and byte <= 102)
        or (byte >= 65 and byte <= 70)
    )

def super_expose_tunnel_client_version_line_valid(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    if start < 0 or end <= start:
        return False
    var ptr = rich_view_ptr(view)
    var cursor = start
    for component in range(3):
        var digits: Int64 = 0
        while cursor < end and super_expose_ascii_digit(ptr[unsafe_offset=cursor]):
            cursor += 1
            digits += 1
        if digits == 0:
            return False
        if component < 2:
            if cursor >= end or ptr[unsafe_offset=cursor] != 46:
                return False
            cursor += 1
    if cursor >= end or ptr[unsafe_offset=cursor] != 43:
        return False
    cursor += 1
    var first_sha = cursor
    for _ in range(40):
        if cursor >= end or not super_expose_ascii_hex(ptr[unsafe_offset=cursor]):
            return False
        cursor += 1
    var marker = StringSlice(" (git sha: ")
    var marker_len = Int64(marker.byte_length())
    if not super_expose_range_matches_literal(
        view, cursor, cursor + marker_len, marker
    ):
        return False
    cursor += marker_len
    var second_sha = cursor
    for index in range(40):
        if cursor >= end or not super_expose_ascii_hex(ptr[unsafe_offset=cursor]):
            return False
        var sha_index = Int64(index)
        if ptr[unsafe_offset=first_sha + sha_index] != ptr[unsafe_offset=second_sha + sha_index]:
            return False
        cursor += 1
    if cursor >= end or ptr[unsafe_offset=cursor] != 41:
        return False
    cursor += 1
    return cursor == end

def super_expose_tunnel_client_version_output_valid(
    view: ProdexRichStringView,
) -> Bool:
    var length = Int64(view.len)
    var line_start: Int64 = 0
    var cursor: Int64 = 0
    while cursor <= length:
        if cursor == length or rich_view_ptr(view)[unsafe_offset=cursor] == 10:
            var bounds = super_expose_ascii_trim_bounds(view, line_start, cursor)
            if super_expose_tunnel_client_version_line_valid(
                view, bounds[0], bounds[1]
            ):
                return True
            line_start = cursor + 1
        cursor += 1
    return False

@export("prodex_mojo_super_expose_tunnel_client_version_valid_v1")
def prodex_mojo_super_expose_tunnel_client_version_valid_v1(
    abi_version: Int64,
    value_address: UInt,
    value_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != SUPER_EXPOSE_ABI_VERSION:
        return 4
    if (
        value_length < 0
        or value_length > 16_384
        or output_address == 0
        or (value_length > 0 and value_address == 0)
    ):
        return 1
    var value = ProdexRichStringView(value_address, UInt(value_length))
    if not rich_view_valid(value, value_length):
        return 2
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = (
        1 if super_expose_tunnel_client_version_output_valid(value) else 0
    )
    return 0


comptime SUPER_EXPOSE_PROTOCOL_OK: Int64 = 0
comptime SUPER_EXPOSE_PROTOCOL_UNSUPPORTED: Int64 = 1
comptime SUPER_EXPOSE_PROTOCOL_VERSION_MISMATCH: Int64 = 2
comptime SUPER_EXPOSE_PROTOCOL_METADATA_REQUIRED: Int64 = 3
comptime SUPER_EXPOSE_PROTOCOL_METHOD_MISMATCH: Int64 = 4
comptime SUPER_EXPOSE_PROTOCOL_NAME_MISMATCH: Int64 = 5
comptime SUPER_EXPOSE_CURRENT_PROTOCOL = "2026-07-28"


def super_expose_protocol_supported(view: ProdexRichStringView) -> Bool:
    return (
        rich_view_matches_literal["2026-07-28"](view, False)
        or rich_view_matches_literal["2025-11-25"](view, False)
        or rich_view_matches_literal["2025-06-18"](view, False)
        or rich_view_matches_literal["2025-03-26"](view, False)
        or rich_view_matches_literal["2024-11-05"](view, False)
    )


def super_expose_optional_view(
    present: Int64, address: UInt, length: Int64
) -> ProdexRichStringView:
    if present == 0:
        return ProdexRichStringView(UInt(0), UInt(0))
    return ProdexRichStringView(address, UInt(length))


def super_expose_optional_equal(
    left_present: Int64,
    left: ProdexRichStringView,
    right_present: Int64,
    right: ProdexRichStringView,
) -> Bool:
    if left_present != right_present:
        return False
    if left_present == 0:
        return True
    if left.len != right.len:
        return False
    var left_ptr = rich_view_ptr(left)
    var right_ptr = rich_view_ptr(right)
    for index in range(Int64(left.len)):
        if left_ptr[unsafe_offset=index] != right_ptr[unsafe_offset=index]:
            return False
    return True



comptime SUPER_EXPOSE_DISPATCH_OK: Int64 = 0
comptime SUPER_EXPOSE_DISPATCH_NOTIFICATION_ACCEPTED: Int64 = 1
comptime SUPER_EXPOSE_DISPATCH_NOTIFICATION_UNSUPPORTED: Int64 = 2
comptime SUPER_EXPOSE_DISPATCH_INVALID_REQUEST_ID: Int64 = 3
comptime SUPER_EXPOSE_DISPATCH_INITIALIZE_PARAMS_REQUIRED: Int64 = 4
comptime SUPER_EXPOSE_DISPATCH_PROTOCOL_VERSION_REQUIRED: Int64 = 5
comptime SUPER_EXPOSE_DISPATCH_TOOL_PARAMS_REQUIRED: Int64 = 6
comptime SUPER_EXPOSE_DISPATCH_TOOL_NAME_REQUIRED: Int64 = 7
comptime SUPER_EXPOSE_DISPATCH_TOOL_ARGUMENTS_OBJECT_REQUIRED: Int64 = 8


@export("prodex_mojo_super_expose_dispatch_validation_v1")
def prodex_mojo_super_expose_dispatch_validation_v1(
    abi_version: Int64,
    method_address: UInt,
    method_length: Int64,
    has_id_field: Int64,
    id_valid: Int64,
    params_is_object: Int64,
    protocol_version_present: Int64,
    tool_name_present: Int64,
    tool_arguments_kind: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != SUPER_EXPOSE_ABI_VERSION:
        return 4
    if (
        method_length < 0
        or method_length > SUPER_EXPOSE_MAX_NAME_BYTES
        or (method_length > 0 and method_address == 0)
        or (has_id_field != 0 and has_id_field != 1)
        or (id_valid != 0 and id_valid != 1)
        or (params_is_object != 0 and params_is_object != 1)
        or (protocol_version_present != 0 and protocol_version_present != 1)
        or (tool_name_present != 0 and tool_name_present != 1)
        or tool_arguments_kind < 0
        or tool_arguments_kind > 2
        or output_address == 0
    ):
        return 1

    var method = ProdexRichStringView(method_address, UInt(method_length))
    if not rich_view_valid(method, SUPER_EXPOSE_MAX_NAME_BYTES):
        return 2

    var decision = SUPER_EXPOSE_DISPATCH_OK
    if has_id_field == 0:
        decision = (
            SUPER_EXPOSE_DISPATCH_NOTIFICATION_ACCEPTED
            if (
                rich_view_matches_literal["notifications/initialized"](method, False)
                or rich_view_matches_literal["notifications/cancelled"](method, False)
            )
            else SUPER_EXPOSE_DISPATCH_NOTIFICATION_UNSUPPORTED
        )
    elif id_valid == 0:
        decision = SUPER_EXPOSE_DISPATCH_INVALID_REQUEST_ID
    else:
        var method_kind = super_expose_method(method)
        if method_kind == SUPER_EXPOSE_METHOD_INITIALIZE:
            if params_is_object == 0:
                decision = SUPER_EXPOSE_DISPATCH_INITIALIZE_PARAMS_REQUIRED
            elif protocol_version_present == 0:
                decision = SUPER_EXPOSE_DISPATCH_PROTOCOL_VERSION_REQUIRED
        elif method_kind == SUPER_EXPOSE_METHOD_TOOLS_CALL:
            if params_is_object == 0:
                decision = SUPER_EXPOSE_DISPATCH_TOOL_PARAMS_REQUIRED
            elif tool_name_present == 0:
                decision = SUPER_EXPOSE_DISPATCH_TOOL_NAME_REQUIRED
            elif tool_arguments_kind == 2:
                decision = SUPER_EXPOSE_DISPATCH_TOOL_ARGUMENTS_OBJECT_REQUIRED

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = decision
    return 0


@export("prodex_mojo_super_expose_protocol_version_supported_v1")
def prodex_mojo_super_expose_protocol_version_supported_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != SUPER_EXPOSE_ABI_VERSION:
        return 4
    if (
        length < 0
        or output_address == 0
        or (length > 0 and address == 0)
    ):
        return 1
    var value = ProdexRichStringView(address, UInt(length))
    if not rich_view_valid(value, length):
        return 2
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = Int64(super_expose_protocol_supported(value))
    return 0


@export("prodex_mojo_super_expose_protocol_metadata_v1")
def prodex_mojo_super_expose_protocol_metadata_v1(
    abi_version: Int64,
    method_address: UInt,
    method_length: Int64,
    header_present: Int64,
    header_address: UInt,
    header_length: Int64,
    body_present: Int64,
    body_address: UInt,
    body_length: Int64,
    method_header_present: Int64,
    method_header_address: UInt,
    method_header_length: Int64,
    name_header_present: Int64,
    name_header_address: UInt,
    name_header_length: Int64,
    body_name_present: Int64,
    body_name_address: UInt,
    body_name_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != SUPER_EXPOSE_ABI_VERSION:
        return 4
    if (
        method_length < 0
        or method_address == 0
        or output_address == 0
        or (header_present != 0 and header_present != 1)
        or (body_present != 0 and body_present != 1)
        or (method_header_present != 0 and method_header_present != 1)
        or (name_header_present != 0 and name_header_present != 1)
        or (body_name_present != 0 and body_name_present != 1)
        or header_length < 0
        or body_length < 0
        or method_header_length < 0
        or name_header_length < 0
        or body_name_length < 0
        or (header_present == 1 and header_length > 0 and header_address == 0)
        or (body_present == 1 and body_length > 0 and body_address == 0)
        or (
            method_header_present == 1
            and method_header_length > 0
            and method_header_address == 0
        )
        or (
            name_header_present == 1
            and name_header_length > 0
            and name_header_address == 0
        )
        or (
            body_name_present == 1
            and body_name_length > 0
            and body_name_address == 0
        )
    ):
        return 1

    var method = ProdexRichStringView(method_address, UInt(method_length))
    var header = super_expose_optional_view(
        header_present, header_address, header_length
    )
    var body = super_expose_optional_view(body_present, body_address, body_length)
    var method_header = super_expose_optional_view(
        method_header_present, method_header_address, method_header_length
    )
    var name_header = super_expose_optional_view(
        name_header_present, name_header_address, name_header_length
    )
    var body_name = super_expose_optional_view(
        body_name_present, body_name_address, body_name_length
    )

    if not rich_view_valid(method, method_length):
        return 2
    if header_present == 1 and not rich_view_valid(header, header_length):
        return 2
    if body_present == 1 and not rich_view_valid(body, body_length):
        return 2
    if method_header_present == 1 and not rich_view_valid(
        method_header, method_header_length
    ):
        return 2
    if name_header_present == 1 and not rich_view_valid(
        name_header, name_header_length
    ):
        return 2
    if body_name_present == 1 and not rich_view_valid(
        body_name, body_name_length
    ):
        return 2

    var decision = SUPER_EXPOSE_PROTOCOL_OK
    if header_present == 1:
        if not super_expose_protocol_supported(header):
            decision = SUPER_EXPOSE_PROTOCOL_UNSUPPORTED
    elif body_present == 1:
        if not super_expose_protocol_supported(body):
            decision = SUPER_EXPOSE_PROTOCOL_UNSUPPORTED

    if decision == SUPER_EXPOSE_PROTOCOL_OK and (
        header_present == 1
        and body_present == 1
        and not super_expose_optional_equal(1, header, 1, body)
    ):
        decision = SUPER_EXPOSE_PROTOCOL_VERSION_MISMATCH

    var current = (
        header_present == 1
        and rich_view_matches_literal["2026-07-28"](header, False)
    ) or (
        body_present == 1
        and rich_view_matches_literal["2026-07-28"](body, False)
    )

    if decision == SUPER_EXPOSE_PROTOCOL_OK:
        if current:
            if not (
                header_present == 1
                and body_present == 1
                and rich_view_matches_literal["2026-07-28"](
                    header, False
                )
                and rich_view_matches_literal["2026-07-28"](
                    body, False
                )
            ):
                decision = SUPER_EXPOSE_PROTOCOL_METADATA_REQUIRED
            elif not (
                method_header_present == 1
                and super_expose_optional_equal(
                    1, method_header, 1, method
                )
            ):
                decision = SUPER_EXPOSE_PROTOCOL_METHOD_MISMATCH
            elif rich_view_matches_literal["tools/call"](method, False) and not (
                super_expose_optional_equal(
                    name_header_present,
                    name_header,
                    body_name_present,
                    body_name,
                )
            ):
                decision = SUPER_EXPOSE_PROTOCOL_NAME_MISMATCH
        elif method_header_present == 1 and not super_expose_optional_equal(
            1, method_header, 1, method
        ):
            decision = SUPER_EXPOSE_PROTOCOL_METHOD_MISMATCH

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = decision
    return 0


def super_expose_ascii_lower(byte: UInt8) -> UInt8:
    if byte >= 65 and byte <= 90:
        return byte + 32
    return byte


def super_expose_range_equals_ascii_ignore_case(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    var n = Int64(literal.byte_length())
    if start < 0 or end < start or end - start != n:
        return False
    var ptr = rich_view_ptr(view)
    var wanted = literal.unsafe_ptr()
    for index in range(n):
        if super_expose_ascii_lower(ptr[unsafe_offset=start + index]) != (
            super_expose_ascii_lower(wanted[unsafe_offset=index])
        ):
            return False
    return True


def super_expose_unicode_trim_range(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Array[Int64, 2]:
    var result = Array[Int64, 2](fill=0)
    if start < 0 or end < start or end > Int64(view.len):
        result[0] = start
        result[1] = start
        return result^
    var subview = ProdexRichStringView(
        UInt(Int(view.ptr) + Int(start)), UInt(end - start)
    )
    var bounds = rich_trim_bounds(subview)
    result[0] = start + bounds[0]
    result[1] = start + bounds[1]
    return result^


def super_expose_media_part_is_json(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    var semicolon = end
    var ptr = rich_view_ptr(view)
    for index in range(start, end):
        if ptr[unsafe_offset=index] == 59:
            semicolon = index
            break
    var bounds = super_expose_unicode_trim_range(view, start, semicolon)
    return super_expose_range_equals_ascii_ignore_case(
        view, bounds[0], bounds[1], StringSlice("application/json")
    )


@export("prodex_mojo_super_expose_media_header_v1")
def prodex_mojo_super_expose_media_header_v1(
    abi_version: Int64,
    kind: Int64,
    present: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != SUPER_EXPOSE_ABI_VERSION:
        return 4
    if (
        kind < 0
        or kind > 1
        or (present != 0 and present != 1)
        or length < 0
        or output_address == 0
        or (present == 1 and length > 0 and address == 0)
    ):
        return 1
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    if present == 0:
        output[] = 0
        return 0
    var value = ProdexRichStringView(address, UInt(length))
    if not rich_view_valid(value, length):
        return 2

    if kind == 0:
        output[] = Int64(
            super_expose_media_part_is_json(value, 0, length)
        )
        return 0

    var ptr = rich_view_ptr(value)
    var start: Int64 = 0
    while start <= length:
        var end = start
        while end < length and ptr[unsafe_offset=end] != 44:
            end += 1
        if super_expose_media_part_is_json(value, start, end):
            output[] = 1
            return 0
        if end >= length:
            break
        start = end + 1
    output[] = 0
    return 0


@export("prodex_mojo_super_expose_json_nesting_v1")
def prodex_mojo_super_expose_json_nesting_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
    limit: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != SUPER_EXPOSE_ABI_VERSION:
        return 4
    if (
        length < 0
        or length > 16 * 1024 * 1024
        or limit < 0
        or output_address == 0
        or (length > 0 and address == 0)
    ):
        return 1
    var ptr = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    var depth: Int64 = 0
    var escaped = False
    var in_string = False
    var valid = True
    for index in range(length):
        var byte = ptr[unsafe_offset=index]
        if in_string:
            if escaped:
                escaped = False
            elif byte == 92:
                escaped = True
            elif byte == 34:
                in_string = False
            continue
        if byte == 34:
            in_string = True
        elif byte == 123 or byte == 91:
            depth += 1
            if depth > limit:
                valid = False
                break
        elif byte == 125 or byte == 93:
            if depth <= 0:
                valid = False
                break
            depth -= 1
    if in_string or escaped or depth != 0:
        valid = False

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = Int64(valid)
    return 0


def super_expose_tool_argument_allowed(
    tool_kind: Int64,
    key: ProdexRichStringView,
) -> Bool:
    if tool_kind == SUPER_EXPOSE_TOOL_UNKNOWN:
        return True
    if tool_kind == SUPER_EXPOSE_TOOL_START:
        return (
            rich_view_matches_literal["task"](key, False)
            or rich_view_matches_literal["model"](key, False)
            or rich_view_matches_literal["reasoning_effort"](key, False)
            or rich_view_matches_literal["provider"](key, False)
            or rich_view_matches_literal["profile"](key, False)
            or rich_view_matches_literal["sub_agents"](key, False)
        )
    if (
        tool_kind == SUPER_EXPOSE_TOOL_STATUS
        or tool_kind == SUPER_EXPOSE_TOOL_RESULT
        or tool_kind == SUPER_EXPOSE_TOOL_CANCEL
    ):
        return rich_view_matches_literal["run_id"](key, False)
    if tool_kind == SUPER_EXPOSE_TOOL_EVENTS:
        return (
            rich_view_matches_literal["run_id"](key, False)
            or rich_view_matches_literal["after_seq"](key, False)
            or rich_view_matches_literal["limit"](key, False)
        )
    if tool_kind == SUPER_EXPOSE_TOOL_LIST:
        return False
    if tool_kind == SUPER_EXPOSE_TOOL_EXEC:
        return (
            rich_view_matches_literal["program"](key, False)
            or rich_view_matches_literal["args"](key, False)
            or rich_view_matches_literal["cwd"](key, False)
            or rich_view_matches_literal["env"](key, False)
            or rich_view_matches_literal["stdin"](key, False)
            or rich_view_matches_literal["timeout_ms"](key, False)
        )
    if tool_kind == SUPER_EXPOSE_TOOL_SESSION_PROMPT_WRITE:
        return (
            rich_view_matches_literal["message"](key, False)
            or rich_view_matches_literal["cwd"](key, False)
            or rich_view_matches_literal["prodex_pid"](key, False)
            or rich_view_matches_literal["thread_id"](key, False)
        )
    if tool_kind == SUPER_EXPOSE_TOOL_SESSION_PREEMPT:
        return (
            rich_view_matches_literal["cwd"](key, False)
            or rich_view_matches_literal["prodex_pid"](key, False)
            or rich_view_matches_literal["thread_id"](key, False)
        )
    if tool_kind == SUPER_EXPOSE_TOOL_SESSION_OUTPUT_READ:
        return (
            rich_view_matches_literal["cursor"](key, False)
            or rich_view_matches_literal["limit"](key, False)
            or rich_view_matches_literal["wait_ms"](key, False)
            or rich_view_matches_literal["prodex_pid"](key, False)
            or rich_view_matches_literal["thread_id"](key, False)
        )
    return False


@export("prodex_mojo_super_expose_tool_argument_allowed_v1")
def prodex_mojo_super_expose_tool_argument_allowed_v1(
    abi_version: Int64,
    tool_address: UInt,
    tool_length: Int64,
    key_address: UInt,
    key_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != SUPER_EXPOSE_ABI_VERSION:
        return 4
    if (
        tool_length < 0
        or tool_length > SUPER_EXPOSE_MAX_NAME_BYTES
        or key_length < 0
        or output_address == 0
        or (tool_length > 0 and tool_address == 0)
        or (key_length > 0 and key_address == 0)
    ):
        return 1
    var tool = ProdexRichStringView(tool_address, UInt(tool_length))
    var key = ProdexRichStringView(key_address, UInt(key_length))
    if (
        not rich_view_valid(tool, SUPER_EXPOSE_MAX_NAME_BYTES)
        or not rich_view_valid(key, key_length)
    ):
        return 2
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = Int64(
        super_expose_tool_argument_allowed(super_expose_tool(tool), key)
    )
    return 0


@export("prodex_mojo_super_expose_run_id_valid_v1")
def prodex_mojo_super_expose_run_id_valid_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != SUPER_EXPOSE_ABI_VERSION:
        return 4
    if (
        length < 0
        or length > SUPER_EXPOSE_MAX_NAME_BYTES
        or output_address == 0
        or (length > 0 and address == 0)
    ):
        return 1
    var value = ProdexRichStringView(address, UInt(length))
    if not rich_view_valid(value, SUPER_EXPOSE_MAX_NAME_BYTES):
        return 2
    var valid = length >= 4 and rich_view_matches_literal["spr_"](
        ProdexRichStringView(address, UInt(4)), False
    )
    if valid:
        var ptr = rich_view_ptr(value)
        for index in range(length):
            var byte = ptr[unsafe_offset=index]
            if not (
                (byte >= 48 and byte <= 57)
                or (byte >= 65 and byte <= 90)
                or (byte >= 97 and byte <= 122)
                or byte == 95
                or byte == 45
            ):
                valid = False
                break
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = Int64(valid)
    return 0


@export("prodex_mojo_super_expose_string_valid_v1")
def prodex_mojo_super_expose_string_valid_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
    max_bytes: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != SUPER_EXPOSE_ABI_VERSION:
        return 4
    if (
        length < 0
        or max_bytes < 0
        or output_address == 0
        or (length > 0 and address == 0)
    ):
        return 1
    var value = ProdexRichStringView(address, UInt(length))
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    if length == 0 or length > max_bytes:
        output[] = 0
        return 0
    if not rich_view_valid(value, length):
        return 2
    var valid = True
    if valid:
        var ptr = rich_view_ptr(value)
        var index: Int64 = 0
        while index < length:
            var width = rich_codepoint_width(ptr[unsafe_offset=index])
            var codepoint = rich_codepoint(ptr, index, width)
            if codepoint <= 31 or (codepoint >= 127 and codepoint <= 159):
                valid = False
                break
            index += width
    output[] = Int64(valid)
    return 0
