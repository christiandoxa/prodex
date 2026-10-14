from std.memory import Pointer

# Deterministic terminal layout policies. Rust measures true terminal cell
# widths (including Unicode) and owns styling, strings and the terminal host.
# Mojo exclusively decides label, session-column and viewport dimensions.
comptime TERMINAL_LAYOUT_ABI: Int64 = 1
comptime TERMINAL_LAYOUT_OK: Int64 = 0
comptime TERMINAL_LAYOUT_INVALID: Int64 = 1
comptime TERMINAL_LAYOUT_VERSION: Int64 = 4


def terminal_clamp(value: UInt64, low: UInt64, high: UInt64) -> UInt64:
    if value < low:
        return low
    if value > high:
        return high
    return value


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
