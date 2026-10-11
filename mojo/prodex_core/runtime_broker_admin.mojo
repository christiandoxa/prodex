from std.memory import Pointer

from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime BROKER_ADMIN_ABI_VERSION: Int64 = 1
comptime BROKER_ADMIN_INVALID: Int64 = -1

comptime BROKER_ADMIN_OPERATION_ACTIVATE: Int64 = 0
comptime BROKER_ADMIN_OPERATION_RELEASE_SESSION_AFFINITY: Int64 = 1

comptime BROKER_ADMIN_VALUE_ACTIVATION_PROFILE: Int64 = 0
comptime BROKER_ADMIN_VALUE_SESSION_ID: Int64 = 1
comptime BROKER_ADMIN_MAX_VALUE_BYTES: Int64 = 256


def broker_admin_view(address: UInt, length: Int64) -> ProdexRichStringView:
    return ProdexRichStringView(address, UInt(length))


@export("prodex_runtime_broker_admin_method_v1")
def prodex_runtime_broker_admin_method_v1(
    abi_version: Int64,
    operation: Int64,
    method_address: UInt,
    method_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != BROKER_ADMIN_ABI_VERSION
        or operation < BROKER_ADMIN_OPERATION_ACTIVATE
        or operation > BROKER_ADMIN_OPERATION_RELEASE_SESSION_AFFINITY
        or method_length < 0
        or (method_length > 0 and method_address == 0)
        or output_address == 0
    ):
        return BROKER_ADMIN_INVALID
    var method = broker_admin_view(method_address, method_length)
    if not rich_view_valid(method, method_length):
        return BROKER_ADMIN_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = 0
    if method_length == 4:
        var ptr = rich_view_ptr(method)
        if (
            ptr[unsafe_offset=0] == 80
            and ptr[unsafe_offset=1] == 79
            and ptr[unsafe_offset=2] == 83
            and ptr[unsafe_offset=3] == 84
        ):
            output[] = 1
    return 0


@export("prodex_runtime_broker_admin_value_v1")
def prodex_runtime_broker_admin_value_v1(
    abi_version: Int64,
    value_kind: Int64,
    value_address: UInt,
    value_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != BROKER_ADMIN_ABI_VERSION
        or value_kind < BROKER_ADMIN_VALUE_ACTIVATION_PROFILE
        or value_kind > BROKER_ADMIN_VALUE_SESSION_ID
        or value_length < 0
        or (value_length > 0 and value_address == 0)
        or output_address == 0
    ):
        return BROKER_ADMIN_INVALID
    var value = broker_admin_view(value_address, value_length)
    if not rich_view_valid(value, value_length):
        return BROKER_ADMIN_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = 0
    output[unsafe_offset=2] = 0
    var bounds = rich_trim_bounds(value)
    var max_value_bytes = BROKER_ADMIN_MAX_VALUE_BYTES
    if value_kind == BROKER_ADMIN_VALUE_ACTIVATION_PROFILE:
        max_value_bytes = 0x7FFFFFFFFFFFFFFF
    if bounds[1] - bounds[0] > 0 and bounds[1] - bounds[0] <= max_value_bytes:
        output[unsafe_offset=0] = 1
        output[unsafe_offset=1] = bounds[0]
        output[unsafe_offset=2] = bounds[1]
    return 0
