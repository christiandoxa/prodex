from std.memory import Pointer

from rich_text import rich_view_valid
from rich_types import ProdexRichStringView
from runtime_doctor_marker import (
    RUNTIME_DOCTOR_MARKER_MAX_BYTES,
    runtime_doctor_marker_failure_class,
    runtime_doctor_marker_selection_bucket,
)
from runtime_math import INT64_MAX

comptime RUNTIME_DOCTOR_MARKER_SUMMARY_COUNTS_ABI_VERSION: Int64 = 1
comptime RUNTIME_DOCTOR_MARKER_SUMMARY_COUNTS_MAX_BATCH: Int64 = 256


@export("prodex_mojo_runtime_doctor_marker_summary_counts_v1")
def prodex_mojo_runtime_doctor_marker_summary_counts_v1(
    abi_version: Int64,
    marker_views_address: UInt,
    counts_address: UInt,
    count: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != RUNTIME_DOCTOR_MARKER_SUMMARY_COUNTS_ABI_VERSION:
        return 4
    if (
        count < 0
        or marker_views_address == 0
        or counts_address == 0
        or output_address == 0
    ):
        return 1
    if count > RUNTIME_DOCTOR_MARKER_SUMMARY_COUNTS_MAX_BATCH:
        return 3

    var markers = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(marker_views_address)
    )
    var counts = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(counts_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(10):
        if output[unsafe_offset=index] < 0:
            return 2
    for index in range(count):
        var marker = markers[unsafe_offset=index].copy()
        if (
            not rich_view_valid(marker, INT64_MAX)
            or counts[unsafe_offset=index] < 0
        ):
            return 2

    for index in range(count):
        var marker = markers[unsafe_offset=index].copy()
        if marker.len > UInt(RUNTIME_DOCTOR_MARKER_MAX_BYTES):
            continue
        var amount = counts[unsafe_offset=index]
        var selection = runtime_doctor_marker_selection_bucket(marker)
        if selection > 0:
            var output_index = selection - 1
            if amount > INT64_MAX - output[unsafe_offset=output_index]:
                return 5
            output[unsafe_offset=output_index] += amount
        var failure_class = runtime_doctor_marker_failure_class(marker)
        if failure_class > 0:
            var output_index = failure_class + 3
            if amount > INT64_MAX - output[unsafe_offset=output_index]:
                return 5
            output[unsafe_offset=output_index] += amount
    return 0
