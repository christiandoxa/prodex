from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_matches_literal, rich_view_valid

# Compiled ACP session/update discriminant and required-field precedence.
# Rust supplies Serde-acquired field presence/type facts and constructs the
# typed host enum, never an independent update classifier.
comptime KIRO_ACP_UPDATE_ABI_VERSION: Int64 = 1
comptime KIRO_ACP_UPDATE_FIELD_CONTENT: Int64 = 1
comptime KIRO_ACP_UPDATE_FIELD_TOOL_CALL_ID: Int64 = 2
comptime KIRO_ACP_UPDATE_FIELD_TITLE: Int64 = 4
comptime KIRO_ACP_UPDATE_FIELD_ENTRIES: Int64 = 8
comptime KIRO_ACP_UPDATE_FIELD_USED: Int64 = 16
comptime KIRO_ACP_UPDATE_FIELD_SIZE: Int64 = 32
comptime KIRO_ACP_UPDATE_FIELD_COMMANDS: Int64 = 64
comptime KIRO_ACP_UPDATE_FIELD_MODE: Int64 = 128


@export("prodex_kiro_acp_session_update_plan_v1")
def prodex_kiro_acp_session_update_plan_v1(
    abi_version: Int64,
    discriminator_present: Int64,
    discriminator_address: UInt,
    discriminator_length: Int64,
    field_mask: Int64,
    output_address: UInt,
    output_capacity: Int64,
) abi("C") -> Int64:
    if abi_version != KIRO_ACP_UPDATE_ABI_VERSION:
        return 4
    if (
        (discriminator_present != 0 and discriminator_present != 1)
        or discriminator_length < 0
        or (discriminator_length > 0 and discriminator_address == 0)
        or field_mask < 0 or field_mask > 255
        or output_address == 0 or output_capacity < 2
    ):
        return 1
    var label = ProdexRichStringView(
        discriminator_address, UInt(discriminator_length)
    )
    if not rich_view_valid(label, 0x7FFFFFFFFFFFFFFF):
        return 1
    # Output indices: enum kind, first missing required field.
    # Kind 0 = Unknown. Other tags encode known ACP update variants.
    var kind: Int64 = 0
    var issue: Int64 = 0
    if discriminator_present == 0:
        issue = 1
    elif rich_view_matches_literal["user_message_chunk"](label, False):
        kind = 1
        if (field_mask & KIRO_ACP_UPDATE_FIELD_CONTENT) == 0:
            issue = 2
    elif rich_view_matches_literal["agent_message_chunk"](label, False):
        kind = 2
        if (field_mask & KIRO_ACP_UPDATE_FIELD_CONTENT) == 0:
            issue = 3
    elif rich_view_matches_literal["agent_thought_chunk"](label, False):
        kind = 3
        if (field_mask & KIRO_ACP_UPDATE_FIELD_CONTENT) == 0:
            issue = 4
    elif rich_view_matches_literal["tool_call"](label, False):
        kind = 4
        if (field_mask & KIRO_ACP_UPDATE_FIELD_TOOL_CALL_ID) == 0:
            issue = 5
        elif (field_mask & KIRO_ACP_UPDATE_FIELD_TITLE) == 0:
            issue = 6
    elif rich_view_matches_literal["tool_call_update"](label, False):
        kind = 5
        if (field_mask & KIRO_ACP_UPDATE_FIELD_TOOL_CALL_ID) == 0:
            issue = 7
    elif rich_view_matches_literal["plan"](label, False):
        kind = 6
        if (field_mask & KIRO_ACP_UPDATE_FIELD_ENTRIES) == 0:
            issue = 8
    elif rich_view_matches_literal["usage_update"](label, False):
        kind = 7
        if (field_mask & KIRO_ACP_UPDATE_FIELD_USED) == 0:
            issue = 9
        elif (field_mask & KIRO_ACP_UPDATE_FIELD_SIZE) == 0:
            issue = 10
    elif rich_view_matches_literal["session_info_update"](label, False):
        kind = 8
    elif rich_view_matches_literal["available_commands_update"](label, False):
        kind = 9
        if (field_mask & KIRO_ACP_UPDATE_FIELD_COMMANDS) == 0:
            issue = 11
    elif rich_view_matches_literal["current_mode_update"](label, False):
        kind = 10
        if (field_mask & KIRO_ACP_UPDATE_FIELD_MODE) == 0:
            issue = 12

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = kind
    output[unsafe_offset=1] = issue
    return 0
