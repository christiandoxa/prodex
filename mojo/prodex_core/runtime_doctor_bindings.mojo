from std.memory import Pointer

# Runtime-doctor profile binding audits. Rust owns map traversal, known-name
# normalization and string materialization; Mojo owns the complete numeric
# aggregation, oldest/newest bounds, missing-sample admission and ranking.
comptime DOCTOR_BINDING_ABI: Int64 = 1
comptime DOCTOR_BINDING_MAX_ROWS: Int64 = 1_000_000
comptime DOCTOR_BINDING_MAX_PROFILES: Int64 = 1_000_000
comptime DOCTOR_BINDING_ROW_WIDTH: Int64 = 4
comptime DOCTOR_BINDING_PROFILE_WIDTH: Int64 = 5
comptime DOCTOR_BINDING_SAMPLES_MAX: Int64 = 8
comptime DOCTOR_BINDING_SUMMARY_WIDTH: Int64 = 10


@export("prodex_mojo_runtime_doctor_binding_source_v1")
def prodex_mojo_runtime_doctor_binding_source_v1(
    abi_version: Int64,
    rows_address: UInt,
    row_count: Int64,
    profile_count: Int64,
    profile_counts_address: UInt,
    ordered_indices_address: UInt,
    scratch_address: UInt,
    sample_indices_address: UInt,
    summary_address: UInt,
) abi("C") -> Int64:
    if abi_version != DOCTOR_BINDING_ABI:
        return 4
    if (
        row_count < 0
        or row_count > DOCTOR_BINDING_MAX_ROWS
        or profile_count < 0
        or profile_count > DOCTOR_BINDING_MAX_PROFILES
        or summary_address == 0
        or (row_count > 0 and (rows_address == 0 or sample_indices_address == 0))
        or (
            profile_count > 0
            and (
                profile_counts_address == 0
                or ordered_indices_address == 0
                or scratch_address == 0
            )
        )
    ):
        return 1

    var rows = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(rows_address)
    )
    var counts = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(profile_counts_address)
    )
    var order = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(ordered_indices_address)
    )
    var scratch = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(scratch_address)
    )
    var samples = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(sample_indices_address)
    )
    var summary = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(summary_address)
    )

    # Validate the whole input before publishing any output.
    for i in range(row_count):
        var base = i * DOCTOR_BINDING_ROW_WIDTH
        var profile = rows[unsafe_offset=base]
        var kind = rows[unsafe_offset=base + 1]
        var known = rows[unsafe_offset=base + 3]
        if (
            profile < 0
            or profile >= profile_count
            or kind < 0
            or kind > 3
            or (known != 0 and known != 1)
        ):
            return 1

    for i in range(DOCTOR_BINDING_SUMMARY_WIDTH):
        summary[unsafe_offset=i] = 0
    for i in range(profile_count):
        order[unsafe_offset=i] = i
        for kind in range(DOCTOR_BINDING_PROFILE_WIDTH):
            counts[unsafe_offset=i * DOCTOR_BINDING_PROFILE_WIDTH + kind] = 0

    for i in range(row_count):
        var base = i * DOCTOR_BINDING_ROW_WIDTH
        var profile = rows[unsafe_offset=base]
        var kind = rows[unsafe_offset=base + 1]
        var bound_at = rows[unsafe_offset=base + 2]
        var known = rows[unsafe_offset=base + 3]
        counts[unsafe_offset=profile * DOCTOR_BINDING_PROFILE_WIDTH + kind] += 1
        counts[unsafe_offset=profile * DOCTOR_BINDING_PROFILE_WIDTH + 4] += 1
        summary[unsafe_offset=5 + kind] += 1
        summary[unsafe_offset=9] += 1
        if summary[unsafe_offset=0] == 0:
            summary[unsafe_offset=0] = 1
            summary[unsafe_offset=1] = bound_at
            summary[unsafe_offset=2] = bound_at
        else:
            summary[unsafe_offset=1] = min(summary[unsafe_offset=1], bound_at)
            summary[unsafe_offset=2] = max(summary[unsafe_offset=2], bound_at)
        if known == 0:
            summary[unsafe_offset=3] += 1
            var taken = summary[unsafe_offset=4]
            if taken < DOCTOR_BINDING_SAMPLES_MAX:
                samples[unsafe_offset=taken] = i
                summary[unsafe_offset=4] = taken + 1

    # Stable merge sort: descending binding totals, then ascending caller
    # BTreeMap name index (which preserves UTF-8 lexical tie-breaking).
    var width: Int64 = 1
    while width < profile_count:
        var begin: Int64 = 0
        while begin < profile_count:
            var middle = min(begin + width, profile_count)
            var finish = min(middle + width, profile_count)
            var left = begin
            var right = middle
            var out = begin
            while left < middle and right < finish:
                var left_index = order[unsafe_offset=left]
                var right_index = order[unsafe_offset=right]
                var left_total = counts[
                    unsafe_offset=left_index * DOCTOR_BINDING_PROFILE_WIDTH + 4
                ]
                var right_total = counts[
                    unsafe_offset=right_index * DOCTOR_BINDING_PROFILE_WIDTH + 4
                ]
                if (
                    right_total > left_total
                    or (right_total == left_total and right_index < left_index)
                ):
                    scratch[unsafe_offset=out] = right_index
                    right += 1
                else:
                    scratch[unsafe_offset=out] = left_index
                    left += 1
                out += 1
            while left < middle:
                scratch[unsafe_offset=out] = order[unsafe_offset=left]
                left += 1
                out += 1
            while right < finish:
                scratch[unsafe_offset=out] = order[unsafe_offset=right]
                right += 1
                out += 1
            begin = finish
        for i in range(profile_count):
            order[unsafe_offset=i] = scratch[unsafe_offset=i]
        width *= 2
    return 0
