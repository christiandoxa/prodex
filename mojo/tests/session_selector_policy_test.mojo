# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/session_selector_policy_test.mojo`.
from std.collections.list import List

from rich_types import ProdexRichStringView
from session_selector import (
    prodex_session_report_query_matches_v1,
    prodex_session_selector_match_plan_v1,
    prodex_session_selector_path_span_v1,
)


def view(value: String) -> ProdexRichStringView:
    return ProdexRichStringView(UInt(Int(value.unsafe_ptr())), UInt(value.byte_length()))


def call_plan(
    selector: String,
    ids: List[ProdexRichStringView],
    output: List[Int64],
) -> Int64:
    return prodex_session_selector_match_plan_v1(
        1,
        UInt(Int(ids.unsafe_ptr())),
        Int64(len(ids)),
        UInt(Int(selector.unsafe_ptr())),
        Int64(selector.byte_length()),
        UInt(Int(output.unsafe_ptr())),
        Int64(len(output)),
    )


def call_path_span(
    stem: String,
    selector: String,
    exact: Int64,
    require_full: Int64,
    output: List[Int64],
) -> Int64:
    return prodex_session_selector_path_span_v1(
        1,
        UInt(Int(stem.unsafe_ptr())),
        Int64(stem.byte_length()),
        UInt(Int(selector.unsafe_ptr())),
        Int64(selector.byte_length()),
        exact,
        require_full,
        UInt(Int(output.unsafe_ptr())),
    )


def test_exact_prefix_and_mutation_sensitivity() raises:
    var first = String("a1111111-aaaa-1111-1111-111111111111")
    var second = String("a1111111-aaaa-1111-1111-222222222222")
    var ids = List[ProdexRichStringView]()
    ids.append(view(first))
    ids.append(view(second))

    var exact_output = List[Int64]()
    for _ in range(6):
        exact_output.append(-1)
    var uppercase = String("A1111111-AAAA-1111-1111-111111111111")
    assert call_plan(uppercase, ids, exact_output) == 0
    assert exact_output[0] == 1
    assert exact_output[1] == 1
    assert exact_output[2] == 0
    assert exact_output[4] == 0

    var prefix_output = List[Int64]()
    for _ in range(6):
        prefix_output.append(-1)
    var prefix = String("a1111111")
    assert call_plan(prefix, ids, prefix_output) == 0
    assert prefix_output[0] == 0
    assert prefix_output[1] == 2
    assert prefix_output[4] == 0
    assert prefix_output[5] == 1

    var changed_output = List[Int64]()
    for _ in range(6):
        changed_output.append(-1)
    var changed = String("a1111112")
    assert call_plan(changed, ids, changed_output) == 0
    assert changed_output[0] == 0
    assert changed_output[1] == 0
    assert changed_output[1] != prefix_output[1]


def test_report_query_matches_fixed_fields() raises:
    var query = String("i\u0307stanbul")
    var id = String("01900000-0000-7000-8000-000000000301")
    var thread_name = String("straße i\u0307stanbul")
    var cwd = String("/home/test-user/workspace")
    var profile = String("main")
    var provider = String("openai")
    var path = String("/home/test-user/.codex/sessions/unicode.jsonl")
    var fields = List[ProdexRichStringView]()
    fields.append(view(id))
    fields.append(view(thread_name))
    fields.append(view(cwd))
    fields.append(view(profile))
    fields.append(view(provider))
    fields.append(view(path))

    var output = List[Int64]()
    output.append(-1)
    var status = prodex_session_report_query_matches_v1(
        1,
        UInt(Int(query.unsafe_ptr())),
        Int64(query.byte_length()),
        UInt(Int(fields.unsafe_ptr())),
        Int64(len(fields)),
        UInt(Int(output.unsafe_ptr())),
    )
    assert status == 0
    assert output[0] == 1

    var invalid_output = List[Int64]()
    invalid_output.append(-1)
    status = prodex_session_report_query_matches_v1(
        1,
        UInt(Int(query.unsafe_ptr())),
        Int64(query.byte_length()),
        UInt(Int(fields.unsafe_ptr())),
        5,
        UInt(Int(invalid_output.unsafe_ptr())),
    )
    assert status == 1
    assert invalid_output[0] == -1


def test_path_span_preserves_stem_and_window_contract() raises:
    var output = List[Int64]()
    output.append(-1)
    output.append(-1)
    var stem = String("é--prefix-a-b-c-d")
    var selector = String("-prefix-a-b")
    assert call_path_span(stem, selector, 0, 0, output) == 0
    assert output[0] == 3
    assert output[1] == 16

    output[0] = -1
    output[1] = -1
    stem = String("prefix-a-b-c-d-e")
    selector = String("prefix")
    assert call_path_span(stem, selector, 0, 0, output) == 0
    assert output[0] == 0
    assert output[1] == Int64(stem.byte_length())

    output[0] = -1
    output[1] = -1
    var id = String("01900000-0000-7000-8000-000000000301")
    selector = String("")
    assert call_path_span(id, selector, 1, 1, output) == 0
    assert output[0] == 0
    assert output[1] == 36
    stem = String("prefix-01900000-0000-7000-8000-000000000301-suffix")
    assert call_path_span(stem, selector, 1, 1, output) == 0
    assert output[0] == 7
    assert output[1] == 43


def main() raises:
    test_exact_prefix_and_mutation_sensitivity()
    test_report_query_matches_fixed_fields()
    test_path_span_preserves_stem_and_window_contract()
