from std.memory import Pointer

from rich_text import (
    rich_trim_bounds,
    rich_view_matches_literal,
    rich_view_prefix,
    rich_view_valid,
)
from rich_types import ProdexRichStringView, rich_view_ptr

comptime RUNTIME_OVERLAY_ABI_VERSION: Int64 = 1
comptime RUNTIME_OVERLAY_OK: Int64 = 0
comptime RUNTIME_OVERLAY_INVALID: Int64 = 1
comptime RUNTIME_OVERLAY_CAPACITY: Int64 = 2
comptime RUNTIME_OVERLAY_ABI: Int64 = 4

comptime FRESH_ACTION_KEEP: Int64 = 0
comptime FRESH_ACTION_CONFIG_ARG: Int64 = 1
comptime FRESH_ACTION_FEATURE: Int64 = 2
comptime OVERLAY_OPTIONAL_TOOL_LIMIT: Int64 = 6
comptime OVERLAY_TOOL_STATUS_INSTALLED: Int64 = 0
comptime OVERLAY_TOOL_STATUS_INVALID: Int64 = 2
comptime OVERLAY_TOOL_PLAN_READY: Int64 = 0
comptime OVERLAY_TOOL_PLAN_REQUIRED_UNAVAILABLE: Int64 = 1
comptime OVERLAY_TOOL_PLAN_SKIP_INCOMPATIBLE: Int64 = 2


@fieldwise_init
struct RuntimeOverlayArgView(Copyable, Movable):
    var address: UInt64
    var length: UInt64
    var valid_utf8: Int64


def runtime_overlay_arg(arguments: UInt, index: Int64) -> RuntimeOverlayArgView:
    return Pointer[mut=False, RuntimeOverlayArgView, ImmUntrackedOrigin](
        unsafe_from_address=Int(arguments)
    )[unsafe_offset=index].copy()


def runtime_overlay_view(arg: RuntimeOverlayArgView) -> ProdexRichStringView:
    return ProdexRichStringView(UInt(arg.address), UInt(arg.length))


def runtime_overlay_put_record(
    records: Pointer[mut=True, Int64, _],
    record_index: Int64,
    first: Int64,
    second: Int64,
    third: Int64,
):
    var base = record_index * 3
    records[unsafe_offset=base] = first
    records[unsafe_offset=base + 1] = second
    records[unsafe_offset=base + 2] = third


def runtime_overlay_starts_projects(
    view: ProdexRichStringView,
    start: Int64,
    strip_equals: Bool,
) -> Bool:
    if start < 0 or start > Int64(view.len):
        return False
    var source = rich_view_ptr(view)
    var offset = start
    if strip_equals:
        while offset < Int64(view.len) and source[unsafe_offset=offset] == 61:
            offset += 1
    var tail = ProdexRichStringView(
        UInt(Int(view.ptr) + Int(offset)),
        UInt(Int64(view.len) - offset),
    )
    var bounds = rich_trim_bounds(tail)
    if bounds[1] <= bounds[0]:
        return False
    var trimmed = ProdexRichStringView(
        UInt(Int(tail.ptr) + Int(bounds[0])),
        UInt(bounds[1] - bounds[0]),
    )
    return rich_view_prefix["projects="](trimmed, False)


@export("prodex_runtime_overlay_config_assignments_v1")
def prodex_runtime_overlay_config_assignments_v1(
    abi_version: Int64,
    arguments_address: UInt,
    count: Int64,
    records_address: UInt,
    record_capacity: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != RUNTIME_OVERLAY_ABI_VERSION:
        return RUNTIME_OVERLAY_ABI
    if (
        count < 0
        or record_capacity < 0
        or result_address == 0
        or (count > 0 and arguments_address == 0)
        or (record_capacity > 0 and records_address == 0)
    ):
        return RUNTIME_OVERLAY_INVALID

    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[unsafe_offset=0] = 0
    result[unsafe_offset=1] = 0
    var records = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(records_address)
    )

    var index: Int64 = 0
    var written: Int64 = 0
    while index < count:
        var arg = runtime_overlay_arg(arguments_address, index)
        if arg.valid_utf8 == 0:
            index += 1
            continue
        var view = runtime_overlay_view(arg)
        if not rich_view_valid(view, Int64(arg.length)):
            return RUNTIME_OVERLAY_INVALID

        var assignment_index = index
        var assignment_start: Int64 = -1
        var assignment_length: Int64 = 0

        if (
            rich_view_matches_literal["-c"](view, False)
            or rich_view_matches_literal["--config"](view, False)
        ):
            index += 1
            if index >= count:
                result[unsafe_offset=0] = 1
                return RUNTIME_OVERLAY_OK
            var value_arg = runtime_overlay_arg(arguments_address, index)
            if value_arg.valid_utf8 == 0:
                result[unsafe_offset=0] = 2
                return RUNTIME_OVERLAY_OK
            var value_view = runtime_overlay_view(value_arg)
            if not rich_view_valid(value_view, Int64(value_arg.length)):
                return RUNTIME_OVERLAY_INVALID
            assignment_index = index
            assignment_start = 0
            assignment_length = Int64(value_arg.length)
        elif rich_view_prefix["--config="](view, False):
            assignment_start = 9
            assignment_length = Int64(view.len) - assignment_start
        elif rich_view_prefix["-c"](view, False) and Int64(view.len) > 2:
            assignment_start = 2
            assignment_length = Int64(view.len) - assignment_start

        if assignment_start >= 0:
            if written >= record_capacity:
                return RUNTIME_OVERLAY_CAPACITY
            runtime_overlay_put_record(
                records,
                written,
                assignment_index,
                assignment_start,
                assignment_length,
            )
            written += 1
        index += 1

    result[unsafe_offset=1] = written
    return RUNTIME_OVERLAY_OK


@export("prodex_runtime_overlay_workspace_trust_indices_v1")
def prodex_runtime_overlay_workspace_trust_indices_v1(
    abi_version: Int64,
    arguments_address: UInt,
    count: Int64,
    indices_address: UInt,
    index_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != RUNTIME_OVERLAY_ABI_VERSION:
        return RUNTIME_OVERLAY_ABI
    if (
        count < 0
        or index_capacity < 0
        or written_address == 0
        or (count > 0 and arguments_address == 0)
        or (index_capacity > 0 and indices_address == 0)
    ):
        return RUNTIME_OVERLAY_INVALID

    var indices = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(indices_address)
    )
    var written_ptr = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var written: Int64 = 0
    var index: Int64 = 0
    while index < count:
        var arg = runtime_overlay_arg(arguments_address, index)
        if arg.valid_utf8 == 0:
            index += 1
            continue
        var view = runtime_overlay_view(arg)
        if not rich_view_valid(view, Int64(arg.length)):
            return RUNTIME_OVERLAY_INVALID

        if (
            rich_view_matches_literal["-c"](view, False)
            or rich_view_matches_literal["--config"](view, False)
        ):
            var flag_index = index
            index += 1
            if index < count:
                var value_arg = runtime_overlay_arg(arguments_address, index)
                if value_arg.valid_utf8 == 1:
                    var value_view = runtime_overlay_view(value_arg)
                    if not rich_view_valid(value_view, Int64(value_arg.length)):
                        return RUNTIME_OVERLAY_INVALID
                    if runtime_overlay_starts_projects(value_view, 0, False):
                        if written + 2 > index_capacity:
                            return RUNTIME_OVERLAY_CAPACITY
                        indices[unsafe_offset=written] = flag_index
                        indices[unsafe_offset=written + 1] = index
                        written += 2
            index += 1
            continue

        var include = False
        if rich_view_prefix["--config="](view, False):
            include = runtime_overlay_starts_projects(view, 9, False)
        elif rich_view_prefix["-c"](view, False) and Int64(view.len) > 2:
            include = runtime_overlay_starts_projects(view, 2, True)

        if include:
            if written >= index_capacity:
                return RUNTIME_OVERLAY_CAPACITY
            indices[unsafe_offset=written] = index
            written += 1
        index += 1

    written_ptr[] = written
    return RUNTIME_OVERLAY_OK


@export("prodex_runtime_overlay_transport_flags_v1")
def prodex_runtime_overlay_transport_flags_v1(
    abi_version: Int64,
    arguments_address: UInt,
    count: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != RUNTIME_OVERLAY_ABI_VERSION:
        return RUNTIME_OVERLAY_ABI
    if (
        count < 0
        or result_address == 0
        or (count > 0 and arguments_address == 0)
    ):
        return RUNTIME_OVERLAY_INVALID

    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[unsafe_offset=0] = 0
    result[unsafe_offset=1] = 0
    for index in range(count):
        var arg = runtime_overlay_arg(arguments_address, index)
        if arg.valid_utf8 == 0:
            continue
        var view = runtime_overlay_view(arg)
        if not rich_view_valid(view, Int64(arg.length)):
            return RUNTIME_OVERLAY_INVALID
        if (
            rich_view_matches_literal["--remote"](view, False)
            or rich_view_prefix["--remote="](view, False)
        ):
            result[unsafe_offset=0] = 1
        if rich_view_matches_literal["--no-daemon"](view, False):
            result[unsafe_offset=1] = 1
    return RUNTIME_OVERLAY_OK


@export("prodex_runtime_overlay_fresh_projection_v1")
def prodex_runtime_overlay_fresh_projection_v1(
    abi_version: Int64,
    arguments_address: UInt,
    count: Int64,
    records_address: UInt,
    record_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != RUNTIME_OVERLAY_ABI_VERSION:
        return RUNTIME_OVERLAY_ABI
    if (
        count < 0
        or record_capacity < 0
        or written_address == 0
        or (count > 0 and arguments_address == 0)
        or (record_capacity > 0 and records_address == 0)
    ):
        return RUNTIME_OVERLAY_INVALID

    var records = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(records_address)
    )
    var written_ptr = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var written: Int64 = 0

    var index: Int64 = 0
    while index < count:
        var arg = runtime_overlay_arg(arguments_address, index)
        if arg.valid_utf8 == 0:
            return RUNTIME_OVERLAY_INVALID
        var view = runtime_overlay_view(arg)
        if not rich_view_valid(view, Int64(arg.length)):
            return RUNTIME_OVERLAY_INVALID

        if (
            rich_view_matches_literal["-c"](view, False)
            or rich_view_matches_literal["--config"](view, False)
        ):
            if index + 1 < count:
                if written + 2 > record_capacity:
                    return RUNTIME_OVERLAY_CAPACITY
                runtime_overlay_put_record(
                    records, written, FRESH_ACTION_CONFIG_ARG, index, 0
                )
                written += 1
                runtime_overlay_put_record(
                    records, written, FRESH_ACTION_CONFIG_ARG, index + 1, 0
                )
                written += 1
                index += 2
                continue
        elif (
            rich_view_matches_literal["--enable"](view, False)
            or rich_view_matches_literal["--disable"](view, False)
        ):
            if index + 1 < count:
                if written >= record_capacity:
                    return RUNTIME_OVERLAY_CAPACITY
                runtime_overlay_put_record(
                    records,
                    written,
                    FRESH_ACTION_FEATURE,
                    index + 1,
                    Int64(1) if rich_view_matches_literal["--enable"](view, False) else Int64(0),
                )
                written += 1
                index += 2
                continue
        elif rich_view_matches_literal[
            "--dangerously-bypass-approvals-and-sandbox"
        ](view, False):
            index += 1
            continue
        elif (
            rich_view_prefix["--config="](view, False)
            or (
                rich_view_prefix["-c"](view, False)
                and Int64(view.len) > 2
            )
        ):
            if written >= record_capacity:
                return RUNTIME_OVERLAY_CAPACITY
            runtime_overlay_put_record(
                records, written, FRESH_ACTION_CONFIG_ARG, index, 0
            )
            written += 1
            index += 1
            continue

        if written >= record_capacity:
            return RUNTIME_OVERLAY_CAPACITY
        runtime_overlay_put_record(records, written, FRESH_ACTION_KEEP, index, 0)
        written += 1
        index += 1

    written_ptr[] = written
    return RUNTIME_OVERLAY_OK


@export("prodex_runtime_overlay_optional_tool_plan_v1")
def prodex_runtime_overlay_optional_tool_plan_v1(
    abi_version: Int64,
    availability_mask: Int64,
    required_mask: Int64,
    count: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != RUNTIME_OVERLAY_ABI_VERSION:
        return RUNTIME_OVERLAY_ABI
    if (
        count < 0
        or count > OVERLAY_OPTIONAL_TOOL_LIMIT
        or availability_mask < 0
        or required_mask < 0
        or (availability_mask >> (count * 2)) != 0
        or (required_mask >> count) != 0
        or result_address == 0
    ):
        return RUNTIME_OVERLAY_INVALID

    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[unsafe_offset=0] = OVERLAY_TOOL_PLAN_READY
    result[unsafe_offset=1] = -1
    result[unsafe_offset=2] = 0

    var first_required_unavailable: Int64 = -1
    var optional_incompatible_mask: Int64 = 0
    for index in range(count):
        var tool_status = (availability_mask >> (index * 2)) & 3
        var is_required = (required_mask >> index) & 1
        if tool_status == OVERLAY_TOOL_STATUS_INSTALLED:
            return RUNTIME_OVERLAY_INVALID
        if is_required == 1 and first_required_unavailable < 0:
            first_required_unavailable = index
        elif is_required == 0 and tool_status == OVERLAY_TOOL_STATUS_INVALID:
            optional_incompatible_mask |= 1 << index

    if first_required_unavailable >= 0:
        result[unsafe_offset=0] = OVERLAY_TOOL_PLAN_REQUIRED_UNAVAILABLE
        result[unsafe_offset=1] = first_required_unavailable
        return RUNTIME_OVERLAY_OK
    if optional_incompatible_mask > 0:
        result[unsafe_offset=0] = OVERLAY_TOOL_PLAN_SKIP_INCOMPATIBLE
    result[unsafe_offset=2] = optional_incompatible_mask
    return RUNTIME_OVERLAY_OK
