from std.memory import Pointer

comptime APP_COMMAND_POLICY_ABI_VERSION: Int64 = 1
comptime APP_COMMAND_POLICY_STATUS_OK: Int64 = 0
comptime APP_COMMAND_POLICY_STATUS_INVALID: Int64 = 1
comptime APP_COMMAND_POLICY_STATUS_ABI: Int64 = 4

comptime APP_COMMAND_OTHER: Int64 = 0
comptime APP_COMMAND_RUNTIME_BROKER: Int64 = 1
comptime APP_COMMAND_INFO: Int64 = 2
comptime APP_COMMAND_LOG: Int64 = 3
comptime APP_COMMAND_UPDATE: Int64 = 4
comptime APP_COMMAND_PING: Int64 = 5
comptime APP_COMMAND_SUPER_EXPOSE: Int64 = 6
comptime APP_COMMAND_MCP_BRIDGE: Int64 = 7
comptime APP_COMMAND_SUB_AGENT_EXEC: Int64 = 8
comptime APP_COMMAND_PROFILE_REMOVE: Int64 = 9
comptime APP_COMMAND_DOCTOR: Int64 = 10
comptime APP_COMMAND_SUPER: Int64 = 11

comptime APP_COMMAND_POLICY_SUPER_DRY_RUN: Int64 = 1
comptime APP_COMMAND_POLICY_UPDATE_NOTICE: Int64 = 2
comptime APP_COMMAND_POLICY_PROFILE_RECOVERY: Int64 = 4
comptime APP_COMMAND_POLICY_AUTO_HOUSEKEEPING: Int64 = 8
comptime APP_COMMAND_POLICY_MINIMAL_STARTUP: Int64 = 16


def app_command_policy_flag_valid(value: Int64) -> Bool:
    return value == 0 or value == 1


@export("prodex_app_command_startup_policy_v1")
def prodex_app_command_startup_policy_v1(
    abi_version: Int64,
    command_class: Int64,
    super_direct_dry_run: Int64,
    super_codex_dry_run: Int64,
    super_expose_direct_dry_run: Int64,
    super_expose_codex_dry_run: Int64,
    native_agy: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != APP_COMMAND_POLICY_ABI_VERSION:
        return APP_COMMAND_POLICY_STATUS_ABI
    if (
        command_class < APP_COMMAND_OTHER
        or command_class > APP_COMMAND_SUPER
        or not app_command_policy_flag_valid(super_direct_dry_run)
        or not app_command_policy_flag_valid(super_codex_dry_run)
        or not app_command_policy_flag_valid(super_expose_direct_dry_run)
        or not app_command_policy_flag_valid(super_expose_codex_dry_run)
        or not app_command_policy_flag_valid(native_agy)
        or output_address == 0
    ):
        return APP_COMMAND_POLICY_STATUS_INVALID

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var super_dry_run = False
    if command_class == APP_COMMAND_SUPER:
        super_dry_run = super_direct_dry_run == 1 or super_codex_dry_run == 1
    elif command_class == APP_COMMAND_SUPER_EXPOSE:
        super_dry_run = (
            super_expose_direct_dry_run == 1
            or super_expose_codex_dry_run == 1
        )

    var policy: Int64 = 0
    if super_dry_run:
        policy |= APP_COMMAND_POLICY_SUPER_DRY_RUN

    var native = native_agy == 1
    if (
        not super_dry_run
        and not native
        and command_class != APP_COMMAND_RUNTIME_BROKER
        and command_class != APP_COMMAND_INFO
        and command_class != APP_COMMAND_LOG
        and command_class != APP_COMMAND_UPDATE
        and command_class != APP_COMMAND_PING
        and command_class != APP_COMMAND_SUPER_EXPOSE
        and command_class != APP_COMMAND_MCP_BRIDGE
        and command_class != APP_COMMAND_SUB_AGENT_EXEC
    ):
        policy |= APP_COMMAND_POLICY_UPDATE_NOTICE

    if (
        not super_dry_run
        and not native
        and command_class != APP_COMMAND_PROFILE_REMOVE
        and command_class != APP_COMMAND_INFO
        and command_class != APP_COMMAND_LOG
        and command_class != APP_COMMAND_DOCTOR
        and command_class != APP_COMMAND_PING
        and command_class != APP_COMMAND_MCP_BRIDGE
        and command_class != APP_COMMAND_SUB_AGENT_EXEC
    ):
        policy |= APP_COMMAND_POLICY_PROFILE_RECOVERY

    if (
        not super_dry_run
        and not native
        and command_class != APP_COMMAND_RUNTIME_BROKER
        and command_class != APP_COMMAND_INFO
        and command_class != APP_COMMAND_LOG
        and command_class != APP_COMMAND_UPDATE
        and command_class != APP_COMMAND_PING
    ):
        policy |= APP_COMMAND_POLICY_AUTO_HOUSEKEEPING

    if (
        native
        or command_class == APP_COMMAND_PING
        or command_class == APP_COMMAND_MCP_BRIDGE
    ):
        policy |= APP_COMMAND_POLICY_MINIMAL_STARTUP

    output[] = policy
    return APP_COMMAND_POLICY_STATUS_OK
