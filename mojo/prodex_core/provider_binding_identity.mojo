from std.memory import Pointer

from rich_text import rich_codepoint, rich_codepoint_width, rich_trim_bounds, rich_view_valid
from rich_types import ProdexRichStringView, rich_view_ptr

comptime PROVIDER_BINDING_IDENTITY_ABI_VERSION: Int64 = 1
comptime PROVIDER_BINDING_IDENTITY_OK: Int64 = 0
comptime PROVIDER_BINDING_IDENTITY_REJECTED: Int64 = 1
comptime PROVIDER_BINDING_IDENTITY_INVALID: Int64 = 2
comptime PROVIDER_BINDING_IDENTITY_ABI: Int64 = 4
comptime PROVIDER_BINDING_IDENTITY_MAX_INPUT_BYTES: Int64 = 9223372036854775807
comptime PROVIDER_BINDING_IDENTITY_MAX_CREDENTIAL_BYTES: Int64 = 4_096
comptime PROVIDER_BINDING_IDENTITY_MAX_PROFILE_BYTES: Int64 = 256


def provider_binding_view(address: UInt, length: Int64) -> ProdexRichStringView:
    return ProdexRichStringView(address, UInt(length))


def provider_binding_has_control(view: ProdexRichStringView) -> Bool:
    var source = rich_view_ptr(view)
    var index: Int64 = 0
    var end = Int64(view.len)
    while index < end:
        var width = rich_codepoint_width(source[unsafe_offset=index])
        var codepoint = rich_codepoint(source, index, width)
        if codepoint <= 0x1F or (codepoint >= 0x7F and codepoint <= 0x9F):
            return True
        index += width
    return False


def provider_binding_trimmed_value(
    view: ProdexRichStringView,
    maximum: Int64,
) -> Tuple[Int64, Int64]:
    var bounds = rich_trim_bounds(view)
    if bounds[0] == bounds[1] or bounds[1] - bounds[0] > maximum:
        return (-1, -1)
    var trimmed = ProdexRichStringView(
        view.ptr + UInt(bounds[0]), UInt(bounds[1] - bounds[0])
    )
    if provider_binding_has_control(trimmed):
        return (-1, -1)
    return bounds


def provider_binding_endpoint_bounds(view: ProdexRichStringView) -> Tuple[Int64, Int64]:
    var bounds = rich_trim_bounds(view)
    if bounds[0] == bounds[1]:
        return (-1, -1)
    var source = rich_view_ptr(view)
    var end = bounds[1]
    while end > bounds[0] and source[unsafe_offset=end - 1] == 47:
        end -= 1
    if end == bounds[0] or end - bounds[0] > PROVIDER_BINDING_IDENTITY_MAX_INPUT_BYTES:
        return (-1, -1)
    var trimmed = ProdexRichStringView(
        view.ptr + UInt(bounds[0]), UInt(end - bounds[0])
    )
    if provider_binding_has_control(trimmed):
        return (-1, -1)
    return (bounds[0], end)


@export("prodex_mojo_provider_binding_identity_plan_v1")
def prodex_mojo_provider_binding_identity_plan_v1(
    abi_version: Int64,
    credential_address: UInt,
    credential_length: Int64,
    endpoint_address: UInt,
    endpoint_length: Int64,
    profile_address: UInt,
    profile_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != PROVIDER_BINDING_IDENTITY_ABI_VERSION:
        return PROVIDER_BINDING_IDENTITY_ABI
    if (
        credential_length < 0
        or endpoint_length < 0
        or profile_length < 0
        or output_address == 0
        or (credential_length > 0 and credential_address == 0)
        or (endpoint_length > 0 and endpoint_address == 0)
        or (profile_length > 0 and profile_address == 0)
    ):
        return PROVIDER_BINDING_IDENTITY_INVALID

    var credential = provider_binding_view(credential_address, credential_length)
    var endpoint = provider_binding_view(endpoint_address, endpoint_length)
    var profile = provider_binding_view(profile_address, profile_length)
    if (
        not rich_view_valid(credential, PROVIDER_BINDING_IDENTITY_MAX_INPUT_BYTES)
        or not rich_view_valid(endpoint, PROVIDER_BINDING_IDENTITY_MAX_INPUT_BYTES)
        or not rich_view_valid(profile, PROVIDER_BINDING_IDENTITY_MAX_INPUT_BYTES)
    ):
        return PROVIDER_BINDING_IDENTITY_INVALID

    var credential_bounds = provider_binding_trimmed_value(
        credential, PROVIDER_BINDING_IDENTITY_MAX_CREDENTIAL_BYTES
    )
    if credential_bounds[0] < 0:
        return PROVIDER_BINDING_IDENTITY_REJECTED

    var endpoint_bounds = provider_binding_endpoint_bounds(endpoint)
    if endpoint_bounds[0] < 0:
        return PROVIDER_BINDING_IDENTITY_REJECTED

    var profile_bounds = (Int64(0), Int64(0))
    if profile_length > 0:
        var candidate = rich_trim_bounds(profile)
        if candidate[0] != candidate[1]:
            profile_bounds = provider_binding_trimmed_value(
                profile, PROVIDER_BINDING_IDENTITY_MAX_PROFILE_BYTES
            )
            if profile_bounds[0] < 0:
                return PROVIDER_BINDING_IDENTITY_REJECTED

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = credential_bounds[0]
    output[unsafe_offset=1] = credential_bounds[1]
    output[unsafe_offset=2] = endpoint_bounds[0]
    output[unsafe_offset=3] = endpoint_bounds[1]
    output[unsafe_offset=4] = profile_bounds[0]
    output[unsafe_offset=5] = profile_bounds[1]
    return PROVIDER_BINDING_IDENTITY_OK


def provider_binding_hex_digit(value: UInt8) -> Bool:
    return (
        value >= 48 and value <= 57
        or value >= 97 and value <= 102
    )


@export("prodex_mojo_provider_binding_identity_digest_valid_v1")
def prodex_mojo_provider_binding_identity_digest_valid_v1(
    abi_version: Int64,
    value_address: UInt,
    value_length: Int64,
) abi("C") -> Int64:
    if abi_version != PROVIDER_BINDING_IDENTITY_ABI_VERSION:
        return PROVIDER_BINDING_IDENTITY_ABI
    if value_length < 0 or value_length > 71 or (value_length > 0 and value_address == 0):
        return PROVIDER_BINDING_IDENTITY_INVALID
    var value = provider_binding_view(value_address, value_length)
    if not rich_view_valid(value, 71) or value_length != 71:
        return PROVIDER_BINDING_IDENTITY_REJECTED
    var source = rich_view_ptr(value)
    for index in range(7):
        var expected: UInt8 = 115
        if index == 1:
            expected = 104
        elif index == 2:
            expected = 97
        elif index == 3:
            expected = 50
        elif index == 4:
            expected = 53
        elif index == 5:
            expected = 54
        elif index == 6:
            expected = 58
        if source[unsafe_offset=index] != expected:
            return PROVIDER_BINDING_IDENTITY_REJECTED
    for index in range(7, 71):
        if not provider_binding_hex_digit(source[unsafe_offset=index]):
            return PROVIDER_BINDING_IDENTITY_REJECTED
    return PROVIDER_BINDING_IDENTITY_OK
