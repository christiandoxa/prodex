# Domain identifier and secret-error response classification. This kernel
# never receives actual identifiers, UUIDs, secret bytes or secret references;
# only trusted error-kind labels or enum discriminants cross the ABI.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid, rich_view_matches_literal

comptime DOMAIN_ERROR_ABI: Int64 = 1
comptime DOMAIN_ERROR_IDENTIFIER: Int64 = 0
comptime DOMAIN_ERROR_ROTATION: Int64 = 1
comptime DOMAIN_ERROR_RESOLUTION: Int64 = 2

comptime DOMAIN_INVALID_REQUEST: Int64 = 0
comptime DOMAIN_FORBIDDEN: Int64 = 1
comptime DOMAIN_NOT_FOUND: Int64 = 2
comptime DOMAIN_UNAVAILABLE: Int64 = 3
comptime DOMAIN_CONFLICT: Int64 = 4


def domain_identifier_code(kind: ProdexRichStringView) -> Int64:
    if rich_view_matches_literal["tenant_id"](kind, False):
        return 1
    if rich_view_matches_literal["principal_id"](kind, False):
        return 2
    if rich_view_matches_literal["request_id"](kind, False):
        return 3
    if rich_view_matches_literal["call_id"](kind, False):
        return 4
    if rich_view_matches_literal["reservation_id"](kind, False):
        return 5
    if rich_view_matches_literal["virtual_key_id"](kind, False):
        return 6
    if rich_view_matches_literal["role_binding_id"](kind, False):
        return 7
    if rich_view_matches_literal["provider_credential_id"](kind, False):
        return 8
    if rich_view_matches_literal["policy_revision_id"](kind, False):
        return 9
    if rich_view_matches_literal["audit_event_id"](kind, False):
        return 10
    return 0


@export("prodex_mojo_domain_error_plan_v1")
def prodex_mojo_domain_error_plan_v1(
    abi_version: Int64, operation: Int64, kind_address: UInt,
    kind_length: Int64, error_tag: Int64, output_address: UInt,
) abi("C") -> Int64:
    if abi_version != DOMAIN_ERROR_ABI:
        return 4
    if (
        operation < DOMAIN_ERROR_IDENTIFIER or operation > DOMAIN_ERROR_RESOLUTION
        or output_address == 0 or output_address % 8 != 0
        or kind_length < 0
    ):
        return 1
    var status: Int64 = DOMAIN_INVALID_REQUEST
    var code: Int64 = 0
    var message: Int64 = 0
    if operation == DOMAIN_ERROR_IDENTIFIER:
        if (
            (kind_length > 0 and kind_address == 0)
            or error_tag != 0
        ):
            return 1
        var kind = ProdexRichStringView(kind_address, UInt(kind_length))
        if not rich_view_valid(kind, 128):
            return 1
        code = domain_identifier_code(kind)
    else:
        if kind_address != 0 or kind_length != 0:
            return 1
        if operation == DOMAIN_ERROR_ROTATION:
            if error_tag < 0 or error_tag > 1:
                return 1
            code = 11 + error_tag
            message = 1
        else:
            if error_tag < 0 or error_tag > 3:
                return 1
            code = 13 + error_tag
            if error_tag == 0:
                status = DOMAIN_NOT_FOUND
                message = 2
            elif error_tag == 1:
                status = DOMAIN_FORBIDDEN
                message = 2
            elif error_tag == 2:
                status = DOMAIN_UNAVAILABLE
                message = 3
            else:
                status = DOMAIN_CONFLICT
                message = 4
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = status
    output[unsafe_offset=1] = code
    output[unsafe_offset=2] = message
    return 0
