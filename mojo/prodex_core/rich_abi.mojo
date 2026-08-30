from std.memory import Pointer
from std.sys.info import align_of, size_of

from gemini_sse_state import gemini_response_part_plan
from rich_types import (
    ProdexRichContextRecord,
    ProdexRichContextResult,
    ProdexRichCatalogReasoningResult,
    ProdexRichCatalogPlanChoice,
    ProdexRichCatalogPlanModel,
    ProdexRichCatalogPlanResult,
    ProdexGatewayBillingSummaryBucket,
    ProdexGatewayBillingSummaryInput,
    ProdexGatewayBillingSummaryResult,
    ProdexRichFallbackRecord,
    ProdexRichFallbackResult,
    ProdexRichIssue,
    ProdexRichPlanAction,
    ProdexRichPlanItem,
    ProdexRichPlanResult,
    ProdexRichPolicyInput,
    ProdexRichPolicyModel,
    ProdexRichPolicyResult,
    ProdexRichPolicyRouteInput,
    ProdexRichPolicyRouteResult,
    ProdexRichRouteInput,
    ProdexRichRouteRecord,
    ProdexRichRouteResult,
    ProdexRichSlice,
    ProdexRichStringView,
)
from gemini_response import gemini_response_kernel_v1


comptime PRODEX_RICH_ABI_VERSION: Int64 = 6


@export("prodex_mojo_rich_abi_version")
def prodex_mojo_rich_abi_version() abi("C") -> Int64:
    return PRODEX_RICH_ABI_VERSION


@export("prodex_mojo_rich_abi_layout")
def prodex_mojo_rich_abi_layout(
    output: Pointer[mut=True, UInt64, _], output_count: Int64
) abi("C") -> Int64:
    if output_count != 46:
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
    output[unsafe_offset=16] = UInt64(size_of[ProdexRichPolicyInput]())
    output[unsafe_offset=17] = UInt64(align_of[ProdexRichPolicyInput]())
    output[unsafe_offset=18] = UInt64(size_of[ProdexRichPolicyModel]())
    output[unsafe_offset=19] = UInt64(align_of[ProdexRichPolicyModel]())
    output[unsafe_offset=20] = UInt64(size_of[ProdexRichPolicyResult]())
    output[unsafe_offset=21] = UInt64(align_of[ProdexRichPolicyResult]())
    output[unsafe_offset=22] = UInt64(size_of[ProdexRichPlanItem]())
    output[unsafe_offset=23] = UInt64(align_of[ProdexRichPlanItem]())
    output[unsafe_offset=24] = UInt64(size_of[ProdexRichPlanAction]())
    output[unsafe_offset=25] = UInt64(align_of[ProdexRichPlanAction]())
    output[unsafe_offset=26] = UInt64(size_of[ProdexRichPlanResult]())
    output[unsafe_offset=27] = UInt64(align_of[ProdexRichPlanResult]())
    output[unsafe_offset=28] = UInt64(size_of[ProdexRichCatalogReasoningResult]())
    output[unsafe_offset=29] = UInt64(align_of[ProdexRichCatalogReasoningResult]())
    output[unsafe_offset=30] = UInt64(size_of[ProdexRichPolicyRouteInput]())
    output[unsafe_offset=31] = UInt64(align_of[ProdexRichPolicyRouteInput]())
    output[unsafe_offset=32] = UInt64(size_of[ProdexRichPolicyRouteResult]())
    output[unsafe_offset=33] = UInt64(align_of[ProdexRichPolicyRouteResult]())
    output[unsafe_offset=34] = UInt64(size_of[ProdexRichCatalogPlanModel]())
    output[unsafe_offset=35] = UInt64(align_of[ProdexRichCatalogPlanModel]())
    output[unsafe_offset=36] = UInt64(size_of[ProdexRichCatalogPlanChoice]())
    output[unsafe_offset=37] = UInt64(align_of[ProdexRichCatalogPlanChoice]())
    output[unsafe_offset=38] = UInt64(size_of[ProdexRichCatalogPlanResult]())
    output[unsafe_offset=39] = UInt64(align_of[ProdexRichCatalogPlanResult]())
    output[unsafe_offset=40] = UInt64(size_of[ProdexGatewayBillingSummaryInput]())
    output[unsafe_offset=41] = UInt64(align_of[ProdexGatewayBillingSummaryInput]())
    output[unsafe_offset=42] = UInt64(size_of[ProdexGatewayBillingSummaryBucket]())
    output[unsafe_offset=43] = UInt64(align_of[ProdexGatewayBillingSummaryBucket]())
    output[unsafe_offset=44] = UInt64(size_of[ProdexGatewayBillingSummaryResult]())
    output[unsafe_offset=45] = UInt64(align_of[ProdexGatewayBillingSummaryResult]())
    return 0


comptime ANTHROPIC_RESPONSE_PLAN_MAX_BLOCKS: Int64 = 65_536
comptime ANTHROPIC_RESPONSE_PLAN_STATUS_OK: Int64 = 0
comptime ANTHROPIC_RESPONSE_PLAN_STATUS_INVALID: Int64 = 1
comptime ANTHROPIC_RESPONSE_PLAN_STATUS_CAPACITY: Int64 = 3
comptime ANTHROPIC_RESPONSE_PLAN_STATUS_ABI: Int64 = 4
comptime ANTHROPIC_RESPONSE_PLAN_ABI_VERSION: Int64 = 6


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


@export("prodex_mojo_rich_anthropic_response_plan_v1")
def prodex_mojo_rich_anthropic_response_plan_v1(
    abi_version: Int64,
    input_kinds_address: UInt,
    input_has_text_address: UInt,
    output_kinds_address: UInt,
    output_starts_address: UInt,
    output_counts_address: UInt,
    output_indices_address: UInt,
    output_capacity: Int64,
    output_count_address: UInt,
    input_count: Int64,
) abi("C") -> Int64:
    if abi_version != ANTHROPIC_RESPONSE_PLAN_ABI_VERSION:
        return ANTHROPIC_RESPONSE_PLAN_STATUS_ABI
    if output_count_address == 0:
        return ANTHROPIC_RESPONSE_PLAN_STATUS_INVALID
    if input_count < 0 or input_count > ANTHROPIC_RESPONSE_PLAN_MAX_BLOCKS:
        return ANTHROPIC_RESPONSE_PLAN_STATUS_INVALID
    if output_capacity < input_count or output_capacity > ANTHROPIC_RESPONSE_PLAN_MAX_BLOCKS:
        return ANTHROPIC_RESPONSE_PLAN_STATUS_CAPACITY

    var output_count = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_count_address)
    )
    output_count[] = 0
    if input_count == 0:
        return ANTHROPIC_RESPONSE_PLAN_STATUS_OK
    if (
        input_kinds_address == 0
        or input_has_text_address == 0
        or output_kinds_address == 0
        or output_starts_address == 0
        or output_counts_address == 0
        or output_indices_address == 0
    ):
        return ANTHROPIC_RESPONSE_PLAN_STATUS_INVALID

    var input_kinds = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_kinds_address)
    )
    var input_has_text = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_has_text_address)
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
        var kind = input_kinds[unsafe_offset=index]
        var has_text = input_has_text[unsafe_offset=index]
        if kind < 0 or kind > 4 or has_text < 0 or has_text > 1:
            return ANTHROPIC_RESPONSE_PLAN_STATUS_INVALID
        if kind == 0 and has_text != 1:
            return ANTHROPIC_RESPONSE_PLAN_STATUS_INVALID
        if kind != 0 and kind != 4 and has_text != 0:
            return ANTHROPIC_RESPONSE_PLAN_STATUS_INVALID

    var open_start: Int64 = -1
    var open_count: Int64 = 0
    for index in range(input_count):
        var kind = input_kinds[unsafe_offset=index]
        var has_text = input_has_text[unsafe_offset=index]
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
