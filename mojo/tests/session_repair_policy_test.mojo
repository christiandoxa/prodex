# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/session_repair_policy_test.mojo`.
from std.collections.list import List

from rich_types import ProdexRichStringView
from session_repair import SessionRepairLine, prodex_session_repair_plan_v1


def view(value: String) -> ProdexRichStringView:
    return ProdexRichStringView(UInt(Int(value.unsafe_ptr())), UInt(value.byte_length()))


def call_plan(
    selector: String,
    lines: List[SessionRepairLine],
    synthesize: Int64,
    output: List[Int64],
) -> Int64:
    return prodex_session_repair_plan_v1(
        1,
        UInt(Int(selector.unsafe_ptr())),
        Int64(selector.byte_length()),
        UInt(Int(lines.unsafe_ptr())),
        Int64(len(lines)),
        synthesize,
        UInt(Int(output.unsafe_ptr())),
        Int64(len(output)),
    )


def test_golden_rewrite_plan() raises:
    var selector = String("session")
    var event = String("event")
    var broken = String("{broken")
    var metadata = String("metadata")
    var duplicate = String("duplicate")
    var chat = String("chat")
    var lines = List[SessionRepairLine]()
    lines.append(SessionRepairLine(view(event), 0, 1, 0, 0, 0))
    lines.append(SessionRepairLine(view(broken), 0, 0, 0, 0, 0))
    lines.append(SessionRepairLine(view(metadata), 0, 1, 1, 1, 1))
    lines.append(SessionRepairLine(view(duplicate), 0, 1, 1, 1, 0))
    lines.append(SessionRepairLine(view(chat), 0, 1, 0, 0, 0))
    var output = List[Int64]()
    for _ in range(10):
        output.append(-1)
    assert call_plan(selector, lines, 0, output) == 0
    assert output[0] == 1
    assert output[1] == 2
    assert output[2] == 0
    assert output[3] == 0
    assert output[5] == 1
    assert output[6] == 0
    assert output[7] == 0
    assert output[8] == 0
    assert output[9] == 1


def test_unicode_clean_prefix_is_noop() raises:
    var selector = String("session")
    var unicode_line = String("\u3000metadata\u00a0")
    var lines = List[SessionRepairLine]()
    lines.append(SessionRepairLine(view(unicode_line), 0, 1, 1, 1, 1))
    var output = List[Int64]()
    for _ in range(6):
        output.append(-1)
    assert call_plan(selector, lines, 1, output) == 0
    assert output[0] == 0
    assert output[1] == -1
    assert output[2] == 0
    assert output[5] == 0


def test_invalid_observation_is_rejected_at_abi_boundary() raises:
    var selector = String("session")
    var line = String("chat")
    var lines = List[SessionRepairLine]()
    lines.append(SessionRepairLine(view(line), 2, 1, 0, 0, 0))
    var output = List[Int64]()
    for _ in range(6):
        output.append(-1)
    assert call_plan(selector, lines, 0, output) == 1


def main() raises:
    test_golden_rewrite_plan()
    test_unicode_clean_prefix_is_noop()
    test_invalid_observation_is_rejected_at_abi_boundary()
