from parsed_json import ParsedJson, JSON_NUMBER, pj_kind
from json_sink import js_raw_view
from rich_text import rich_view_ptr


def ocr_response_u64(tree: ParsedJson, index: Int64) -> Tuple[Bool, UInt64]:
    if pj_kind(tree, index) != JSON_NUMBER:
        return (False, UInt64(0))
    var raw = js_raw_view(tree, index)
    if raw.len == 0:
        return (False, UInt64(0))
    var ptr = rich_view_ptr(raw)
    var value: UInt64 = 0
    var maximum_prefix: UInt64 = 1844674407370955161
    for offset in range(Int64(raw.len)):
        var digit = ptr[unsafe_offset=offset]
        if digit < 48 or digit > 57:
            return (False, UInt64(0))
        var numeric_digit = UInt64(digit - 48)
        if value > maximum_prefix or (
            value == maximum_prefix and numeric_digit > UInt64(5)
        ):
            return (False, UInt64(0))
        value = value * UInt64(10) + numeric_digit
    return (True, value)
