from std.memory import Pointer

from rich_text import rich_view_valid
from rich_types import ProdexRichStringView, rich_view_ptr

comptime SECRET_POLICY_ABI_VERSION: Int64 = 1
comptime SECRET_POLICY_OK: Int64 = 0
comptime SECRET_POLICY_INVALID: Int64 = 1
comptime SECRET_POLICY_ABI: Int64 = 4
comptime SECRET_REFERENCE_MAX_BYTES: Int64 = 128


def secret_reference_part_valid(view: ProdexRichStringView) -> Bool:
    if (
        view.len == 0
        or view.len > UInt(SECRET_REFERENCE_MAX_BYTES)
        or view.ptr == 0
    ):
        return False
    if not rich_view_valid(view, SECRET_REFERENCE_MAX_BYTES):
        return False
    var source = rich_view_ptr(view)
    for index in range(Int64(view.len)):
        var byte = source[unsafe_offset=index]
        if byte < 33 or byte > 126:
            return False
    return True


@export("prodex_mojo_secret_reference_valid_v1")
def prodex_mojo_secret_reference_valid_v1(
    abi_version: Int64,
    provider_address: UInt,
    provider_length: Int64,
    name_address: UInt,
    name_length: Int64,
    version_address: UInt,
    version_length: Int64,
    version_present: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != SECRET_POLICY_ABI_VERSION:
        return SECRET_POLICY_ABI
    if (
        result_address == 0
        or provider_length < 0
        or name_length < 0
        or version_length < 0
        or (version_present != 0 and version_present != 1)
        or (
            version_present == 0
            and (version_address != 0 or version_length != 0)
        )
        or (provider_length > 0 and provider_address == 0)
        or (name_length > 0 and name_address == 0)
        or (
            version_present == 1 and version_length > 0 and version_address == 0
        )
    ):
        return SECRET_POLICY_INVALID

    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[] = 0

    var provider = ProdexRichStringView(provider_address, UInt(provider_length))
    var name = ProdexRichStringView(name_address, UInt(name_length))
    var valid = secret_reference_part_valid(
        provider
    ) and secret_reference_part_valid(name)
    if version_present == 1:
        var version = ProdexRichStringView(
            version_address, UInt(version_length)
        )
        valid = valid and secret_reference_part_valid(version)
    result[] = Int64(valid)
    return SECRET_POLICY_OK


comptime SECRET_ROTATION_POLICY_VALID: Int64 = 0
comptime SECRET_ROTATION_POLICY_ZERO_MAX_AGE: Int64 = 1
comptime SECRET_ROTATION_POLICY_OVERLAP_TOO_LONG: Int64 = 2


@export("prodex_mojo_secret_rotation_policy_validate_v1")
def prodex_mojo_secret_rotation_policy_validate_v1(
    abi_version: Int64,
    max_age_seconds: UInt64,
    overlap_seconds: UInt64,
    decision_address: UInt,
) abi("C") -> Int64:
    if abi_version != SECRET_POLICY_ABI_VERSION:
        return SECRET_POLICY_ABI
    if decision_address == 0:
        return SECRET_POLICY_INVALID

    var decision = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(decision_address)
    )
    decision[] = SECRET_ROTATION_POLICY_VALID
    if max_age_seconds == 0:
        decision[] = SECRET_ROTATION_POLICY_ZERO_MAX_AGE
    elif overlap_seconds >= max_age_seconds:
        decision[] = SECRET_ROTATION_POLICY_OVERLAP_TOO_LONG
    return SECRET_POLICY_OK
