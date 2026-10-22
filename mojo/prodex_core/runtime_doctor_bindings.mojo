from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid
from json_sink import JsonSink, js_literal, js_view, js_u64

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


@fieldwise_init
struct DoctorBindingDisplaySource(Copyable):
    var response: UInt64
    var session: UInt64
    var turn_state: UInt64
    var session_id: UInt64
    var total: UInt64
    var profiles: UInt64
    var missing: UInt64
    var top_present: UInt64
    var top_name: ProdexRichStringView
    var top_count: UInt64


@fieldwise_init
struct DoctorBindingDisplayInput(Copyable):
    var active_present: UInt64
    var active: ProdexRichStringView
    var profiles: UInt64
    var selected: UInt64
    var sources: Array[DoctorBindingDisplaySource, 4]


def doctor_binding_display_source(
    writer: Pointer[mut=True, JsonSink, _], source: DoctorBindingDisplaySource,
):
    js_literal(writer, StringSlice("r="))
    js_u64(writer, source.response)
    js_literal(writer, StringSlice(" s="))
    js_u64(writer, source.session)
    js_literal(writer, StringSlice(" t="))
    js_u64(writer, source.turn_state)
    js_literal(writer, StringSlice(" sid="))
    js_u64(writer, source.session_id)
    js_literal(writer, StringSlice(" total="))
    js_u64(writer, source.total)
    js_literal(writer, StringSlice(" profiles="))
    js_u64(writer, source.profiles)
    js_literal(writer, StringSlice(" top="))
    if source.top_present != 0:
        js_view(writer, source.top_name)
        js_literal(writer, StringSlice("="))
        js_u64(writer, source.top_count)
    else:
        js_literal(writer, StringSlice("-"))


def doctor_binding_display(
    writer: Pointer[mut=True, JsonSink, _], input: DoctorBindingDisplayInput,
):
    js_literal(writer, StringSlice("active="))
    if input.active_present != 0:
        js_view(writer, input.active)
    else:
        js_literal(writer, StringSlice("-"))
    js_literal(writer, StringSlice(" profiles="))
    js_u64(writer, input.profiles)
    js_literal(writer, StringSlice(" selected="))
    js_u64(writer, input.selected)
    var missing: UInt64 = 0
    for index in range(4):
        if index == 0:
            js_literal(writer, StringSlice(" | state "))
        elif index == 1:
            js_literal(writer, StringSlice(" | runtime "))
        elif index == 2:
            js_literal(writer, StringSlice(" | journal "))
        else:
            js_literal(writer, StringSlice(" | merged "))
        doctor_binding_display_source(writer, input.sources[index])
        missing += input.sources[index].missing
    js_literal(writer, StringSlice(" | missing="))
    js_u64(writer, missing)


@export("prodex_mojo_runtime_doctor_binding_display_v1")
def prodex_mojo_runtime_doctor_binding_display_v1(
    version: Int64, input_address: UInt, output_address: UInt,
    capacity: Int64, written_address: UInt,
) abi("C") -> Int64:
    if version != 1:
        return 4
    if (input_address == 0 or input_address % 8 != 0 or capacity < 0
        or written_address == 0 or written_address % 8 != 0):
        return 1
    var input = Pointer[mut=False, DoctorBindingDisplayInput, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_address)
    )[].copy()
    if input.active_present > 1 or not rich_view_valid(input.active, 0x7FFFFFFFFFFFFFFF):
        return 1
    for index in range(4):
        var source = input.sources[index].copy()
        if source.top_present > 1 or not rich_view_valid(source.top_name, 0x7FFFFFFFFFFFFFFF):
            return 1
    var writer = JsonSink(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=1), 0, 0, True, False,
    )
    doctor_binding_display(Pointer(to=writer), input)
    if writer.failed or writer.written > capacity:
        return 3
    if output_address == 0:
        return 1
    writer.output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    writer.capacity = capacity
    writer.written = 0
    writer.measuring = False
    doctor_binding_display(Pointer(to=writer), input)
    if writer.failed:
        return 3
    Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))[] = writer.written
    return 0
