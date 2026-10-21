# Borrowed Gemini counters and numeric plans share one bounded record ABI.
from std.memory import Pointer
from quota import QUOTA_GEMINI_BUCKET_BATCH_MAX_COUNT, GeminiBucketNumericPlan, quota_gemini_numeric_plan
from rich_types import ProdexRichStringView
from rich_text import rich_trim_bounds, rich_view_valid
from runtime_tuning import runtime_config_scalar_parse_signed


@fieldwise_init
struct GeminiBucketRecord(Copyable):
    var amount_address: UInt
    var amount_length: Int64
    var amount_source: Int64
    var amount: Int64
    var fraction: Float64
    var fraction_present: Int64


@export("prodex_quota_gemini_bucket_records_v1")
def prodex_quota_gemini_bucket_records_v1(
    version: Int64, rows_address: UInt, count: Int64,
    output_address: UInt, output_capacity: Int64,
) abi("C") -> Int64:
    if version != 1:
        return 4
    if count < 0 or count > QUOTA_GEMINI_BUCKET_BATCH_MAX_COUNT:
        return 1
    if count > 0 and (rows_address == 0 or rows_address % 8 != 0 or output_address == 0 or output_address % 8 != 0):
        return 1
    if output_capacity < count:
        return 3
    var rows = Pointer[mut=False, GeminiBucketRecord, ImmUntrackedOrigin](unsafe_from_address=Int(rows_address))
    for index in range(count):
        var row = rows[unsafe_offset=index].copy()
        if row.amount_source < 0 or row.amount_source > 3 or row.amount_length < 0 or row.fraction_present < 0 or row.fraction_present > 1:
            return 1
        if row.amount_source != 3:
            if row.amount_address != 0 or row.amount_length != 0:
                return 1
        elif not rich_view_valid(ProdexRichStringView(row.amount_address, UInt(row.amount_length)), 9223372036854775807):
            return 1
    var output = Pointer[mut=True, GeminiBucketNumericPlan, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var words = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    for index in range(count):
        var row = rows[unsafe_offset=index].copy()
        var state = row.amount_source
        var amount = row.amount
        if state == 3:
            state = 2
            var view = ProdexRichStringView(row.amount_address, UInt(row.amount_length))
            var bounds = rich_trim_bounds(view)
            var parsed = words.unsafe_offset(Int(index * 7))
            parsed[] = 0
            if runtime_config_scalar_parse_signed(view, bounds[0], bounds[1], parsed):
                state = 1
                amount = parsed[]
        output[unsafe_offset=index] = quota_gemini_numeric_plan(state, amount, row.fraction, row.fraction_present)
    return 0
