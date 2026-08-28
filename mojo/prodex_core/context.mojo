comptime CONTEXT_SIGNAL_COUNTER_COUNT: Int64 = 7
comptime CONTEXT_SIGNAL_ROW_WIDTH: Int64 = 8
comptime CONTEXT_SIGNAL_MAX_LINES: Int64 = 65_536
comptime CONTEXT_SIGNAL_MAX_KEYS: Int64 = 65_536
comptime CONTEXT_SIGNAL_MAX_RANGES: Int64 = 1_024
comptime UINT64_MAX: UInt64 = 18446744073709551615

@export("prodex_context_estimate_tokens")
def prodex_context_estimate_tokens(chars: UInt64, words: UInt64) abi("C") -> UInt64:
    var char_tokens = chars / 4
    if chars % 4 != 0:
        char_tokens += 1

    var word_groups = words / 3
    if word_groups > UINT64_MAX / 4:
        return UINT64_MAX
    var word_tokens = word_groups * 4
    if words % 3 == 1:
        if word_tokens > UINT64_MAX - 2:
            return UINT64_MAX
        word_tokens += 2
    elif words % 3 == 2:
        if word_tokens > UINT64_MAX - 3:
            return UINT64_MAX
        word_tokens += 3

    if char_tokens > word_tokens:
        return char_tokens
    return word_tokens

def critical_signal_row_value(
    rows: Pointer[mut=False, Int64, _],
    line: Int64,
    field: Int64,
) -> Int64:
    return rows[unsafe_offset=(line * CONTEXT_SIGNAL_ROW_WIDTH) + field]

def critical_signal_ranges_overlap(
    rows: Pointer[mut=False, Int64, _],
    line: Int64,
    remaining_errors: Int64,
    remaining_file_locations: Int64,
    remaining_diff_hunks: Int64,
    remaining_test_failures: Int64,
    remaining_exit_codes: Int64,
    remaining_stack_markers: Int64,
    remaining_rust_diagnostics: Int64,
) -> Bool:
    return (
        (critical_signal_row_value(rows, line, 1) > 0 and remaining_errors > 0)
        or (critical_signal_row_value(rows, line, 2) > 0 and remaining_file_locations > 0)
        or (critical_signal_row_value(rows, line, 3) > 0 and remaining_diff_hunks > 0)
        or (critical_signal_row_value(rows, line, 4) > 0 and remaining_test_failures > 0)
        or (critical_signal_row_value(rows, line, 5) > 0 and remaining_exit_codes > 0)
        or (critical_signal_row_value(rows, line, 6) > 0 and remaining_stack_markers > 0)
        or (critical_signal_row_value(rows, line, 7) > 0 and remaining_rust_diagnostics > 0)
    )

def critical_signal_saturating_sub(left: Int64, right: Int64) -> Int64:
    if right >= left:
        return 0
    return left - right

@export("prodex_context_lost_line_ranges_batch")
def prodex_context_lost_line_ranges_batch(
    before_rows: Pointer[mut=False, Int64, _],
    after_available: Pointer[mut=True, Int64, _],
    initial_loss: Pointer[mut=False, Int64, _],
    output_ranges: Pointer[mut=True, Int64, _],
    output_count: Pointer[mut=True, Int64, _],
    line_count: Int64,
    key_count: Int64,
    context_lines: Int64,
    max_ranges: Int64,
    max_range_lines: Int64,
) abi("C") -> Int64:
    if line_count < 0 or line_count > CONTEXT_SIGNAL_MAX_LINES:
        return 1
    if key_count < 0 or key_count > CONTEXT_SIGNAL_MAX_KEYS:
        return 1
    if context_lines < 0 or context_lines > CONTEXT_SIGNAL_MAX_LINES:
        return 1
    if max_ranges < 0 or max_ranges > CONTEXT_SIGNAL_MAX_RANGES:
        return 1
    if max_range_lines < 0 or max_range_lines > CONTEXT_SIGNAL_MAX_LINES:
        return 1
    for counter in range(CONTEXT_SIGNAL_COUNTER_COUNT):
        if initial_loss[unsafe_offset=counter] < 0:
            return 2

    var remaining_errors = initial_loss[unsafe_offset=0]
    var remaining_file_locations = initial_loss[unsafe_offset=1]
    var remaining_diff_hunks = initial_loss[unsafe_offset=2]
    var remaining_test_failures = initial_loss[unsafe_offset=3]
    var remaining_exit_codes = initial_loss[unsafe_offset=4]
    var remaining_stack_markers = initial_loss[unsafe_offset=5]
    var remaining_rust_diagnostics = initial_loss[unsafe_offset=6]
    var emitted: Int64 = 0
    var index: Int64 = 0
    while index < line_count:
        if (
            remaining_errors == 0
            and remaining_file_locations == 0
            and remaining_diff_hunks == 0
            and remaining_test_failures == 0
            and remaining_exit_codes == 0
            and remaining_stack_markers == 0
            and remaining_rust_diagnostics == 0
        ):
            break

        var key_id = critical_signal_row_value(before_rows, index, 0)
        if key_id < -1 or key_id >= key_count:
            return 2
        for counter in range(CONTEXT_SIGNAL_COUNTER_COUNT):
            if critical_signal_row_value(before_rows, index, counter + 1) < 0:
                return 2
        if key_id >= 0 and after_available[unsafe_offset=key_id] > 0:
            after_available[unsafe_offset=key_id] -= 1
            index += 1
            continue
        if key_id < 0 or not critical_signal_ranges_overlap(
            before_rows,
            index,
            remaining_errors,
            remaining_file_locations,
            remaining_diff_hunks,
            remaining_test_failures,
            remaining_exit_codes,
            remaining_stack_markers,
            remaining_rust_diagnostics,
        ):
            index += 1
            continue

        var signal_line = index + 1
        var start = signal_line - context_lines
        if start < 1:
            start = 1
        var end = signal_line + context_lines
        if end > line_count:
            end = line_count
        var bounded_max_lines = max_range_lines
        if bounded_max_lines < 1:
            bounded_max_lines = 1
        while end - start + 1 > bounded_max_lines:
            if signal_line - start > end - signal_line:
                start += 1
            else:
                end -= 1

        var can_emit = True
        if emitted > 0:
            var previous_start = output_ranges[unsafe_offset=(emitted - 1) * 2]
            var previous_end = output_ranges[unsafe_offset=(emitted - 1) * 2 + 1]
            if start < previous_end:
                if end > previous_end:
                    output_ranges[unsafe_offset=(emitted - 1) * 2 + 1] = end
                can_emit = False
            elif emitted >= max_ranges:
                break
        elif emitted >= max_ranges:
            break
        if can_emit:
            output_ranges[unsafe_offset=emitted * 2] = start
            output_ranges[unsafe_offset=emitted * 2 + 1] = end
            emitted += 1

        remaining_errors = critical_signal_saturating_sub(
            remaining_errors,
            critical_signal_row_value(before_rows, index, 1),
        )
        remaining_file_locations = critical_signal_saturating_sub(
            remaining_file_locations,
            critical_signal_row_value(before_rows, index, 2),
        )
        remaining_diff_hunks = critical_signal_saturating_sub(
            remaining_diff_hunks,
            critical_signal_row_value(before_rows, index, 3),
        )
        remaining_test_failures = critical_signal_saturating_sub(
            remaining_test_failures,
            critical_signal_row_value(before_rows, index, 4),
        )
        remaining_exit_codes = critical_signal_saturating_sub(
            remaining_exit_codes,
            critical_signal_row_value(before_rows, index, 5),
        )
        remaining_stack_markers = critical_signal_saturating_sub(
            remaining_stack_markers,
            critical_signal_row_value(before_rows, index, 6),
        )
        remaining_rust_diagnostics = critical_signal_saturating_sub(
            remaining_rust_diagnostics,
            critical_signal_row_value(before_rows, index, 7),
        )
        index += 1

    output_count[unsafe_offset=0] = emitted
    return 0

@export("prodex_context_signal_diff")
def prodex_context_signal_diff(
    before: Pointer[mut=False, Int64, _],
    after: Pointer[mut=False, Int64, _],
    lost: Pointer[mut=True, Int64, _],
    gained: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    for index in range(7):
        var before_value = before[unsafe_offset=index]
        var after_value = after[unsafe_offset=index]
        if before_value < 0 or after_value < 0:
            return 1
        if before_value > after_value:
            lost[unsafe_offset=index] = before_value - after_value
            gained[unsafe_offset=index] = 0
        else:
            lost[unsafe_offset=index] = 0
            gained[unsafe_offset=index] = after_value - before_value
    return 0
