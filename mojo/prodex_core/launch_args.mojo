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
from rich_text import rich_codepoint, rich_codepoint_width
from rich_types import ProdexRichStringView

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
comptime LAUNCH_FIND_SUPER_EXPOSE_ALIAS: Int64 = 11
comptime LAUNCH_LOGIN_POLICY: Int64 = 12
comptime LAUNCH_CLI_DEFAULT_RUN: Int64 = 13
comptime LAUNCH_SUPER_VALIDATE: Int64 = 14
comptime LAUNCH_SUPER_VALIDATE_SUB_AGENT: Int64 = 15
comptime LAUNCH_SUPER_COMPAT_ALIAS: Int64 = 16
comptime LAUNCH_SUPER_LITERAL_BOUNDARY: Int64 = 17
comptime LAUNCH_SUPER_POSITIONED_ALIAS: Int64 = 18
comptime LAUNCH_CODEX_COMMAND_SERVER: Int64 = 19

comptime SUPER_ALIAS_NONE: Int64 = 0
comptime SUPER_ALIAS_GEMINI: Int64 = 1
comptime SUPER_ALIAS_DEEPSEEK: Int64 = 2

comptime SUPER_VALIDATE_AUTO_ROTATE: Int64 = 1 << 0
comptime SUPER_VALIDATE_NO_AUTO_ROTATE: Int64 = 1 << 1
comptime SUPER_VALIDATE_PRESIDIO: Int64 = 1 << 2
comptime SUPER_VALIDATE_NO_PRESIDIO: Int64 = 1 << 3
comptime SUPER_VALIDATE_REQUIRED_PRESIDIO: Int64 = 1 << 4
comptime SUPER_VALIDATE_PROVIDER: Int64 = 1 << 5
comptime SUPER_VALIDATE_URL: Int64 = 1 << 6
comptime SUPER_VALIDATE_BASE_URL: Int64 = 1 << 7
comptime SUPER_VALIDATE_API_KEY: Int64 = 1 << 8
comptime SUPER_VALIDATE_CONTEXT_WINDOW: Int64 = 1 << 9
comptime SUPER_VALIDATE_AUTO_COMPACT: Int64 = 1 << 10
comptime SUPER_VALIDATE_SUB_AGENT: Int64 = 1 << 11
comptime SUPER_VALIDATE_NO_SUB_AGENT: Int64 = 1 << 12
comptime SUPER_VALIDATE_SUB_AGENT_PROVIDER: Int64 = 1 << 13
comptime SUPER_VALIDATE_SUB_AGENT_MODEL: Int64 = 1 << 14
comptime SUPER_VALIDATE_SUB_AGENT_MODEL_NONEMPTY: Int64 = 1 << 15
comptime SUPER_VALIDATE_SUB_AGENT_REASONING: Int64 = 1 << 16
comptime SUPER_VALIDATE_SUB_AGENT_URL: Int64 = 1 << 17
comptime SUPER_VALIDATE_SUB_AGENT_MAX_CONCURRENCY: Int64 = 1 << 18
comptime SUPER_VALIDATE_LOCAL_REQUIRES_URL: Int64 = 1 << 19
comptime SUPER_VALIDATE_URL_REQUIRES_LOCAL: Int64 = 1 << 20
comptime SUPER_VALIDATE_MAX_FLAGS: Int64 = 2_097_151

comptime SUPER_VALIDATION_OK: Int64 = 0
comptime SUPER_VALIDATION_SUB_AGENT_CONFLICT: Int64 = 1
comptime SUPER_VALIDATION_SUB_AGENT_DETAILS_REQUIRE_ENABLE: Int64 = 2
comptime SUPER_VALIDATION_SUB_AGENT_MODEL_EMPTY: Int64 = 3
comptime SUPER_VALIDATION_LOCAL_SUB_AGENT_REQUIRES_URL: Int64 = 4
comptime SUPER_VALIDATION_SUB_AGENT_URL_REQUIRES_LOCAL: Int64 = 5
comptime SUPER_VALIDATION_AUTO_ROTATE_CONFLICT: Int64 = 6
comptime SUPER_VALIDATION_PRESIDIO_CONFLICT: Int64 = 7
comptime SUPER_VALIDATION_NO_PRESIDIO_REQUIRES_TOOL: Int64 = 8
comptime SUPER_VALIDATION_PROVIDER_URL_CONFLICT: Int64 = 9
comptime SUPER_VALIDATION_BASE_URL_URL_CONFLICT: Int64 = 10
comptime SUPER_VALIDATION_API_KEY_REQUIRES_PROVIDER: Int64 = 11
comptime SUPER_VALIDATION_CONTEXT_WINDOW_REQUIRES_PROVIDER_OR_URL: Int64 = 12
comptime SUPER_VALIDATION_SUB_AGENT_UNSUPPORTED_WITH_DESKTOP: Int64 = 13

comptime LOGIN_METHOD_CHATGPT: Int64 = 0
comptime LOGIN_METHOD_DEVICE_CODE: Int64 = 1
comptime LOGIN_METHOD_API_KEY: Int64 = 2
comptime LOGIN_METHOD_ACCESS_TOKEN: Int64 = 3
comptime LOGIN_METHOD_CLAUDE: Int64 = 4
comptime LOGIN_METHOD_ANTIGRAVITY: Int64 = 5
comptime LOGIN_METHOD_STATUS: Int64 = 6
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


def launch_super_expose_option_takes_value(arg: LaunchArgView) -> Bool:
    var kind = launch_super_override_kind(arg)
    # Preserve the expose alias's historical subset: --cli and newer overrides
    # remain unpaired here even when their own parser consumes a value.
    return (
        kind == 1
        or (kind >= 3 and kind <= 16)
        or launch_is["-p"](arg)
        or launch_is["-c"](arg)
    )


def launch_find_super_expose_alias(arguments: UInt64, count: Int64) -> Int64:
    if count < 3:
        return -1
    var alias = launch_arg(arguments, 0, 1)
    if not launch_is["super"](alias) and not launch_is["s"](alias):
        return -1
    var index: Int64 = 2
    while index < count:
        var arg = launch_arg(arguments, 0, index)
        if arg.valid_utf8 == 0 or launch_is["--"](arg):
            return -1
        if launch_is["expose"](arg):
            return index
        if not launch_prefix["-"](arg):
            return -1
        if (
            launch_super_expose_option_takes_value(arg)
            and launch_equals_offset(arg) < 0
        ):
            index += 2
        else:
            index += 1
    return -1


def launch_cli_command_is_prodex(arg: LaunchArgView) -> Bool:
    return (
        launch_is["-h"](arg)
        or launch_is["--help"](arg)
        or launch_is["-V"](arg)
        or launch_is["--version"](arg)
        or launch_is["profile"](arg)
        or launch_is["use"](arg)
        or launch_is["current"](arg)
        or launch_is["info"](arg)
        or launch_is["status"](arg)
        or launch_is["log"](arg)
        or launch_is["session"](arg)
        or launch_is["doctor"](arg)
        or launch_is["login"](arg)
        or launch_is["logout"](arg)
        or launch_is["update"](arg)
        or launch_is["quota"](arg)
        or launch_is["redeem"](arg)
        or launch_is["ping"](arg)
        or launch_is["run"](arg)
        or launch_is["super"](arg)
        or launch_is["s"](arg)
        or launch_is["gateway"](arg)
        or launch_is["gui"](arg)
        or launch_is["dashboard"](arg)
        or launch_is["claude"](arg)
        or launch_is["help"](arg)
        or launch_is["__super-expose"](arg)
        or launch_is["__runtime-broker"](arg)
        or launch_is["__mcp-jsonl-bridge"](arg)
        or launch_is["__sub-agent-exec"](arg)
    )


def launch_cli_default_run_policy(
    arguments: UInt64,
    count: Int64,
    metadata: Pointer[mut=True, Int64, _],
):
    var should_default = True
    if count > 1:
        var command = launch_arg(arguments, 0, 1)
        if command.valid_utf8 == 1:
            should_default = not launch_cli_command_is_prodex(command)
    metadata[unsafe_offset=0] = Int64(should_default)


def launch_super_compat_alias_policy(
    arguments: UInt64,
    count: Int64,
    metadata: Pointer[mut=True, Int64, _],
):
    var alias = SUPER_ALIAS_NONE
    if count >= 3:
        var command = launch_arg(arguments, 0, 1)
        if launch_is["super"](command) or launch_is["s"](command):
            var provider = launch_arg(arguments, 0, 2)
            if launch_is["gemini"](provider):
                alias = SUPER_ALIAS_GEMINI
            elif launch_is["deepseek"](provider):
                alias = SUPER_ALIAS_DEEPSEEK
    metadata[unsafe_offset=0] = alias


def launch_super_literal_boundary_policy(
    arguments: UInt64,
    count: Int64,
    metadata: Pointer[mut=True, Int64, _],
):
    var boundary: Int64 = -1
    if count >= 3:
        var command = launch_arg(arguments, 0, 1)
        if launch_is["super"](command) or launch_is["s"](command):
            for index in range(2, Int(count)):
                if launch_is["--"](launch_arg(arguments, 0, Int64(index))):
                    boundary = Int64(index)
                    break
    metadata[unsafe_offset=0] = boundary


def launch_super_positioned_alias_policy(
    arguments: UInt64,
    count: Int64,
    provider_present: Bool,
    url_present: Bool,
    metadata: Pointer[mut=True, Int64, _],
):
    var alias = SUPER_ALIAS_NONE
    if count == 1 and not provider_present and not url_present:
        var value = launch_arg(arguments, 0, 0)
        if launch_is["gemini"](value):
            alias = SUPER_ALIAS_GEMINI
        elif launch_is["deepseek"](value):
            alias = SUPER_ALIAS_DEEPSEEK
    metadata[unsafe_offset=0] = alias


def launch_codex_command_server_policy(
    arguments: UInt64,
    count: Int64,
    metadata: Pointer[mut=True, Int64, _],
):
    var is_server = False
    if count > 0:
        var command = launch_arg(arguments, 0, 0)
        is_server = (
            launch_is["mcp-server"](command)
            or launch_is["app-server"](command)
            or launch_is["exec-server"](command)
        )
    metadata[unsafe_offset=0] = Int64(is_server)


def launch_super_validation_policy(
    arguments: UInt64,
    count: Int64,
    flags: Int64,
    sub_agent_only: Bool,
    metadata: Pointer[mut=True, Int64, _],
):
    var sub_agent = (flags & SUPER_VALIDATE_SUB_AGENT) != 0
    var no_sub_agent = (flags & SUPER_VALIDATE_NO_SUB_AGENT) != 0
    var sub_agent_details = (
        (flags & SUPER_VALIDATE_SUB_AGENT_PROVIDER) != 0
        or (flags & SUPER_VALIDATE_SUB_AGENT_MODEL) != 0
        or (flags & SUPER_VALIDATE_SUB_AGENT_REASONING) != 0
        or (flags & SUPER_VALIDATE_SUB_AGENT_URL) != 0
        or (flags & SUPER_VALIDATE_SUB_AGENT_MAX_CONCURRENCY) != 0
    )
    if sub_agent and no_sub_agent:
        metadata[unsafe_offset=0] = SUPER_VALIDATION_SUB_AGENT_CONFLICT
        return
    if not sub_agent and sub_agent_details:
        metadata[unsafe_offset=0] = SUPER_VALIDATION_SUB_AGENT_DETAILS_REQUIRE_ENABLE
        return
    if (
        (flags & SUPER_VALIDATE_SUB_AGENT_MODEL) != 0
        and (flags & SUPER_VALIDATE_SUB_AGENT_MODEL_NONEMPTY) == 0
    ):
        metadata[unsafe_offset=0] = SUPER_VALIDATION_SUB_AGENT_MODEL_EMPTY
        return
    if (flags & SUPER_VALIDATE_LOCAL_REQUIRES_URL) != 0:
        metadata[unsafe_offset=0] = SUPER_VALIDATION_LOCAL_SUB_AGENT_REQUIRES_URL
        return
    if (flags & SUPER_VALIDATE_URL_REQUIRES_LOCAL) != 0:
        metadata[unsafe_offset=0] = SUPER_VALIDATION_SUB_AGENT_URL_REQUIRES_LOCAL
        return
    if sub_agent_only:
        metadata[unsafe_offset=0] = SUPER_VALIDATION_OK
        return
    if (
        (flags & SUPER_VALIDATE_AUTO_ROTATE) != 0
        and (flags & SUPER_VALIDATE_NO_AUTO_ROTATE) != 0
    ):
        metadata[unsafe_offset=0] = SUPER_VALIDATION_AUTO_ROTATE_CONFLICT
        return
    if (
        (flags & SUPER_VALIDATE_PRESIDIO) != 0
        and (flags & SUPER_VALIDATE_NO_PRESIDIO) != 0
    ):
        metadata[unsafe_offset=0] = SUPER_VALIDATION_PRESIDIO_CONFLICT
        return
    if (
        (flags & SUPER_VALIDATE_NO_PRESIDIO) != 0
        and (flags & SUPER_VALIDATE_REQUIRED_PRESIDIO) != 0
    ):
        metadata[unsafe_offset=0] = SUPER_VALIDATION_NO_PRESIDIO_REQUIRES_TOOL
        return
    if (
        (flags & SUPER_VALIDATE_PROVIDER) != 0
        and (flags & SUPER_VALIDATE_URL) != 0
    ):
        metadata[unsafe_offset=0] = SUPER_VALIDATION_PROVIDER_URL_CONFLICT
        return
    if (
        (flags & SUPER_VALIDATE_BASE_URL) != 0
        and (flags & SUPER_VALIDATE_URL) != 0
    ):
        metadata[unsafe_offset=0] = SUPER_VALIDATION_BASE_URL_URL_CONFLICT
        return
    if (
        (flags & SUPER_VALIDATE_API_KEY) != 0
        and (flags & SUPER_VALIDATE_PROVIDER) == 0
    ):
        metadata[unsafe_offset=0] = SUPER_VALIDATION_API_KEY_REQUIRES_PROVIDER
        return
    if (
        (
            (flags & SUPER_VALIDATE_CONTEXT_WINDOW) != 0
            or (flags & SUPER_VALIDATE_AUTO_COMPACT) != 0
        )
        and (flags & (SUPER_VALIDATE_PROVIDER | SUPER_VALIDATE_URL)) == 0
    ):
        metadata[unsafe_offset=0] = SUPER_VALIDATION_CONTEXT_WINDOW_REQUIRES_PROVIDER_OR_URL
        return
    if (
        sub_agent
        and count > 0
        and launch_is["gui"](launch_arg(arguments, 0, 0))
    ):
        metadata[unsafe_offset=0] = SUPER_VALIDATION_SUB_AGENT_UNSUPPORTED_WITH_DESKTOP
        return
    metadata[unsafe_offset=0] = SUPER_VALIDATION_OK


def launch_login_policy(
    arguments: UInt64,
    count: Int64,
    metadata: Pointer[mut=True, Int64, _],
):
    var has_api_key = False
    var has_claude = False
    var has_antigravity = False
    var has_access_token = False
    var has_device_code = False
    var removed_gemini = False

    for index in range(count):
        var arg = launch_arg(arguments, 0, index)
        if arg.valid_utf8 == 0:
            continue
        if index == 0 and launch_is["status"](arg):
            metadata[unsafe_offset=0] = LOGIN_METHOD_STATUS
            metadata[unsafe_offset=1] = 0
            metadata[unsafe_offset=2] = 0
            return
        if launch_is["--with-api-key"](arg):
            has_api_key = True
        elif launch_is["--with-claude"](arg) or launch_is["--claude"](arg):
            has_claude = True
        elif (
            launch_is["--with-antigravity"](arg)
            or launch_is["--antigravity"](arg)
            or launch_is["--with-agy"](arg)
            or launch_is["--agy"](arg)
        ):
            has_antigravity = True
        elif launch_is["--with-access-token"](arg):
            has_access_token = True
        elif launch_is["--device-auth"](arg):
            has_device_code = True
        if launch_is["--with-google"](arg) or launch_is["--google"](arg):
            removed_gemini = True

    var method = LOGIN_METHOD_CHATGPT
    if has_api_key:
        method = LOGIN_METHOD_API_KEY
    elif has_claude:
        method = LOGIN_METHOD_CLAUDE
    elif has_antigravity:
        method = LOGIN_METHOD_ANTIGRAVITY
    elif has_access_token:
        method = LOGIN_METHOD_ACCESS_TOKEN
    elif has_device_code:
        method = LOGIN_METHOD_DEVICE_CODE

    metadata[unsafe_offset=0] = method
    metadata[unsafe_offset=1] = Int64(removed_gemini)
    metadata[unsafe_offset=2] = Int64(method == LOGIN_METHOD_API_KEY)


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
        or operation > LAUNCH_CODEX_COMMAND_SERVER
        or operation == 4
        or operation == 5
        or full_access < 0
        or (
            operation < LAUNCH_SUPER_VALIDATE
            and operation != LAUNCH_SUPER_POSITIONED_ALIAS
            and full_access > 1
        )
        or (
            operation == LAUNCH_SUPER_POSITIONED_ALIAS
            and full_access > 3
        )
        or (
            operation >= LAUNCH_SUPER_VALIDATE
            and full_access > SUPER_VALIDATE_MAX_FLAGS
        )
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
    if operation == LAUNCH_FIND_SUPER_EXPOSE_ALIAS:
        metadata[unsafe_offset=0] = launch_find_super_expose_alias(
            arguments, count
        )
        return 0
    if operation == LAUNCH_LOGIN_POLICY:
        launch_login_policy(arguments, count, metadata)
        return 0
    if operation == LAUNCH_CLI_DEFAULT_RUN:
        launch_cli_default_run_policy(arguments, count, metadata)
        return 0
    if operation == LAUNCH_SUPER_COMPAT_ALIAS:
        launch_super_compat_alias_policy(arguments, count, metadata)
        return 0
    if operation == LAUNCH_SUPER_LITERAL_BOUNDARY:
        launch_super_literal_boundary_policy(arguments, count, metadata)
        return 0
    if operation == LAUNCH_SUPER_POSITIONED_ALIAS:
        if full_access > 3 or count > 1:
            return 1
        launch_super_positioned_alias_policy(
            arguments,
            count,
            (full_access & 1) != 0,
            (full_access & 2) != 0,
            metadata,
        )
        return 0
    if operation == LAUNCH_CODEX_COMMAND_SERVER:
        launch_codex_command_server_policy(arguments, count, metadata)
        return 0
    if (
        operation == LAUNCH_SUPER_VALIDATE
        or operation == LAUNCH_SUPER_VALIDATE_SUB_AGENT
    ):
        launch_super_validation_policy(
            arguments,
            count,
            full_access,
            operation == LAUNCH_SUPER_VALIDATE_SUB_AGENT,
            metadata,
        )
        return 0
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


comptime LOGIN_MENU_POLICY_ABI_VERSION: Int64 = 1
comptime LOGIN_MENU_POLICY_LAYOUT: Int64 = 1
comptime LOGIN_MENU_POLICY_OFFSET: Int64 = 2
comptime LOGIN_MENU_POLICY_KEY: Int64 = 3

comptime LOGIN_MENU_KEY_UP: Int64 = 0
comptime LOGIN_MENU_KEY_DOWN: Int64 = 1
comptime LOGIN_MENU_KEY_PAGE_UP: Int64 = 2
comptime LOGIN_MENU_KEY_PAGE_DOWN: Int64 = 3
comptime LOGIN_MENU_KEY_HOME: Int64 = 4
comptime LOGIN_MENU_KEY_END: Int64 = 5
comptime LOGIN_MENU_KEY_ENTER: Int64 = 6
comptime LOGIN_MENU_KEY_CANCEL: Int64 = 7
comptime LOGIN_MENU_KEY_DIGIT: Int64 = 8
comptime LOGIN_MENU_KEY_IGNORE: Int64 = 9

comptime LOGIN_MENU_ACTION_NONE: Int64 = 0
comptime LOGIN_MENU_ACTION_SELECT: Int64 = 1
comptime LOGIN_MENU_ACTION_CANCEL: Int64 = 2


@export("prodex_login_menu_policy_v1")
def prodex_login_menu_policy_v1(
    abi_version: Int64,
    operation: Int64,
    input0: Int64,
    input1: Int64,
    input2: Int64,
    input3: Int64,
    input4: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != LOGIN_MENU_POLICY_ABI_VERSION
        or operation < LOGIN_MENU_POLICY_LAYOUT
        or operation > LOGIN_MENU_POLICY_KEY
        or output_address == 0
    ):
        return 1
    for value in [input0, input1, input2, input3, input4]:
        if value < 0:
            return 1

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(4):
        output[unsafe_offset=index] = 0

    if operation == LOGIN_MENU_POLICY_LAYOUT:
        var rows = input0
        var entry_count = input1
        if entry_count == 0:
            output[unsafe_offset=0] = 0
            output[unsafe_offset=1] = 1
            return 0
        if rows < Int64(8):
            rows = Int64(8)
        var compact = rows < Int64(16)
        var reserved = Int64(5) if compact else Int64(7)
        var min_visible = Int64(3)
        if entry_count < min_visible:
            min_visible = entry_count
        var visible = rows - reserved
        if visible < min_visible:
            visible = min_visible
        if visible > entry_count:
            visible = entry_count
        output[unsafe_offset=0] = visible
        output[unsafe_offset=1] = Int64(compact)
        return 0

    if operation == LOGIN_MENU_POLICY_OFFSET:
        var selected = input0
        var current_offset = input1
        var visible_items = input2
        var entry_count = input3
        if entry_count == 0 or visible_items == 0:
            return 0
        var max_offset = entry_count - visible_items
        if max_offset < 0:
            max_offset = 0
        var next_offset = current_offset
        if next_offset > max_offset:
            next_offset = max_offset
        if selected < current_offset:
            next_offset = selected
            if next_offset > max_offset:
                next_offset = max_offset
        elif selected >= current_offset + visible_items:
            next_offset = selected + 1 - visible_items
            if next_offset < 0:
                next_offset = 0
            if next_offset > max_offset:
                next_offset = max_offset
        output[unsafe_offset=0] = next_offset
        return 0

    var key = input0
    var digit = input1
    var selected = input2
    var visible_items = input3
    var entry_count = input4
    if key < LOGIN_MENU_KEY_UP or key > LOGIN_MENU_KEY_IGNORE:
        return 1
    var last = entry_count - 1
    if last < 0:
        last = 0
    var next_selected = selected
    if next_selected > last:
        next_selected = last
    var action = LOGIN_MENU_ACTION_NONE
    var action_index: Int64 = -1
    var page_step = visible_items - 1
    if page_step < 1:
        page_step = 1

    if key == LOGIN_MENU_KEY_UP:
        next_selected -= 1
        if next_selected < 0:
            next_selected = 0
    elif key == LOGIN_MENU_KEY_DOWN:
        next_selected += 1
        if next_selected > last:
            next_selected = last
    elif key == LOGIN_MENU_KEY_PAGE_UP:
        next_selected -= page_step
        if next_selected < 0:
            next_selected = 0
    elif key == LOGIN_MENU_KEY_PAGE_DOWN:
        next_selected += page_step
        if next_selected > last:
            next_selected = last
    elif key == LOGIN_MENU_KEY_HOME:
        next_selected = 0
    elif key == LOGIN_MENU_KEY_END:
        next_selected = last
    elif key == LOGIN_MENU_KEY_ENTER:
        if entry_count > 0:
            action = LOGIN_MENU_ACTION_SELECT
            action_index = next_selected
    elif key == LOGIN_MENU_KEY_CANCEL:
        action = LOGIN_MENU_ACTION_CANCEL
    elif key == LOGIN_MENU_KEY_DIGIT:
        if digit >= 1 and digit <= entry_count:
            action = LOGIN_MENU_ACTION_SELECT
            action_index = digit - 1

    output[unsafe_offset=0] = next_selected
    output[unsafe_offset=1] = action
    output[unsafe_offset=2] = action_index
    return 0


comptime SUPER_CHOICE_POLICY_ABI_VERSION: Int64 = 1
comptime SUPER_CHOICE_POLICY_RANGE: Int64 = 1
comptime SUPER_CHOICE_POLICY_KEY: Int64 = 2
comptime SUPER_CHOICE_POLICY_INITIAL: Int64 = 3

comptime SUPER_CHOICE_KEY_UP: Int64 = 0
comptime SUPER_CHOICE_KEY_DOWN: Int64 = 1
comptime SUPER_CHOICE_KEY_PAGE_UP: Int64 = 2
comptime SUPER_CHOICE_KEY_PAGE_DOWN: Int64 = 3
comptime SUPER_CHOICE_KEY_HOME: Int64 = 4
comptime SUPER_CHOICE_KEY_END: Int64 = 5
comptime SUPER_CHOICE_KEY_ENTER: Int64 = 6
comptime SUPER_CHOICE_KEY_ESCAPE: Int64 = 7
comptime SUPER_CHOICE_KEY_CANCEL: Int64 = 8
comptime SUPER_CHOICE_KEY_IGNORE: Int64 = 9

comptime SUPER_CHOICE_ACTION_NONE: Int64 = 0
comptime SUPER_CHOICE_ACTION_SELECT: Int64 = 1
comptime SUPER_CHOICE_ACTION_CANCEL: Int64 = 2


@export("prodex_super_choice_policy_v1")
def prodex_super_choice_policy_v1(
    abi_version: Int64,
    operation: Int64,
    input0: Int64,
    input1: Int64,
    input2: Int64,
    input3: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != SUPER_CHOICE_POLICY_ABI_VERSION
        or operation < SUPER_CHOICE_POLICY_RANGE
        or operation > SUPER_CHOICE_POLICY_INITIAL
        or output_address == 0
    ):
        return 1
    for value in [input0, input1, input2, input3]:
        if value < 0:
            return 1

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(4):
        output[unsafe_offset=index] = 0

    if operation == SUPER_CHOICE_POLICY_RANGE:
        var selected = input0
        var length = input1
        var height = input2
        if length == 0:
            return 1
        var visible = height
        if visible < 1:
            visible = 1
        if visible > length:
            visible = length
        var half = visible // 2
        var start = selected - half
        if start < 0:
            start = 0
        var max_start = length - visible
        if start > max_start:
            start = max_start
        output[unsafe_offset=0] = start
        output[unsafe_offset=1] = start + visible
        return 0

    if operation == SUPER_CHOICE_POLICY_INITIAL:
        if input0 < 0 or input1 <= 0 or input2 != 0 or input3 != 0:
            return 1
        var selected = input0
        if selected >= input1:
            selected = input1 - 1
        output[unsafe_offset=0] = selected
        return 0

    var key = input0
    var selected = input1
    var length = input2
    var escape_selects_last = input3 == 1
    if (
        key < SUPER_CHOICE_KEY_UP
        or key > SUPER_CHOICE_KEY_IGNORE
        or length == 0
        or selected >= length
        or input3 > 1
    ):
        return 1

    var last = length - 1
    var next_selected = selected
    var action = SUPER_CHOICE_ACTION_NONE
    var action_index: Int64 = -1

    if key == SUPER_CHOICE_KEY_UP:
        if next_selected == 0:
            next_selected = last
        else:
            next_selected -= 1
    elif key == SUPER_CHOICE_KEY_DOWN:
        next_selected += 1
        if next_selected >= length:
            next_selected = 0
    elif key == SUPER_CHOICE_KEY_PAGE_UP:
        next_selected -= 10
        if next_selected < 0:
            next_selected = 0
    elif key == SUPER_CHOICE_KEY_PAGE_DOWN:
        next_selected += 10
        if next_selected > last:
            next_selected = last
    elif key == SUPER_CHOICE_KEY_HOME:
        next_selected = 0
    elif key == SUPER_CHOICE_KEY_END:
        next_selected = last
    elif key == SUPER_CHOICE_KEY_ENTER:
        action = SUPER_CHOICE_ACTION_SELECT
        action_index = next_selected
    elif key == SUPER_CHOICE_KEY_ESCAPE:
        if escape_selects_last:
            action = SUPER_CHOICE_ACTION_SELECT
            action_index = last
        else:
            action = SUPER_CHOICE_ACTION_CANCEL
    elif key == SUPER_CHOICE_KEY_CANCEL:
        action = SUPER_CHOICE_ACTION_CANCEL

    output[unsafe_offset=0] = next_selected
    output[unsafe_offset=1] = action
    output[unsafe_offset=2] = action_index
    return 0


comptime SUPER_PROMPT_POLICY_ABI_VERSION: Int64 = 1
comptime SUPER_PROMPT_POLICY_CHOICE_EVENT: Int64 = 1
comptime SUPER_PROMPT_POLICY_TEXT_EVENT: Int64 = 2
comptime SUPER_PROMPT_POLICY_TEXT_PREFIX: Int64 = 3
comptime SUPER_PROMPT_POLICY_CHOICE_INDEX: Int64 = 4
comptime SUPER_PROMPT_POLICY_VALUE_INDEX: Int64 = 5
comptime SUPER_PROMPT_POLICY_LABEL: Int64 = 6
comptime SUPER_PROMPT_POLICY_MAX_CHOICES: Int64 = 1_024
comptime SUPER_PROMPT_POLICY_MAX_TEXT_BYTES: Int64 = 65_536
comptime SUPER_PROMPT_POLICY_MAX_LABEL_BYTES: Int64 = 65_600
comptime SUPER_PROMPT_MAX_TEXT_CHARS: Int64 = 256

comptime SUPER_PROMPT_KEY_UP: Int64 = 0
comptime SUPER_PROMPT_KEY_DOWN: Int64 = 1
comptime SUPER_PROMPT_KEY_PAGE_UP: Int64 = 2
comptime SUPER_PROMPT_KEY_PAGE_DOWN: Int64 = 3
comptime SUPER_PROMPT_KEY_HOME: Int64 = 4
comptime SUPER_PROMPT_KEY_END: Int64 = 5
comptime SUPER_PROMPT_KEY_ENTER: Int64 = 6
comptime SUPER_PROMPT_KEY_ESCAPE: Int64 = 7
comptime SUPER_PROMPT_KEY_CHAR: Int64 = 8
comptime SUPER_PROMPT_KEY_OTHER: Int64 = 9

comptime SUPER_PROMPT_TEXT_CHAR: Int64 = 0
comptime SUPER_PROMPT_TEXT_BACKSPACE: Int64 = 1
comptime SUPER_PROMPT_TEXT_ENTER: Int64 = 2
comptime SUPER_PROMPT_TEXT_ESCAPE: Int64 = 3
comptime SUPER_PROMPT_TEXT_OTHER: Int64 = 4

comptime SUPER_PROMPT_TEXT_IGNORE: Int64 = 0
comptime SUPER_PROMPT_TEXT_APPEND: Int64 = 1
comptime SUPER_PROMPT_TEXT_BACKSPACE_ACTION: Int64 = 2
comptime SUPER_PROMPT_TEXT_ACCEPT: Int64 = 3
comptime SUPER_PROMPT_TEXT_RETURN_NONE: Int64 = 4
comptime SUPER_PROMPT_TEXT_CANCEL: Int64 = 5

comptime SUPER_PROMPT_LABEL_PROVIDER: Int64 = 0
comptime SUPER_PROMPT_LABEL_VALUE: Int64 = 1
comptime SUPER_PROMPT_LABEL_PROVIDER_DEFAULT: Int64 = 2
comptime SUPER_PROMPT_LABEL_CUSTOM_MODEL: Int64 = 3
comptime SUPER_PROMPT_LABEL_CUSTOM_CONCURRENCY: Int64 = 4
comptime SUPER_PROMPT_LABEL_DEFAULT_CONCURRENCY: Int64 = 5
comptime SUPER_PROMPT_LABEL_PRESET_CONCURRENCY: Int64 = 6
comptime SUPER_PROMPT_LABEL_CUSTOM_CONCURRENCY_TITLE: Int64 = 7

@fieldwise_init
struct SuperPromptChoice(Copyable):
    var kind: Int64
    var value: ProdexRichStringView


def super_prompt_choice_event(
    key_kind: Int64,
    character: Int64,
    control: Int64,
    selected: Int64,
    length: Int64,
    escape_selects_last: Int64,
    output_address: UInt,
) -> Int64:
    if (
        key_kind < SUPER_PROMPT_KEY_UP
        or key_kind > SUPER_PROMPT_KEY_OTHER
        or character < 0
        or character > 0x10FFFF
        or character >= 0xD800 and character <= 0xDFFF
        or control < 0
        or control > 1
        or selected < 0
        or length <= 0
        or selected >= length
        or escape_selects_last < 0
        or escape_selects_last > 1
    ):
        return 1

    var key = SUPER_CHOICE_KEY_IGNORE
    if key_kind == SUPER_PROMPT_KEY_UP or key_kind == SUPER_PROMPT_KEY_CHAR and character == 107:
        key = SUPER_CHOICE_KEY_UP
    elif key_kind == SUPER_PROMPT_KEY_DOWN or key_kind == SUPER_PROMPT_KEY_CHAR and character == 106:
        key = SUPER_CHOICE_KEY_DOWN
    elif key_kind == SUPER_PROMPT_KEY_PAGE_UP:
        key = SUPER_CHOICE_KEY_PAGE_UP
    elif key_kind == SUPER_PROMPT_KEY_PAGE_DOWN:
        key = SUPER_CHOICE_KEY_PAGE_DOWN
    elif key_kind == SUPER_PROMPT_KEY_HOME:
        key = SUPER_CHOICE_KEY_HOME
    elif key_kind == SUPER_PROMPT_KEY_END:
        key = SUPER_CHOICE_KEY_END
    elif key_kind == SUPER_PROMPT_KEY_ENTER:
        key = SUPER_CHOICE_KEY_ENTER
    elif key_kind == SUPER_PROMPT_KEY_ESCAPE:
        key = SUPER_CHOICE_KEY_ESCAPE
    elif key_kind == SUPER_PROMPT_KEY_CHAR and control == 1 and (character == 99 or character == 122):
        key = SUPER_CHOICE_KEY_CANCEL

    return prodex_super_choice_policy_v1(
        SUPER_CHOICE_POLICY_ABI_VERSION,
        SUPER_CHOICE_POLICY_KEY,
        key,
        selected,
        length,
        escape_selects_last,
        output_address,
    )


def super_prompt_text_event(
    key_kind: Int64,
    character: Int64,
    control: Int64,
    current_characters: Int64,
    escape_returns_none: Int64,
    output: Pointer[mut=True, Int64, _],
) -> Int64:
    if (
        key_kind < SUPER_PROMPT_TEXT_CHAR
        or key_kind > SUPER_PROMPT_TEXT_OTHER
        or character < 0
        or character > 0x10FFFF
        or character >= 0xD800 and character <= 0xDFFF
        or control < 0
        or control > 1
        or current_characters < 0
        or escape_returns_none < 0
        or escape_returns_none > 1
    ):
        return 1

    var action = SUPER_PROMPT_TEXT_IGNORE
    var clear_validation: Int64 = 0
    if key_kind == SUPER_PROMPT_TEXT_ENTER:
        action = SUPER_PROMPT_TEXT_ACCEPT
    elif key_kind == SUPER_PROMPT_TEXT_BACKSPACE:
        action = SUPER_PROMPT_TEXT_BACKSPACE_ACTION
        clear_validation = 1
    elif key_kind == SUPER_PROMPT_TEXT_CHAR:
        if control == 1 and (character == 99 or character == 122):
            action = SUPER_PROMPT_TEXT_CANCEL
        elif control == 0 and current_characters < SUPER_PROMPT_MAX_TEXT_CHARS:
            action = SUPER_PROMPT_TEXT_APPEND
            clear_validation = 1
    elif key_kind == SUPER_PROMPT_TEXT_ESCAPE:
        if escape_returns_none == 1:
            action = SUPER_PROMPT_TEXT_RETURN_NONE
        else:
            action = SUPER_PROMPT_TEXT_CANCEL

    output[unsafe_offset=0] = action
    output[unsafe_offset=1] = character
    output[unsafe_offset=2] = clear_validation
    return 0


def super_prompt_text_prefix_length(
    value: ProdexRichStringView, maximum_characters: Int64
) -> Int64:
    if maximum_characters < 0 or not rich_view_valid(value, SUPER_PROMPT_POLICY_MAX_TEXT_BYTES):
        return -1
    if maximum_characters == 0 or value.len == 0:
        return 0
    var ptr = rich_view_ptr(value)
    var offset: Int64 = 0
    var characters: Int64 = 0
    while offset < Int64(value.len) and characters < maximum_characters:
        offset += rich_codepoint_width(ptr[unsafe_offset=offset])
        characters += 1
    return offset


def super_prompt_choice_index(
    choices_address: UInt,
    choices_count: Int64,
    current: ProdexRichStringView,
    current_present: Int64,
    case_insensitive: Int64,
    fallback_index: Int64,
) -> Int64:
    if (
        choices_count <= 0
        or choices_count > SUPER_PROMPT_POLICY_MAX_CHOICES
        or choices_address == 0
        or current_present < 0
        or current_present > 1
        or case_insensitive < 0
        or case_insensitive > 1
        or fallback_index < 0
        or fallback_index >= choices_count
        or (current_present == 1 and not rich_view_valid(current, SUPER_PROMPT_POLICY_MAX_TEXT_BYTES))
    ):
        return -1
    var choices = Pointer[mut=False, SuperPromptChoice, ImmUntrackedOrigin](
        unsafe_from_address=Int(choices_address)
    )
    for index in range(choices_count):
        var choice = choices[unsafe_offset=index].copy()
        if choice.kind < 0 or choice.kind > 2:
            return -1
        if choice.kind == 1 and not rich_view_valid(choice.value, SUPER_PROMPT_POLICY_MAX_TEXT_BYTES):
            return -1
    if current_present == 1:
        var current_ptr = rich_view_ptr(current)
        for index in range(choices_count):
            var choice = choices[unsafe_offset=index].copy()
            if choice.kind != 1:
                continue
            if choice.value.len != current.len:
                continue
            var equal = True
            var choice_ptr = rich_view_ptr(choice.value)
            for byte_index in range(Int64(current.len)):
                var left = choice_ptr[unsafe_offset=byte_index]
                var right = current_ptr[unsafe_offset=byte_index]
                if case_insensitive == 1:
                    if left >= 65 and left <= 90:
                        left += 32
                    if right >= 65 and right <= 90:
                        right += 32
                if left != right:
                    equal = False
                    break
            if equal:
                return index
    return fallback_index


def super_prompt_value_index(
    values_address: UInt,
    values_count: Int64,
    wanted: Int64,
    fallback_index: Int64,
) -> Int64:
    if (
        values_count <= 0
        or values_count > SUPER_PROMPT_POLICY_MAX_CHOICES
        or values_address == 0
        or wanted < 0
        or fallback_index < 0
        or fallback_index >= values_count
    ):
        return -1
    var values = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(values_address)
    )
    for index in range(values_count):
        if values[unsafe_offset=index] < 0:
            return -1
        if values[unsafe_offset=index] == wanted:
            return index
    return fallback_index


def super_prompt_put_byte(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    value: UInt8,
) -> Bool:
    if written[] < 0 or written[] >= capacity:
        return False
    output[unsafe_offset=written[]] = value
    written[] += 1
    return True


def super_prompt_put_text(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    value: StringSlice,
) -> Bool:
    var source = value.unsafe_ptr()
    for index in range(Int64(value.byte_length())):
        if not super_prompt_put_byte(output, capacity, written, source[unsafe_offset=index]):
            return False
    return True


def super_prompt_put_view(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    value: ProdexRichStringView,
) -> Bool:
    if value.len == 0:
        return True
    var source = rich_view_ptr(value)
    for index in range(Int64(value.len)):
        if not super_prompt_put_byte(output, capacity, written, source[unsafe_offset=index]):
            return False
    return True


def super_prompt_put_decimal(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    value: Int64,
) -> Bool:
    if value < 0 or value > 65_535:
        return False
    var started = False
    var remaining = value
    for index in range(5):
        var divisor: Int64 = 1
        if index == 0:
            divisor = 10_000
        elif index == 1:
            divisor = 1_000
        elif index == 2:
            divisor = 100
        elif index == 3:
            divisor = 10
        var digit = remaining // divisor
        remaining = remaining % divisor
        if digit > 0 or started or index == 4:
            started = True
            if not super_prompt_put_byte(output, capacity, written, UInt8(digit + 48)):
                return False
    return True


def super_prompt_write_label(
    label_kind: Int64,
    value: Int64,
    text: ProdexRichStringView,
    flag: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) -> Int64:
    if (
        label_kind < SUPER_PROMPT_LABEL_PROVIDER
        or label_kind > SUPER_PROMPT_LABEL_CUSTOM_CONCURRENCY_TITLE
        or value < 0
        or value > 65_535
        or flag < 0
        or flag > 1
        or output_capacity < 0
        or output_capacity > SUPER_PROMPT_POLICY_MAX_LABEL_BYTES
        or written_address == 0
        or (output_capacity > 0 and output_address == 0)
    ):
        return 1
    if (label_kind == SUPER_PROMPT_LABEL_PROVIDER or label_kind == SUPER_PROMPT_LABEL_VALUE) and not rich_view_valid(text, SUPER_PROMPT_POLICY_MAX_TEXT_BYTES):
        return 1
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if label_kind == SUPER_PROMPT_LABEL_PROVIDER or label_kind == SUPER_PROMPT_LABEL_VALUE:
        if not super_prompt_put_view(output, output_capacity, written, text):
            return 3
        if label_kind == SUPER_PROMPT_LABEL_PROVIDER and flag == 1:
            if not super_prompt_put_text(output, output_capacity, written, StringSlice(" (session affinity)")):
                return 3
    elif label_kind == SUPER_PROMPT_LABEL_PROVIDER_DEFAULT:
        if not super_prompt_put_text(output, output_capacity, written, StringSlice("provider default")):
            return 3
    elif label_kind == SUPER_PROMPT_LABEL_CUSTOM_MODEL:
        if not super_prompt_put_text(output, output_capacity, written, StringSlice("custom model...")):
            return 3
    elif label_kind == SUPER_PROMPT_LABEL_CUSTOM_CONCURRENCY:
        if not super_prompt_put_text(output, output_capacity, written, StringSlice("custom...")):
            return 3
    elif label_kind == SUPER_PROMPT_LABEL_DEFAULT_CONCURRENCY:
        if not super_prompt_put_text(output, output_capacity, written, StringSlice("default (")):
            return 3
        if not super_prompt_put_decimal(output, output_capacity, written, value):
            return 3
        if not super_prompt_put_byte(output, output_capacity, written, UInt8(41)):
            return 3
    elif label_kind == SUPER_PROMPT_LABEL_PRESET_CONCURRENCY:
        if not super_prompt_put_decimal(output, output_capacity, written, value):
            return 3
    elif label_kind == SUPER_PROMPT_LABEL_CUSTOM_CONCURRENCY_TITLE:
        if not super_prompt_put_text(output, output_capacity, written, StringSlice("Enter maximum active sub-agents (1-")):
            return 3
        if not super_prompt_put_decimal(output, output_capacity, written, value):
            return 3
        if not super_prompt_put_byte(output, output_capacity, written, UInt8(41)):
            return 3
    return 0


@export("prodex_super_prompt_policy_v1")
def prodex_super_prompt_policy_v1(
    abi_version: Int64,
    operation: Int64,
    input0: Int64,
    input1: Int64,
    input2: Int64,
    input3: Int64,
    input4: Int64,
    input5: Int64,
    records_address: UInt,
    records_count: Int64,
    text_address: UInt,
    text_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    output_count_address: UInt,
) abi("C") -> Int64:
    if abi_version != SUPER_PROMPT_POLICY_ABI_VERSION or operation < SUPER_PROMPT_POLICY_CHOICE_EVENT or operation > SUPER_PROMPT_POLICY_LABEL:
        return 4
    if operation == SUPER_PROMPT_POLICY_LABEL:
        return super_prompt_write_label(
            input0,
            input1,
            ProdexRichStringView(text_address, UInt(text_length)),
            input2,
            output_address,
            output_capacity,
            output_count_address,
        )
    if output_address == 0:
        return 1
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(4):
        output[unsafe_offset=index] = -1
    if operation == SUPER_PROMPT_POLICY_CHOICE_EVENT:
        return super_prompt_choice_event(input0, input1, input2, input3, input4, input5, output_address)
    if operation == SUPER_PROMPT_POLICY_TEXT_EVENT:
        return super_prompt_text_event(input0, input1, input2, input3, input4, output)
    if operation == SUPER_PROMPT_POLICY_TEXT_PREFIX:
        if input0 < 0 or input0 > SUPER_PROMPT_MAX_TEXT_CHARS or text_length < 0:
            return 1
        var value = ProdexRichStringView(text_address, UInt(text_length))
        var maximum = SUPER_PROMPT_MAX_TEXT_CHARS
        if input0 > 0:
            maximum = input0
        var length = super_prompt_text_prefix_length(value, maximum)
        if length < 0:
            return 1
        output[unsafe_offset=0] = length
        return 0
    if operation == SUPER_PROMPT_POLICY_CHOICE_INDEX:
        if records_count <= 0 or records_count > SUPER_PROMPT_POLICY_MAX_CHOICES or input0 < 0 or input0 > 1 or input1 < 0 or input2 < 0 or input2 > 1:
            return 1
        var current = ProdexRichStringView(text_address, UInt(text_length))
        var index = super_prompt_choice_index(records_address, records_count, current, input2, input0, input1)
        if index < 0:
            return 1
        output[unsafe_offset=0] = index
        return 0
    if operation == SUPER_PROMPT_POLICY_VALUE_INDEX:
        if records_count <= 0 or records_count > SUPER_PROMPT_POLICY_MAX_CHOICES:
            return 1
        var index = super_prompt_value_index(records_address, records_count, input0, input1)
        if index < 0:
            return 1
        output[unsafe_offset=0] = index
        return 0
    return 1
