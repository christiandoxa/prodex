# Codex version-probe output, terminal identity and User-Agent projection.
# Compiled Mojo owns Unicode whitespace/token rules and scalar replacement.
# Rust alone executes processes, validates reqwest HeaderValue and sends HTTP.
from std.memory import Pointer
from rich_text import (
    rich_codepoint, rich_codepoint_width, rich_unicode_space,
    rich_trim_bounds, rich_view_matches_literal, rich_view_ptr, rich_view_valid,
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


def codex_terminal_label(kind: Int64) -> ProdexRichStringView:
    var label = StringSlice("unknown")
    if kind == 0:
        label = StringSlice("Apple_Terminal")
    elif kind == 1:
        label = StringSlice("Ghostty")
    elif kind == 2:
        label = StringSlice("iTerm.app")
    elif kind == 3:
        label = StringSlice("WarpTerminal")
    elif kind == 4:
        label = StringSlice("vscode")
    elif kind == 5:
        label = StringSlice("WezTerm")
    elif kind == 6:
        label = StringSlice("kitty")
    elif kind == 7:
        label = StringSlice("Alacritty")
    elif kind == 8:
        label = StringSlice("Konsole")
    elif kind == 9:
        label = StringSlice("gnome-terminal")
    elif kind == 10:
        label = StringSlice("VTE")
    elif kind == 11:
        label = StringSlice("WindowsTerminal")
    elif kind == 12:
        label = StringSlice("dumb")
    return ProdexRichStringView(UInt(Int(label.unsafe_ptr())), UInt(label.byte_length()))

def codex_auth_write_ascii_part(
    input_view: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, MutUntrackedOrigin],
    offset: Int64,
    terminal_mode: Bool,
) -> Int64:
    var bytes = rich_view_ptr(input_view)
    var index: Int64 = 0
    var written = offset
    while index < Int64(input_view.len):
        var width = rich_codepoint_width(bytes[unsafe_offset=index])
        var codepoint = rich_codepoint(bytes, index, width)
        var allowed = codepoint >= 32 and codepoint <= 126
        if terminal_mode:
            allowed = (
                (codepoint >= 48 and codepoint <= 57)
                or (codepoint >= 65 and codepoint <= 90)
                or (codepoint >= 97 and codepoint <= 122)
                or codepoint == 45 or codepoint == 95
                or codepoint == 46 or codepoint == 47
            )
        output[unsafe_offset=written] = UInt8(codepoint) if allowed else UInt8(95)
        written += 1
        index += width
    return written


# fields holds exactly three borrowed UTF-8 views: program, version, TERM.
# program_present distinguishes an absent program from an explicitly empty one.
@export("prodex_codex_auth_terminal_token_v1")
def prodex_codex_auth_terminal_token_v1(
    abi_version: Int64,
    kind: Int64,
    program_present: Int64,
    fields_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != CODEX_AUTH_HEADER_ABI_VERSION:
        return 4
    if (
        kind < 0 or kind > 13
        or (program_present != 0 and program_present != 1)
        or fields_address == 0 or fields_address % 8 != 0
        or output_capacity < 0
        or written_address == 0 or written_address % 8 != 0
    ):
        return 1
    var fields = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(fields_address)
    )
    for index in range(3):
        if not rich_view_valid(fields[unsafe_offset=index], CODEX_AUTH_HEADER_MAX_BYTES):
            return 1
    var program = fields[unsafe_offset=0].copy()
    var version = fields[unsafe_offset=1].copy()
    var term = fields[unsafe_offset=2].copy()
    var base = codex_terminal_label(kind)
    var with_version = kind < 6 or kind == 8 or kind == 10
    if program_present == 1:
        base = program.copy()
        with_version = True
    elif term.len > 0:
        base = term.copy()
        with_version = False
    with_version = with_version and version.len > 0
    var required_capacity = Int64(base.len)
    if with_version:
        required_capacity += 1 + Int64(version.len)
    if required_capacity > 0 and output_address == 0:
        return 1
    if output_capacity < required_capacity:
        return 3
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = codex_auth_write_ascii_part(base, output, 0, True)
    if with_version:
        output[unsafe_offset=written] = 47
        written = codex_auth_write_ascii_part(version, output, written + 1, True)
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )[] = written
    return 0


# Shared Unicode-scalar ASCII projection for standard Codex User-Agent
# fallback and terminal metadata token. Both modes write one byte per scalar.
def codex_auth_project_ascii(
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
    terminal_mode: Bool,
) -> Int64:
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
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = codex_auth_write_ascii_part(input_view, output, 0, terminal_mode)
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )[] = written
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
    return codex_auth_project_ascii(
        input_address, input_length,
        output_address, output_capacity, written_address, False
    )


@export("prodex_codex_auth_terminal_token_ascii_v1")
def prodex_codex_auth_terminal_token_ascii_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != CODEX_AUTH_HEADER_ABI_VERSION:
        return 4
    return codex_auth_project_ascii(
        input_address, input_length,
        output_address, output_capacity, written_address, True
    )


@export("prodex_codex_auth_nonwhitespace_v1")
def prodex_codex_auth_nonwhitespace_v1(
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
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, CODEX_AUTH_HEADER_MAX_BYTES):
        return 1
    var bounds = rich_trim_bounds(view)
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )[] = Int64(bounds[1] > bounds[0])
    return 0


# Terminal kind normalization is an exact projection of TERM_PROGRAM strings
# (Unicode trim, casefold ASCII, remove ASCII space/hyphen/underscore/dot),
# or exact TERM names for the fallback mode. Kind=13 is Unknown.
comptime CODEX_TERMINAL_PROGRAM_MODE: Int64 = 0
comptime CODEX_TERMINAL_TERM_MODE: Int64 = 1


def codex_terminal_normalized_matches[literal: StaticString](
    value: ProdexRichStringView
) -> Bool:
    var bounds = rich_trim_bounds(value)
    var source = rich_view_ptr(value)
    var wanted = literal.unsafe_ptr()
    var output_index: Int64 = 0
    for index in range(bounds[0], bounds[1]):
        var byte = source[unsafe_offset=index]
        if byte == 32 or byte == 45 or byte == 95 or byte == 46:
            continue
        if byte >= 128:
            return False
        if byte >= 65 and byte <= 90:
            byte += 32
        if (
            output_index >= Int64(literal.byte_length())
            or wanted[unsafe_offset=output_index] != byte
        ):
            return False
        output_index += 1
    return output_index == Int64(literal.byte_length())


@export("prodex_codex_auth_terminal_name_v1")
def prodex_codex_auth_terminal_name_v1(
    abi_version: Int64,
    mode: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != CODEX_AUTH_HEADER_ABI_VERSION:
        return 4
    if (
        (mode != CODEX_TERMINAL_PROGRAM_MODE and mode != CODEX_TERMINAL_TERM_MODE)
        or input_length < 0 or input_length > CODEX_AUTH_HEADER_MAX_BYTES
        or (input_length > 0 and input_address == 0)
        or output_address == 0 or output_address % 8 != 0
    ):
        return 1
    var value = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(value, CODEX_AUTH_HEADER_MAX_BYTES):
        return 1
    var result: Int64 = 13
    if mode == CODEX_TERMINAL_TERM_MODE:
        if rich_view_matches_literal["dumb"](value, False):
            result = 12
        elif (
            rich_view_matches_literal["wezterm"](value, False)
            or rich_view_matches_literal["wezterm-mux"](value, False)
        ):
            result = 5
    else:
        if codex_terminal_normalized_matches["appleterminal"](value):
            result = 0
        elif codex_terminal_normalized_matches["ghostty"](value):
            result = 1
        elif (
            codex_terminal_normalized_matches["iterm"](value)
            or codex_terminal_normalized_matches["iterm2"](value)
            or codex_terminal_normalized_matches["itermapp"](value)
        ):
            result = 2
        elif (
            codex_terminal_normalized_matches["warp"](value)
            or codex_terminal_normalized_matches["warpterminal"](value)
        ):
            result = 3
        elif codex_terminal_normalized_matches["vscode"](value):
            result = 4
        elif codex_terminal_normalized_matches["wezterm"](value):
            result = 5
        elif codex_terminal_normalized_matches["kitty"](value):
            result = 6
        elif codex_terminal_normalized_matches["alacritty"](value):
            result = 7
        elif codex_terminal_normalized_matches["konsole"](value):
            result = 8
        elif codex_terminal_normalized_matches["gnometerminal"](value):
            result = 9
        elif codex_terminal_normalized_matches["vte"](value):
            result = 10
        elif codex_terminal_normalized_matches["windowsterminal"](value):
            result = 11
        elif codex_terminal_normalized_matches["dumb"](value):
            result = 12
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )[] = result
    return 0


# Split the first two Unicode whitespace-separated tmux client termtype tokens.
# The host only materializes source ranges, ignoring any later tokens.
@export("prodex_codex_auth_terminal_parts_v1")
def prodex_codex_auth_terminal_parts_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != CODEX_AUTH_HEADER_ABI_VERSION:
        return 4
    if (
        input_length < 0 or input_length > CODEX_AUTH_HEADER_MAX_BYTES
        or (input_length > 0 and input_address == 0)
        or output_address == 0 or output_address % 8 != 0
    ):
        return 1
    var input_view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(input_view, CODEX_AUTH_HEADER_MAX_BYTES):
        return 1
    var input_bytes = rich_view_ptr(input_view)
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var position: Int64 = 0
    var observed: Int64 = 0
    var starts = [Int64(-1), Int64(-1)]
    var ends = [Int64(-1), Int64(-1)]
    while position < input_length and observed < 2:
        var width = rich_codepoint_width(input_bytes[unsafe_offset=position])
        if rich_unicode_space(rich_codepoint(input_bytes, position, width)):
            position += width
            continue
        starts[observed] = position
        while position < input_length:
            width = rich_codepoint_width(input_bytes[unsafe_offset=position])
            if rich_unicode_space(rich_codepoint(input_bytes, position, width)):
                break
            position += width
        ends[observed] = position
        observed += 1
    output[unsafe_offset=0] = starts[0]
    output[unsafe_offset=1] = ends[0]
    output[unsafe_offset=2] = starts[1]
    output[unsafe_offset=3] = ends[1]
    return 0
