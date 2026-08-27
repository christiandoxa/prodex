from std.memory import Pointer
from std.sys.info import align_of, size_of

from rich_types import (
    ProdexRichContextRecord,
    ProdexRichContextResult,
    ProdexRichFallbackRecord,
    ProdexRichFallbackResult,
    ProdexRichIssue,
    ProdexRichPlanAction,
    ProdexRichPlanItem,
    ProdexRichPlanResult,
    ProdexRichPolicyInput,
    ProdexRichPolicyModel,
    ProdexRichPolicyResult,
    ProdexRichRouteInput,
    ProdexRichRouteRecord,
    ProdexRichRouteResult,
    ProdexRichSlice,
    ProdexRichStringView,
)


comptime PRODEX_RICH_ABI_VERSION: Int64 = 3


@export("prodex_mojo_rich_abi_version")
def prodex_mojo_rich_abi_version() abi("C") -> Int64:
    return PRODEX_RICH_ABI_VERSION


@export("prodex_mojo_rich_abi_layout")
def prodex_mojo_rich_abi_layout(
    output: Pointer[mut=True, UInt64, _], output_count: Int64
) abi("C") -> Int64:
    if output_count != 28:
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
    return 0
