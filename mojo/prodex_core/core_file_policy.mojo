from std.memory import Pointer

from rich_text import rich_view_prefix, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime CORE_FILE_POLICY_ABI_VERSION: Int64 = 1
comptime CORE_FILE_POLICY_OK: Int64 = 0
comptime CORE_FILE_POLICY_INVALID: Int64 = 1
comptime CORE_FILE_POLICY_ABI: Int64 = 4
comptime CORE_FILE_POLICY_MAX_TEXT_BYTES: Int64 = 1_048_576

comptime CORE_FILE_POLICY_OWNED_ROOT_TEMP: Int64 = 0
comptime CORE_FILE_POLICY_ROOT_TEMP_PID: Int64 = 1
comptime CORE_FILE_POLICY_STALE_ROOT_TEMP: Int64 = 2
comptime CORE_FILE_POLICY_RUNTIME_LOG_NAME: Int64 = 3
comptime CORE_FILE_POLICY_LOGIN_TEMP_NAME: Int64 = 4
comptime CORE_FILE_POLICY_BROKER_ARTIFACT_KEY: Int64 = 5
comptime CORE_FILE_POLICY_BROKER_LEASE_PID: Int64 = 6

comptime CORE_FILE_POLICY_U32_MAX: UInt64 = 4_294_967_295


def core_file_view_suffix[literal: StaticString](view: ProdexRichStringView) -> Bool:
    var suffix_length = Int64(literal.byte_length())
    if Int64(view.len) < suffix_length:
        return False
    if suffix_length == 0:
        return True
    var source = rich_view_ptr(view)
    var suffix = literal.unsafe_ptr()
    var start = Int64(view.len) - suffix_length
    for index in range(suffix_length):
        if source[unsafe_offset=start + index] != suffix[unsafe_offset=index]:
            return False
    return True


def core_file_dynamic_prefix(
    value: ProdexRichStringView, prefix: ProdexRichStringView
) -> Bool:
    if prefix.len > value.len:
        return False
    if prefix.len == 0:
        return True
    var left = rich_view_ptr(value)
    var right = rich_view_ptr(prefix)
    for index in range(Int64(prefix.len)):
        if left[unsafe_offset=index] != right[unsafe_offset=index]:
            return False
    return True


def core_file_parse_u32_range(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Tuple[Bool, UInt64]:
    if start < 0 or end <= start or end > Int64(view.len):
        return (False, UInt64(0))
    var source = rich_view_ptr(view)
    var value: UInt64 = 0
    for index in range(start, end):
        var byte = source[unsafe_offset=index]
        if byte < 48 or byte > 57:
            return (False, UInt64(0))
        var digit = UInt64(byte - 48)
        if value > CORE_FILE_POLICY_U32_MAX // UInt64(10):
            return (False, UInt64(0))
        value = value * UInt64(10) + digit
        if value > CORE_FILE_POLICY_U32_MAX:
            return (False, UInt64(0))
    return (True, value)


def core_file_root_temp_pid(
    view: ProdexRichStringView
) -> Tuple[Bool, UInt64]:
    if not core_file_view_suffix[".tmp"](view):
        return (False, UInt64(0))
    var stem_end = Int64(view.len) - Int64(4)
    var source = rich_view_ptr(view)
    var separators: Int64 = 0
    var cursor = stem_end
    var pid_start: Int64 = -1
    var pid_end: Int64 = -1
    while cursor > 0:
        cursor -= 1
        if source[unsafe_offset=cursor] != 46:
            continue
        separators += 1
        if separators == 2:
            pid_end = cursor
        elif separators == 3:
            pid_start = cursor + 1
            break
    if separators < 3:
        return (False, UInt64(0))
    return core_file_parse_u32_range(view, pid_start, pid_end)


def core_file_broker_key(
    view: ProdexRichStringView, is_dir: Bool
) -> Tuple[Bool, UInt64, UInt64]:
    if not rich_view_prefix["runtime-broker-"](view, False):
        return (False, UInt64(0), UInt64(0))
    var start = Int64(15)
    var end = Int64(view.len)
    if core_file_view_suffix[".json"](view):
        end -= Int64(5)
    elif core_file_view_suffix[".json.last-good"](view):
        end -= Int64(15)
    elif core_file_view_suffix[".capability"](view):
        end -= Int64(11)
    elif is_dir and core_file_view_suffix["-leases"](view):
        end -= Int64(7)
    else:
        return (False, UInt64(0), UInt64(0))
    return (True, UInt64(start), UInt64(end - start))


@export("prodex_core_file_policy_v1")
def prodex_core_file_policy_v1(
    abi_version: Int64,
    operation: Int64,
    name_address: UInt,
    name_length: Int64,
    prefix_address: UInt,
    prefix_length: Int64,
    flag: Int64,
    signed0: Int64,
    signed1: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != CORE_FILE_POLICY_ABI_VERSION:
        return CORE_FILE_POLICY_ABI
    if (
        operation < CORE_FILE_POLICY_OWNED_ROOT_TEMP
        or operation > CORE_FILE_POLICY_BROKER_LEASE_PID
        or name_length < 0
        or prefix_length < 0
        or name_length > CORE_FILE_POLICY_MAX_TEXT_BYTES
        or prefix_length > CORE_FILE_POLICY_MAX_TEXT_BYTES
        or (name_length > 0 and name_address == 0)
        or (prefix_length > 0 and prefix_address == 0)
        or (flag != 0 and flag != 1)
        or output_address == 0
    ):
        return CORE_FILE_POLICY_INVALID

    var name = ProdexRichStringView(name_address, UInt(name_length))
    var prefix = ProdexRichStringView(prefix_address, UInt(prefix_length))
    if (
        not rich_view_valid(name, CORE_FILE_POLICY_MAX_TEXT_BYTES)
        or not rich_view_valid(prefix, CORE_FILE_POLICY_MAX_TEXT_BYTES)
    ):
        return CORE_FILE_POLICY_INVALID

    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(4):
        output[unsafe_offset=index] = 0

    if operation == CORE_FILE_POLICY_OWNED_ROOT_TEMP:
        output[unsafe_offset=0] = UInt64(
            rich_view_prefix["state.json."](name, False)
            or rich_view_prefix["runtime-"](name, False)
            or rich_view_prefix["update-check.json."](name, False)
        )
        return CORE_FILE_POLICY_OK

    if operation == CORE_FILE_POLICY_ROOT_TEMP_PID:
        var parsed = core_file_root_temp_pid(name)
        output[unsafe_offset=0] = UInt64(parsed[0])
        output[unsafe_offset=1] = parsed[1]
        return CORE_FILE_POLICY_OK

    if operation == CORE_FILE_POLICY_STALE_ROOT_TEMP:
        var parsed = core_file_root_temp_pid(name)
        output[unsafe_offset=0] = UInt64(
            flag == 0 and (signed0 < signed1 or parsed[0])
        )
        return CORE_FILE_POLICY_OK

    if operation == CORE_FILE_POLICY_RUNTIME_LOG_NAME:
        output[unsafe_offset=0] = UInt64(
            core_file_dynamic_prefix(name, prefix)
            and core_file_view_suffix[".log"](name)
        )
        return CORE_FILE_POLICY_OK

    if operation == CORE_FILE_POLICY_LOGIN_TEMP_NAME:
        output[unsafe_offset=0] = UInt64(
            rich_view_prefix[".login-"](name, False)
        )
        return CORE_FILE_POLICY_OK

    if operation == CORE_FILE_POLICY_BROKER_ARTIFACT_KEY:
        var key = core_file_broker_key(name, flag == 1)
        output[unsafe_offset=0] = UInt64(key[0])
        output[unsafe_offset=1] = key[1]
        output[unsafe_offset=2] = key[2]
        return CORE_FILE_POLICY_OK

    var separator = Int64(name.len)
    var source = rich_view_ptr(name)
    for index in range(Int64(name.len)):
        if source[unsafe_offset=index] == 45:
            separator = index
            break
    var parsed = core_file_parse_u32_range(name, 0, separator)
    output[unsafe_offset=0] = UInt64(parsed[0])
    output[unsafe_offset=1] = parsed[1]
    return CORE_FILE_POLICY_OK
