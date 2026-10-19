from std.memory import Pointer
from std.math import ceil, isfinite
from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView


# Units: seconds=0, milliseconds=1, minutes=2, hours=3, days=4.
def quota_duration_scan[emit: Bool](
    view: ProdexRichStringView,
    numbers: Pointer[mut=True, UInt8, MutUntrackedOrigin],
    rows: Pointer[mut=True, Int64, MutUntrackedOrigin],
) -> Tuple[Bool, Int64, Int64]:
    var bounds = rich_trim_bounds(view)
    var source = rich_view_ptr(view)
    var cursor = bounds[0]
    var count: Int64 = 0
    var written: Int64 = 0
    var number_start: Int64 = 0
    while cursor < bounds[1]:
        var byte = source[unsafe_offset=cursor]
        cursor += 1
        if byte == 46 or byte >= 48 and byte <= 57:
            if emit:
                numbers[unsafe_offset=written] = byte
            written += 1
            continue
        # Rust's ASCII whitespace excludes interior vertical tab.
        if byte == 9 or byte == 10 or byte == 12 or byte == 13 or byte == 32:
            continue
        if number_start == written:
            return (False, Int64(0), Int64(0))
        var unit: Int64 = 0
        if byte == 100:
            unit = 4
        elif byte == 104:
            unit = 3
        elif byte == 109:
            unit = 2
            if cursor < bounds[1] and source[unsafe_offset=cursor] == 115:
                unit = 1
                cursor += 1
        elif byte != 115:
            return (False, Int64(0), Int64(0))
        if emit:
            rows[unsafe_offset=count * 3] = number_start
            rows[unsafe_offset=count * 3 + 1] = written
            rows[unsafe_offset=count * 3 + 2] = unit
        count += 1
        number_start = written
    if number_start < written:
        if emit:
            rows[unsafe_offset=count * 3] = number_start
            rows[unsafe_offset=count * 3 + 1] = written
            rows[unsafe_offset=count * 3 + 2] = 0
        count += 1
    return (count > 0, count, written)


@export("prodex_provider_quota_duration_plan_v1")
def prodex_provider_quota_duration_plan_v1(
    version: Int64, address: UInt, length: Int64,
    numbers_address: UInt, numbers_capacity: Int64,
    rows_address: UInt, rows_capacity: Int64, result_address: UInt,
) abi("C") -> Int64:
    if version != 1:
        return 4
    if length < 0 or length > 9223372036854775807 // 24 or numbers_capacity < 0 or rows_capacity < 0 or result_address == 0 or result_address % 8 != 0:
        return 1
    var view = ProdexRichStringView(address, UInt(length))
    if not rich_view_valid(view, length):
        return 1
    var numbers = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(numbers_address))
    var rows = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(rows_address))
    var plan = quota_duration_scan[False](view, numbers, rows)
    var probe = numbers_address == 0 and rows_address == 0
    if not probe:
        if numbers_address == 0 or rows_address == 0 or rows_address % 8 != 0:
            return 1
        if numbers_capacity < plan[2] or rows_capacity < plan[1]:
            return 3
        if plan[0]:
            _ = quota_duration_scan[True](view, numbers, rows)
    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(result_address))
    result[unsafe_offset=0] = Int64(plan[0])
    result[unsafe_offset=1] = plan[1]
    result[unsafe_offset=2] = plan[2]
    return 0


# Rust acquires standard floats; Mojo alone applies duration policy.
@export("prodex_provider_quota_duration_finish_v1")
def prodex_provider_quota_duration_finish_v1(
    version: Int64, values_address: UInt, count: Int64,
    output_address: UInt, output_capacity: Int64,
) abi("C") -> Int64:
    if version != 1:
        return 4
    if values_address == 0 or values_address % 8 != 0 or count <= 0 or count > 9223372036854775807 // 24 or output_address == 0 or output_address % 8 != 0:
        return 1
    if output_capacity < 2:
        return 3
    var integers = Pointer[mut=False, Int64, ImmUntrackedOrigin](unsafe_from_address=Int(values_address))
    var floats = Pointer[mut=False, Float64, ImmUntrackedOrigin](unsafe_from_address=Int(values_address))
    var parsed = True
    for index in range(count):
        var unit = integers[unsafe_offset=index * 3]
        var present = integers[unsafe_offset=index * 3 + 1]
        var amount = floats[unsafe_offset=index * 3 + 2]
        if unit < 0 or unit > 4 or present < 0 or present > 1 or amount < 0.0 or amount != amount or (present == 0 and amount != 0.0):
            return 1
        parsed = parsed and present == 1
    var seconds: Int64 = 0
    if parsed:
        var total = Float64(0)
        for index in range(count):
            var unit = integers[unsafe_offset=index * 3]
            var amount = floats[unsafe_offset=index * 3 + 2]
            var contribution = amount
            if unit == 1:
                contribution = amount / 1000.0
            elif unit == 2:
                contribution = amount * 60.0
            elif unit == 3:
                contribution = amount * 3600.0
            elif unit == 4:
                contribution = amount * 86400.0
            total += contribution
        var rounded = ceil(total)
        seconds = 9223372036854775807
        if isfinite(rounded) and rounded < Float64(seconds):
            seconds = Int64(rounded)
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    output[unsafe_offset=0] = Int64(parsed)
    output[unsafe_offset=1] = seconds
    return 0
