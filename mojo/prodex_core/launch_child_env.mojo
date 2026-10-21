# Security-critical child environment key classification.
# The host owns raw OsString keys, inherited environment and removal effects.
from std.memory import Pointer
from rich_text import rich_view_matches_literal, rich_view_prefix, rich_view_valid
from rich_types import ProdexRichStringView

comptime CHILD_ENV_ABI: Int64 = 1
comptime CHILD_ENV_MAX_KEYS: Int64 = 8192
comptime CHILD_ENV_MAX_KEY_BYTES: Int64 = 65_536
comptime CHILD_ENV_SANDBOX: Int64 = 1
comptime CHILD_ENV_DYLD: Int64 = 2
comptime CHILD_ENV_HARDENING_BYPASS: Int64 = 4


# Each input has three 64-bit words: borrowed UTF-8 pointer, length, valid flag.
# valid=0 is an opaque non-UTF-8 OsString and must not match any prefix.
@export("prodex_launch_child_env_flags_v1")
def prodex_launch_child_env_flags_v1(
    abi_version: Int64,
    keys_address: UInt,
    count: Int64,
    flags_address: UInt,
    flags_capacity: Int64,
    hardening_bypass_address: UInt,
) abi("C") -> Int64:
    if abi_version != CHILD_ENV_ABI:
        return 4
    if (
        count < 0 or count > CHILD_ENV_MAX_KEYS
        or flags_capacity != count
        or (count > 0 and (
            keys_address == 0 or keys_address % 8 != 0
            or flags_address == 0 or flags_address % 8 != 0
        ))
        or hardening_bypass_address == 0
        or hardening_bypass_address % 8 != 0
    ):
        return 1
    var keys = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(keys_address)
    )
    var flags = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(flags_address)
    )
    var bypass = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(hardening_bypass_address)
    )
    # Validate the entire batch before any output mutation.
    for index in range(count):
        var offset = index * 3
        var valid = keys[unsafe_offset=offset + 2]
        var address = keys[unsafe_offset=offset]
        var length = keys[unsafe_offset=offset + 1]
        if valid == 0:
            if address != 0 or length != 0:
                return 1
            continue
        if valid != 1 or length > UInt64(CHILD_ENV_MAX_KEY_BYTES):
            return 1
        if not rich_view_valid(
            ProdexRichStringView(UInt(address), UInt(length)),
            CHILD_ENV_MAX_KEY_BYTES,
        ):
            return 1

    var allow_unsafe: Int64 = 0
    for index in range(count):
        var offset = index * 3
        var tag: Int64 = 0
        if keys[unsafe_offset=offset + 2] == 1:
            var view = ProdexRichStringView(
                UInt(keys[unsafe_offset=offset]),
                UInt(keys[unsafe_offset=offset + 1]),
            )
            if rich_view_prefix["CODEX_SANDBOX"](view, False):
                tag |= CHILD_ENV_SANDBOX
            if rich_view_prefix["DYLD_"](view, False):
                tag |= CHILD_ENV_DYLD
            if rich_view_matches_literal["PRODEX_ALLOW_UNSAFE_CHILD_ENV"](view, False):
                tag |= CHILD_ENV_HARDENING_BYPASS
                allow_unsafe = 1
        flags[unsafe_offset=index] = tag
    bypass[unsafe_offset=0] = allow_unsafe
    return 0
