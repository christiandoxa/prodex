# Presidio's stable analyzer ordering and adjacent overlap reduction.
from std.memory import Pointer
from std.math import isfinite
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid, rich_view_compare, rich_view_order_swap


@fieldwise_init
struct AnalyzerFinding(Copyable):
    var start: UInt64
    var end: UInt64
    var score: Float64
    var entity: ProdexRichStringView


@fieldwise_init
struct AnalyzerSelection(Copyable):
    var source: UInt64
    var start: UInt64
    var end: UInt64
    var score: Float64


def analyzer_before(
    findings: Pointer[mut=False, AnalyzerFinding, ImmUntrackedOrigin],
    left_index: Int64, right_index: Int64,
) -> Bool:
    var left = findings[unsafe_offset=left_index].copy()
    var right = findings[unsafe_offset=right_index].copy()
    if left.start != right.start:
        return left.start < right.start
    if left.end != right.end:
        return left.end < right.end
    if left.score != right.score:
        return left.score > right.score
    var compared = rich_view_compare(left.entity, right.entity)
    return compared > 0 if compared != 0 else left_index < right_index


def analyzer_sift_down(
    findings: Pointer[mut=False, AnalyzerFinding, ImmUntrackedOrigin],
    order: Pointer[mut=True, Int64, MutUntrackedOrigin], root_index: Int64, count: Int64,
):
    var root = root_index
    while root < count // 2:
        var child = root * 2 + 1
        if child + 1 < count and analyzer_before(
            findings, order[unsafe_offset=child], order[unsafe_offset=child + 1]
        ):
            child += 1
        if not analyzer_before(findings, order[unsafe_offset=root], order[unsafe_offset=child]):
            break
        rich_view_order_swap(order, root, child)
        root = child


@export("prodex_redaction_analyzer_merge_v1")
def prodex_redaction_analyzer_merge_v1(
    abi: Int64, findings_address: UInt64, count: Int64,
    order_address: UInt64, output_address: UInt64, capacity: Int64,
    selected_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (
        count < 0 or count > 0x7FFFFFFFFFFFFFFF // 40 or capacity < count
        or selected_address == 0
        or (count > 0 and (findings_address == 0 or order_address == 0 or output_address == 0))
    ):
        return 1
    var findings = Pointer[mut=False, AnalyzerFinding, ImmUntrackedOrigin](unsafe_from_address=Int(findings_address))
    for index in range(count):
        var finding = findings[unsafe_offset=index].copy()
        if finding.start > finding.end or not isfinite(finding.score) or not rich_view_valid(finding.entity, 0x7FFFFFFFFFFFFFFF):
            return 1
    var order = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(order_address))
    for index in range(count):
        order[unsafe_offset=index] = index
    var start = count // 2
    while start > 0:
        start -= 1
        analyzer_sift_down(findings, order, start, count)
    var end = count
    while end > 1:
        end -= 1
        rich_view_order_swap(order, 0, end)
        analyzer_sift_down(findings, order, 0, end)

    var output = Pointer[mut=True, AnalyzerSelection, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var selected: Int64 = 0
    for position in range(count):
        var source = order[unsafe_offset=position]
        var candidate = findings[unsafe_offset=source].copy()
        var merged = False
        if selected > 0:
            var last = output[unsafe_offset=selected - 1].copy()
            var duplicate = (
                last.start == candidate.start and last.end == candidate.end
                and rich_view_compare(findings[unsafe_offset=Int64(last.source)].entity, candidate.entity) == 0
            )
            var overlaps = candidate.start < last.end and candidate.end > last.start
            var stronger = candidate.score > last.score
            var longer = candidate.score == last.score and candidate.end - candidate.start > last.end - last.start
            if duplicate or (overlaps and (stronger or longer)):
                var contained = (
                    (candidate.start >= last.start and candidate.end <= last.end)
                    or (last.start >= candidate.start and last.end <= candidate.end)
                )
                if stronger:
                    if duplicate or contained:
                        output[unsafe_offset=selected - 1] = AnalyzerSelection(UInt64(source), candidate.start, candidate.end, candidate.score)
                    else:
                        output[unsafe_offset=selected - 1] = AnalyzerSelection(UInt64(source), min(last.start, candidate.start), max(last.end, candidate.end), candidate.score)
                merged = duplicate or contained or stronger
        if not merged:
            output[unsafe_offset=selected] = AnalyzerSelection(UInt64(source), candidate.start, candidate.end, candidate.score)
            selected += 1
    var selected_output = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(selected_address))
    selected_output[] = selected
    return 0
