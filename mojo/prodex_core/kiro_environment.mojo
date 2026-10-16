from std.memory import Pointer


comptime KIRO_ENVIRONMENT_ABI_VERSION: Int64 = 1
comptime KIRO_ENVIRONMENT_STATUS_OK: Int64 = 0
comptime KIRO_ENVIRONMENT_STATUS_INVALID: Int64 = 1
comptime KIRO_ENVIRONMENT_STATUS_CAPACITY: Int64 = 3
comptime KIRO_ENVIRONMENT_STATUS_ABI: Int64 = 4
comptime KIRO_ENVIRONMENT_MAX_SERVERS: Int64 = 65_536

comptime KIRO_MCP_DISABLED_MISSING: Int64 = 0
comptime KIRO_MCP_DISABLED_NULL: Int64 = 1
comptime KIRO_MCP_DISABLED_ARRAY: Int64 = 2
comptime KIRO_MCP_DISABLED_INVALID: Int64 = 3

comptime KIRO_MCP_ACTION_KEEP: Int64 = 0
comptime KIRO_MCP_ACTION_INITIALIZE: Int64 = 1
comptime KIRO_MCP_ACTION_APPEND: Int64 = 2
comptime KIRO_MCP_ACTION_INVALID_DISABLED_TOOLS: Int64 = 3


@fieldwise_init
struct ProdexKiroMcpServerFacts(Copyable):
    var server_name_matches: Int64
    var command_name_matches: Int64
    var disabled_tools_state: Int64
    var incompatible_tool_present: Int64


def kiro_environment_bool(value: Int64) -> Bool:
    return value == 0 or value == 1


@export("prodex_kiro_mcp_normalization_plan_v1")
def prodex_kiro_mcp_normalization_plan_v1(
    abi_version: Int64,
    facts_address: UInt,
    fact_count: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != KIRO_ENVIRONMENT_ABI_VERSION:
        return KIRO_ENVIRONMENT_STATUS_ABI
    if (
        fact_count < 0
        or fact_count > KIRO_ENVIRONMENT_MAX_SERVERS
        or output_capacity < 0
        or (fact_count > 0 and facts_address == 0)
        or (fact_count > 0 and output_address == 0)
        or written_address == 0
    ):
        return KIRO_ENVIRONMENT_STATUS_INVALID
    if output_capacity < fact_count:
        return KIRO_ENVIRONMENT_STATUS_CAPACITY

    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if fact_count == 0:
        return KIRO_ENVIRONMENT_STATUS_OK

    var facts = Pointer[mut=False, ProdexKiroMcpServerFacts, ImmUntrackedOrigin](
        unsafe_from_address=Int(facts_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(fact_count):
        var fact = facts[unsafe_offset=index].copy()
        if (
            not kiro_environment_bool(fact.server_name_matches)
            or not kiro_environment_bool(fact.command_name_matches)
            or fact.disabled_tools_state < KIRO_MCP_DISABLED_MISSING
            or fact.disabled_tools_state > KIRO_MCP_DISABLED_INVALID
            or not kiro_environment_bool(fact.incompatible_tool_present)
        ):
            return KIRO_ENVIRONMENT_STATUS_INVALID
    for index in range(fact_count):
        var fact = facts[unsafe_offset=index].copy()
        var action = KIRO_MCP_ACTION_KEEP
        if fact.server_name_matches == 1 or fact.command_name_matches == 1:
            if fact.disabled_tools_state == KIRO_MCP_DISABLED_INVALID:
                action = KIRO_MCP_ACTION_INVALID_DISABLED_TOOLS
            elif fact.disabled_tools_state == KIRO_MCP_DISABLED_MISSING or fact.disabled_tools_state == KIRO_MCP_DISABLED_NULL:
                action = KIRO_MCP_ACTION_INITIALIZE
            elif fact.incompatible_tool_present == 0:
                action = KIRO_MCP_ACTION_APPEND
        output[unsafe_offset=index] = action
    written[] = fact_count
    return KIRO_ENVIRONMENT_STATUS_OK
