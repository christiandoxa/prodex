from std.memory import Pointer
from std.sys.info import align_of, size_of

from gemini_sse_state import gemini_response_part_plan
from rich_text import rich_view_matches_literal
from rich_types import (
    ProdexRichContextRecord,
    ProdexRichContextResult,
    ProdexRichCatalogReasoningResult,
    ProdexRichCatalogPlanChoice,
    ProdexRichCatalogPlanModel,
    ProdexRichCatalogPlanResult,
    ProdexRichFallbackRecord,
    ProdexRichFallbackResult,
    ProdexRichIssue,
    ProdexRichPlanAction,
    ProdexRichPlanItem,
    ProdexRichPlanResult,
    ProdexRichRouteInput,
    ProdexRichRouteRecord,
    ProdexRichRouteResult,
    ProdexRichSlice,
    ProdexRichStringView,
)
from gemini_response import gemini_response_kernel_v1
from gemini_config import gemini_config_kernel_v1
# DeepSeek shares the rich ABI while keeping its provider wire semantics isolated.
from deepseek import deepseek_kernel_v2, deepseek_request_policy_v1
from anthropic_request import anthropic_request_kernel_v1
from openai_compat import openai_compat_kernel_v1
# Kiro shares the rich ABI while keeping ACP transport and session behavior in Rust.
from kiro import (
    kiro_chat_request_rewrite_v1,
    kiro_chat_response_rewrite_v1,
    kiro_kernel_v1,
    kiro_request_validation_json_v1,
    kiro_request_validation_v1,
)
from smart_context_normalization import (
    prodex_mojo_smart_context_normalization_v1,
    prodex_mojo_smart_context_budget_tier_v1,
    prodex_mojo_smart_context_memory_capsule_budget_v1,
    prodex_mojo_smart_context_capsule_plan_v1,
)

comptime PRODEX_RICH_ABI_VERSION: Int64 = 6


@export("prodex_mojo_rich_abi_version")
def prodex_mojo_rich_abi_version() abi("C") -> Int64:
    return PRODEX_RICH_ABI_VERSION


@export("prodex_mojo_rich_abi_layout")
def prodex_mojo_rich_abi_layout(
    output: Pointer[mut=True, UInt64, _], output_count: Int64
) abi("C") -> Int64:
    if output_count != 30:
        return 1
    output[unsafe_offset=0] = UInt64(size_of[ProdexRichStringView]())
    output[unsafe_offset=1] = UInt64(align_of[ProdexRichStringView]())
    output[unsafe_offset=2] = UInt64(size_of[ProdexRichSlice]())
    output[unsafe_offset=3] = UInt64(align_of[ProdexRichSlice]())
    output[unsafe_offset=4] = UInt64(size_of[ProdexRichIssue]())
    output[unsafe_offset=5] = UInt64(align_of[ProdexRichIssue]())
    output[unsafe_offset=6] = UInt64(size_of[ProdexRichContextRecord]())
    output[unsafe_offset=7] = UInt64(align_of[ProdexRichContextRecord]())
    output[unsafe_offset=8] = UInt64(size_of[ProdexRichContextResult]())
    output[unsafe_offset=9] = UInt64(align_of[ProdexRichContextResult]())
    output[unsafe_offset=10] = UInt64(size_of[ProdexRichRouteInput]())
    output[unsafe_offset=11] = UInt64(align_of[ProdexRichRouteInput]())
    output[unsafe_offset=12] = UInt64(size_of[ProdexRichRouteRecord]())
    output[unsafe_offset=13] = UInt64(align_of[ProdexRichRouteRecord]())
    output[unsafe_offset=14] = UInt64(size_of[ProdexRichRouteResult]())
    output[unsafe_offset=15] = UInt64(align_of[ProdexRichRouteResult]())
    output[unsafe_offset=16] = UInt64(size_of[ProdexRichPlanItem]())
    output[unsafe_offset=17] = UInt64(align_of[ProdexRichPlanItem]())
    output[unsafe_offset=18] = UInt64(size_of[ProdexRichPlanAction]())
    output[unsafe_offset=19] = UInt64(align_of[ProdexRichPlanAction]())
    output[unsafe_offset=20] = UInt64(size_of[ProdexRichPlanResult]())
    output[unsafe_offset=21] = UInt64(align_of[ProdexRichPlanResult]())
    output[unsafe_offset=22] = UInt64(size_of[ProdexRichCatalogReasoningResult]())
    output[unsafe_offset=23] = UInt64(align_of[ProdexRichCatalogReasoningResult]())
    output[unsafe_offset=24] = UInt64(size_of[ProdexRichCatalogPlanModel]())
    output[unsafe_offset=25] = UInt64(align_of[ProdexRichCatalogPlanModel]())
    output[unsafe_offset=26] = UInt64(size_of[ProdexRichCatalogPlanChoice]())
    output[unsafe_offset=27] = UInt64(align_of[ProdexRichCatalogPlanChoice]())
    output[unsafe_offset=28] = UInt64(size_of[ProdexRichCatalogPlanResult]())
    output[unsafe_offset=29] = UInt64(align_of[ProdexRichCatalogPlanResult]())
    return 0


comptime ANTHROPIC_RESPONSE_PLAN_MAX_BLOCKS: Int64 = 65_536
comptime ANTHROPIC_RESPONSE_PLAN_STATUS_OK: Int64 = 0
comptime ANTHROPIC_RESPONSE_PLAN_STATUS_INVALID: Int64 = 1
comptime ANTHROPIC_RESPONSE_PLAN_STATUS_CAPACITY: Int64 = 3
comptime ANTHROPIC_RESPONSE_PLAN_STATUS_ABI: Int64 = 4
comptime ANTHROPIC_RESPONSE_PLAN_ABI_VERSION: Int64 = 7
comptime ANTHROPIC_RESPONSE_PLAN_STATUS_MISSING_TYPE: Int64 = 5
comptime ANTHROPIC_RESPONSE_PLAN_STATUS_UNSUPPORTED_TYPE: Int64 = 6
comptime ANTHROPIC_RESPONSE_PLAN_STATUS_MISSING_TEXT: Int64 = 7
comptime ANTHROPIC_RESPONSE_PLAN_STATUS_MISSING_TOOL_USE_ID: Int64 = 8
comptime ANTHROPIC_RESPONSE_PLAN_STATUS_MISSING_TOOL_USE_NAME: Int64 = 9
comptime ANTHROPIC_RESPONSE_PLAN_STATUS_MISSING_SERVER_TOOL_ID: Int64 = 10
comptime ANTHROPIC_RESPONSE_PLAN_STATUS_UNSUPPORTED_SERVER_TOOL: Int64 = 11


def anthropic_response_plan_append(
    kinds: Pointer[mut=True, Int64, _],
    starts: Pointer[mut=True, Int64, _],
    counts: Pointer[mut=True, Int64, _],
    indices: Pointer[mut=True, Int64, _],
    output_count: Pointer[mut=True, Int64, _],
    output_capacity: Int64,
    kind: Int64,
    start: Int64,
    count: Int64,
    input_index: Int64,
) -> Bool:
    if output_count[] >= output_capacity:
        return False
    var index = output_count[]
    kinds[unsafe_offset=index] = kind
    starts[unsafe_offset=index] = start
    counts[unsafe_offset=index] = count
    indices[unsafe_offset=index] = input_index
    output_count[] = index + 1
    return True


@export("prodex_mojo_rich_anthropic_response_plan_v2")
def prodex_mojo_rich_anthropic_response_plan_v2(
    abi_version: Int64,
    input_types_address: UInt,
    input_flags_address: UInt,
    output_block_kinds_address: UInt,
    output_block_has_text_address: UInt,
    output_kinds_address: UInt,
    output_starts_address: UInt,
    output_counts_address: UInt,
    output_indices_address: UInt,
    output_capacity: Int64,
    output_count_address: UInt,
    input_count: Int64,
    issue_index_address: UInt,
) abi("C") -> Int64:
    if abi_version != ANTHROPIC_RESPONSE_PLAN_ABI_VERSION:
        return ANTHROPIC_RESPONSE_PLAN_STATUS_ABI
    if output_count_address == 0 or issue_index_address == 0:
        return ANTHROPIC_RESPONSE_PLAN_STATUS_INVALID
    if input_count < 0 or input_count > ANTHROPIC_RESPONSE_PLAN_MAX_BLOCKS:
        return ANTHROPIC_RESPONSE_PLAN_STATUS_INVALID
    if output_capacity < input_count or output_capacity > ANTHROPIC_RESPONSE_PLAN_MAX_BLOCKS:
        return ANTHROPIC_RESPONSE_PLAN_STATUS_CAPACITY

    var output_count = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_count_address)
    )
    var issue_index = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(issue_index_address)
    )
    output_count[] = 0
    issue_index[] = -1
    if input_count == 0:
        return ANTHROPIC_RESPONSE_PLAN_STATUS_OK
    if (
        input_types_address == 0
        or input_flags_address == 0
        or output_block_kinds_address == 0
        or output_block_has_text_address == 0
        or output_kinds_address == 0
        or output_starts_address == 0
        or output_counts_address == 0
        or output_indices_address == 0
    ):
        return ANTHROPIC_RESPONSE_PLAN_STATUS_INVALID

    var input_types = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_types_address)
    )
    var input_flags = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_flags_address)
    )
    var output_block_kinds = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_block_kinds_address)
    )
    var output_block_has_text = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_block_has_text_address)
    )
    var output_kinds = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_kinds_address)
    )
    var output_starts = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_starts_address)
    )
    var output_counts = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_counts_address)
    )
    var output_indices = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_indices_address)
    )

    for index in range(input_count):
        var type_view = input_types[unsafe_offset=index].copy()
        var flags = input_flags[unsafe_offset=index]
        if flags < 0 or flags > 63:
            return ANTHROPIC_RESPONSE_PLAN_STATUS_INVALID
        if flags % 2 == 0:
            issue_index[] = index
            return ANTHROPIC_RESPONSE_PLAN_STATUS_MISSING_TYPE
        var kind: Int64
        var has_text: Int64 = 0
        if rich_view_matches_literal["text"](type_view, False):
            kind = 0
            has_text = flags / 2 % 2
            if has_text == 0:
                issue_index[] = index
                return ANTHROPIC_RESPONSE_PLAN_STATUS_MISSING_TEXT
        elif rich_view_matches_literal["tool_use"](type_view, False):
            if flags / 8 % 2 == 0:
                issue_index[] = index
                return ANTHROPIC_RESPONSE_PLAN_STATUS_MISSING_TOOL_USE_ID
            if flags / 16 % 2 == 0:
                issue_index[] = index
                return ANTHROPIC_RESPONSE_PLAN_STATUS_MISSING_TOOL_USE_NAME
            kind = 1
        elif rich_view_matches_literal["server_tool_use"](type_view, False):
            if flags / 8 % 2 == 0:
                issue_index[] = index
                return ANTHROPIC_RESPONSE_PLAN_STATUS_MISSING_SERVER_TOOL_ID
            if flags / 32 % 2 == 0:
                issue_index[] = index
                return ANTHROPIC_RESPONSE_PLAN_STATUS_UNSUPPORTED_SERVER_TOOL
            kind = 2
        elif rich_view_matches_literal["web_search_tool_result"](type_view, False):
            kind = 3
        elif rich_view_matches_literal["thinking"](type_view, False):
            kind = 4
            has_text = flags / 4 % 2
        else:
            issue_index[] = index
            return ANTHROPIC_RESPONSE_PLAN_STATUS_UNSUPPORTED_TYPE
        output_block_kinds[unsafe_offset=index] = kind
        output_block_has_text[unsafe_offset=index] = has_text

    var open_start: Int64 = -1
    var open_count: Int64 = 0
    for index in range(input_count):
        var kind = output_block_kinds[unsafe_offset=index]
        var has_text = output_block_has_text[unsafe_offset=index]
        if kind == 0 and has_text == 1:
            if open_start < 0:
                open_start = index
            open_count += 1
            continue
        if open_start >= 0:
            if not anthropic_response_plan_append(
                output_kinds,
                output_starts,
                output_counts,
                output_indices,
                output_count,
                output_capacity,
                0,
                open_start,
                open_count,
                0,
            ):
                return ANTHROPIC_RESPONSE_PLAN_STATUS_CAPACITY
            open_start = -1
            open_count = 0
        if kind == 1:
            if not anthropic_response_plan_append(
                output_kinds,
                output_starts,
                output_counts,
                output_indices,
                output_count,
                output_capacity,
                1,
                0,
                0,
                index,
            ):
                return ANTHROPIC_RESPONSE_PLAN_STATUS_CAPACITY
        elif kind == 2:
            if not anthropic_response_plan_append(
                output_kinds,
                output_starts,
                output_counts,
                output_indices,
                output_count,
                output_capacity,
                2,
                0,
                0,
                index,
            ):
                return ANTHROPIC_RESPONSE_PLAN_STATUS_CAPACITY
        elif kind == 3:
            if not anthropic_response_plan_append(
                output_kinds,
                output_starts,
                output_counts,
                output_indices,
                output_count,
                output_capacity,
                3,
                0,
                0,
                index,
            ):
                return ANTHROPIC_RESPONSE_PLAN_STATUS_CAPACITY
        elif kind == 4 and has_text == 1:
            if not anthropic_response_plan_append(
                output_kinds,
                output_starts,
                output_counts,
                output_indices,
                output_count,
                output_capacity,
                4,
                0,
                0,
                index,
            ):
                return ANTHROPIC_RESPONSE_PLAN_STATUS_CAPACITY
    if open_start >= 0 and not anthropic_response_plan_append(
        output_kinds,
        output_starts,
        output_counts,
        output_indices,
        output_count,
        output_capacity,
        0,
        open_start,
        open_count,
        0,
    ):
        return ANTHROPIC_RESPONSE_PLAN_STATUS_CAPACITY
    return ANTHROPIC_RESPONSE_PLAN_STATUS_OK

@export("prodex_mojo_rich_gemini_response_part_plan_v1")
def prodex_mojo_rich_gemini_response_part_plan_v1(
    abi_version: Int64,
    has_text: Int64,
    is_thought: Int64,
    has_visible_text: Int64,
    has_special_text: Int64,
    has_media: Int64,
    has_video_metadata: Int64,
    has_image_generation: Int64,
    has_function_call: Int64,
    command_output_only: Int64,
    forced_output: Int64,
    internal_instruction_echo: Int64,
    suppress_visible_text: Int64,
    output_actions: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    if abi_version != 1:
        return 4
    return gemini_response_part_plan(
        has_text,
        is_thought,
        has_visible_text,
        has_special_text,
        has_media,
        has_video_metadata,
        has_image_generation,
        has_function_call,
        command_output_only,
        forced_output,
        internal_instruction_echo,
        suppress_visible_text,
        output_actions,
    )

@export("prodex_mojo_gemini_response_kernel_v1")
def prodex_mojo_gemini_response_kernel_v1(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    return gemini_response_kernel_v1(
        abi_version, input_address, output_address, output_capacity, written_address
    )


@export("prodex_mojo_gemini_config_kernel_v1")
def prodex_mojo_gemini_config_kernel_v1(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    return gemini_config_kernel_v1(
        abi_version, input_address, output_address, output_capacity, written_address
    )


@export("prodex_mojo_deepseek_kernel_v2")
def prodex_mojo_deepseek_kernel_v2(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    return deepseek_kernel_v2(
        abi_version, input_address, output_address, output_capacity, written_address
    )


@export("prodex_mojo_deepseek_request_policy_v1")
def prodex_mojo_deepseek_request_policy_export_v1(
    abi_version: Int64,
    operation: Int64,
    input_address: UInt,
    input_length: Int64,
    flag: Int64,
    scalar: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    return deepseek_request_policy_v1(
        abi_version, operation, input_address, input_length, flag, scalar, output_address
    )


@export("prodex_mojo_rich_anthropic_request_kernel_v1")
def prodex_mojo_rich_anthropic_request_kernel_v1(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    return anthropic_request_kernel_v1(
        abi_version, input_address, output_address, output_capacity, written_address
    )


@export("prodex_mojo_openai_compat_kernel_v1")
def prodex_mojo_openai_compat_kernel_v1(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    return openai_compat_kernel_v1(
        abi_version, input_address, output_address, output_capacity, written_address
    )


@export("prodex_mojo_kiro_kernel_v1")
def prodex_mojo_kiro_kernel_v1(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    return kiro_kernel_v1(
        abi_version, input_address, output_address, output_capacity, written_address
    )


@export("prodex_mojo_kiro_request_validation_v1")
def prodex_mojo_kiro_request_validation_v1(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
) abi("C") -> Int64:
    return kiro_request_validation_v1(abi_version, input_address, output_address)


@export("prodex_mojo_kiro_request_validation_json_v1")
def prodex_mojo_kiro_request_validation_json_v1(
    abi_version: Int64,
    mode: Int64,
    input_address: UInt,
    input_length: Int64,
    allow_token_limit: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    return kiro_request_validation_json_v1(
        abi_version,
        mode,
        input_address,
        input_length,
        allow_token_limit,
        output_address,
    )


@export("prodex_mojo_kiro_chat_request_rewrite_v1")
def prodex_mojo_kiro_chat_request_rewrite_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
    issue_address: UInt,
) abi("C") -> Int64:
    return kiro_chat_request_rewrite_v1(
        abi_version,
        input_address,
        input_length,
        output_address,
        output_capacity,
        written_address,
        issue_address,
    )


@export("prodex_mojo_kiro_chat_response_rewrite_v1")
def prodex_mojo_kiro_chat_response_rewrite_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    request_id: UInt64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    return kiro_chat_response_rewrite_v1(
        abi_version,
        input_address,
        input_length,
        request_id,
        output_address,
        output_capacity,
        written_address,
    )
