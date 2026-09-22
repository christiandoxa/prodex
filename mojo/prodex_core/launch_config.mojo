from std.memory import Pointer

from launch_args_common import (
    LaunchArgView, launch_arg, launch_is, launch_prefix, launch_rust_space, launch_view,
)
from rich_text import rich_codepoint, rich_codepoint_width, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime LAUNCH_CONFIG_ABI_VERSION: Int64 = 1
comptime CONFIG_ORIGINAL: Int64 = 0
comptime CONFIG_SEPARATE: Int64 = 1
comptime CONFIG_LONG_INLINE: Int64 = 2
comptime CONFIG_SHORT_INLINE: Int64 = 3


def launch_config_key(arg: LaunchArgView, start: Int64) -> ProdexRichStringView:
    if arg.valid_utf8 == 0:
        return ProdexRichStringView(0, 0)
    var view = launch_view(arg)
    var ptr = rich_view_ptr(view)
    var end = start
    while end < Int64(view.len) and ptr[unsafe_offset=end] != 61:
        end += 1
    if end == Int64(view.len):
        return ProdexRichStringView(0, 0)
    var left = start
    var last_nonspace = start
    var cursor = start
    var leading = True
    while cursor < end:
        var width = rich_codepoint_width(ptr[unsafe_offset=cursor])
        var space = launch_rust_space(rich_codepoint(ptr, cursor, width))
        cursor += width
        if leading and space:
            left = cursor
        else:
            leading = False
        if not space:
            last_nonspace = cursor
    if last_nonspace <= left:
        return ProdexRichStringView(0, 0)
    return ProdexRichStringView(view.ptr + UInt(left), UInt(last_nonspace - left))


def launch_config_match(key: ProdexRichStringView, keys_address: UInt64, key_count: Int64) -> Int64:
    if key.len == 0:
        return -1
    var keys = Pointer[mut=False, LaunchArgView, MutUntrackedOrigin](unsafe_from_address=Int(keys_address))
    var left = rich_view_ptr(key)
    for index in range(key_count):
        var candidate = keys[unsafe_offset=index].copy()
        if candidate.length != UInt64(key.len):
            continue
        var right = rich_view_ptr(launch_view(candidate))
        var equal = True
        for offset in range(Int64(key.len)):
            if left[unsafe_offset=offset] != right[unsafe_offset=offset]:
                equal = False
                break
        if equal:
            return index
    return -1


@export("prodex_mojo_launch_config_v1")
def prodex_mojo_launch_config_v1(
    abi_version: Int64,
    arguments_address: UInt64, argument_count: Int64,
    keys_address: UInt64, key_count: Int64,
    output_address: UInt64, output_count: Int64,
    replaced_address: UInt64, replaced_count: Int64,
) abi("C") -> Int64:
    if abi_version != LAUNCH_CONFIG_ABI_VERSION:
        return 4
    if (
        argument_count < 0 or key_count < 0
        or argument_count > 0x7FFFFFFFFFFFFFFF // 24
        or key_count > 0x7FFFFFFFFFFFFFFF // 24
        or (argument_count > 0 and arguments_address == 0)
        or (key_count > 0 and keys_address == 0)
    ):
        return 1
    if output_count != argument_count or replaced_count != key_count:
        return 3
    if (output_count > 0 and output_address == 0) or (replaced_count > 0 and replaced_address == 0):
        return 1
    for table in range(2):
        var address = arguments_address if table == 0 else keys_address
        var count = argument_count if table == 0 else key_count
        var records = Pointer[mut=False, LaunchArgView, MutUntrackedOrigin](unsafe_from_address=Int(address))
        for index in range(count):
            var value = records[unsafe_offset=index].copy()
            if value.valid_utf8 == 0 and table == 0:
                if value.address != 0 or value.length != 0:
                    return 1
            elif value.valid_utf8 == 1:
                if not rich_view_valid(launch_view(value), 0x7FFFFFFFFFFFFFFF):
                    return 2
            else:
                return 1
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var replaced = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(replaced_address))
    for index in range(key_count):
        replaced[unsafe_offset=index] = 0
    for index in range(argument_count):
        output[unsafe_offset=index * 2] = CONFIG_ORIGINAL
        output[unsafe_offset=index * 2 + 1] = -1
    var index: Int64 = 0
    while index < argument_count:
        var arg = launch_arg(arguments_address, 0, index)
        if launch_is["--"](arg):
            break
        var kind = CONFIG_ORIGINAL
        var start: Int64 = 0
        if (launch_is["-c"](arg) or launch_is["--config"](arg)) and index + 1 < argument_count:
            kind = CONFIG_SEPARATE
            index += 1
            arg = launch_arg(arguments_address, 0, index)
        elif launch_prefix["--config="](arg):
            kind = CONFIG_LONG_INLINE
            start = 9
        elif launch_prefix["-c"](arg) and arg.length > 2:
            kind = CONFIG_SHORT_INLINE
            start = 2
        if kind != CONFIG_ORIGINAL:
            var matched = launch_config_match(launch_config_key(arg, start), keys_address, key_count)
            if matched >= 0:
                output[unsafe_offset=index * 2] = kind
                output[unsafe_offset=index * 2 + 1] = matched
                replaced[unsafe_offset=matched] = 1
        index += 1
    return 0
