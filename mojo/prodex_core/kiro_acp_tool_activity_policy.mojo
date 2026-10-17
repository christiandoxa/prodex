from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid

# Kiro ACP tool-activity ledger limits and metadata inheritance are compiled
# policies. Rust retains the BTreeMap/Vec host storage and JSON presentation.
comptime KIRO_ACP_ACTIVITY_LEDGER_ABI_VERSION: Int64 = 1
comptime KIRO_ACP_MAX_ACTIVITY_EVENTS: UInt64 = 128
comptime KIRO_ACP_MAX_TOOL_ID_BYTES: Int64 = 256


@export("prodex_kiro_acp_tool_activity_ledger_v1")
def prodex_kiro_acp_tool_activity_ledger_v1(
    abi_version: Int64,
    id_address: UInt,
    id_length: Int64,
    already_truncated: Int64,
    activity_count: UInt64,
    label_count: UInt64,
    previous_present: Int64,
    title_present: Int64,
    kind_present: Int64,
    output_address: UInt,
    output_capacity: Int64,
) abi("C") -> Int64:
    if abi_version != KIRO_ACP_ACTIVITY_LEDGER_ABI_VERSION:
        return 4
    if (
        id_length < 0 or (id_length > 0 and id_address == 0)
        or (already_truncated != 0 and already_truncated != 1)
        or (previous_present != 0 and previous_present != 1)
        or (title_present != 0 and title_present != 1)
        or (kind_present != 0 and kind_present != 1)
        or output_address == 0 or output_capacity < 4
    ):
        return 1
    var id = ProdexRichStringView(id_address, UInt(id_length))
    if not rich_view_valid(id, 0x7FFFFFFFFFFFFFFF):
        return 1

    # Action 0 means no further activity, 1 a regular activity, and 2 the
    # single bounded truncation marker. No further markers after that.
    var action: Int64 = 0
    var record_label: Int64 = 0
    var use_previous_title: Int64 = 0
    var use_previous_kind: Int64 = 0
    if already_truncated == 0:
        if activity_count >= KIRO_ACP_MAX_ACTIVITY_EVENTS - 1:
            action = 2
        else:
            action = 1
            var previous_allowed = (
                previous_present == 1 and id_length <= KIRO_ACP_MAX_TOOL_ID_BYTES
            )
            record_label = Int64(
                id_length <= KIRO_ACP_MAX_TOOL_ID_BYTES
                and (
                    previous_allowed or label_count < KIRO_ACP_MAX_ACTIVITY_EVENTS
                )
            )
            use_previous_title = Int64(previous_allowed and title_present == 0)
            use_previous_kind = Int64(previous_allowed and kind_present == 0)

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = action
    output[unsafe_offset=1] = record_label
    output[unsafe_offset=2] = use_previous_title
    output[unsafe_offset=3] = use_previous_kind
    return 0
