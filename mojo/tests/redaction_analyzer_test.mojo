# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/redaction_analyzer_test.mojo`.
from std.collections import List
from rich_types import ProdexRichStringView
from redaction_analyzer import AnalyzerFinding, AnalyzerSelection, prodex_redaction_analyzer_merge_v1


def test_merge_retains_source_metadata_and_expands_stronger_partial_overlap() raises:
    var entity = StringSlice("EMAIL_ADDRESS")
    var view = ProdexRichStringView(UInt(Int(entity.unsafe_ptr())), UInt(entity.byte_length()))
    var findings = List[AnalyzerFinding]()
    findings.append(AnalyzerFinding(3, 8, 0.9, view.copy()))
    findings.append(AnalyzerFinding(0, 5, 0.7, view.copy()))
    findings.append(AnalyzerFinding(3, 8, 0.9, view.copy()))
    var order = List[Int64](length=3, fill=71)
    var output = List[AnalyzerSelection]()
    for _ in range(3):
        output.append(AnalyzerSelection(71, 71, 71, 71.0))
    var selected = List[Int64](length=1, fill=71)
    assert prodex_redaction_analyzer_merge_v1(
        1, UInt64(Int(findings.unsafe_ptr())), 3, UInt64(Int(order.unsafe_ptr())),
        UInt64(Int(output.unsafe_ptr())), 3, UInt64(Int(selected.unsafe_ptr())),
    ) == 0
    assert selected[0] == 2
    assert output[0].source == 0 and output[0].start == 0 and output[0].end == 8
    assert output[0].score == 0.9
    assert output[1].source == 2 and output[1].start == 3 and output[1].end == 8
    selected[0] = 71
    assert prodex_redaction_analyzer_merge_v1(
        2, 0, 0, 0, 0, 0, UInt64(Int(selected.unsafe_ptr())),
    ) == 4
    assert selected[0] == 71
    assert prodex_redaction_analyzer_merge_v1(
        1, UInt64(Int(findings.unsafe_ptr())), 3, UInt64(Int(order.unsafe_ptr())),
        UInt64(Int(output.unsafe_ptr())), 2, UInt64(Int(selected.unsafe_ptr())),
    ) == 1
    assert selected[0] == 71


def main() raises:
    test_merge_retains_source_metadata_and_expands_stronger_partial_overlap()
