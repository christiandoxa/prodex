from std.memory import Pointer

from launch_args_common import (
    ARG_ORIGINAL,
    ARG_PROFILE,
    ARG_PROFILE_INLINE,
    ARG_RESUME,
    ARG_EXEC,
    ARG_SESSION,
    ARG_FULL_ACCESS,
    LaunchArgView,
    LaunchArgPiece,
    launch_arg,
    launch_first,
    launch_inline_config,
    launch_inspect,
    launch_is,
    launch_piece,
    launch_prefix,
    launch_separator,
    launch_uuid,
    launch_view,
)
from rich_text import rich_view_ptr, rich_view_valid

comptime LAUNCH_ARGS_ABI_VERSION: Int64 = 1
comptime LAUNCH_INSPECT: Int64 = 0
comptime LAUNCH_NORMALIZE_RUN: Int64 = 1
comptime LAUNCH_RETARGET_TUI: Int64 = 2
comptime LAUNCH_RETARGET_EXEC: Int64 = 3
comptime LAUNCH_NORMALIZE_PROFILE: Int64 = 6
comptime LAUNCH_EXTRACT_DRY_RUN: Int64 = 7
comptime LAUNCH_PREPARE: Int64 = 8
comptime LAUNCH_SCOPE_CONFIG: Int64 = 9
comptime LAUNCH_SCAN_SUPER_OVERRIDES: Int64 = 10
comptime SUPER_VALUE_KIND_LAST: Int64 = 23


def launch_super_override_kind(arg: LaunchArgView) -> Int64:
    if launch_is["--provider"](arg) or launch_prefix["--provider="](arg):
        return 1
    if launch_is["--cli"](arg) or launch_prefix["--cli="](arg):
        return 2
    if launch_is["--api-key"](arg) or launch_prefix["--api-key="](arg):
        return 3
    if launch_is["--sub-agent-provider"](arg) or launch_prefix[
        "--sub-agent-provider="
    ](arg):
        return 4
    if launch_is["--sub-agent-model"](arg) or launch_prefix[
        "--sub-agent-model="
    ](arg):
        return 5
    if launch_is["--sub-agent-model-reasoning-effort"](arg) or launch_prefix[
        "--sub-agent-model-reasoning-effort="
    ](arg):
        return 6
    if launch_is["--sub-agent-url"](arg) or launch_prefix["--sub-agent-url="](
        arg
    ):
        return 7
    if launch_is["--sub-agent-max-concurrency"](arg) or launch_prefix[
        "--sub-agent-max-concurrency="
    ](arg):
        return 8
    if (
        launch_is["--model"](arg)
        or launch_prefix["--model="](arg)
        or launch_is["--local-model"](arg)
        or launch_prefix["--local-model="](arg)
    ):
        return 9
    if launch_is["--profile"](arg) or launch_prefix["--profile="](arg):
        return 10
    if launch_is["--base-url"](arg) or launch_prefix["--base-url="](arg):
        return 11
    if launch_is["--url"](arg) or launch_prefix["--url="](arg):
        return 12
    if (
        launch_is["--context-window"](arg)
        or launch_prefix["--context-window="](arg)
        or launch_is["--local-context-window"](arg)
        or launch_prefix["--local-context-window="](arg)
    ):
        return 13
    if (
        launch_is["--auto-compact-token-limit"](arg)
        or launch_prefix["--auto-compact-token-limit="](arg)
        or launch_is["--local-auto-compact-token-limit"](arg)
        or launch_prefix["--local-auto-compact-token-limit="](arg)
    ):
        return 14
    if launch_is["--tool"](arg) or launch_prefix["--tool="](arg):
        return 15
    if launch_is["--require-tool"](arg) or launch_prefix["--require-tool="](
        arg
    ):
        return 16
    if launch_is["--web-search"](arg) or launch_prefix["--web-search="](arg):
        return 17
    if launch_is["--rollout-budget-tokens"](arg) or launch_prefix[
        "--rollout-budget-tokens="
    ](arg):
        return 18
    if launch_is["--rollout-budget-reminders"](arg) or launch_prefix[
        "--rollout-budget-reminders="
    ](arg):
        return 19
    if launch_is["--rollout-budget-sampling-weight"](arg) or launch_prefix[
        "--rollout-budget-sampling-weight="
    ](arg):
        return 20
    if launch_is["--rollout-budget-prefill-weight"](arg) or launch_prefix[
        "--rollout-budget-prefill-weight="
    ](arg):
        return 21
    if launch_is["--current-time-reminder-interval"](arg) or launch_prefix[
        "--current-time-reminder-interval="
    ](arg):
        return 22
    if launch_is["--current-time-clock-source"](arg) or launch_prefix[
        "--current-time-clock-source="
    ](arg):
        return 23
    if launch_is["--no-auto-rotate"](arg):
        return 24
    if launch_is["--auto-rotate"](arg):
        return 25
    if launch_is["--auto-redeem"](arg):
        return 26
    if launch_is["--skip-quota-check"](arg):
        return 27
    if launch_is["--dry-run"](arg):
        return 28
    if launch_is["--no-proxy"](arg):
        return 29
    if launch_is["--presidio"](arg):
        return 30
    if launch_is["--no-presidio"](arg):
        return 31
    if launch_is["--sub-agent"](arg):
        return 32
    if launch_is["--no-sub-agent"](arg):
        return 33
    if launch_is["--full-access"](arg):
        return 34
    if launch_is["--current-time-reminder"](arg):
        return 35
    if launch_is["--respect-system-proxy"](arg):
        return 36
    if launch_is["--no-respect-system-proxy"](arg):
        return 37
    if launch_prefix["--no-auto-rotate="](arg):
        return -24
    if launch_prefix["--auto-rotate="](arg):
        return -25
    if launch_prefix["--auto-redeem="](arg):
        return -26
    if launch_prefix["--skip-quota-check="](arg):
        return -27
    if launch_prefix["--dry-run="](arg):
        return -28
    if launch_prefix["--no-proxy="](arg):
        return -29
    if launch_prefix["--presidio="](arg):
        return -30
    if launch_prefix["--no-presidio="](arg):
        return -31
    if launch_prefix["--sub-agent="](arg):
        return -32
    if launch_prefix["--no-sub-agent="](arg):
        return -33
    if launch_prefix["--full-access="](arg):
        return -34
    if launch_prefix["--current-time-reminder="](arg):
        return -35
    if launch_prefix["--respect-system-proxy="](arg):
        return -36
    if launch_prefix["--no-respect-system-proxy="](arg):
        return -37
    return 0


def launch_equals_offset(arg: LaunchArgView) -> Int64:
    if arg.valid_utf8 == 0:
        return -1
    var ptr = rich_view_ptr(launch_view(arg))
    for index in range(Int64(arg.length)):
        if ptr[unsafe_offset=index] == 61:
            return index + 1
    return -1


def launch_scan_super_overrides(
    arguments: UInt64,
    count: Int64,
    output: Pointer[mut=True, LaunchArgPiece, _],
) -> Int64:
    var after_separator = False
    for index in range(count):
        var arg = launch_arg(arguments, 0, index)
        var piece = LaunchArgPiece(ARG_ORIGINAL, index, 0)
        if not after_separator:
            if launch_is["--"](arg):
                after_separator = True
            else:
                var kind = launch_super_override_kind(arg)
                if kind > 0:
                    var value_index: Int64 = -1
                    var offset: Int64 = 0
                    if kind <= SUPER_VALUE_KIND_LAST:
                        var inline_offset = launch_equals_offset(arg)
                        if inline_offset >= 0:
                            value_index = index
                            offset = inline_offset
                        elif index + 1 < count:
                            var candidate = launch_arg(arguments, 0, index + 1)
                            if (
                                candidate.valid_utf8 == 1
                                and not launch_is["--"](candidate)
                                and launch_super_override_kind(candidate) == 0
                            ):
                                value_index = index + 1
                    piece = LaunchArgPiece(kind, value_index, offset)
        output[unsafe_offset=index] = piece^
    return count


def launch_append_range(
    source: UInt64,
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, LaunchArgPiece, _],
    initial_written: Int64,
) -> Int64:
    var written = initial_written
    for index in range(start, end):
        output[unsafe_offset=written] = launch_piece(source, index)
        written += 1
    return written


def launch_strip_thread_source(
    arguments: UInt64,
    source: UInt64,
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, LaunchArgPiece, _],
    initial_written: Int64,
    drop_last: Bool,
) -> Int64:
    var written = initial_written
    var index = start
    while index < end:
        var arg = launch_arg(arguments, source, index)
        if launch_is["--thread-source"](arg):
            index += 2
            continue
        if not launch_prefix["--thread-source="](arg) and not (
            drop_last and launch_is["--last"](arg)
        ):
            output[unsafe_offset=written] = launch_piece(source, index)
            written += 1
        index += 1
    return written


def launch_strip_positionals(
    arguments: UInt64,
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, LaunchArgPiece, _],
    initial_written: Int64,
    drop_last: Bool,
) -> Int64:
    var written = initial_written
    var index = start
    while index < end:
        var separator = launch_separator(arguments, 0, index, end)
        var positional = launch_first(arguments, 0, index, separator)
        if positional < 0:
            return launch_strip_thread_source(
                arguments, 0, index, separator, output, written, drop_last
            )
        written = launch_strip_thread_source(
            arguments, 0, index, positional, output, written, drop_last
        )
        index = positional + 1
    return written


def launch_normalize_run(
    arguments: UInt64,
    source: UInt64,
    count: Int64,
    output: Pointer[mut=True, LaunchArgPiece, _],
    initial_written: Int64,
) -> Int64:
    var written = initial_written
    var first = launch_first(arguments, source, 0, count)
    if first < 0 or not launch_uuid(launch_arg(arguments, source, first)):
        return launch_append_range(source, 0, count, output, written)
    written = launch_strip_thread_source(
        arguments, source, 0, first, output, written, False
    )
    output[unsafe_offset=written] = LaunchArgPiece(ARG_RESUME, -1, 0)
    return launch_append_range(source, first, count, output, written + 1)


def launch_normalize_profile(
    arguments: UInt64,
    count: Int64,
    output: Pointer[mut=True, LaunchArgPiece, _],
    remove_full_access: Bool,
    metadata: Pointer[mut=True, Int64, _],
) -> Int64:
    var after_separator = False
    var written: Int64 = 0
    for index in range(count):
        var arg = launch_arg(arguments, 0, index)
        var piece = LaunchArgPiece(ARG_ORIGINAL, index, 0)
        if not after_separator:
            if launch_is["--"](arg):
                after_separator = True
            elif remove_full_access and launch_is["--full-access"](arg):
                metadata[unsafe_offset=1] = 1
                continue
            elif launch_is["--profile-v2"](arg):
                piece = LaunchArgPiece(ARG_PROFILE, -1, 0)
            elif launch_prefix["--profile-v2="](arg):
                piece = LaunchArgPiece(ARG_PROFILE_INLINE, index, 13)
        output[unsafe_offset=written] = piece^
        written += 1
    return written


def launch_scope_segment(
    arguments: UInt64,
    source: UInt64,
    count: Int64,
    start: Int64,
    command: Int64,
    output: Pointer[mut=True, LaunchArgPiece, _],
) -> Int64:
    var written = launch_append_range(source, 0, start, output, 0)
    # Stable partition around the command; preserve every config occurrence and
    # the historical option-value behavior rather than reparsing with Clap.
    for config_pass in range(2):
        var index = start
        while index < command:
            var arg = launch_arg(arguments, source, index)
            var pair = (
                launch_is["-c"](arg) or launch_is["--config"](arg)
            ) and index + 1 < command
            var width: Int64 = 2 if pair else 1
            var config = pair or launch_inline_config(arg)
            if config == (config_pass == 1):
                written = launch_append_range(
                    source, index, index + width, output, written
                )
            index += width
        if config_pass == 0:
            output[unsafe_offset=written] = launch_piece(source, command)
            written += 1
    return launch_append_range(source, command + 1, count, output, written)


@export("prodex_mojo_launch_args_v1")
def prodex_mojo_launch_args_v1(
    abi_version: Int64,
    operation: Int64,
    full_access: Int64,
    arguments: UInt64,
    count: Int64,
    output_address: UInt64,
    capacity: Int64,
    scratch_address: UInt64,
    metadata_address: UInt64,
) abi("C") -> Int64:
    if abi_version != LAUNCH_ARGS_ABI_VERSION:
        return 4
    # Lengths are bounded by caller-owned storage and signed address arithmetic.
    if (
        operation < LAUNCH_INSPECT
        or operation > LAUNCH_SCAN_SUPER_OVERRIDES
        or operation == 4
        or operation == 5
        or full_access < 0
        or full_access > 1
        or count < 0
        or count > 0x7FFFFFFFFFFFFFFF // 24 - 3
        or (count > 0 and arguments == 0)
        or metadata_address == 0
    ):
        return 1
    var input = Pointer[mut=False, LaunchArgView, MutUntrackedOrigin](
        unsafe_from_address=Int(arguments)
    )
    for index in range(count):
        var arg = input[unsafe_offset=index].copy()
        if arg.valid_utf8 == 0:
            if arg.address != 0 or arg.length != 0:
                return 1
        elif arg.valid_utf8 == 1:
            if not rich_view_valid(launch_view(arg), 0x7FFFFFFFFFFFFFFF):
                return 2
        else:
            return 1
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(metadata_address)
    )
    if operation == LAUNCH_INSPECT:
        launch_inspect(arguments, count, metadata)
        return 0
    if (
        capacity < count + 3
        or output_address == 0
        or scratch_address == 0
        or output_address == scratch_address
    ):
        return 3
    var output = Pointer[mut=True, LaunchArgPiece, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var scratch = Pointer[mut=True, LaunchArgPiece, MutUntrackedOrigin](
        unsafe_from_address=Int(scratch_address)
    )
    metadata[unsafe_offset=1] = 0
    var written: Int64 = 0
    if operation == LAUNCH_SCAN_SUPER_OVERRIDES:
        written = launch_scan_super_overrides(arguments, count, output)
    elif operation == LAUNCH_NORMALIZE_RUN:
        written = launch_normalize_run(arguments, 0, count, output, 0)
    elif operation == LAUNCH_RETARGET_TUI or operation == LAUNCH_RETARGET_EXEC:
        var positional = launch_first(arguments, 0, 0, count)
        var command = positional if positional >= 0 else launch_separator(
            arguments, 0, 0, count
        )
        var exec_mode = operation == LAUNCH_RETARGET_EXEC
        written = launch_strip_thread_source(
            arguments, 0, 0, command, output, 0, exec_mode
        )
        if exec_mode:
            output[unsafe_offset=written] = LaunchArgPiece(ARG_EXEC, -1, 0)
            written += 1
        output[unsafe_offset=written] = LaunchArgPiece(ARG_RESUME, -1, 0)
        output[unsafe_offset=written + 1] = LaunchArgPiece(ARG_SESSION, -1, 0)
        written += 2
        if positional >= 0:
            written = launch_strip_positionals(
                arguments, command + 1, count, output, written, exec_mode
            )
    elif operation == LAUNCH_NORMALIZE_PROFILE:
        written = launch_normalize_profile(
            arguments, count, output, False, metadata
        )
    elif operation == LAUNCH_EXTRACT_DRY_RUN:
        var after_separator = False
        for index in range(count):
            var arg = launch_arg(arguments, 0, index)
            if launch_is["--"](arg):
                after_separator = True
            elif not after_separator and launch_is["--dry-run"](arg):
                metadata[unsafe_offset=1] = 1
                continue
            output[unsafe_offset=written] = LaunchArgPiece(
                ARG_ORIGINAL, index, 0
            )
            written += 1
    elif operation == LAUNCH_PREPARE:
        var normalized = launch_normalize_profile(
            arguments, count, scratch, True, metadata
        )
        if full_access == 1 or metadata[unsafe_offset=1] == 1:
            output[unsafe_offset=0] = LaunchArgPiece(ARG_FULL_ACCESS, -1, 0)
            written = 1
        written = launch_normalize_run(
            arguments, scratch_address, normalized, output, written
        )
        metadata[unsafe_offset=1] = 0
        for index in range(
            launch_separator(arguments, output_address, 0, written)
        ):
            if launch_is["review"](
                launch_arg(arguments, output_address, index)
            ):
                metadata[unsafe_offset=1] = 1
    elif operation == LAUNCH_SCOPE_CONFIG:
        var first = launch_first(arguments, 0, 0, count)
        if first >= 0 and launch_is["exec"](launch_arg(arguments, 0, first)):
            written = launch_scope_segment(
                arguments, 0, count, 0, first, output
            )
            first = launch_first(arguments, output_address, 0, written)
            var nested = launch_first(
                arguments, output_address, first + 1, written
            )
            if nested >= 0 and launch_is["resume"](
                launch_arg(arguments, output_address, nested)
            ):
                written = launch_scope_segment(
                    arguments,
                    output_address,
                    written,
                    first + 1,
                    nested,
                    scratch,
                )
                for index in range(written):
                    output[unsafe_offset=index] = scratch[
                        unsafe_offset=index
                    ].copy()
        else:
            written = launch_append_range(0, 0, count, output, 0)
    metadata[unsafe_offset=0] = written
    return 0
