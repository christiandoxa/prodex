from std.memory import Pointer

from rich_text import rich_view_matches_literal
from rich_types import ProdexRichStringView


comptime PROFILE_LOGIN_POLICY_ABI_VERSION: Int64 = 1
comptime PROFILE_LOGIN_POLICY_OK: Int64 = 0
comptime PROFILE_LOGIN_POLICY_INVALID: Int64 = 1
comptime PROFILE_LOGIN_POLICY_ABI: Int64 = 4

comptime PROFILE_LOGIN_VALIDATE_PROVIDER: Int64 = 0
comptime PROFILE_LOGIN_VALIDATE_TARGET: Int64 = 1
comptime PROFILE_LOGIN_EXECUTION: Int64 = 2
comptime PROFILE_LOGIN_AUTH_COMMIT: Int64 = 3
comptime PROFILE_LOGIN_AUTO_ROUTE: Int64 = 4
comptime PROFILE_LOGIN_METHOD_ROUTE: Int64 = 5

comptime PROFILE_LOGIN_CLAUDE: Int64 = 4
comptime PROFILE_LOGIN_STATUS: Int64 = 6

comptime PROFILE_LOGIN_ALLOWED: Int64 = 0
comptime PROFILE_LOGIN_CODEX_PROVIDER_UNSUPPORTED: Int64 = 1
comptime PROFILE_LOGIN_CLAUDE_PROVIDER_UNSUPPORTED: Int64 = 2

comptime PROFILE_LOGIN_TARGET_VALID: Int64 = 0
comptime PROFILE_LOGIN_TARGET_MISSING: Int64 = 1
comptime PROFILE_LOGIN_TARGET_CHANGED: Int64 = 2

comptime PROFILE_LOGIN_DIRECT_HOME: Int64 = 0
comptime PROFILE_LOGIN_TEMPORARY_HOME: Int64 = 1

comptime PROFILE_LOGIN_ROUTE_STATUS: Int64 = 0
comptime PROFILE_LOGIN_ROUTE_ANTHROPIC: Int64 = 1
comptime PROFILE_LOGIN_ROUTE_API_KEY: Int64 = 2
comptime PROFILE_LOGIN_ROUTE_IDENTITY: Int64 = 3
comptime PROFILE_LOGIN_ROUTE_NEEDS_AUTH_LABEL: Int64 = 4

comptime PROFILE_LOGIN_METHOD_EXTERNAL_CLAUDE: Int64 = 0
comptime PROFILE_LOGIN_METHOD_DIRECT_API_KEY: Int64 = 1
comptime PROFILE_LOGIN_METHOD_CODEX_CHILD: Int64 = 2


def profile_login_view(address: UInt64, length: Int64) -> ProdexRichStringView:
    return ProdexRichStringView(UInt(address), UInt(length))


def profile_login_bool(value: Int64) -> Bool:
    return value == 1


def profile_login_known_provider(view: ProdexRichStringView) -> Bool:
    return (
        rich_view_matches_literal["openai"](view, False)
        or rich_view_matches_literal["gemini"](view, False)
        or rich_view_matches_literal["anthropic"](view, False)
        or rich_view_matches_literal["copilot"](view, False)
        or rich_view_matches_literal["kiro"](view, False)
        or rich_view_matches_literal["agy"](view, False)
    )


def profile_login_provider_validation(
    provider: ProdexRichStringView,
    method: Int64,
    output: Pointer[mut=True, Int64, _],
) -> Int64:
    if method < 0 or method > 6 or not profile_login_known_provider(provider):
        return PROFILE_LOGIN_POLICY_INVALID
    if method == PROFILE_LOGIN_CLAUDE:
        if (
            rich_view_matches_literal["openai"](provider, False)
            or rich_view_matches_literal["anthropic"](provider, False)
            or rich_view_matches_literal["kiro"](provider, False)
        ):
            output[0] = PROFILE_LOGIN_ALLOWED
        else:
            output[0] = PROFILE_LOGIN_CLAUDE_PROVIDER_UNSUPPORTED
    elif rich_view_matches_literal["openai"](provider, False):
        output[0] = PROFILE_LOGIN_ALLOWED
    else:
        output[0] = PROFILE_LOGIN_CODEX_PROVIDER_UNSUPPORTED
    return PROFILE_LOGIN_POLICY_OK


def profile_login_target_validation(
    profile_present: Int64,
    profile_matches: Int64,
    output: Pointer[mut=True, Int64, _],
) -> Int64:
    if (
        (profile_present != 0 and profile_present != 1)
        or (profile_matches != 0 and profile_matches != 1)
    ):
        return PROFILE_LOGIN_POLICY_INVALID
    if not profile_login_bool(profile_present):
        output[0] = PROFILE_LOGIN_TARGET_MISSING
    elif not profile_login_bool(profile_matches):
        output[0] = PROFILE_LOGIN_TARGET_CHANGED
    else:
        output[0] = PROFILE_LOGIN_TARGET_VALID
    return PROFILE_LOGIN_POLICY_OK


def profile_login_execution(
    method: Int64, output: Pointer[mut=True, Int64, _]
) -> Int64:
    if method < 0 or method > 6:
        return PROFILE_LOGIN_POLICY_INVALID
    output[0] = (
        PROFILE_LOGIN_DIRECT_HOME
        if method == PROFILE_LOGIN_STATUS
        else PROFILE_LOGIN_TEMPORARY_HOME
    )
    return PROFILE_LOGIN_POLICY_OK


def profile_login_auth_commit(
    auth_label: ProdexRichStringView,
    base_url_specified: Int64,
    output: Pointer[mut=True, Int64, _],
) -> Int64:
    if base_url_specified != 0 and base_url_specified != 1:
        return PROFILE_LOGIN_POLICY_INVALID
    var is_api_key = rich_view_matches_literal["api-key"](auth_label, False)
    output[0] = Int64(is_api_key)
    output[1] = Int64(is_api_key)
    output[2] = Int64(is_api_key and profile_login_bool(base_url_specified))
    return PROFILE_LOGIN_POLICY_OK


def profile_login_auto_route(
    method: Int64,
    auth_label: ProdexRichStringView,
    auth_label_present: Int64,
    output: Pointer[mut=True, Int64, _],
) -> Int64:
    if (
        method < 0
        or method > 6
        or (auth_label_present != 0 and auth_label_present != 1)
    ):
        return PROFILE_LOGIN_POLICY_INVALID
    if method == PROFILE_LOGIN_STATUS:
        output[0] = PROFILE_LOGIN_ROUTE_STATUS
    elif method == PROFILE_LOGIN_CLAUDE:
        output[0] = PROFILE_LOGIN_ROUTE_ANTHROPIC
    elif not profile_login_bool(auth_label_present):
        output[0] = PROFILE_LOGIN_ROUTE_NEEDS_AUTH_LABEL
    elif rich_view_matches_literal["api-key"](auth_label, False):
        output[0] = PROFILE_LOGIN_ROUTE_API_KEY
    else:
        output[0] = PROFILE_LOGIN_ROUTE_IDENTITY
    return PROFILE_LOGIN_POLICY_OK


def profile_login_method_route(
    method: Int64,
    api_key_present: Int64,
    output: Pointer[mut=True, Int64, _],
) -> Int64:
    if (
        method < 0
        or method > 6
        or (api_key_present != 0 and api_key_present != 1)
    ):
        return PROFILE_LOGIN_POLICY_INVALID
    if method == PROFILE_LOGIN_CLAUDE:
        output[0] = PROFILE_LOGIN_METHOD_EXTERNAL_CLAUDE
        output[1] = 0
    elif method == 2 and profile_login_bool(api_key_present):
        output[0] = PROFILE_LOGIN_METHOD_DIRECT_API_KEY
        output[1] = 1
    else:
        output[0] = PROFILE_LOGIN_METHOD_CODEX_CHILD
        output[1] = Int64(method == 2)
    return PROFILE_LOGIN_POLICY_OK


@export("prodex_profile_login_policy_v1")
def prodex_profile_login_policy_v1(
    abi_version: Int64,
    operation: Int64,
    label_address: UInt64,
    label_length: Int64,
    method: Int64,
    input0: Int64,
    input1: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != PROFILE_LOGIN_POLICY_ABI_VERSION:
        return PROFILE_LOGIN_POLICY_ABI
    if (
        operation < PROFILE_LOGIN_VALIDATE_PROVIDER
        or operation > PROFILE_LOGIN_METHOD_ROUTE
        or label_length < 0
        or (label_length > 0 and label_address == 0)
        or output_address == 0
    ):
        return PROFILE_LOGIN_POLICY_INVALID

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(4):
        output[unsafe_offset=index] = 0

    var label = profile_login_view(label_address, label_length)
    if operation == PROFILE_LOGIN_VALIDATE_PROVIDER:
        return profile_login_provider_validation(label, method, output)
    if operation == PROFILE_LOGIN_VALIDATE_TARGET:
        return profile_login_target_validation(input0, input1, output)
    if operation == PROFILE_LOGIN_EXECUTION:
        return profile_login_execution(method, output)
    if operation == PROFILE_LOGIN_AUTH_COMMIT:
        return profile_login_auth_commit(label, input0, output)
    if operation == PROFILE_LOGIN_AUTO_ROUTE:
        return profile_login_auto_route(method, label, input0, output)
    return profile_login_method_route(method, input0, output)
