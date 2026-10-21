# Codex version-probe output and printable User-Agent projection.
# Compiled Mojo owns Unicode whitespace/token rules and scalar replacement.
# Rust alone executes processes, validates reqwest HeaderValue and sends HTTP.
from std.memory import Pointer
from rich_text import (
    rich_codepoint, rich_codepoint_width, rich_unicode_space,
    rich_view_ptr, rich_view_valid,
)
from rich_types import ProdexRichStringView

comptime CODEX_AUTH_HEADER_ABI_VERSION: Int64 = 1
comptime CODEX_AUTH_HEADER_MAX_BYTES: Int64 = 1_048_576


@export("prodex_codex_auth_probe_version_span_v1")
def prodex_codex_auth_probe_version_span_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != CODEX_AUTH_HEADER_ABI_VERSION:
        return 4
    if (
        input_length < 0 or input_length > CODEX_AUTH_HEADER_MAX_BYTES
        or (input_length > 0 and input_address == 0)
        or result_address == 0 or result_address % 8 != 0
    ):
        return 1
    var input_view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(input_view, CODEX_AUTH_HEADER_MAX_BYTES):
        return 1
    var bytes = rich_view_ptr(input_view)
    var position: Int64 = 0
    var chosen_start: Int64 = -1
    var chosen_end: Int64 = -1
    while position < input_length:
        var width = rich_codepoint_width(bytes[unsafe_offset=position])
        var codepoint = rich_codepoint(bytes, position, width)
        if rich_unicode_space(codepoint):
            position += width
            continue
        var token_start = position
        var saw_dot = False
        var last_kept_end = position
        while position < input_length:
            width = rich_codepoint_width(bytes[unsafe_offset=position])
            codepoint = rich_codepoint(bytes, position, width)
            if rich_unicode_space(codepoint):
                break
            var char_accepted = (
                (codepoint >= 48 and codepoint <= 57)
                or (codepoint >= 65 and codepoint <= 90)
                or (codepoint >= 97 and codepoint <= 122)
                or codepoint == 46 or codepoint == 45
            )
            if codepoint == 46:
                saw_dot = True
            if char_accepted:
                last_kept_end = position + width
            position += width
        # Rust's split_whitespace().rev().find first tests for an ASCII
        # digit at the start of the token and an ASCII dot anywhere.
        if (
            bytes[unsafe_offset=token_start] >= 48
            and bytes[unsafe_offset=token_start] <= 57
            and saw_dot
        ):
            chosen_start = token_start
            chosen_end = last_kept_end
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    output[unsafe_offset=0] = chosen_start
    output[unsafe_offset=1] = chosen_end
    return 0


@export("prodex_codex_auth_user_agent_ascii_v1")
def prodex_codex_auth_user_agent_ascii_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != CODEX_AUTH_HEADER_ABI_VERSION:
        return 4
    if (
        input_length < 0 or input_length > CODEX_AUTH_HEADER_MAX_BYTES
        or (input_length > 0 and input_address == 0)
        or output_capacity < input_length
        or (input_length > 0 and output_address == 0)
        or written_address == 0 or written_address % 8 != 0
    ):
        return 1
    var input_view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(input_view, CODEX_AUTH_HEADER_MAX_BYTES):
        return 1
    var bytes = rich_view_ptr(input_view)
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var index: Int64 = 0
    var written: Int64 = 0
    while index < input_length:
        var width = rich_codepoint_width(bytes[unsafe_offset=index])
        var codepoint = rich_codepoint(bytes, index, width)
        # One underscore per Unicode scalar outside printable ASCII.
        if codepoint >= 32 and codepoint <= 126:
            output[unsafe_offset=written] = UInt8(codepoint)
        else:
            output[unsafe_offset=written] = 95
        written += 1
        index += width
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )[] = written
    return 0
