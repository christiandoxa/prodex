from std.memory import Pointer
from index_order import index_order_sort

from rich_text import (
    rich_trim_bounds,
    rich_view_compare,
    rich_views_equal,
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
comptime APP_SERVER_ACTION_KEEP: Int64 = 0
comptime APP_SERVER_ACTION_KEEP_PAIR: Int64 = 1
comptime APP_SERVER_ACTION_MODEL_PAIR: Int64 = 2
comptime APP_SERVER_ACTION_MODEL_INLINE: Int64 = 3
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


@export("prodex_runtime_overlay_app_server_config_v1")
def prodex_runtime_overlay_app_server_config_v1(
    abi_version: Int64,
    arguments_address: UInt,
    count: Int64,
    include_inline_config: Int64,
    records_address: UInt,
    record_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != RUNTIME_OVERLAY_ABI_VERSION:
        return RUNTIME_OVERLAY_ABI
    if (
        count < 0
        or include_inline_config < 0
        or include_inline_config > 1
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
            index += 1
            continue
        var view = runtime_overlay_view(arg)
        if not rich_view_valid(view, Int64(arg.length)):
            return RUNTIME_OVERLAY_INVALID

        if (
            rich_view_matches_literal["-c"](view, False)
            or rich_view_matches_literal["--config"](view, False)
            or rich_view_matches_literal["--enable"](view, False)
            or rich_view_matches_literal["--disable"](view, False)
            or rich_view_matches_literal["--code-mode-host"](view, False)
        ):
            if index + 1 < count:
                if written >= record_capacity:
                    return RUNTIME_OVERLAY_CAPACITY
                runtime_overlay_put_record(
                    records, written, APP_SERVER_ACTION_KEEP_PAIR, index, 0
                )
                written += 1
                index += 2
                continue
        elif rich_view_matches_literal["--strict-config"](view, False):
            if written >= record_capacity:
                return RUNTIME_OVERLAY_CAPACITY
            runtime_overlay_put_record(
                records, written, APP_SERVER_ACTION_KEEP, index, 0
            )
            written += 1
        elif (
            rich_view_matches_literal["-m"](view, False)
            or rich_view_matches_literal["--model"](view, False)
        ):
            if index + 1 < count:
                var next = runtime_overlay_arg(arguments_address, index + 1)
                if next.valid_utf8 == 1:
                    var next_view = runtime_overlay_view(next)
                    if not rich_view_valid(next_view, Int64(next.length)):
                        return RUNTIME_OVERLAY_INVALID
                    if written >= record_capacity:
                        return RUNTIME_OVERLAY_CAPACITY
                    runtime_overlay_put_record(
                        records,
                        written,
                        APP_SERVER_ACTION_MODEL_PAIR,
                        index,
                        index + 1,
                    )
                    written += 1
                    index += 2
                    continue
                index += 1
                continue
        elif rich_view_prefix["--model="](view, False):
            if written >= record_capacity:
                return RUNTIME_OVERLAY_CAPACITY
            runtime_overlay_put_record(
                records, written, APP_SERVER_ACTION_MODEL_INLINE, index, 8
            )
            written += 1
        elif (
            include_inline_config == 1
            and (
                rich_view_prefix["--config="](view, False)
                or rich_view_prefix["-c="](view, False)
            )
        ):
            if written >= record_capacity:
                return RUNTIME_OVERLAY_CAPACITY
            runtime_overlay_put_record(
                records, written, APP_SERVER_ACTION_KEEP, index, 0
            )
            written += 1

        index += 1

    written_ptr[] = written
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

# Planned trusted workspace entries are serialized by Rust, but Mojo owns
# first-occurrence de-duplication, input order and the total projects cap.
# Records are [path pointer, UTF-8 byte len, serialized entry byte len].
comptime OVERLAY_TRUST_ENTRY_ABI_VERSION: Int64 = 1
comptime OVERLAY_TRUST_ENTRY_MAX_COUNT: Int64 = 65_536
comptime OVERLAY_TRUST_ENTRY_MAX_PATH_BYTES: Int64 = 1_048_576


def runtime_overlay_trust_path(
    records: Pointer[mut=False, UInt64, _], index: Int64
) -> ProdexRichStringView:
    return ProdexRichStringView(
        UInt(records[unsafe_offset=index * 3]),
        UInt(records[unsafe_offset=index * 3 + 1]),
    )


def runtime_overlay_trust_entry_compare(
    records_address: UInt, _width: Int64, left: Int64, right: Int64
) -> Int64:
    var records = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(records_address)
    )
    var comparison = rich_view_compare(
        runtime_overlay_trust_path(records, left),
        runtime_overlay_trust_path(records, right),
    )
    if comparison != 0:
        return comparison
    # Heap sort itself is not stable. Exact ties retain the earlier source.
    return Int64(-1) if left < right else Int64(1) if left > right else Int64(0)


@export("prodex_runtime_overlay_trust_entry_plan_v1")
def prodex_runtime_overlay_trust_entry_plan_v1(
    abi_version: Int64,
    records_address: UInt,
    count: Int64,
    max_encoded_bytes: UInt64,
    output_address: UInt,
    output_capacity: Int64,
    scratch_address: UInt,
    scratch_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != OVERLAY_TRUST_ENTRY_ABI_VERSION:
        return RUNTIME_OVERLAY_ABI
    if (
        count < 0 or count > OVERLAY_TRUST_ENTRY_MAX_COUNT
        or output_capacity < count
        or scratch_capacity < count * 2
        or (count > 0 and (
            records_address == 0 or records_address % 8 != 0
            or output_address == 0 or output_address % 8 != 0
            or scratch_address == 0 or scratch_address % 8 != 0
        ))
        or written_address == 0 or written_address % 8 != 0
    ):
        return RUNTIME_OVERLAY_INVALID
    var records = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(records_address)
    )
    for index in range(count):
        var path = runtime_overlay_trust_path(records, index)
        if not rich_view_valid(path, OVERLAY_TRUST_ENTRY_MAX_PATH_BYTES):
            return RUNTIME_OVERLAY_INVALID

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var scratch = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(scratch_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    for index in range(count):
        scratch[unsafe_offset=index] = index
        scratch[unsafe_offset=count + index] = 0
    if count > 1:
        index_order_sort[runtime_overlay_trust_entry_compare](
            records_address, 3, scratch, count
        )
    var previous: Int64 = -1
    for position in range(count):
        var index = scratch[unsafe_offset=position]
        if previous == -1 or not rich_views_equal(
            runtime_overlay_trust_path(records, previous),
            runtime_overlay_trust_path(records, index),
        ):
            scratch[unsafe_offset=count + index] = 1
        previous = index

    var selected_count: Int64 = 0
    var encoded_bytes = UInt128(11)  # len("projects={}") in UTF-8
    for index in range(count):
        if scratch[unsafe_offset=count + index] == 0:
            continue
        var separator = UInt128(1) if selected_count > 0 else UInt128(0)
        var length = UInt128(records[unsafe_offset=index * 3 + 2])
        var projected = encoded_bytes + separator + length
        # Preserve original behavior: first entry is retained unconditionally
        # even if the resulting projects assignment exceeds the target size.
        if selected_count > 0 and projected > UInt128(max_encoded_bytes):
            break
        output[unsafe_offset=selected_count] = index
        selected_count += 1
        encoded_bytes = projected
    written[unsafe_offset=0] = selected_count
    return RUNTIME_OVERLAY_OK

# Component-only lexical alias normalization. Rust's std::path owns
# platform-specific component acquisition, prefix representation and push.
# Kinds: 0 prefix, 1 root, 2 current, 3 normal, 4 parent.
comptime OVERLAY_LEXICAL_PATH_ABI_VERSION: Int64 = 1
comptime OVERLAY_LEXICAL_PATH_MAX_COMPONENTS: Int64 = 65_536


@export("prodex_runtime_overlay_lexical_path_v1")
def prodex_runtime_overlay_lexical_path_v1(
    abi_version: Int64,
    kinds_address: UInt,
    count: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != OVERLAY_LEXICAL_PATH_ABI_VERSION:
        return RUNTIME_OVERLAY_ABI
    if (
        count < 0 or count > OVERLAY_LEXICAL_PATH_MAX_COMPONENTS
        or output_capacity < count
        or (count > 0 and (
            kinds_address == 0 or kinds_address % 8 != 0
            or output_address == 0 or output_address % 8 != 0
        ))
        or written_address == 0 or written_address % 8 != 0
    ):
        return RUNTIME_OVERLAY_INVALID
    var kinds = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(kinds_address)
    )
    # All tags must be valid before any caller-owned output is mutated.
    for index in range(count):
        var kind = kinds[unsafe_offset=index]
        if kind < 0 or kind > 4:
            return RUNTIME_OVERLAY_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var rooted = False
    var length: Int64 = 0
    for index in range(count):
        var kind = kinds[unsafe_offset=index]
        if kind == 0 or kind == 1:
            output[unsafe_offset=length] = index
            length += 1
            rooted = True
        elif kind == 2:
            continue
        elif kind == 3:
            output[unsafe_offset=length] = index
            length += 1
        else:
            var previous = kinds[unsafe_offset=output[unsafe_offset=length - 1]] if length > 0 else Int64(-1)
            if previous == 3:
                length -= 1
            elif not rooted and (length == 0 or previous == 4):
                output[unsafe_offset=length] = index
                length += 1
    written[unsafe_offset=0] = length
    return RUNTIME_OVERLAY_OK

# Hook-trust read-only JSON status classification. Rust owns app-server RPC,
# DOM parsing, key/hash validation and the eventual config write.
comptime OVERLAY_HOOK_TRUST_ABI_VERSION: Int64 = 1
comptime OVERLAY_HOOK_TRUST_MAX_COUNT: Int64 = 65_536
comptime OVERLAY_HOOK_TRUST_MAX_STATUS_BYTES: Int64 = 65_536


@export("prodex_runtime_overlay_hook_trust_policy_v1")
def prodex_runtime_overlay_hook_trust_policy_v1(
    abi_version: Int64,
    operation: Int64,
    statuses_address: UInt,
    count: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != OVERLAY_HOOK_TRUST_ABI_VERSION:
        return RUNTIME_OVERLAY_ABI
    if (
        operation < 0 or operation > 1
        or count < 0 or count > OVERLAY_HOOK_TRUST_MAX_COUNT
        or (operation == 1 and count != 1)
        or output_capacity < count
        or (count > 0 and (
            statuses_address == 0 or statuses_address % 8 != 0
            or output_address == 0 or output_address % 8 != 0
        ))
        or written_address == 0 or written_address % 8 != 0
    ):
        return RUNTIME_OVERLAY_INVALID
    var statuses = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(statuses_address))
    for index in range(count):
        var status = statuses[unsafe_offset=index].copy()
        if not rich_view_valid(status, OVERLAY_HOOK_TRUST_MAX_STATUS_BYTES):
            return RUNTIME_OVERLAY_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    if operation == 1:
        var status = statuses[unsafe_offset=0].copy()
        output[unsafe_offset=0] = Int64(
            rich_view_matches_literal["ok"](status, False)
            or rich_view_matches_literal["okOverridden"](status, False)
        )
        written[unsafe_offset=0] = 1
        return RUNTIME_OVERLAY_OK

    var selected: Int64 = 0
    for index in range(count):
        var status = statuses[unsafe_offset=index].copy()
        if rich_view_matches_literal["untrusted"](status, False) or rich_view_matches_literal["modified"](status, False):
            output[unsafe_offset=selected] = index
            selected += 1
    written[unsafe_offset=0] = selected
    return RUNTIME_OVERLAY_OK
