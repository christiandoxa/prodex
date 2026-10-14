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
