# Presidio field-boundary selection, Unicode offsets, confidence and entity tags.
from std.memory import Pointer
from std.math import isfinite
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid, rich_view_ptr, rich_codepoint_width
from redaction import reaction_view_equals_literal
from redaction_analyzer import AnalyzerFinding


@fieldwise_init
struct FindingLocation(Copyable):
    var source: UInt64
    var value_index: UInt64
    var start_byte: UInt64
    var end_byte: UInt64
    var confidence: UInt64
    var kind: UInt64


def finding_scalar_count(text: ProdexRichStringView) -> UInt64:
    var ptr = rich_view_ptr(text)
    var cursor: Int64 = 0
    var count: UInt64 = 0
    while cursor < Int64(text.len):
        cursor += rich_codepoint_width(ptr[unsafe_offset=cursor])
        count += 1
    return count


def finding_byte_offset(text: ProdexRichStringView, offset: UInt64) -> UInt64:
    var ptr = rich_view_ptr(text)
    var cursor: Int64 = 0
    var count: UInt64 = 0
    while cursor < Int64(text.len) and count < offset:
        cursor += rich_codepoint_width(ptr[unsafe_offset=cursor])
        count += 1
    return UInt64(cursor)


def finding_entity_kind(entity: ProdexRichStringView) -> UInt64:
    for literal, kind in [
        (StringSlice("EMAIL_ADDRESS"), UInt64(0)),
        (StringSlice("PHONE_NUMBER"), UInt64(1)),
        (StringSlice("PERSON"), UInt64(2)),
        (StringSlice("LOCATION"), UInt64(3)),
        (StringSlice("CREDIT_CARD"), UInt64(6)),
        (StringSlice("IBAN_CODE"), UInt64(5)),
        (StringSlice("CRYPTO"), UInt64(5)),
        (StringSlice("API_KEY"), UInt64(8)),
        (StringSlice("ACCESS_TOKEN"), UInt64(7)),
        (StringSlice("PRIVATE_KEY"), UInt64(9)),
        (StringSlice("PASSWORD"), UInt64(10)),
    ]:
        if reaction_view_equals_literal(entity, literal):
            return kind
    for suffix in [StringSlice("_ID"), StringSlice("_SSN")]:
        var length = UInt(suffix.byte_length())
        if entity.len >= length and reaction_view_equals_literal(
            ProdexRichStringView(entity.ptr + entity.len - length, length), suffix
        ):
            return 4
    return 11


def finding_confidence(score: Float64) -> UInt64:
    var scaled = score * 10000.0
    var whole = UInt64(scaled)
    # Compare the fraction directly: adding 0.5 can round a near-half upward.
    return whole + UInt64(scaled - Float64(whole) >= 0.5)


@export("prodex_redaction_findings_plan_v1")
def prodex_redaction_findings_plan_v1(
    abi: Int64, mode: Int64,
    findings_address: UInt64, count: Int64,
    values_address: UInt64, value_count: Int64,
    separator_address: UInt64, separator_length: Int64,
    ranges_address: UInt64,
    output_address: UInt64, capacity: Int64, selected_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (
        mode < 0 or mode > 1 or count < 0 or count > 0x7FFFFFFFFFFFFFFF // 48
        or value_count < 0 or value_count > 0x7FFFFFFFFFFFFFFF // 16
        or separator_length < 0 or capacity < count or selected_address == 0
        or (count > 0 and (findings_address == 0 or output_address == 0))
        or (value_count > 0 and (values_address == 0 or ranges_address == 0))
    ):
        return 1
    var separator = ProdexRichStringView(UInt(separator_address), UInt(separator_length))
    if not rich_view_valid(separator, separator_length):
        return 1
    var findings = Pointer[mut=False, AnalyzerFinding, ImmUntrackedOrigin](unsafe_from_address=Int(findings_address))
    for index in range(count):
        var finding = findings[unsafe_offset=index].copy()
        if finding.start > finding.end or not rich_view_valid(finding.entity, 0x7FFFFFFFFFFFFFFF):
            return 1
    var values = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(values_address))
    for index in range(value_count):
        if not rich_view_valid(values[unsafe_offset=index], 0x7FFFFFFFFFFFFFFF):
            return 1
    var ranges = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(ranges_address))
    var separator_chars = finding_scalar_count(separator)
    var cursor: UInt64 = 0
    for index in range(value_count):
        var length = finding_scalar_count(values[unsafe_offset=index])
        if length > 0xFFFFFFFFFFFFFFFF - cursor or separator_chars > 0xFFFFFFFFFFFFFFFF - cursor - length:
            return 7
        ranges[unsafe_offset=index * 2] = cursor
        cursor += length
        ranges[unsafe_offset=index * 2 + 1] = cursor
        cursor += separator_chars
    var output = Pointer[mut=True, FindingLocation, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var selected: Int64 = 0
    for index in range(count):
        var finding = findings[unsafe_offset=index].copy()
        var value_index: Int64 = -1
        for candidate in range(value_count):
            if finding.start >= ranges[unsafe_offset=candidate * 2] and finding.end <= ranges[unsafe_offset=candidate * 2 + 1]:
                value_index = candidate
                break
        if value_index < 0:
            if mode == 0:
                return 5
            continue
        var confidence: UInt64 = 0
        var kind: UInt64 = 0
        var start_byte: UInt64 = 0
        var end_byte: UInt64 = 0
        if mode == 0:
            if not isfinite(finding.score) or finding.score < 0.0 or finding.score > 1.0:
                return 6
            confidence = finding_confidence(finding.score)
            kind = finding_entity_kind(finding.entity)
            var start = ranges[unsafe_offset=value_index * 2]
            start_byte = finding_byte_offset(values[unsafe_offset=value_index], finding.start - start)
            end_byte = finding_byte_offset(values[unsafe_offset=value_index], finding.end - start)
        output[unsafe_offset=selected] = FindingLocation(UInt64(index), UInt64(value_index), start_byte, end_byte, confidence, kind)
        selected += 1
    var selected_output = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(selected_address))
    selected_output[] = selected
    return 0
