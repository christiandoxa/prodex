# Ordered managed-plugin manifest checks; host file reads stay between phases.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid, rich_view_ptr, rich_ascii_hex
from parsed_json import pj_equal, pj_literal
from json_sink import JsonSink, js_literal, js_view
from optional_tools_policy import OPTIONAL_TOOL_CAVEMAN, OPTIONAL_TOOL_PONYTAIL


def optional_tool_manifest_hex(view: ProdexRichStringView, width: Int64) -> Bool:
    if Int64(view.len) != width:
        return False
    for index in range(width):
        if not rich_ascii_hex(rich_view_ptr(view)[unsafe_offset=index]):
            return False
    return True


def optional_tool_manifest_release_matches(
    fields: Pointer[mut=False, ProdexRichStringView, _], base: Int64,
    allow_legacy_digest: Bool,
) -> Bool:
    return (
        pj_equal(fields[unsafe_offset=0], fields[unsafe_offset=base])
        and pj_equal(fields[unsafe_offset=1], fields[unsafe_offset=base + 1])
        and pj_equal(fields[unsafe_offset=3], fields[unsafe_offset=base + 2])
        and (
            pj_equal(fields[unsafe_offset=2], fields[unsafe_offset=base + 2])
            or allow_legacy_digest and pj_equal(fields[unsafe_offset=2], fields[unsafe_offset=13])
        )
    )


def optional_tool_manifest_content_issue(
    tool: Int64, fields: Pointer[mut=False, ProdexRichStringView, _],
) -> Int64:
    # Manifest version/commit/digest, actual digest, latest/previous/legacy triples, alias.
    if pj_equal(fields[unsafe_offset=0], fields[unsafe_offset=4]):
        if not pj_equal(fields[unsafe_offset=1], fields[unsafe_offset=5]):
            return 9
        if not pj_equal(fields[unsafe_offset=2], fields[unsafe_offset=6]):
            return 10
        return Int64(0) if pj_equal(fields[unsafe_offset=3], fields[unsafe_offset=6]) else Int64(11)
    if pj_equal(fields[unsafe_offset=0], fields[unsafe_offset=7]):
        return Int64(0) if optional_tool_manifest_release_matches(fields, 7, tool == OPTIONAL_TOOL_PONYTAIL) else Int64(12)
    if tool == OPTIONAL_TOOL_CAVEMAN and optional_tool_manifest_release_matches(fields, 10, True):
        return 0
    return Int64(0) if pj_equal(fields[unsafe_offset=2], fields[unsafe_offset=3]) else Int64(13)


def optional_tool_manifest_issue(
    tool: Int64, phase: Int64, tag: Int64,
    fields: Pointer[mut=False, ProdexRichStringView, _],
) -> Int64:
    if phase == 0:
        if tag != 1:
            return 1
        var expected = pj_literal("caveman") if tool == OPTIONAL_TOOL_CAVEMAN else pj_literal("ponytail")
        return Int64(0) if pj_equal(fields[unsafe_offset=0], expected) else Int64(2)
    if phase == 1:
        return Int64(3) if tag == 0 else Int64(4) if tag == 1 else Int64(0)
    if phase == 3:
        return optional_tool_manifest_content_issue(tool, fields)
    if tag == 0 or not pj_equal(fields[unsafe_offset=0], fields[unsafe_offset=1]):
        return 5
    if not pj_equal(fields[unsafe_offset=2], fields[unsafe_offset=3]):
        return 6
    if not optional_tool_manifest_hex(fields[unsafe_offset=4], 40):
        return 7
    if not optional_tool_manifest_hex(fields[unsafe_offset=5], 64):
        return 8
    return 0


def optional_tool_manifest_diagnostic(
    sink: Pointer[mut=True, JsonSink, _], tool: Int64, issue: Int64,
    fields: Pointer[mut=False, ProdexRichStringView, _],
):
    if issue == 0:
        return
    if issue == 12:
        js_literal(sink, StringSlice("previously release-qualified Caveman 3.2.0 identity or content changed") if tool == OPTIONAL_TOOL_CAVEMAN else StringSlice("Ponytail previous-stable metadata does not match release-qualified compatibility metadata"))
        return
    var name = StringSlice("Caveman") if tool == OPTIONAL_TOOL_CAVEMAN else StringSlice("Ponytail")
    if issue == 1:
        js_literal(sink, StringSlice("unsupported "))
    elif issue == 3:
        js_literal(sink, StringSlice("invalid "))
    elif issue == 6:
        js_literal(sink, StringSlice("unexpected "))
    js_literal(sink, name)
    if issue == 1:
        js_literal(sink, StringSlice(" manifest schema"))
    elif issue == 2:
        js_literal(sink, StringSlice(" manifest id must be "))
        js_literal(sink, StringSlice("caveman") if tool == OPTIONAL_TOOL_CAVEMAN else StringSlice("ponytail"))
    elif issue == 3:
        js_literal(sink, StringSlice(" version "))
        js_view(sink, fields[unsafe_offset=0])
    elif issue == 4:
        js_literal(sink, StringSlice(" "))
        js_view(sink, fields[unsafe_offset=0])
        js_literal(sink, StringSlice(" is incompatible; Prodex requires "))
        js_view(sink, fields[unsafe_offset=1])
        js_literal(sink, StringSlice(" or newer. Update "))
        js_literal(sink, name)
        js_literal(sink, StringSlice(" to the latest stable release (release-qualified reference: "))
        js_view(sink, fields[unsafe_offset=2])
        js_literal(sink, StringSlice(")"))
    elif issue == 5:
        js_literal(sink, StringSlice(" version directory does not match manifest version "))
        js_view(sink, fields[unsafe_offset=0])
    elif issue == 6:
        js_literal(sink, StringSlice(" source"))
    elif issue == 7:
        js_literal(sink, StringSlice(" manifest commit must be a 40-character Git SHA"))
    elif issue == 8:
        js_literal(sink, StringSlice(" manifest tree digest must be SHA-256"))
    elif issue == 9:
        js_literal(sink, StringSlice(" latest-stable commit does not match release-qualified metadata"))
    elif issue == 10:
        js_literal(sink, StringSlice(" latest-stable manifest tree digest does not match release-qualified metadata"))
    elif issue == 11 or issue == 13:
        js_literal(sink, StringSlice(" tree digest mismatch: expected ") if issue == 11 else StringSlice(" tree digest mismatch: manifest "))
        var digest_index = Int64(6) if issue == 11 else Int64(2)
        js_view(sink, fields[unsafe_offset=digest_index])
        js_literal(sink, StringSlice(", got "))
        js_view(sink, fields[unsafe_offset=3])


@export("prodex_optional_tool_install_policy_v1")
def prodex_optional_tool_install_policy_v1(
    abi_version: Int64, tool: Int64, phase: Int64, tag: Int64,
    fields_address: UInt, field_count: Int64, measuring: Int64,
    output_address: UInt, capacity: Int64, written_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return 4
    if (
        tool != OPTIONAL_TOOL_CAVEMAN and tool != OPTIONAL_TOOL_PONYTAIL
        or phase < 0 or phase > 3 or tag < 0
        or phase == 0 and tag > 0xFFFFFFFF
        or phase == 1 and tag > 2 or phase == 2 and tag > 1
        or phase == 3 and tag != 0
        or fields_address == 0 or fields_address % 8 != 0
        or field_count != (Int64(14) if phase == 3 else Int64(6))
        or measuring < 0 or measuring > 1 or capacity < 0
        or written_address == 0 or written_address % 8 != 0
        or measuring == 1 and (output_address != 0 or capacity != 0)
        or measuring == 0 and output_address == 0
    ):
        return 1
    var fields = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(fields_address))
    for index in range(field_count):
        var field = fields[unsafe_offset=index].copy()
        if not rich_view_valid(field, 1_048_576):
            return 1
        if (phase == 0 and index > 0 or phase == 1 and index > 2) and field.len != 0:
            return 1
        if phase == 3 and tool == OPTIONAL_TOOL_PONYTAIL and index >= 10 and index <= 12 and field.len != 0:
            return 1
    var issue = optional_tool_manifest_issue(tool, phase, tag, fields)
    var sink = JsonSink(Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)), capacity, 0, measuring == 1, False)
    optional_tool_manifest_diagnostic(Pointer(to=sink), tool, issue, fields)
    if sink.failed:
        return 2
    Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))[] = sink.written
    return 0
