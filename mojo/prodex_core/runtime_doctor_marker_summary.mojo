from std.memory import Pointer

from rich_text import rich_view_matches_literal, rich_view_valid, rich_views_equal
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
comptime RUNTIME_DOCTOR_MARKER_AGGREGATION_ABI_VERSION: Int64 = 1
comptime RUNTIME_DOCTOR_MARKER_AGGREGATION_MAX_BATCH: Int64 = 256
comptime RUNTIME_DOCTOR_MARKER_AGGREGATION_FACET_COUNT: Int64 = 32
comptime RUNTIME_DOCTOR_MARKER_AGGREGATION_VALUE_WIDTH: Int64 = 38
comptime RUNTIME_DOCTOR_MARKER_AGGREGATION_MAX_ROWS_PER_EVENT: Int64 = 39
comptime RUNTIME_DOCTOR_MARKER_AGGREGATION_MAX_ROWS: Int64 = 256 * 39
comptime RUNTIME_DOCTOR_MARKER_AGGREGATION_MAX_TEXT_BYTES: Int64 = 4 * 1024 * 1024
comptime RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_MARKER_TOTAL: Int64 = 0
comptime RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CONTEXT_ROUTE: Int64 = 1
comptime RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CONTEXT_LANE: Int64 = 2
comptime RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CONTEXT_PROFILE: Int64 = 3
comptime RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CHAIN_RETRIED_REASON: Int64 = 4
comptime RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CHAIN_DEAD_REASON: Int64 = 5
comptime RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_STALE_REASON: Int64 = 6
comptime RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_PREVIOUS_ROUTE: Int64 = 7
comptime RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_PREVIOUS_TRANSPORT: Int64 = 8
comptime RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_PREVIOUS_REQUEST_SHAPE: Int64 = 9
comptime RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_FACET_BASE: Int64 = 10


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


def runtime_doctor_marker_aggregation_value(
    values: Pointer[mut=False, ProdexRichStringView, _],
    event_index: Int64,
    slot: Int64,
) -> ProdexRichStringView:
    return values[
        unsafe_offset=event_index * RUNTIME_DOCTOR_MARKER_AGGREGATION_VALUE_WIDTH + slot
    ].copy()


def runtime_doctor_marker_aggregation_present(
    presence: Pointer[mut=False, Int64, _], event_index: Int64, slot: Int64
) -> Bool:
    return (
        presence[unsafe_offset=event_index]
        & (Int64(1) << slot)
    ) != 0


def runtime_doctor_marker_aggregation_reason_kind(
    marker: ProdexRichStringView,
) -> Int64:
    if rich_view_matches_literal["chain_retried_owner"](marker, False):
        return RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CHAIN_RETRIED_REASON
    if rich_view_matches_literal["chain_dead_upstream_confirmed"](marker, False):
        return RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CHAIN_DEAD_REASON
    if rich_view_matches_literal["stale_continuation"](marker, False):
        return RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_STALE_REASON
    return -1


def runtime_doctor_marker_aggregation_add(
    event_index: Int64,
    kind: Int64,
    value_slot: Int64,
    markers: Pointer[mut=False, ProdexRichStringView, _],
    values: Pointer[mut=False, ProdexRichStringView, _],
    output_representatives: Pointer[mut=True, Int64, _],
    output_kinds: Pointer[mut=True, Int64, _],
    output_value_slots: Pointer[mut=True, Int64, _],
    output_counts: Pointer[mut=True, Int64, _],
    output_count: Pointer[mut=True, Int64, _],
    output_capacity: Int64,
    weight: Int64,
) -> Int64:
    var group: Int64 = -1
    for candidate in range(output_count[]):
        var representative = output_representatives[unsafe_offset=candidate]
        if output_kinds[unsafe_offset=candidate] != kind:
            continue
        if not rich_views_equal(
            markers[unsafe_offset=event_index].copy(),
            markers[unsafe_offset=representative].copy(),
        ):
            continue
        if kind != RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_MARKER_TOTAL and not rich_views_equal(
            runtime_doctor_marker_aggregation_value(values, event_index, value_slot),
            runtime_doctor_marker_aggregation_value(values, representative, value_slot),
        ):
            continue
        group = candidate
        break
    if group < 0:
        if output_count[] >= output_capacity:
            return 3
        group = output_count[]
        output_representatives[unsafe_offset=group] = event_index
        output_kinds[unsafe_offset=group] = kind
        output_value_slots[unsafe_offset=group] = value_slot
        output_counts[unsafe_offset=group] = 0
        output_count[] = group + 1
    if weight > INT64_MAX - output_counts[unsafe_offset=group]:
        return 5
    output_counts[unsafe_offset=group] += weight
    return 0


@export("prodex_mojo_runtime_doctor_marker_aggregation_v1")
def prodex_mojo_runtime_doctor_marker_aggregation_v1(
    abi_version: Int64,
    markers_address: UInt,
    values_address: UInt,
    presence_address: UInt,
    weights_address: UInt,
    input_count: Int64,
    output_representatives_address: UInt,
    output_kinds_address: UInt,
    output_value_slots_address: UInt,
    output_counts_address: UInt,
    output_capacity: Int64,
    output_count_address: UInt,
    latest_stale_reason_index_address: UInt,
) abi("C") -> Int64:
    if abi_version != RUNTIME_DOCTOR_MARKER_AGGREGATION_ABI_VERSION:
        return 4
    if (
        input_count < 0
        or input_count > RUNTIME_DOCTOR_MARKER_AGGREGATION_MAX_BATCH
        or output_capacity < 0
        or output_capacity > RUNTIME_DOCTOR_MARKER_AGGREGATION_MAX_ROWS
        or output_count_address == 0
        or latest_stale_reason_index_address == 0
    ):
        return 1
    var output_count = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_count_address)
    )
    var latest_stale_reason_index = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(latest_stale_reason_index_address)
    )
    output_count[] = 0
    latest_stale_reason_index[] = -1
    if input_count == 0:
        return 0
    if (
        markers_address == 0
        or values_address == 0
        or presence_address == 0
        or weights_address == 0
        or output_representatives_address == 0
        or output_kinds_address == 0
        or output_value_slots_address == 0
        or output_counts_address == 0
    ):
        return 1

    var markers = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(markers_address)
    )
    var values = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(values_address)
    )
    var presence = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(presence_address)
    )
    var weights = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(weights_address)
    )
    var output_representatives = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_representatives_address)
    )
    var output_kinds = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_kinds_address)
    )
    var output_value_slots = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_value_slots_address)
    )
    var output_counts = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_counts_address)
    )
    var valid_presence_mask = (Int64(1) << RUNTIME_DOCTOR_MARKER_AGGREGATION_VALUE_WIDTH) - 1
    for index in range(input_count):
        if (
            not rich_view_valid(
                markers[unsafe_offset=index].copy(),
                RUNTIME_DOCTOR_MARKER_AGGREGATION_MAX_TEXT_BYTES,
            )
            or weights[unsafe_offset=index] < 0
            or presence[unsafe_offset=index] < 0
            or presence[unsafe_offset=index] & ~valid_presence_mask != 0
        ):
            return 2
        for slot in range(RUNTIME_DOCTOR_MARKER_AGGREGATION_VALUE_WIDTH):
            if runtime_doctor_marker_aggregation_present(presence, index, slot) and not rich_view_valid(
                runtime_doctor_marker_aggregation_value(values, index, slot),
                RUNTIME_DOCTOR_MARKER_AGGREGATION_MAX_TEXT_BYTES,
            ):
                return 2

    for index in range(input_count):
        var marker = markers[unsafe_offset=index].copy()
        var weight = weights[unsafe_offset=index]
        var status = runtime_doctor_marker_aggregation_add(
            index,
            RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_MARKER_TOTAL,
            -1,
            markers,
            values,
            output_representatives,
            output_kinds,
            output_value_slots,
            output_counts,
            output_count,
            output_capacity,
            weight,
        )
        if status != 0:
            return status
        if runtime_doctor_marker_aggregation_present(presence, index, 0) and not rich_view_matches_literal[
            ""
        ](runtime_doctor_marker_aggregation_value(values, index, 0), False) and not rich_view_matches_literal[
            "-"
        ](runtime_doctor_marker_aggregation_value(values, index, 0), False):
            status = runtime_doctor_marker_aggregation_add(
                index,
                RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CONTEXT_ROUTE,
                0,
                markers,
                values,
                output_representatives,
                output_kinds,
                output_value_slots,
                output_counts,
                output_count,
                output_capacity,
                weight,
            )
            if status != 0:
                return status
        if runtime_doctor_marker_aggregation_present(presence, index, 1) and not rich_view_matches_literal[
            ""
        ](runtime_doctor_marker_aggregation_value(values, index, 1), False) and not rich_view_matches_literal[
            "-"
        ](runtime_doctor_marker_aggregation_value(values, index, 1), False):
            status = runtime_doctor_marker_aggregation_add(
                index,
                RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CONTEXT_LANE,
                1,
                markers,
                values,
                output_representatives,
                output_kinds,
                output_value_slots,
                output_counts,
                output_count,
                output_capacity,
                weight,
            )
            if status != 0:
                return status
        if runtime_doctor_marker_aggregation_present(presence, index, 2) and not rich_view_matches_literal[
            ""
        ](runtime_doctor_marker_aggregation_value(values, index, 2), False) and not rich_view_matches_literal[
            "-"
        ](runtime_doctor_marker_aggregation_value(values, index, 2), False):
            status = runtime_doctor_marker_aggregation_add(
                index,
                RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CONTEXT_PROFILE,
                2,
                markers,
                values,
                output_representatives,
                output_kinds,
                output_value_slots,
                output_counts,
                output_count,
                output_capacity,
                weight,
            )
            if status != 0:
                return status

        var reason_kind = runtime_doctor_marker_aggregation_reason_kind(marker)
        if reason_kind >= 0 and runtime_doctor_marker_aggregation_present(presence, index, 3):
            status = runtime_doctor_marker_aggregation_add(
                index,
                reason_kind,
                3,
                markers,
                values,
                output_representatives,
                output_kinds,
                output_value_slots,
                output_counts,
                output_count,
                output_capacity,
                weight,
            )
            if status != 0:
                return status
            if reason_kind == RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_STALE_REASON:
                latest_stale_reason_index[] = index
        if rich_view_matches_literal["previous_response_not_found"](marker, False):
            if runtime_doctor_marker_aggregation_present(presence, index, 0):
                status = runtime_doctor_marker_aggregation_add(
                    index,
                    RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_PREVIOUS_ROUTE,
                    0,
                    markers,
                    values,
                    output_representatives,
                    output_kinds,
                    output_value_slots,
                    output_counts,
                    output_count,
                    output_capacity,
                    weight,
                )
                if status != 0:
                    return status
            if runtime_doctor_marker_aggregation_present(presence, index, 4):
                status = runtime_doctor_marker_aggregation_add(
                    index,
                    RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_PREVIOUS_TRANSPORT,
                    4,
                    markers,
                    values,
                    output_representatives,
                    output_kinds,
                    output_value_slots,
                    output_counts,
                    output_count,
                    output_capacity,
                    weight,
                )
                if status != 0:
                    return status
        if rich_view_matches_literal["previous_response_fresh_fallback_blocked"](
            marker, False
        ) and runtime_doctor_marker_aggregation_present(presence, index, 5):
            status = runtime_doctor_marker_aggregation_add(
                index,
                RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_PREVIOUS_REQUEST_SHAPE,
                5,
                markers,
                values,
                output_representatives,
                output_kinds,
                output_value_slots,
                output_counts,
                output_count,
                output_capacity,
                weight,
            )
            if status != 0:
                return status
        for facet_index in range(RUNTIME_DOCTOR_MARKER_AGGREGATION_FACET_COUNT):
            var slot = 6 + facet_index
            if runtime_doctor_marker_aggregation_present(presence, index, slot):
                status = runtime_doctor_marker_aggregation_add(
                    index,
                    RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_FACET_BASE + facet_index,
                    slot,
                    markers,
                    values,
                    output_representatives,
                    output_kinds,
                    output_value_slots,
                    output_counts,
                    output_count,
                    output_capacity,
                    weight,
                )
                if status != 0:
                    return status
    return 0
