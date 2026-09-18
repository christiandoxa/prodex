from std.memory import Pointer

from rich_text import rich_view_matches_literal, rich_view_valid
from rich_types import ProdexRichStringView

comptime PRODEX_RICH_ABI_VERSION: Int64 = 6
comptime RICH_MAX_IDENTIFIER_BYTES: Int64 = 4_096
comptime RICH_STATUS_OK: Int64 = 0
comptime RICH_STATUS_INVALID: Int64 = 1
comptime RICH_STATUS_UTF8: Int64 = 2


@export("prodex_runtime_websocket_event_kind_v1")
def prodex_runtime_websocket_event_kind_v1(
    abi_version: Int64,
    kind_address: UInt,
    output: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION or kind_address == 0:
        return RICH_STATUS_INVALID
    var kind_ptr = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(kind_address))
    var kind = kind_ptr[].copy()
    if not rich_view_valid(kind, RICH_MAX_IDENTIFIER_BYTES):
        return RICH_STATUS_UTF8

    var precommit_hold = (
        rich_view_matches_literal["codex.rate_limits"](kind, False)
        or rich_view_matches_literal["codex.response.metadata"](kind, False)
        or rich_view_matches_literal["response.metadata"](kind, False)
        or rich_view_matches_literal["response.created"](kind, False)
        or rich_view_matches_literal["response.in_progress"](kind, False)
        or rich_view_matches_literal["response.queued"](kind, False)
        or rich_view_matches_literal["response.output_item.added"](kind, False)
        or rich_view_matches_literal["response.content_part.added"](kind, False)
        or rich_view_matches_literal["response.reasoning_summary_part.added"](
            kind, False
        )
    )
    var realtime_terminal = (
        rich_view_matches_literal["session.started"](kind, False)
        or rich_view_matches_literal["session.updated"](kind, False)
        or rich_view_matches_literal["conversation.item.added"](kind, False)
        or rich_view_matches_literal["conversation.item.done"](kind, False)
        or rich_view_matches_literal["delegation.created"](kind, False)
        or rich_view_matches_literal["response.cancelled"](kind, False)
        or rich_view_matches_literal["response.done"](kind, False)
        or rich_view_matches_literal["turn.done"](kind, False)
        or rich_view_matches_literal["error"](kind, False)
    )
    var responses_terminal = (
        rich_view_matches_literal["response.completed"](kind, False)
        or rich_view_matches_literal["response.failed"](kind, False)
        or rich_view_matches_literal["response.incomplete"](kind, False)
    )
    output[unsafe_offset=0] = Int64(precommit_hold)
    output[unsafe_offset=1] = Int64(realtime_terminal)
    output[unsafe_offset=2] = Int64(responses_terminal)
    return RICH_STATUS_OK
