# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/info_token_usage_test.mojo`.
from std.memory.alloc import alloc, dealloc, Layout
from rich_types import ProdexRichStringView
from parsed_json import pj_literal
from log_number import log_parse_u64_text, log_positive_finite_decimal
from log_throughput_policy import prodex_info_token_usage_numbers_v1


def main() raises:
    assert log_parse_u64_text(UInt64(pj_literal(StringSlice("+1")).ptr), 2) == (True, UInt64(1))
    assert not log_positive_finite_decimal(pj_literal(StringSlice(" 1")))
    assert log_positive_finite_decimal(pj_literal(StringSlice("5e-324")))
    assert not log_positive_finite_decimal(pj_literal(StringSlice("1e999")))
    var text_storage = alloc(Layout[ProdexRichStringView](count=7))
    var output_storage = alloc(Layout[UInt64](count=9))
    var texts = text_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    for index in range(7):
        texts[unsafe_offset=index] = pj_literal(StringSlice(""))
    texts[unsafe_offset=0] = pj_literal(StringSlice("1"))
    texts[unsafe_offset=2] = pj_literal(StringSlice("15"))
    texts[unsafe_offset=5] = pj_literal(StringSlice("1000"))
    assert prodex_info_token_usage_numbers_v1(1, 0, UInt(Int(texts)), 7, 37, UInt(Int(output)), 9) == 0
    assert output[unsafe_offset=0] == 1
    assert output[unsafe_offset=3] == 15
    assert output[unsafe_offset=6] == 1000
    assert output[unsafe_offset=7] == 6
    assert prodex_info_token_usage_numbers_v1(2, 0, UInt(Int(texts)), 7, 37, UInt(Int(output)), 9) == 4
    dealloc(text_storage^)
    dealloc(output_storage^)
