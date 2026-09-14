from std.memory import Pointer

from context_text import (
    CONTEXT_SIGNAL_MAX_KEYS,
    CONTEXT_SIGNAL_ROW_WIDTH,
    CONTEXT_TEXT_SOURCE_AFTER,
    CONTEXT_TEXT_SOURCE_BEFORE,
    ContextTextRowsResult,
    CONTEXT_OUTPUT_DIAGNOSTIC_TARGET,
    CONTEXT_OUTPUT_DIAGNOSTIC_FAILURE,
    CONTEXT_OUTPUT_ESLINT,
    CONTEXT_OUTPUT_FAILURE,
    CONTEXT_OUTPUT_JUNIT_FAILURE,
    CONTEXT_OUTPUT_LABEL_NONE,
    CONTEXT_OUTPUT_MAX_BYTES,
    CONTEXT_OUTPUT_NOISY_KEY,
    CONTEXT_OUTPUT_RUST_BACKTRACE,
    CONTEXT_OUTPUT_RUST_EXIT,
    CONTEXT_OUTPUT_WARNING,
    CONTEXT_SIGNAL_COUNTER_COUNT,
    CONTEXT_SIGNAL_MAX_LINES,
    CONTEXT_TEXT_ABI_VERSION,
    ProdexStringView,
    context_output_contains,
    context_output_has_file_location,
    context_output_is_diagnostic_failure_summary,
    context_output_line_semantics,
    context_output_looks_like_location_path,
    context_output_starts,
    context_text_trim_bounds,
    context_text_counts_have_signal,
    context_text_intern,
    context_text_line_counts,
    context_text_required_hash_capacity,
    context_text_view_is_valid,
    context_text_views_equal,
)


comptime CONTEXT_OUTPUT_ANALYSIS_WIDTH: Int64 = 10
comptime CONTEXT_SIGNAL_MAX_RANGES: Int64 = 1_024


@export("prodex_context_analyze_command_output_v1")
def prodex_context_analyze_command_output_v1(
    abi_version: Int64,
    lines: Pointer[mut=False, ProdexStringView, _],
    line_count: Int64,
    output: Pointer[mut=True, Int64, _],
    output_count: Int64,
) abi("C") -> Int64:
    if output_count != CONTEXT_OUTPUT_ANALYSIS_WIDTH:
        return 1
    for index in range(CONTEXT_OUTPUT_ANALYSIS_WIDTH):
        output[unsafe_offset=index] = 0
    if abi_version != CONTEXT_TEXT_ABI_VERSION:
        return 4
    if line_count < 0 or line_count > CONTEXT_SIGNAL_MAX_LINES:
        return 1

    for index in range(line_count):
        var view = lines[unsafe_offset=index].copy()
        if view.len > UInt(CONTEXT_OUTPUT_MAX_BYTES):
            return 1
        if not context_text_view_is_valid(view):
            return 2
        var bounds = context_text_trim_bounds(
            view.ptr.unsafe_value(), 0, Int64(view.len)
        )
        if bounds[0] < bounds[1]:
            output[unsafe_offset=6] += 1
        var semantics = context_output_line_semantics(
            view.ptr.unsafe_value(), Int64(view.len)
        )
        var flags = semantics.flags
        if flags & (CONTEXT_OUTPUT_JUNIT_FAILURE | CONTEXT_OUTPUT_ESLINT) != 0:
            output[unsafe_offset=0] = 1
        if (
            flags & CONTEXT_OUTPUT_DIAGNOSTIC_TARGET != 0
            or flags & CONTEXT_OUTPUT_RUST_BACKTRACE != 0
            or flags & CONTEXT_OUTPUT_FAILURE != 0
        ):
            output[unsafe_offset=1] += 1
        if context_output_has_file_location(
            view.ptr.unsafe_value(), bounds[0], bounds[1]
        ):
            output[unsafe_offset=2] += 1
        if flags & CONTEXT_OUTPUT_RUST_BACKTRACE != 0:
            output[unsafe_offset=3] += 1
        if flags & CONTEXT_OUTPUT_RUST_EXIT != 0:
            output[unsafe_offset=4] += 1
        if semantics.diagnostic_label != CONTEXT_OUTPUT_LABEL_NONE:
            output[unsafe_offset=5] += 1
        if semantics.noisy_label != CONTEXT_OUTPUT_LABEL_NONE:
            output[unsafe_offset=7] += 1
        if flags & CONTEXT_OUTPUT_NOISY_KEY != 0:
            output[unsafe_offset=8] += 1
        if flags & (CONTEXT_OUTPUT_FAILURE | CONTEXT_OUTPUT_WARNING) != 0:
            output[unsafe_offset=9] = 1
    return 0


def context_critical_generated_header(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_starts["pcs:"](ptr, start, end)
        or context_output_starts["# prodex context saver:"](ptr, start, end)
        or context_output_starts["sum:"](ptr, start, end)
        or context_output_starts["rust/cargo summary:"](ptr, start, end)
        or context_output_starts["diagnostic summary:"](ptr, start, end)
        or context_output_starts["success output summary:"](ptr, start, end)
        or context_output_starts["command output summary:"](ptr, start, end)
        or context_output_starts["baseline compaction:"](ptr, start, end)
        or context_output_starts["base:"](ptr, start, end)
        or context_output_starts["intent matches:"](ptr, start, end)
        or context_output_starts["int:"](ptr, start, end)
        or context_output_starts["diagnostics ("](ptr, start, end)
        or context_output_starts["locations ("](ptr, start, end)
        or context_output_starts["failed tests ("](ptr, start, end)
        or context_output_starts["exit statuses ("](ptr, start, end)
        or context_output_starts["key lines ("](ptr, start, end)
        or context_output_starts["critical blocks:"](ptr, start, end)
    )


def context_critical_warning_only(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var warning = (
        context_output_starts["warning"](ptr, start, end)
        or context_output_contains[" warning "](ptr, start, end)
        or context_output_contains["warning ts"](ptr, start, end)
        or context_output_contains[": warning ts"](ptr, start, end)
        or context_output_contains[" - warning ts"](ptr, start, end)
    )
    return (
        warning
        and not context_output_contains["error"](ptr, start, end)
        and not context_output_contains["failed"](ptr, start, end)
        and not context_output_contains["panicked"](ptr, start, end)
    )


def context_critical_priority(
    view: ProdexStringView,
    counts: Pointer[mut=False, Int64, _],
    index: Int64,
) -> Int64:
    ref ptr = view.ptr.unsafe_value()
    var bounds = context_text_trim_bounds(ptr, 0, Int64(view.len))
    if context_critical_generated_header(ptr, bounds[0], bounds[1]):
        return 5
    if context_critical_warning_only(ptr, bounds[0], bounds[1]):
        return 4
    var offset = index * CONTEXT_SIGNAL_COUNTER_COUNT
    if (
        counts[unsafe_offset=offset] > 0
        or counts[unsafe_offset=offset + 3] > 0
        or counts[unsafe_offset=offset + 4] > 0
        or context_output_is_diagnostic_failure_summary(ptr, bounds[0], bounds[1])
    ):
        return 0
    if counts[unsafe_offset=offset + 1] > 0 or counts[unsafe_offset=offset + 5] > 0:
        return 1
    if counts[unsafe_offset=offset + 6] > 0:
        return 2
    return 3


@export("prodex_context_select_critical_lines_v1")
def prodex_context_select_critical_lines_v1(
    abi_version: Int64,
    lines: Pointer[mut=False, ProdexStringView, _],
    normalized_keys: Pointer[mut=False, ProdexStringView, _],
    counts: Pointer[mut=False, Int64, _],
    line_count: Int64,
    budget: Int64,
    output: Pointer[mut=True, Int64, _],
    output_count: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    output_count[] = 0
    if abi_version != CONTEXT_TEXT_ABI_VERSION:
        return 4
    if line_count < 0 or line_count > CONTEXT_SIGNAL_MAX_LINES or budget < 1:
        return 1
    var selected_budget = budget - 1
    if selected_budget < 1:
        selected_budget = 1
    var failure_present = False
    for index in range(line_count):
        var view = lines[unsafe_offset=index].copy()
        if view.len > UInt(CONTEXT_OUTPUT_MAX_BYTES) or not context_text_view_is_valid(view):
            return 2
        var key = normalized_keys[unsafe_offset=index].copy()
        if key.len > UInt(CONTEXT_OUTPUT_MAX_BYTES) or not context_text_view_is_valid(key):
            return 2
        for counter in range(CONTEXT_SIGNAL_COUNTER_COUNT):
            if counts[unsafe_offset=index * CONTEXT_SIGNAL_COUNTER_COUNT + counter] < 0:
                return 1
        var priority = context_critical_priority(view, counts, index)
        if priority == 0:
            failure_present = True

    for priority in range(6):
        for index in range(line_count):
            var view = lines[unsafe_offset=index].copy()
            if context_critical_priority(view, counts, index) != Int64(priority):
                continue
            ref ptr = view.ptr.unsafe_value()
            var bounds = context_text_trim_bounds(ptr, 0, Int64(view.len))
            if failure_present and context_critical_warning_only(ptr, bounds[0], bounds[1]):
                continue
            var duplicate = False
            for selected in range(output_count[]):
                var selected_index = output[unsafe_offset=selected]
                if context_text_views_equal(
                    normalized_keys[unsafe_offset=index],
                    normalized_keys[unsafe_offset=selected_index],
                ):
                    duplicate = True
                    break
            if duplicate:
                continue
            output[unsafe_offset=output_count[]] = index
            output_count[] += 1
            if output_count[] >= selected_budget:
                return 0
    return 0



@export("prodex_context_looks_like_location_path_v1")
def prodex_context_looks_like_location_path_v1(
    abi_version: Int64,
    path: Pointer[mut=False, ProdexStringView, _],
    output: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    output[] = 0
    if abi_version != CONTEXT_TEXT_ABI_VERSION:
        return 4
    var view = path[].copy()
    if view.len > UInt(CONTEXT_OUTPUT_MAX_BYTES):
        return 1
    if not context_text_view_is_valid(view):
        return 2
    var bounds = Tuple[Int64, Int64](0, Int64(view.len))
    while bounds[0] < bounds[1] and (
        view.ptr.unsafe_value()[unsafe_offset=bounds[0]] == 60
        or view.ptr.unsafe_value()[unsafe_offset=bounds[0]] == 62
        or view.ptr.unsafe_value()[unsafe_offset=bounds[0]] == 45
        or view.ptr.unsafe_value()[unsafe_offset=bounds[0]] == 58
        or view.ptr.unsafe_value()[unsafe_offset=bounds[0]] == 32
    ):
        bounds[0] += 1
    while bounds[1] > bounds[0] and (
        view.ptr.unsafe_value()[unsafe_offset=bounds[1] - 1] == 60
        or view.ptr.unsafe_value()[unsafe_offset=bounds[1] - 1] == 62
        or view.ptr.unsafe_value()[unsafe_offset=bounds[1] - 1] == 45
        or view.ptr.unsafe_value()[unsafe_offset=bounds[1] - 1] == 58
        or view.ptr.unsafe_value()[unsafe_offset=bounds[1] - 1] == 32
    ):
        bounds[1] -= 1
    if context_output_looks_like_location_path(
        view.ptr.unsafe_value(), bounds[0], bounds[1]
    ):
        output[] = 1
    return 0
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


@export("prodex_context_prepare_signal_rows_v1")
def prodex_context_prepare_signal_rows_v1(
    abi_version: Int64,
    before_views: Pointer[mut=False, ProdexStringView, _],
    before_counts: Pointer[mut=False, Int64, _],
    before_count: Int64,
    after_views: Pointer[mut=False, ProdexStringView, _],
    after_counts: Pointer[mut=False, Int64, _],
    after_count: Int64,
    before_rows: Pointer[mut=True, Int64, _],
    before_rows_capacity: Int64,
    after_available: Pointer[mut=True, Int64, _],
    key_capacity: Int64,
    hash_slots: Pointer[mut=True, Int64, _],
    hash_capacity: Int64,
    key_hashes: Pointer[mut=True, UInt64, _],
    key_sources: Pointer[mut=True, Int64, _],
    key_indices: Pointer[mut=True, Int64, _],
    result: Pointer[mut=True, ContextTextRowsResult, _],
) abi("C") -> Int64:
    if abi_version != CONTEXT_TEXT_ABI_VERSION:
        return 4
    if (
        before_count < 0
        or before_count > CONTEXT_SIGNAL_MAX_LINES
        or after_count < 0
        or after_count > CONTEXT_SIGNAL_MAX_LINES
        or key_capacity < 0
        or key_capacity > CONTEXT_SIGNAL_MAX_KEYS
        or hash_capacity < 1
    ):
        return 1

    var required_before_rows = before_count * CONTEXT_SIGNAL_ROW_WIDTH
    var total_lines = before_count + after_count
    var required_key_capacity = total_lines
    if required_key_capacity > CONTEXT_SIGNAL_MAX_KEYS:
        required_key_capacity = CONTEXT_SIGNAL_MAX_KEYS
    var required_hash_capacity = context_text_required_hash_capacity(
        required_key_capacity
    )
    result[] = ContextTextRowsResult(
        CONTEXT_TEXT_ABI_VERSION,
        before_count,
        after_count,
        0,
        0,
        0,
        required_before_rows,
        required_key_capacity,
        required_hash_capacity,
    )
    if (
        before_rows_capacity < required_before_rows
        or key_capacity < required_key_capacity
        or hash_capacity < required_hash_capacity
    ):
        return 1

    for line in range(before_count):
        if not context_text_view_is_valid(before_views[unsafe_offset=line]):
            return 2
        for counter in range(CONTEXT_SIGNAL_COUNTER_COUNT):
            if (
                before_counts[
                    unsafe_offset=line * CONTEXT_SIGNAL_COUNTER_COUNT + counter
                ]
                < 0
            ):
                return 2
    for line in range(after_count):
        if not context_text_view_is_valid(after_views[unsafe_offset=line]):
            return 2
        for counter in range(CONTEXT_SIGNAL_COUNTER_COUNT):
            if (
                after_counts[
                    unsafe_offset=line * CONTEXT_SIGNAL_COUNTER_COUNT + counter
                ]
                < 0
            ):
                return 2

    for slot in range(hash_capacity):
        hash_slots[unsafe_offset=slot] = -1
    var key_count: Int64 = 0
    var after_signal_line_count: Int64 = 0

    for line in range(after_count):
        var line_counts = context_text_line_counts(after_counts, line)
        if not context_text_counts_have_signal(line_counts):
            continue
        var key_id = context_text_intern(
            after_views[unsafe_offset=line],
            CONTEXT_TEXT_SOURCE_AFTER,
            line,
            before_views,
            after_views,
            hash_slots,
            key_hashes,
            key_sources,
            key_indices,
            after_available,
            Pointer(to=key_count),
            key_capacity,
            hash_capacity,
        )
        if key_id < 0:
            return 3
        if after_available[unsafe_offset=key_id] == 9223372036854775807:
            return 2
        after_available[unsafe_offset=key_id] += 1
        after_signal_line_count += 1

    for line in range(before_count):
        var line_counts = context_text_line_counts(before_counts, line)
        var key_id: Int64 = -1
        if context_text_counts_have_signal(line_counts):
            key_id = context_text_intern(
                before_views[unsafe_offset=line],
                CONTEXT_TEXT_SOURCE_BEFORE,
                line,
                before_views,
                after_views,
                hash_slots,
                key_hashes,
                key_sources,
                key_indices,
                after_available,
                Pointer(to=key_count),
                key_capacity,
                hash_capacity,
            )
            if key_id < 0:
                return 3
        before_rows[unsafe_offset=line * CONTEXT_SIGNAL_ROW_WIDTH] = key_id
        var row = line * CONTEXT_SIGNAL_ROW_WIDTH
        before_rows[unsafe_offset=row + 1] = line_counts[0]
        before_rows[unsafe_offset=row + 2] = line_counts[1]
        before_rows[unsafe_offset=row + 3] = line_counts[2]
        before_rows[unsafe_offset=row + 4] = line_counts[3]
        before_rows[unsafe_offset=row + 5] = line_counts[4]
        before_rows[unsafe_offset=row + 6] = line_counts[5]
        before_rows[unsafe_offset=row + 7] = line_counts[6]

    result[] = ContextTextRowsResult(
        CONTEXT_TEXT_ABI_VERSION,
        before_count,
        after_count,
        required_before_rows,
        key_count,
        after_signal_line_count,
        required_before_rows,
        required_key_capacity,
        required_hash_capacity,
    )
    return 0
