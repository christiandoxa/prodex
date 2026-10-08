from std.collections import Array
from std.memory import Pointer

from rich_text import rich_codepoint, rich_codepoint_width, rich_unicode_space, rich_utf8_valid, rich_view_ptr
from rich_types import ProdexRichStringView

comptime RTK_NOISY_ABI_VERSION: Int64 = 1
comptime RTK_NOISY_STATUS_OK: Int64 = 0
comptime RTK_NOISY_STATUS_INVALID: Int64 = 1
comptime RTK_NOISY_STATUS_UTF8: Int64 = 2
comptime RTK_NOISY_STATUS_CAPACITY: Int64 = 3
comptime RTK_NOISY_STATUS_ABI: Int64 = 4
comptime RTK_NOISY_MODE_WRAPPED: Int64 = 0
comptime RTK_NOISY_MODE_PREFIXED: Int64 = 1
comptime RTK_NOISY_MAX_INPUT_BYTES: Int64 = 64 * 1024 * 1024
# ponytail: 4096-byte decoded token buffer; stream matching if longer tokens matter.
comptime RTK_NOISY_TOKEN_CAPACITY: Int64 = 4096


def rtk_noisy_separator_length(
    ptr: Pointer[mut=False, UInt8, _], index: Int64, end: Int64
) -> Int64:
    if index < 0 or index >= end:
        return 0
    var value = ptr[unsafe_offset=index]
    if index + 1 < end:
        var next = ptr[unsafe_offset=index + 1]
        if value == 38 and next == 38 or value == 124 and next == 124:
            return 2
    if value == 59 or value == 124 or value == 10:
        return 1
    return 0


def rtk_noisy_skip_whitespace(
    ptr: Pointer[mut=False, UInt8, _], index: Int64, end: Int64
) -> Int64:
    var cursor = index
    while cursor < end:
        var width = rich_codepoint_width(ptr[unsafe_offset=cursor])
        if not rich_unicode_space(rich_codepoint(ptr, cursor, width)):
            break
        cursor += width
    return cursor


def rtk_noisy_token_end(
    ptr: Pointer[mut=False, UInt8, _], index: Int64, end: Int64
) -> Int64:
    var cursor = index
    while cursor < end:
        var width = rich_codepoint_width(ptr[unsafe_offset=cursor])
        if rich_unicode_space(rich_codepoint(ptr, cursor, width)):
            break
        cursor += width
    return cursor


def rtk_noisy_token_buffer(
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
) -> Array[Int64, 2]:
    var result = Array[Int64, 2](fill=-1)
    var cursor = start
    var written: Int64 = 0
    var quote: UInt8 = 0
    var escaped = False
    while cursor < end:
        var value = ptr[unsafe_offset=cursor]
        if escaped:
            if written >= capacity:
                return result^
            output[unsafe_offset=written] = value
            written += 1
            escaped = False
            cursor += 1
            continue
        if quote == 39:
            if value == 39:
                quote = 0
            else:
                if written >= capacity:
                    return result^
                output[unsafe_offset=written] = value
                written += 1
            cursor += 1
            continue
        if quote == 34:
            if value == 34:
                quote = 0
            elif value == 92:
                escaped = True
            else:
                if written >= capacity:
                    return result^
                output[unsafe_offset=written] = value
                written += 1
            cursor += 1
            continue
        if value == 39 or value == 34:
            quote = value
        elif value == 92:
            escaped = True
        else:
            if written >= capacity:
                return result^
            output[unsafe_offset=written] = value
            written += 1
        cursor += 1
    result[0] = written
    result[1] = 1
    return result^


def rtk_noisy_buffer_matches(
    buffer: Pointer[mut=False, UInt8, _],
    start: Int64,
    length: Int64,
    literal: StringSlice,
) -> Bool:
    if start < 0 or length < 0 or start + length > RTK_NOISY_TOKEN_CAPACITY:
        return False
    if length != Int64(literal.byte_length()):
        return False
    var expected = literal.unsafe_ptr()
    for index in range(length):
        if buffer[unsafe_offset=start + index] != expected[unsafe_offset=index]:
            return False
    return True


def rtk_noisy_command_kind(
    buffer: Pointer[mut=False, UInt8, _], length: Int64
) -> Int64:
    var component_start: Int64 = 0
    for index in range(length):
        if buffer[unsafe_offset=index] == 47:
            component_start = index + 1
    var component_length = length - component_start
    if rtk_noisy_buffer_matches(buffer, component_start, component_length, StringSlice("git")):
        return 0
    if rtk_noisy_buffer_matches(buffer, component_start, component_length, StringSlice("cargo")):
        return 1
    if rtk_noisy_buffer_matches(buffer, component_start, component_length, StringSlice("npm")):
        return 2
    if rtk_noisy_buffer_matches(buffer, component_start, component_length, StringSlice("yarn")):
        return 3
    if rtk_noisy_buffer_matches(buffer, component_start, component_length, StringSlice("pnpm")):
        return 4
    if rtk_noisy_buffer_matches(buffer, component_start, component_length, StringSlice("bun")):
        return 5
    if rtk_noisy_buffer_matches(buffer, component_start, component_length, StringSlice("pytest")):
        return 6
    if rtk_noisy_buffer_matches(buffer, component_start, component_length, StringSlice("go")):
        return 7
    if rtk_noisy_buffer_matches(buffer, component_start, component_length, StringSlice("docker")):
        return 8
    if rtk_noisy_buffer_matches(buffer, component_start, component_length, StringSlice("kubectl")):
        return 9
    if rtk_noisy_buffer_matches(buffer, component_start, component_length, StringSlice("rg")):
        return 10
    if rtk_noisy_buffer_matches(buffer, component_start, component_length, StringSlice("find")):
        return 11
    if rtk_noisy_buffer_matches(buffer, component_start, component_length, StringSlice("ls")):
        return 12
    if rtk_noisy_buffer_matches(buffer, component_start, component_length, StringSlice("tree")):
        return 13
    if rtk_noisy_buffer_matches(buffer, component_start, component_length, StringSlice("echo")):
        return 14
    if rtk_noisy_buffer_matches(buffer, component_start, component_length, StringSlice("claw-compactor")):
        return 15
    return -1


def rtk_noisy_is_env_assignment(
    buffer: Pointer[mut=False, UInt8, _], length: Int64
) -> Bool:
    var equals: Int64 = -1
    for index in range(length):
        if buffer[unsafe_offset=index] == 61:
            equals = index
            break
    if equals <= 0 or equals + 1 >= length:
        return False
    var first = buffer[unsafe_offset=0]
    if not (
        first == 95
        or first >= 65 and first <= 90
        or first >= 97 and first <= 122
    ):
        return False
    var index: Int64 = 1
    while index < equals:
        var value = buffer[unsafe_offset=index]
        if not (
            value == 95
            or value >= 65 and value <= 90
            or value >= 97 and value <= 122
            or value >= 48 and value <= 57
        ):
            return False
        index += 1
    return True


def rtk_noisy_raw_env_assignment(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    # Long environment values are valid. Match only their fixed-size ASCII
    # NAME= prefix directly, without an artificial decoded-token limit.
    if start >= end:
        return False
    var first = ptr[unsafe_offset=start]
    if not (
        first == 95
        or first >= 65 and first <= 90
        or first >= 97 and first <= 122
    ):
        return False
    var index = start + 1
    while index < end:
        var value = ptr[unsafe_offset=index]
        if value == 61:
            return index + 1 < end
        if not (
            value == 95
            or value >= 65 and value <= 90
            or value >= 97 and value <= 122
            or value >= 48 and value <= 57
        ):
            return False
        index += 1
    return False


def rtk_noisy_subcommand(
    kind: Int64, buffer: Pointer[mut=False, UInt8, _], length: Int64
) -> Bool:
    if kind == 0:
        return (
            rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("diff"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("show"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("log"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("status"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("grep"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("blame"))
        )
    if kind == 1:
        return (
            rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("test"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("build"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("check"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("clippy"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("bench"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("run"))
        )
    if kind == 2:
        return (
            rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("test"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("run"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("build"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("install"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("ci"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("update"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("audit"))
        )
    if kind == 3:
        return (
            rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("test"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("run"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("build"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("install"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("add"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("upgrade"))
        )
    if kind == 4:
        return (
            rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("test"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("run"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("build"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("install"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("add"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("update"))
        )
    if kind == 5:
        return (
            rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("test"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("run"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("build"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("install"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("add"))
        )
    if kind == 7:
        return (
            rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("test"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("build"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("vet"))
        )
    if kind == 8:
        return (
            rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("build"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("compose"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("logs"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("pull"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("push"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("run"))
        )
    if kind == 9:
        return (
            rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("logs"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("describe"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("get"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("events"))
            or rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("top"))
        )
    if kind == 15:
        return rtk_noisy_buffer_matches(buffer, 0, length, StringSlice("benchmark"))
    return kind == 6 or kind == 10 or kind == 11 or kind == 12 or kind == 13 or kind == 14


def rtk_noisy_segment_insert_index(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    var buffer = Array[UInt8, 4096](fill=0)
    var buffer_ptr = buffer.unsafe_ptr()
    var cursor = rtk_noisy_skip_whitespace(ptr, start, end)
    if cursor >= end:
        return -1
    var token_end = rtk_noisy_token_end(ptr, cursor, end)
    var token = rtk_noisy_token_buffer(ptr, cursor, token_end, buffer_ptr, RTK_NOISY_TOKEN_CAPACITY)
    if token[1] == 1 and rtk_noisy_buffer_matches(
        buffer_ptr, 0, token[0], StringSlice("rtk")
    ):
        return -1
    var is_assignment = (
        rtk_noisy_is_env_assignment(buffer_ptr, token[0])
        if token[1] == 1
        else rtk_noisy_raw_env_assignment(ptr, cursor, token_end)
    )
    while is_assignment:
        cursor = rtk_noisy_skip_whitespace(ptr, token_end, end)
        if cursor >= end:
            return -1
        token_end = rtk_noisy_token_end(ptr, cursor, end)
        token = rtk_noisy_token_buffer(
            ptr, cursor, token_end, buffer_ptr, RTK_NOISY_TOKEN_CAPACITY
        )
        if token[1] == 1 and rtk_noisy_buffer_matches(
            buffer_ptr, 0, token[0], StringSlice("rtk")
        ):
            return -1
        is_assignment = (
            rtk_noisy_is_env_assignment(buffer_ptr, token[0])
            if token[1] == 1
            else rtk_noisy_raw_env_assignment(ptr, cursor, token_end)
        )
    if token[1] != 1:
        return -1
    var kind = rtk_noisy_command_kind(buffer_ptr, token[0])
    if kind < 0:
        return -1
    var command_start = cursor
    if kind == 6 or kind == 10 or kind == 11 or kind == 12 or kind == 13 or kind == 14:
        return cursor
    cursor = rtk_noisy_skip_whitespace(ptr, token_end, end)
    while cursor < end:
        token_end = rtk_noisy_token_end(ptr, cursor, end)
        token = rtk_noisy_token_buffer(ptr, cursor, token_end, buffer_ptr, RTK_NOISY_TOKEN_CAPACITY)
        if token[1] == 1 and rtk_noisy_subcommand(kind, buffer_ptr, token[0]):
            return command_start
        cursor = rtk_noisy_skip_whitespace(ptr, token_end, end)
    return -1


def rtk_noisy_insert_index(view: ProdexRichStringView) -> Int64:
    if view.len == 0:
        return -1
    var ptr = rich_view_ptr(view)
    var length = Int64(view.len)
    var segment_start: Int64 = 0
    var index: Int64 = 0
    var quote: UInt8 = 0
    var escaped = False
    while index < length:
        var value = ptr[unsafe_offset=index]
        if escaped:
            escaped = False
            index += 1
            continue
        if quote == 39:
            if value == 39:
                quote = 0
            index += 1
            continue
        if quote == 34:
            if value == 34:
                quote = 0
            elif value == 92:
                escaped = True
            index += 1
            continue
        if value == 39 or value == 34:
            quote = value
            index += 1
            continue
        if value == 92:
            escaped = True
            index += 1
            continue
        var separator = rtk_noisy_separator_length(ptr, index, length)
        if separator > 0:
            var candidate = rtk_noisy_segment_insert_index(ptr, segment_start, index)
            if candidate >= 0:
                return candidate
            index += separator
            segment_start = index
            continue
        index += rich_codepoint_width(value)
    return rtk_noisy_segment_insert_index(ptr, segment_start, length)


def rtk_noisy_write_range(
    source: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    if start < 0 or end < start or end - start > capacity - written[]:
        return False
    var index = start
    while index < end:
        output[unsafe_offset=written[]] = source[unsafe_offset=index]
        written[] += 1
        index += 1
    return True


def rtk_noisy_write_literal(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    var literal = StringSlice("rtk ")
    var ptr = literal.unsafe_ptr()
    if Int64(literal.byte_length()) > capacity - written[]:
        return False
    for index in range(Int64(literal.byte_length())):
        output[unsafe_offset=written[]] = ptr[unsafe_offset=index]
        written[] += 1
    return True


def rtk_noisy_rewrite_index(view: ProdexRichStringView, mode: Int64) -> Int64:
    var insert_at = rtk_noisy_insert_index(view)
    if insert_at < 0:
        return -1
    if mode == RTK_NOISY_MODE_PREFIXED:
        return 0
    return insert_at


@export("prodex_mojo_rtk_noisy_shell_command_v1")
def prodex_mojo_rtk_noisy_shell_command_v1(
    abi_version: Int64,
    mode: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != RTK_NOISY_ABI_VERSION:
        return RTK_NOISY_STATUS_ABI
    if (
        mode < RTK_NOISY_MODE_WRAPPED
        or mode > RTK_NOISY_MODE_PREFIXED
        or input_length < 0
        or input_length > RTK_NOISY_MAX_INPUT_BYTES
        or input_address == 0
        or output_address == 0
        or output_capacity <= 0
        or written_address == 0
    ):
        return RTK_NOISY_STATUS_INVALID
    var input = Pointer[mut=False, UInt8, ImmUntrackedOrigin](unsafe_from_address=Int(input_address))
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))
    written[] = 0
    if not rich_utf8_valid(input, input_length):
        return RTK_NOISY_STATUS_UTF8
    var view = ProdexRichStringView(input_address, UInt(input_length))
    var insert_at = rtk_noisy_rewrite_index(view, mode)
    if insert_at < 0:
        return RTK_NOISY_STATUS_OK
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    if not rtk_noisy_write_range(input, 0, insert_at, output, output_capacity, written):
        return RTK_NOISY_STATUS_CAPACITY
    if not rtk_noisy_write_literal(output, output_capacity, written):
        return RTK_NOISY_STATUS_CAPACITY
    if not rtk_noisy_write_range(input, insert_at, input_length, output, output_capacity, written):
        return RTK_NOISY_STATUS_CAPACITY
    return RTK_NOISY_STATUS_OK
