from std.memory import Pointer

from rich_text import rich_view_matches_literal, rich_view_valid
from rich_types import ProdexRichStringView

comptime PRODEX_RICH_ABI_VERSION: Int64 = 6
comptime RICH_MAX_IDENTIFIER_BYTES: Int64 = 4_096
comptime RICH_STATUS_OK: Int64 = 0
comptime RICH_STATUS_INVALID: Int64 = 1
comptime RICH_STATUS_UTF8: Int64 = 2
comptime RICH_PROVIDER_MODELS_RESPONSE_NONE: Int64 = 0
comptime RICH_PROVIDER_MODELS_RESPONSE_CATALOG_LIMIT: Int64 = 1
comptime RICH_PROVIDER_MODELS_RESPONSE_LIST: Int64 = 2
comptime RICH_PROVIDER_MODELS_RESPONSE_FOUND: Int64 = 3
comptime RICH_PROVIDER_MODELS_RESPONSE_MISSING: Int64 = 4
comptime RICH_PROVIDER_MODELS_RESPONSE_CATALOG_REQUIRED: Int64 = 5


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

    # Responses keepalive is transport liveness, not model output. Committing
    # on this ignored native event would expose a later capacity failure and
    # disable otherwise-safe precommit retry/rotation in both SSE and WebSocket.
    var precommit_hold = (
        rich_view_matches_literal["keepalive"](kind, False)
        or rich_view_matches_literal["codex.rate_limits"](kind, False)
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


@export("prodex_runtime_provider_models_response_plan_v1")
def prodex_runtime_provider_models_response_plan_v1(
    abi_version: Int64,
    method_address: UInt,
    route_kind: Int64,
    catalog_state: Int64,
    model_present: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION or method_address == 0 or output_address == 0:
        return RICH_STATUS_INVALID
    if route_kind < -1 or route_kind > 6 or catalog_state < 0 or catalog_state > 3:
        return RICH_STATUS_INVALID
    if model_present < 0 or model_present > 1:
        return RICH_STATUS_INVALID
    if route_kind != 6 and model_present != 0:
        return RICH_STATUS_INVALID

    var method_ptr = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(method_address))
    var method = method_ptr[].copy()
    if not rich_view_valid(method, RICH_MAX_IDENTIFIER_BYTES):
        return RICH_STATUS_UTF8

    var decision = RICH_PROVIDER_MODELS_RESPONSE_NONE
    if rich_view_matches_literal["get"](method, True) and (
        route_kind == 5 or route_kind == 6
    ):
        if catalog_state == 3:
            decision = RICH_PROVIDER_MODELS_RESPONSE_CATALOG_REQUIRED
        elif catalog_state == 2:
            decision = RICH_PROVIDER_MODELS_RESPONSE_CATALOG_LIMIT
        elif catalog_state == 1:
            if route_kind == 5:
                decision = RICH_PROVIDER_MODELS_RESPONSE_LIST
            elif model_present == 1:
                decision = RICH_PROVIDER_MODELS_RESPONSE_FOUND
            else:
                decision = RICH_PROVIDER_MODELS_RESPONSE_MISSING

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = decision
    return RICH_STATUS_OK
