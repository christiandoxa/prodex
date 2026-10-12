# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/session_selector_policy_test.mojo`.
from std.collections.list import List

from rich_types import ProdexRichStringView
from session_selector import prodex_session_selector_match_plan_v1


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


def main() raises:
    test_exact_prefix_and_mutation_sensitivity()
