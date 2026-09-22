from std.memory import Pointer

from launch_args_common import (
    ARG_ORIGINAL, ARG_PROFILE, ARG_PROFILE_INLINE, ARG_RESUME, ARG_EXEC,
    ARG_SESSION, ARG_FULL_ACCESS, LaunchArgView, LaunchArgPiece,
    launch_arg, launch_first, launch_inline_config, launch_inspect, launch_is,
    launch_piece, launch_prefix, launch_separator, launch_uuid, launch_view,
)
from rich_text import rich_view_valid

comptime LAUNCH_ARGS_ABI_VERSION: Int64 = 1
comptime LAUNCH_INSPECT: Int64 = 0
comptime LAUNCH_NORMALIZE_RUN: Int64 = 1
comptime LAUNCH_RETARGET_TUI: Int64 = 2
comptime LAUNCH_RETARGET_EXEC: Int64 = 3
comptime LAUNCH_NORMALIZE_PROFILE: Int64 = 6
comptime LAUNCH_EXTRACT_DRY_RUN: Int64 = 7
comptime LAUNCH_PREPARE: Int64 = 8
comptime LAUNCH_SCOPE_CONFIG: Int64 = 9


def launch_append_range(
    source: UInt64, start: Int64, end: Int64,
    output: Pointer[mut=True, LaunchArgPiece, _], initial_written: Int64,
) -> Int64:
    var written = initial_written
    for index in range(start, end):
        output[unsafe_offset=written] = launch_piece(source, index)
        written += 1
    return written


def launch_strip_thread_source(
    arguments: UInt64, source: UInt64, start: Int64, end: Int64,
    output: Pointer[mut=True, LaunchArgPiece, _], initial_written: Int64,
    drop_last: Bool,
) -> Int64:
    var written = initial_written
    var index = start
    while index < end:
        var arg = launch_arg(arguments, source, index)
        if launch_is["--thread-source"](arg):
            index += 2
            continue
        if not launch_prefix["--thread-source="](arg) and not (drop_last and launch_is["--last"](arg)):
            output[unsafe_offset=written] = launch_piece(source, index)
            written += 1
        index += 1
    return written


def launch_strip_positionals(
    arguments: UInt64, start: Int64, end: Int64,
    output: Pointer[mut=True, LaunchArgPiece, _], initial_written: Int64,
    drop_last: Bool,
) -> Int64:
    var written = initial_written
    var index = start
    while index < end:
        var separator = launch_separator(arguments, 0, index, end)
        var positional = launch_first(arguments, 0, index, separator)
        if positional < 0:
            return launch_strip_thread_source(arguments, 0, index, separator, output, written, drop_last)
        written = launch_strip_thread_source(arguments, 0, index, positional, output, written, drop_last)
        index = positional + 1
    return written


def launch_normalize_run(
    arguments: UInt64, source: UInt64, count: Int64,
    output: Pointer[mut=True, LaunchArgPiece, _], initial_written: Int64,
) -> Int64:
    var written = initial_written
    var first = launch_first(arguments, source, 0, count)
    if first < 0 or not launch_uuid(launch_arg(arguments, source, first)):
        return launch_append_range(source, 0, count, output, written)
    written = launch_strip_thread_source(arguments, source, 0, first, output, written, False)
    output[unsafe_offset=written] = LaunchArgPiece(ARG_RESUME, -1, 0)
    return launch_append_range(source, first, count, output, written + 1)


def launch_normalize_profile(
    arguments: UInt64, count: Int64, output: Pointer[mut=True, LaunchArgPiece, _],
    remove_full_access: Bool, metadata: Pointer[mut=True, Int64, _],
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
    arguments: UInt64, source: UInt64, count: Int64, start: Int64, command: Int64,
    output: Pointer[mut=True, LaunchArgPiece, _],
) -> Int64:
    var written = launch_append_range(source, 0, start, output, 0)
    # Stable partition around the command; preserve every config occurrence and
    # the historical option-value behavior rather than reparsing with Clap.
    for config_pass in range(2):
        var index = start
        while index < command:
            var arg = launch_arg(arguments, source, index)
            var pair = (launch_is["-c"](arg) or launch_is["--config"](arg)) and index + 1 < command
            var width: Int64 = 2 if pair else 1
            var config = pair or launch_inline_config(arg)
            if config == (config_pass == 1):
                written = launch_append_range(source, index, index + width, output, written)
            index += width
        if config_pass == 0:
            output[unsafe_offset=written] = launch_piece(source, command)
            written += 1
    return launch_append_range(source, command + 1, count, output, written)


@export("prodex_mojo_launch_args_v1")
def prodex_mojo_launch_args_v1(
    abi_version: Int64, operation: Int64, full_access: Int64,
    arguments: UInt64, count: Int64, output_address: UInt64, capacity: Int64,
    scratch_address: UInt64, metadata_address: UInt64,
) abi("C") -> Int64:
    if abi_version != LAUNCH_ARGS_ABI_VERSION:
        return 4
    # Lengths are bounded by caller-owned storage and signed address arithmetic.
    if (
        operation < LAUNCH_INSPECT or operation > LAUNCH_SCOPE_CONFIG or operation == 4 or operation == 5
        or full_access < 0 or full_access > 1 or count < 0
        or count > 0x7FFFFFFFFFFFFFFF // 24 - 3
        or (count > 0 and arguments == 0) or metadata_address == 0
    ):
        return 1
    var input = Pointer[mut=False, LaunchArgView, MutUntrackedOrigin](unsafe_from_address=Int(arguments))
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
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    if operation == LAUNCH_INSPECT:
        launch_inspect(arguments, count, metadata)
        return 0
    if capacity < count + 3 or output_address == 0 or scratch_address == 0 or output_address == scratch_address:
        return 3
    var output = Pointer[mut=True, LaunchArgPiece, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var scratch = Pointer[mut=True, LaunchArgPiece, MutUntrackedOrigin](unsafe_from_address=Int(scratch_address))
    metadata[unsafe_offset=1] = 0
    var written: Int64 = 0
    if operation == LAUNCH_NORMALIZE_RUN:
        written = launch_normalize_run(arguments, 0, count, output, 0)
    elif operation == LAUNCH_RETARGET_TUI or operation == LAUNCH_RETARGET_EXEC:
        var positional = launch_first(arguments, 0, 0, count)
        var command = positional if positional >= 0 else launch_separator(arguments, 0, 0, count)
        var exec_mode = operation == LAUNCH_RETARGET_EXEC
        written = launch_strip_thread_source(arguments, 0, 0, command, output, 0, exec_mode)
        if exec_mode:
            output[unsafe_offset=written] = LaunchArgPiece(ARG_EXEC, -1, 0)
            written += 1
        output[unsafe_offset=written] = LaunchArgPiece(ARG_RESUME, -1, 0)
        output[unsafe_offset=written + 1] = LaunchArgPiece(ARG_SESSION, -1, 0)
        written += 2
        if positional >= 0:
            written = launch_strip_positionals(arguments, command + 1, count, output, written, exec_mode)
    elif operation == LAUNCH_NORMALIZE_PROFILE:
        written = launch_normalize_profile(arguments, count, output, False, metadata)
    elif operation == LAUNCH_EXTRACT_DRY_RUN:
        var after_separator = False
        for index in range(count):
            var arg = launch_arg(arguments, 0, index)
            if launch_is["--"](arg):
                after_separator = True
            elif not after_separator and launch_is["--dry-run"](arg):
                metadata[unsafe_offset=1] = 1
                continue
            output[unsafe_offset=written] = LaunchArgPiece(ARG_ORIGINAL, index, 0)
            written += 1
    elif operation == LAUNCH_PREPARE:
        var normalized = launch_normalize_profile(arguments, count, scratch, True, metadata)
        if full_access == 1 or metadata[unsafe_offset=1] == 1:
            output[unsafe_offset=0] = LaunchArgPiece(ARG_FULL_ACCESS, -1, 0)
            written = 1
        written = launch_normalize_run(arguments, scratch_address, normalized, output, written)
        metadata[unsafe_offset=1] = 0
        for index in range(launch_separator(arguments, output_address, 0, written)):
            if launch_is["review"](launch_arg(arguments, output_address, index)):
                metadata[unsafe_offset=1] = 1
    elif operation == LAUNCH_SCOPE_CONFIG:
        var first = launch_first(arguments, 0, 0, count)
        if first >= 0 and launch_is["exec"](launch_arg(arguments, 0, first)):
            written = launch_scope_segment(arguments, 0, count, 0, first, output)
            first = launch_first(arguments, output_address, 0, written)
            var nested = launch_first(arguments, output_address, first + 1, written)
            if nested >= 0 and launch_is["resume"](launch_arg(arguments, output_address, nested)):
                written = launch_scope_segment(arguments, output_address, written, first + 1, nested, scratch)
                for index in range(written):
                    output[unsafe_offset=index] = scratch[unsafe_offset=index].copy()
        else:
            written = launch_append_range(0, 0, count, output, 0)
    metadata[unsafe_offset=0] = written
    return 0
