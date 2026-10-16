from std.memory import Pointer

from rich_types import ProdexRichStringView, rich_view_ptr
from rich_text import (
    rich_codepoint,
    rich_codepoint_width,
    rich_trim_bounds,
    rich_unicode_space,
    rich_view_valid,
)

# Deterministic terminal layout policies. Rust measures true terminal cell
# widths (including Unicode) and owns styling, strings and the terminal host.
# Mojo exclusively decides label, session-column and viewport dimensions.
comptime TERMINAL_LAYOUT_ABI: Int64 = 1
comptime TERMINAL_LAYOUT_OK: Int64 = 0
comptime TERMINAL_LAYOUT_INVALID: Int64 = 1
comptime TERMINAL_LAYOUT_VERSION: Int64 = 4
comptime TERMINAL_LAYOUT_UINT64_MAX: UInt64 = 18_446_744_073_709_551_615


# The terminal host owns environment/TTY reads. Mojo owns input tokenization,
# Unicode whitespace handling, positive override limits and decimal parsing.


def terminal_geometry_unsigned(
    text: ProdexRichStringView,
    start: Int64,
    end: Int64,
    maximum: UInt64,
) -> Tuple[Bool, UInt64]:
    var cursor = start
    if cursor >= end:
        return (False, UInt64(0))
    var source = rich_view_ptr(text)
    if source[unsafe_offset=cursor] == 43:
        cursor += 1
    if cursor >= end:
        return (False, UInt64(0))
    var parsed = UInt64(0)
    while cursor < end:
        var byte = source[unsafe_offset=cursor]
        if byte < 48 or byte > 57:
            return (False, UInt64(0))
        var digit = UInt64(byte - 48)
        if digit > maximum or parsed > (maximum - digit) // UInt64(10):
            return (False, UInt64(0))
        parsed = parsed * UInt64(10) + digit
        cursor += 1
    return (True, parsed)


def terminal_geometry_skip_whitespace(text: ProdexRichStringView, index: Int64) -> Int64:
    var source = rich_view_ptr(text)
    var cursor = index
    while cursor < Int64(text.len):
        var width = rich_codepoint_width(source[unsafe_offset=cursor])
        if not rich_unicode_space(rich_codepoint(source, cursor, width)):
            break
        cursor += width
    return cursor


def terminal_geometry_token_end(text: ProdexRichStringView, index: Int64) -> Int64:
    var source = rich_view_ptr(text)
    var cursor = index
    while cursor < Int64(text.len):
        var width = rich_codepoint_width(source[unsafe_offset=cursor])
        if rich_unicode_space(rich_codepoint(source, cursor, width)):
            break
        cursor += width
    return cursor


@export("prodex_terminal_positive_size_override_v1")
def prodex_terminal_positive_size_override_v1(
    abi: Int64,
    text_address: UInt,
    text_length: Int64,
    maximum: UInt64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi != TERMINAL_LAYOUT_ABI:
        return TERMINAL_LAYOUT_VERSION
    if (text_length < 0 or (text_address == 0 and text_length != 0)
        or output_address == 0 or maximum == 0):
        return TERMINAL_LAYOUT_INVALID
    var text = ProdexRichStringView(text_address, UInt(text_length))
    if not rich_view_valid(text, 0x7FFFFFFFFFFFFFFF):
        return 2
    var bounds = rich_trim_bounds(text)
    var parsed = terminal_geometry_unsigned(text, bounds[0], bounds[1], maximum)
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = parsed[1] if parsed[0] and parsed[1] > 0 else UInt64(0)
    return TERMINAL_LAYOUT_OK


@export("prodex_terminal_tty_dimensions_v1")
def prodex_terminal_tty_dimensions_v1(
    abi: Int64,
    text_address: UInt,
    text_length: Int64,
    maximum: UInt64,
    output_address: UInt,
    result_capacity: Int64,
) abi("C") -> Int64:
    if abi != TERMINAL_LAYOUT_ABI:
        return TERMINAL_LAYOUT_VERSION
    if (text_length < 0 or (text_address == 0 and text_length != 0)
        or output_address == 0 or result_capacity < 3 or maximum == 0):
        return TERMINAL_LAYOUT_INVALID
    var text = ProdexRichStringView(text_address, UInt(text_length))
    if not rich_view_valid(text, 0x7FFFFFFFFFFFFFFF):
        return 2
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = 0
    output[unsafe_offset=2] = 0
    var start = terminal_geometry_skip_whitespace(text, 0)
    var end = terminal_geometry_token_end(text, start)
    var rows = terminal_geometry_unsigned(text, start, end, maximum)
    if not rows[0]:
        return TERMINAL_LAYOUT_OK
    start = terminal_geometry_skip_whitespace(text, end)
    end = terminal_geometry_token_end(text, start)
    var columns = terminal_geometry_unsigned(text, start, end, maximum)
    if not columns[0]:
        return TERMINAL_LAYOUT_OK
    output[unsafe_offset=0] = 1
    output[unsafe_offset=1] = rows[1]
    output[unsafe_offset=2] = columns[1]
    return TERMINAL_LAYOUT_OK


# Render the connected Ratatui separator in its exact UTF-8 box-drawing
# bytes. Unlike a width-only plan, this leaves no Rust Unicode renderer.
comptime TERMINAL_SEPARATOR_MAX_CELLS: UInt64 = 65535


def terminal_separator_write_glyph(
    output: Pointer[mut=True, UInt8, _],
    index: Int64,
    final_byte: UInt8,
):
    var start = index * 3
    output[unsafe_offset=start] = 0xE2
    output[unsafe_offset=start + 1] = 0x94
    output[unsafe_offset=start + 2] = final_byte


@export("prodex_terminal_connected_separator_line_v1")
def prodex_terminal_connected_separator_line_v1(
    abi: Int64,
    width: UInt64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi != TERMINAL_LAYOUT_ABI:
        return TERMINAL_LAYOUT_VERSION
    if (width > TERMINAL_SEPARATOR_MAX_CELLS or written_address == 0
        or output_capacity < 0 or output_capacity < Int64(width * 3)
        or (width > 0 and output_address == 0)):
        return TERMINAL_LAYOUT_INVALID
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if width == 0:
        return TERMINAL_LAYOUT_OK
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    if width == 1:
        terminal_separator_write_glyph(output, 0, 0x80)
    else:
        terminal_separator_write_glyph(output, 0, 0x9C)
        var index: Int64 = 1
        while index < Int64(width) - 1:
            terminal_separator_write_glyph(output, index, 0x80)
            index += 1
        terminal_separator_write_glyph(output, Int64(width) - 1, 0xA4)
    written[] = Int64(width * 3)
    return TERMINAL_LAYOUT_OK


def terminal_clamp(value: UInt64, low: UInt64, high: UInt64) -> UInt64:
    if value < low:
        return low
    if value > high:
        return high
    return value


def terminal_saturating_mul_four(value: UInt64) -> UInt64:
    if value > TERMINAL_LAYOUT_UINT64_MAX // UInt64(4):
        return TERMINAL_LAYOUT_UINT64_MAX
    return value * UInt64(4)


def terminal_saturating_add_five(value: UInt64) -> UInt64:
    if value > TERMINAL_LAYOUT_UINT64_MAX - UInt64(5):
        return TERMINAL_LAYOUT_UINT64_MAX
    return value + UInt64(5)


@export("prodex_terminal_panel_label_width_v1")
def prodex_terminal_panel_label_width_v1(
    abi: Int64,
    longest: UInt64,
    total_width: UInt64,
    minimum: UInt64,
    maximum: UInt64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi != TERMINAL_LAYOUT_ABI:
        return TERMINAL_LAYOUT_VERSION
    if result_address == 0 or maximum == 0:
        return TERMINAL_LAYOUT_INVALID

    var available: UInt64 = 0
    if total_width > 20:
        available = total_width - 20
    var max_by_width = terminal_clamp(available, UInt64(1), maximum)
    var preferred = terminal_clamp(total_width // 4, UInt64(1), maximum)
    var cap = min(max_by_width, preferred)
    var floor = min(minimum, cap)
    var selected = terminal_clamp(longest, floor, cap)
    Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )[] = selected
    return TERMINAL_LAYOUT_OK


@export("prodex_terminal_field_row_layout_v1")
def prodex_terminal_field_row_layout_v1(
    abi: Int64,
    total_width: UInt64,
    label_width: UInt64,
    label_cells: UInt64,
    result_address: UInt,
    result_capacity: Int64,
) abi("C") -> Int64:
    if abi != TERMINAL_LAYOUT_ABI:
        return TERMINAL_LAYOUT_VERSION
    if result_address == 0 or result_capacity < 3:
        return TERMINAL_LAYOUT_INVALID

    var label_and_separator = label_width
    if label_and_separator < TERMINAL_LAYOUT_UINT64_MAX:
        label_and_separator += UInt64(1)
    var value_width: UInt64 = 1
    if total_width > label_and_separator:
        value_width = total_width - label_and_separator

    var first_padding: UInt64 = 0
    if label_width > label_cells:
        first_padding = label_width - label_cells

    var result = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[unsafe_offset=0] = value_width
    result[unsafe_offset=1] = first_padding
    result[unsafe_offset=2] = label_width
    return TERMINAL_LAYOUT_OK


@export("prodex_terminal_session_columns_v1")
def prodex_terminal_session_columns_v1(
    abi: Int64,
    total_width: UInt64,
    gap_width: UInt64,
    result_address: UInt,
    result_capacity: Int64,
) abi("C") -> Int64:
    if abi != TERMINAL_LAYOUT_ABI:
        return TERMINAL_LAYOUT_VERSION
    if result_address == 0 or result_capacity < 5:
        return TERMINAL_LAYOUT_INVALID

    var available: UInt64 = 0
    if total_width > gap_width:
        available = total_width - gap_width
    available = max(available, UInt64(5))
    var fifth = available // 5
    var id_width = terminal_clamp(fifth, UInt64(1), UInt64(26))
    var updated_width = terminal_clamp(fifth, UInt64(1), UInt64(22))
    var thread_width = terminal_clamp(fifth, UInt64(1), UInt64(24))
    var occupied = id_width + updated_width + thread_width
    var rest: UInt64 = 0
    if available > occupied:
        rest = available - occupied
    var cwd_width = rest // 2
    var path_width = rest - cwd_width

    var result = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[unsafe_offset=0] = id_width
    result[unsafe_offset=1] = updated_width
    result[unsafe_offset=2] = thread_width
    result[unsafe_offset=3] = cwd_width
    result[unsafe_offset=4] = path_width
    return TERMINAL_LAYOUT_OK


@export("prodex_terminal_panel_inline_height_v1")
def prodex_terminal_panel_inline_height_v1(
    abi: Int64,
    line_count: UInt64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi != TERMINAL_LAYOUT_ABI:
        return TERMINAL_LAYOUT_VERSION
    if result_address == 0:
        return TERMINAL_LAYOUT_INVALID
    Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )[] = terminal_clamp(line_count, UInt64(4), UInt64(65535))
    return TERMINAL_LAYOUT_OK


# Plan the session-list scroll budget and inline viewport. Rust owns terminal
# size acquisition and Ratatui; Mojo owns bounded report geometry.
@export("prodex_terminal_session_report_viewport_v1")
def prodex_terminal_session_report_viewport_v1(
    abi: Int64,
    report_count: UInt64,
    terminal_height: UInt64,
    result_address: UInt,
    result_capacity: Int64,
) abi("C") -> Int64:
    if abi != TERMINAL_LAYOUT_ABI:
        return TERMINAL_LAYOUT_VERSION
    if result_address == 0 or result_capacity < 2:
        return TERMINAL_LAYOUT_INVALID

    var total_lines = terminal_saturating_mul_four(report_count)
    var rows = terminal_saturating_add_five(total_lines)
    rows = max(rows, UInt64(8))
    var inline_height = min(rows, terminal_height)
    inline_height = max(inline_height, UInt64(1))
    var result = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[unsafe_offset=0] = total_lines
    result[unsafe_offset=1] = inline_height
    return TERMINAL_LAYOUT_OK


# The host measures Unicode display cells; Mojo decides truncation policy.
# 0: empty, 1: unchanged, 2: ASCII dots, 3: prefix and ellipsis.
@export("prodex_terminal_fit_cell_plan_v1")
def prodex_terminal_fit_cell_plan_v1(
    abi: Int64,
    content_cells: UInt64,
    available_cells: UInt64,
    result_address: UInt,
    result_capacity: Int64,
) abi("C") -> Int64:
    if abi != TERMINAL_LAYOUT_ABI:
        return TERMINAL_LAYOUT_VERSION
    if result_address == 0 or result_capacity < 2:
        return TERMINAL_LAYOUT_INVALID
    var mode: UInt64 = 0
    var budget: UInt64 = 0
    if available_cells == 0:
        mode = 0
    elif content_cells <= available_cells:
        mode = 1
    elif available_cells <= 3:
        mode = 2
        budget = available_cells
    else:
        mode = 3
        budget = available_cells - 3
    var result = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[unsafe_offset=0] = mode
    result[unsafe_offset=1] = budget
    return TERMINAL_LAYOUT_OK


# Plan middle ellipsis presentation from host-measured scalar cell widths.
# 0: empty, 1: unchanged, 2: ASCII dots, 3: prefix/suffix scalar counts.
@export("prodex_terminal_middle_ellipsis_plan_v1")
def prodex_terminal_middle_ellipsis_plan_v1(
    abi: Int64,
    widths_address: UInt,
    scalar_count: Int64,
    content_cells: UInt64,
    available_cells: UInt64,
    result_address: UInt,
    result_capacity: Int64,
) abi("C") -> Int64:
    if abi != TERMINAL_LAYOUT_ABI:
        return TERMINAL_LAYOUT_VERSION
    if (
        scalar_count < 0
        or (scalar_count > 0 and widths_address == 0)
        or result_address == 0
        or result_capacity < 3
    ):
        return TERMINAL_LAYOUT_INVALID

    var widths = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(widths_address)
    )
    for index in range(scalar_count):
        if widths[unsafe_offset=index] > 2:
            return TERMINAL_LAYOUT_INVALID

    var mode: UInt64 = 0
    var first: UInt64 = 0
    var second: UInt64 = 0
    if available_cells == 0:
        mode = 0
    elif content_cells <= available_cells:
        mode = 1
    elif available_cells <= 3:
        mode = 2
        first = available_cells
    else:
        if scalar_count == 0:
            return TERMINAL_LAYOUT_INVALID
        mode = 3
        var left_limit = (available_cells - 3) // 2
        var left_cells: UInt64 = 0
        var left_count: Int64 = 0
        for index in range(scalar_count):
            var cells = widths[unsafe_offset=index]
            if left_cells > left_limit or cells > left_limit - left_cells:
                break
            left_cells += cells
            left_count += 1

        var right_limit = available_cells - 3 - left_cells
        var right_cells: UInt64 = 0
        var right_count: Int64 = 0
        var index = scalar_count - 1
        while index >= 0:
            var cells = widths[unsafe_offset=index]
            if right_cells > right_limit or cells > right_limit - right_cells:
                break
            right_cells += cells
            right_count += 1
            index -= 1
        first = UInt64(left_count)
        second = UInt64(right_count)

    var result = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[unsafe_offset=0] = mode
    result[unsafe_offset=1] = first
    result[unsafe_offset=2] = second
    return TERMINAL_LAYOUT_OK


# Terminal token segmentation: Unicode cell widths from the host, all split
# decisions from Mojo. Output chunk-ending *scalar indexes*, not byte offsets.
@export("prodex_terminal_token_chunk_plan_v1")
def prodex_terminal_token_chunk_plan_v1(
    abi: Int64,
    widths_address: UInt,
    scalar_count: Int64,
    maximum_cells: UInt64,
    boundaries_address: UInt,
    boundaries_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi != TERMINAL_LAYOUT_ABI:
        return TERMINAL_LAYOUT_VERSION
    if (
        scalar_count < 0
        or (scalar_count > 0 and widths_address == 0)
        or boundaries_address == 0
        or written_address == 0
        or boundaries_capacity <= scalar_count
    ):
        return TERMINAL_LAYOUT_INVALID

    var endings = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(boundaries_address)
    )
    if scalar_count == 0 or maximum_cells == 0:
        endings[unsafe_offset=0] = 0
        Pointer[mut=True, Int64, MutUntrackedOrigin](
            unsafe_from_address=Int(written_address)
        )[] = 1
        return TERMINAL_LAYOUT_OK

    var widths = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(widths_address)
    )
    var emitted: Int64 = 0
    var current_cells: UInt64 = 0
    var current_chars: Int64 = 0
    for i in range(scalar_count):
        var cells = widths[unsafe_offset=i]
        if cells > 2:
            return TERMINAL_LAYOUT_INVALID
        if (
            current_cells > 0 and
            (current_cells > maximum_cells
             or cells > maximum_cells - current_cells)
        ):
            endings[unsafe_offset=emitted] = UInt64(i)
            emitted += 1
            current_cells = 0
            current_chars = 0
        current_cells += cells
        current_chars += 1
        if current_cells >= maximum_cells:
            endings[unsafe_offset=emitted] = UInt64(i + 1)
            emitted += 1
            current_cells = 0
            current_chars = 0
    if current_chars > 0:
        endings[unsafe_offset=emitted] = UInt64(scalar_count)
        emitted += 1
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )[] = emitted
    return TERMINAL_LAYOUT_OK


# Prefix truncation counts Unicode scalars, including zero-cell combining
# marks. Host Rust owns only Unicode width measurement and string slicing.
@export("prodex_terminal_take_display_cells_v1")
def prodex_terminal_take_display_cells_v1(
    abi: Int64,
    widths_address: UInt,
    scalar_count: Int64,
    maximum_cells: UInt64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi != TERMINAL_LAYOUT_ABI:
        return TERMINAL_LAYOUT_VERSION
    if (
        scalar_count < 0
        or (scalar_count > 0 and widths_address == 0)
        or result_address == 0
    ):
        return TERMINAL_LAYOUT_INVALID

    var widths = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(widths_address)
    )
    var used: UInt64 = 0
    var prefix: Int64 = 0
    for i in range(scalar_count):
        var cells = widths[unsafe_offset=i]
        if cells > 2:
            return TERMINAL_LAYOUT_INVALID
        if used > maximum_cells or cells > maximum_cells - used:
            break
        used += cells
        prefix += 1
    Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )[] = UInt64(prefix)
    return TERMINAL_LAYOUT_OK


# Decide whether a visible piece starts the first line, joins the current
# line with a space, or flushes the current line. Unicode width facts are
# measured by Rust; the presentation policy is solely Mojo-owned.
@export("prodex_terminal_wrap_piece_action_v1")
def prodex_terminal_wrap_piece_action_v1(
    abi: Int64,
    current_empty: Int64,
    current_cells: UInt64,
    piece_cells: UInt64,
    total_cells: UInt64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi != TERMINAL_LAYOUT_ABI:
        return TERMINAL_LAYOUT_VERSION
    if (current_empty != 0 and current_empty != 1) or result_address == 0:
        return TERMINAL_LAYOUT_INVALID

    var action: UInt64 = 0
    if current_empty == 1:
        action = 0
    elif (
        current_cells < total_cells
        and piece_cells <= total_cells - current_cells - UInt64(1)
    ):
        action = 1
    else:
        action = 2
    Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )[] = action
    return TERMINAL_LAYOUT_OK


@fieldwise_init
struct TerminalWrapState(Copyable):
    var status: Int64
    var tokens_written: Int64
    var lines_written: Int64
    var current_line_start: Int64
    var current_line_cells: UInt64
    var current_line_count: Int64


def terminal_wrap_emit_piece(
    piece_start: Int64,
    piece_end: Int64,
    piece_cells: UInt64,
    maximum_cells: UInt64,
    token_starts: Pointer[mut=True, UInt64, _],
    token_ends: Pointer[mut=True, UInt64, _],
    line_starts: Pointer[mut=True, UInt64, _],
    line_ends: Pointer[mut=True, UInt64, _],
    state: TerminalWrapState,
) -> TerminalWrapState:
    if state.status != TERMINAL_LAYOUT_OK:
        return state.copy()
    var token_index = state.tokens_written
    token_starts[unsafe_offset=token_index] = UInt64(piece_start)
    token_ends[unsafe_offset=token_index] = UInt64(piece_end)
    var next = state.copy()
    next.tokens_written = token_index + 1
    if state.current_line_count == 0:
        next.current_line_start = token_index
        next.current_line_cells = piece_cells
        next.current_line_count = 1
    elif (
        state.current_line_cells < maximum_cells
        and piece_cells <= maximum_cells - state.current_line_cells - UInt64(1)
    ):
        next.current_line_cells = state.current_line_cells + UInt64(1) + piece_cells
        next.current_line_count = state.current_line_count + 1
    else:
        var line_index = state.lines_written
        line_starts[unsafe_offset=line_index] = UInt64(state.current_line_start)
        line_ends[unsafe_offset=line_index] = UInt64(token_index)
        next.lines_written = line_index + 1
        next.current_line_start = token_index
        next.current_line_cells = piece_cells
        next.current_line_count = 1
    return next.copy()


def terminal_wrap_emit_word(
    word_start: Int64,
    word_end: Int64,
    widths: Pointer[mut=False, UInt64, _],
    maximum_cells: UInt64,
    token_starts: Pointer[mut=True, UInt64, _],
    token_ends: Pointer[mut=True, UInt64, _],
    line_starts: Pointer[mut=True, UInt64, _],
    line_ends: Pointer[mut=True, UInt64, _],
    state: TerminalWrapState,
) -> TerminalWrapState:
    var next = state.copy()
    var chunk_start = word_start
    var chunk_cells: UInt64 = 0
    var chunk_count: Int64 = 0
    for index in range(word_start, word_end):
        var cells = widths[unsafe_offset=index]
        if cells > 2:
            next.status = TERMINAL_LAYOUT_INVALID
            return next.copy()
        if (
            chunk_count > 0
            and chunk_cells > 0
            and (chunk_cells > maximum_cells
                 or cells > maximum_cells - chunk_cells)
        ):
            next = terminal_wrap_emit_piece(
                chunk_start,
                index,
                chunk_cells,
                maximum_cells,
                token_starts,
                token_ends,
                line_starts,
                line_ends,
                next,
            )
            chunk_start = index
            chunk_cells = 0
            chunk_count = 0
        chunk_cells += cells
        chunk_count += 1
        if chunk_cells >= maximum_cells:
            next = terminal_wrap_emit_piece(
                chunk_start,
                index + 1,
                chunk_cells,
                maximum_cells,
                token_starts,
                token_ends,
                line_starts,
                line_ends,
                next,
            )
            chunk_start = index + 1
            chunk_cells = 0
            chunk_count = 0
    if chunk_count > 0:
        next = terminal_wrap_emit_piece(
            chunk_start,
            word_end,
            chunk_cells,
            maximum_cells,
            token_starts,
            token_ends,
            line_starts,
            line_ends,
            next,
        )
    return next.copy()


def terminal_wrap_flush_paragraph(
    paragraph_token_start: Int64,
    line_starts: Pointer[mut=True, UInt64, _],
    line_ends: Pointer[mut=True, UInt64, _],
    state: TerminalWrapState,
) -> TerminalWrapState:
    var next = state.copy()
    var line_index = state.lines_written
    if state.current_line_count > 0:
        line_starts[unsafe_offset=line_index] = UInt64(state.current_line_start)
        line_ends[unsafe_offset=line_index] = UInt64(state.tokens_written)
        next.lines_written = line_index + 1
    elif state.tokens_written == paragraph_token_start:
        line_starts[unsafe_offset=line_index] = UInt64(state.tokens_written)
        line_ends[unsafe_offset=line_index] = UInt64(state.tokens_written)
        next.lines_written = line_index + 1
    next.current_line_count = 0
    next.current_line_cells = 0
    return next.copy()


# Mojo owns whitespace normalization, LF paragraph boundaries and all line
# grouping. Rust supplies only the original UTF-8 view and Unicode cell widths.
@export("prodex_terminal_wrap_text_plan_v1")
def prodex_terminal_wrap_text_plan_v1(
    abi: Int64,
    text_address: UInt,
    text_length: Int64,
    widths_address: UInt,
    scalar_count: Int64,
    maximum_cells: UInt64,
    token_starts_address: UInt,
    token_ends_address: UInt,
    token_capacity: Int64,
    line_starts_address: UInt,
    line_ends_address: UInt,
    line_capacity: Int64,
    counts_address: UInt,
    counts_capacity: Int64,
) abi("C") -> Int64:
    if abi != TERMINAL_LAYOUT_ABI:
        return TERMINAL_LAYOUT_VERSION
    if (
        text_length < 0
        or (text_length > 0 and text_address == 0)
        or scalar_count < 0
        or (scalar_count > 0 and widths_address == 0)
        or token_starts_address == 0
        or token_ends_address == 0
        or line_starts_address == 0
        or line_ends_address == 0
        or counts_address == 0
        or token_capacity < max(Int64(1), scalar_count)
        or line_capacity < max(Int64(1), scalar_count)
        or counts_capacity < 2
    ):
        return TERMINAL_LAYOUT_INVALID
    var text = ProdexRichStringView(text_address, UInt(text_length))
    if not rich_view_valid(text, 0x7FFFFFFFFFFFFFFF):
        return 2
    var widths = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(widths_address)
    )
    var source = rich_view_ptr(text)
    var cursor: Int64 = 0
    var index: Int64 = 0
    while cursor < text_length:
        var width = rich_codepoint_width(source[unsafe_offset=cursor])
        if index >= scalar_count or widths[unsafe_offset=index] > 2:
            return TERMINAL_LAYOUT_INVALID
        cursor += width
        index += 1
    if index != scalar_count:
        return TERMINAL_LAYOUT_INVALID

    var token_starts = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(token_starts_address)
    )
    var token_ends = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(token_ends_address)
    )
    var line_starts = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(line_starts_address)
    )
    var line_ends = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(line_ends_address)
    )
    var state = TerminalWrapState(0, 0, 0, 0, 0, 0)
    if maximum_cells == 0:
        line_starts[unsafe_offset=0] = 0
        line_ends[unsafe_offset=0] = 0
        state.lines_written = 1
        var zero_counts = Pointer[mut=True, Int64, MutUntrackedOrigin](
            unsafe_from_address=Int(counts_address)
        )
        zero_counts[unsafe_offset=0] = 0
        zero_counts[unsafe_offset=1] = 1
        return TERMINAL_LAYOUT_OK

    var paragraph_token_start: Int64 = 0
    var word_start: Int64 = -1
    var last_was_lf = False
    cursor = 0
    index = 0
    while cursor < text_length:
        var width = rich_codepoint_width(source[unsafe_offset=cursor])
        var codepoint = rich_codepoint(source, cursor, width)
        if codepoint == 10:
            if word_start >= 0:
                state = terminal_wrap_emit_word(
                    word_start,
                    index,
                    widths,
                    maximum_cells,
                    token_starts,
                    token_ends,
                    line_starts,
                    line_ends,
                    state,
                )
                if state.status != TERMINAL_LAYOUT_OK:
                    return state.status
                word_start = -1
            state = terminal_wrap_flush_paragraph(
                paragraph_token_start,
                line_starts,
                line_ends,
                state,
            )
            paragraph_token_start = state.tokens_written
            last_was_lf = True
        elif rich_unicode_space(codepoint):
            if word_start >= 0:
                state = terminal_wrap_emit_word(
                    word_start,
                    index,
                    widths,
                    maximum_cells,
                    token_starts,
                    token_ends,
                    line_starts,
                    line_ends,
                    state,
                )
                if state.status != TERMINAL_LAYOUT_OK:
                    return state.status
                word_start = -1
            last_was_lf = False
        else:
            if word_start < 0:
                word_start = index
            last_was_lf = False
        cursor += width
        index += 1
    if word_start >= 0:
        state = terminal_wrap_emit_word(
            word_start,
            index,
            widths,
            maximum_cells,
            token_starts,
            token_ends,
            line_starts,
            line_ends,
            state,
        )
        if state.status != TERMINAL_LAYOUT_OK:
            return state.status
    if text_length == 0 or not last_was_lf:
        state = terminal_wrap_flush_paragraph(
            paragraph_token_start,
            line_starts,
            line_ends,
            state,
        )
    var counts = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(counts_address)
    )
    counts[unsafe_offset=0] = state.tokens_written
    counts[unsafe_offset=1] = state.lines_written
    return TERMINAL_LAYOUT_OK


@export("prodex_terminal_section_header_plan_v1")
def prodex_terminal_section_header_plan_v1(
    abi: Int64,
    prefix_cells: UInt64,
    total_width: UInt64,
    result_address: UInt,
    result_capacity: Int64,
) abi("C") -> Int64:
    if abi != TERMINAL_LAYOUT_ABI:
        return TERMINAL_LAYOUT_VERSION
    if result_address == 0 or result_capacity < 2:
        return TERMINAL_LAYOUT_INVALID
    var result = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    if prefix_cells >= total_width:
        result[unsafe_offset=0] = 0
        result[unsafe_offset=1] = 0
    else:
        result[unsafe_offset=0] = 1
        result[unsafe_offset=1] = total_width - prefix_cells
    return TERMINAL_LAYOUT_OK


@export("prodex_terminal_session_report_detail_plan_v1")
def prodex_terminal_session_report_detail_plan_v1(
    abi: Int64,
    id_cells_address: UInt,
    profile_present_address: UInt,
    report_count: Int64,
    id_width: UInt64,
    result_address: UInt,
    result_capacity: Int64,
) abi("C") -> Int64:
    if abi != TERMINAL_LAYOUT_ABI:
        return TERMINAL_LAYOUT_VERSION
    if (
        report_count < 0
        or (report_count > 0 and (id_cells_address == 0 or profile_present_address == 0))
        or result_address == 0
        or result_capacity < report_count * 2
    ):
        return TERMINAL_LAYOUT_INVALID
    var ids = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(id_cells_address)
    )
    var profiles = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(profile_present_address)
    )
    var result = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    for index in range(report_count):
        if profiles[unsafe_offset=index] > 1:
            return TERMINAL_LAYOUT_INVALID
        result[unsafe_offset=index * 2] = UInt64(ids[unsafe_offset=index] > id_width)
        result[unsafe_offset=index * 2 + 1] = profiles[unsafe_offset=index]
    return TERMINAL_LAYOUT_OK


# Determine the requested padding under the visible terminal cell budget.
@export("prodex_terminal_pad_cell_spaces_v1")
def prodex_terminal_pad_cell_spaces_v1(
    abi: Int64,
    visible_cells: UInt64,
    total_cells: UInt64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi != TERMINAL_LAYOUT_ABI:
        return TERMINAL_LAYOUT_VERSION
    if result_address == 0:
        return TERMINAL_LAYOUT_INVALID
    var padding: UInt64 = 0
    if total_cells > visible_cells:
        padding = total_cells - visible_cells
    Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )[] = padding
    return TERMINAL_LAYOUT_OK
