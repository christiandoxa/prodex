from std.memory import Pointer

from rich_text import rich_view_matches_literal, rich_view_valid
from rich_types import ProdexRichStringView
from runtime_doctor_marker import (
    RUNTIME_DOCTOR_MARKER_MAX_BYTES,
    runtime_doctor_marker_failure_class,
    runtime_doctor_marker_selection_bucket,
)
from runtime_math import INT64_MAX

comptime RUNTIME_DOCTOR_MARKER_SUMMARY_COUNTS_ABI_VERSION: Int64 = 1
comptime RUNTIME_DOCTOR_MARKER_SUMMARY_COUNTS_MAX_BATCH: Int64 = 256
comptime RUNTIME_DOCTOR_COMPACT_EXIT_COUNTS_ABI_VERSION: Int64 = 1


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


def runtime_doctor_compact_exit_bucket(marker: ProdexRichStringView) -> Int64:
    if rich_view_matches_literal["compact_candidate_exhausted"](
        marker, False
    ) or rich_view_matches_literal["compact_exit_candidate_exhausted"](
        marker, False
    ):
        return 0
    if rich_view_matches_literal["compact_committed"](
        marker, False
    ) or rich_view_matches_literal["compact_exit_committed"](marker, False):
        return 1
    if rich_view_matches_literal["compact_committed_owner"](
        marker, False
    ) or rich_view_matches_literal["compact_exit_committed_owner"](
        marker, False
    ):
        return 2
    if rich_view_matches_literal["compact_followup_owner"](
        marker, False
    ) or rich_view_matches_literal["compact_exit_followup_owner"](
        marker, False
    ):
        return 3
    if rich_view_matches_literal["compact_lineage_released"](
        marker, False
    ) or rich_view_matches_literal["compact_exit_lineage_released"](
        marker, False
    ):
        return 4
    if rich_view_matches_literal["compact_overload_conservative_retry"](
        marker, False
    ) or rich_view_matches_literal["compact_exit_overload_conservative_retry"](
        marker, False
    ):
        return 5
    if rich_view_matches_literal["compact_precommit_budget_exhausted"](
        marker, False
    ) or rich_view_matches_literal["compact_exit_precommit_budget_exhausted"](
        marker, False
    ):
        return 6
    if rich_view_matches_literal["compact_pressure_shed"](
        marker, False
    ) or rich_view_matches_literal["compact_exit_pressure_shed"](marker, False):
        return 7
    if rich_view_matches_literal["compact_quota_unclassified"](
        marker, False
    ) or rich_view_matches_literal["compact_exit_quota_unclassified"](
        marker, False
    ):
        return 8
    if rich_view_matches_literal["compact_retryable_failure"](
        marker, False
    ) or rich_view_matches_literal["compact_exit_retryable_failure"](
        marker, False
    ):
        return 9
    if rich_view_matches_literal["compact_transport_failure"](marker, False):
        return 10
    return -1


@export("prodex_mojo_runtime_doctor_compact_exit_counts_v1")
def prodex_mojo_runtime_doctor_compact_exit_counts_v1(
    abi_version: Int64,
    marker_views_address: UInt,
    counts_address: UInt,
    count: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != RUNTIME_DOCTOR_COMPACT_EXIT_COUNTS_ABI_VERSION:
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
    for index in range(11):
        if output[unsafe_offset=index] < 0:
            return 2
    for index in range(count):
        if (
            not rich_view_valid(markers[unsafe_offset=index].copy(), INT64_MAX)
            or counts[unsafe_offset=index] < 0
        ):
            return 2

    for index in range(count):
        var bucket = runtime_doctor_compact_exit_bucket(
            markers[unsafe_offset=index].copy()
        )
        if bucket >= 0:
            var amount = counts[unsafe_offset=index]
            if amount > INT64_MAX - output[unsafe_offset=bucket]:
                return 5
            output[unsafe_offset=bucket] += amount
    return 0
