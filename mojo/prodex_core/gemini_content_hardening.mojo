# Gemini conversation content normalization over a borrowed Serde tree.
# This owner never executes tools or invents tool results; subsequent repair
# operations preserve the existing explicit missing-result sentinel contract.
from std.memory import Pointer
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_ARRAY, JSON_STRING,
    pj_child, pj_next, pj_field, pj_kind, pj_text, pj_literal, pj_equal, pj_valid,
)
from json_sink import JsonSink, js_byte, js_literal, js_string, js_raw
from rich_types import ProdexRichStringView


def gch_parts(tree: ParsedJson, content: Int64) -> Int64:
    var parts = pj_field(tree, content, StringSlice("parts"))
    return parts if pj_kind(tree, parts) == JSON_ARRAY else -1


def gch_role(tree: ParsedJson, content: Int64) -> ProdexRichStringView:
    var role = pj_field(tree, content, StringSlice("role"))
    if pj_kind(tree, role) == JSON_STRING:
        return pj_text(tree, role)
    return pj_literal(StringSlice("user"))


def gch_next_content(tree: ParsedJson, start: Int64) -> Int64:
    var current = start
    while current >= 0:
        if pj_child(tree, gch_parts(tree, current)) >= 0:
            return current
        current = pj_next(tree, current)
    return -1


def gch_write_group(
    sink: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson, first: Int64, end: Int64,
    lead: Bool = False, tail: Bool = False,
):
    js_byte(sink, 123)
    # Preserve all non-role/parts metadata of the first retained content.
    var field = -1 if lead else pj_child(tree, first)
    while field >= 0:
        var key = tree.nodes[unsafe_offset=field].key.copy()
        if not pj_equal(key, pj_literal(StringSlice("role"))) and not pj_equal(
            key, pj_literal(StringSlice("parts"))
        ):
            js_string(sink, key)
            js_byte(sink, 58)
            js_raw(sink, tree, field)
            js_byte(sink, 44)
        field = pj_next(tree, field)
    js_literal(sink, StringSlice('"role":'))
    js_string(sink, gch_role(tree, first))
    js_literal(sink, StringSlice(',"parts":['))
    var current = first
    var emitted = lead
    if lead:
        js_literal(sink, StringSlice('{"text":"[Continuing from previous Codex context.]"}'))
    while current >= 0 and current != end:
        var part = pj_child(tree, gch_parts(tree, current))
        while part >= 0:
            if emitted:
                js_byte(sink, 44)
            js_raw(sink, tree, part)
            emitted = True
            part = pj_next(tree, part)
        current = pj_next(tree, current)
    if tail:
        if emitted:
            js_byte(sink, 44)
        js_literal(sink, StringSlice('{"text":"Please continue."}'))
    js_literal(sink, StringSlice(']}'))


def gch_coalesce(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, shape: Bool = False):
    js_byte(sink, 91)
    var first_raw = pj_child(tree, 0)
    var lead = shape and gch_role_is(tree, first_raw, StringSlice("model"))
    var tail = False
    if shape:
        var last = first_raw
        while last >= 0 and pj_next(tree, last) >= 0:
            last = pj_next(tree, last)
        tail = gch_role_is(tree, last, StringSlice("model"))
    var current = gch_next_content(tree, first_raw)
    var emitted = False
    if current < 0 and (lead or tail):
        gch_write_group(sink, tree, -1, -1, lead, tail)
        lead = False
        tail = False
    while current >= 0:
        var role = gch_role(tree, current)
        var user = pj_equal(role, pj_literal(StringSlice("user")))
        var next = gch_next_content(tree, pj_next(tree, current))
        while next >= 0 and pj_equal(role, gch_role(tree, next)):
            next = gch_next_content(tree, pj_next(tree, next))
        if lead and not user:
            gch_write_group(sink, tree, -1, -1, True, False)
            emitted = True
        if emitted:
            js_byte(sink, 44)
        var append_tail = tail and next < 0 and user
        gch_write_group(sink, tree, current, next, lead and user, append_tail)
        lead = False
        if append_tail:
            tail = False
        emitted = True
        current = next
    if tail:
        if emitted:
            js_byte(sink, 44)
        gch_write_group(sink, tree, -1, -1, False, True)
    js_byte(sink, 93)


def gch_role_is(tree: ParsedJson, content: Int64, role: StringSlice) -> Bool:
    var node = pj_field(tree, content, StringSlice("role"))
    return pj_kind(tree, node) == JSON_STRING and pj_equal(pj_text(tree, node), pj_literal(role))


def gch_function_identity(tree: ParsedJson, function: Int64, key: StringSlice) -> ProdexRichStringView:
    var node = pj_field(tree, function, key)
    if pj_kind(tree, node) == JSON_STRING:
        return pj_text(tree, node)
    return pj_literal(StringSlice("")) if key == StringSlice("id") else pj_literal(StringSlice("unknown"))


def gch_pair_present(tree: ParsedJson, function: Int64, other: Int64, key: StringSlice) -> Bool:
    var part = pj_child(tree, gch_parts(tree, other))
    while part >= 0:
        var candidate = pj_field(tree, part, key)
        if candidate >= 0 and pj_equal(
            gch_function_identity(tree, function, StringSlice("id")),
            gch_function_identity(tree, candidate, StringSlice("id")),
        ) and pj_equal(
            gch_function_identity(tree, function, StringSlice("name")),
            gch_function_identity(tree, candidate, StringSlice("name")),
        ):
            return True
        part = pj_next(tree, part)
    return False


def gch_missing_exists(tree: ParsedJson, source: Int64, other: Int64, orphan: Bool) -> Bool:
    var key = StringSlice("functionResponse") if orphan else StringSlice("functionCall")
    var counterpart = StringSlice("functionCall") if orphan else StringSlice("functionResponse")
    var part = pj_child(tree, gch_parts(tree, source))
    while part >= 0:
        var function = pj_field(tree, part, key)
        if function >= 0 and not gch_pair_present(tree, function, other, counterpart):
            return True
        part = pj_next(tree, part)
    return False


def gch_append_missing(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson,
    source: Int64, other: Int64, orphan: Bool, already_emitted: Bool,
):
    var emitted = already_emitted
    var key = StringSlice("functionResponse") if orphan else StringSlice("functionCall")
    var counterpart = StringSlice("functionCall") if orphan else StringSlice("functionResponse")
    var part = pj_child(tree, gch_parts(tree, source))
    while part >= 0:
        var function = pj_field(tree, part, key)
        if function >= 0 and not gch_pair_present(tree, function, other, counterpart):
            if emitted:
                js_byte(sink, 44)
            js_literal(sink, StringSlice('{"functionCall":{"id":') if orphan else StringSlice('{"functionResponse":{"id":'))
            js_string(sink, gch_function_identity(tree, function, StringSlice("id")))
            js_literal(sink, StringSlice(',"name":'))
            js_string(sink, gch_function_identity(tree, function, StringSlice("name")))
            if orphan:
                js_literal(sink, StringSlice(',"args":{}},"thoughtSignature":"skip_thought_signature_validator"}'))
            else:
                js_literal(sink, StringSlice(',"response":{"error":"The tool execution result was missing from the Codex conversation history. Treat this tool result as unavailable and recover by inspecting current workspace state before continuing."}}}'))
            emitted = True
        part = pj_next(tree, part)


def gch_write_extended(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson,
    content: Int64, source: Int64, orphan: Bool,
):
    js_byte(sink, 123)
    var field = pj_child(tree, content)
    var first = True
    while field >= 0:
        if not first:
            js_byte(sink, 44)
        var key = tree.nodes[unsafe_offset=field].key.copy()
        js_string(sink, key)
        js_byte(sink, 58)
        if pj_equal(key, pj_literal(StringSlice("parts"))):
            js_byte(sink, 91)
            var part = pj_child(tree, field)
            var emitted = False
            while part >= 0:
                if emitted:
                    js_byte(sink, 44)
                js_raw(sink, tree, part)
                emitted = True
                part = pj_next(tree, part)
            gch_append_missing(sink, tree, source, content, orphan, emitted)
            js_byte(sink, 93)
        else:
            js_raw(sink, tree, field)
        first = False
        field = pj_next(tree, field)
    js_byte(sink, 125)


def gch_write_synthetic(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, source: Int64, orphan: Bool,
):
    js_literal(sink, StringSlice('{"role":"model","parts":[') if orphan else StringSlice('{"role":"user","parts":['))
    gch_append_missing(sink, tree, source, -1, orphan, False)
    js_literal(sink, StringSlice(']}'))


def gch_pair_tools(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, orphan: Bool):
    js_byte(sink, 91)
    var current = pj_child(tree, 0)
    var previous: Int64 = -1
    var emitted = False
    while current >= 0:
        var next = pj_next(tree, current)
        var is_model = gch_role_is(tree, current, StringSlice("model"))
        var is_user = gch_role_is(tree, current, StringSlice("user"))
        var previous_model = gch_role_is(tree, previous, StringSlice("model"))
        var next_user = gch_role_is(tree, next, StringSlice("user"))
        if emitted:
            js_byte(sink, 44)
        if orphan and is_user and not previous_model and gch_missing_exists(tree, current, -1, True):
            gch_write_synthetic(sink, tree, current, True)
            js_byte(sink, 44)
        var source: Int64 = -1
        if orphan and is_model and next_user:
            source = next
        elif not orphan and is_user and previous_model:
            source = previous
        if source >= 0 and gch_parts(tree, current) >= 0 and gch_missing_exists(tree, source, current, orphan):
            gch_write_extended(sink, tree, current, source, orphan)
        else:
            js_raw(sink, tree, current)
        if not orphan and is_model and (not next_user or gch_parts(tree, next) < 0):
            if gch_missing_exists(tree, current, -1, False):
                js_byte(sink, 44)
                gch_write_synthetic(sink, tree, current, False)
        emitted = True
        previous = current
        current = next
    js_byte(sink, 93)


@export("prodex_mojo_gemini_content_hardening_v1")
def prodex_mojo_gemini_content_hardening_v1(
    abi: Int64, operation: Int64, flag: Int64,
    nodes_address: UInt64, nodes_count: Int64,
    raw_address: UInt64, raw_length: Int64,
    scratch_address: UInt64, scratch_count: Int64,
    measuring: Int64, output_address: UInt64, capacity: Int64,
    metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (
        operation < 0 or operation > 3 or flag != 0 or measuring < 0 or measuring > 1
        or raw_length < 0 or nodes_address == 0 or metadata_address == 0
        or capacity < 0 or scratch_count < nodes_count or scratch_address == 0
    ):
        return 1
    if measuring == 1:
        if output_address != 0 or capacity != 0:
            return 1
    elif output_address == 0:
        return 1
    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](
            unsafe_from_address=Int(nodes_address)
        ), nodes_count, ProdexRichStringView(UInt(raw_address), UInt(raw_length)),
    )
    if not pj_valid(tree) or pj_kind(tree, 0) != JSON_ARRAY:
        return 1
    var sink = JsonSink(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)),
        capacity, 0, measuring == 1, False,
    )
    if operation == 0 or operation == 3:
        gch_coalesce(Pointer(to=sink), tree, operation == 3)
    else:
        gch_pair_tools(Pointer(to=sink), tree, operation == 2)
    if sink.failed:
        return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(metadata_address)
    )
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = sink.written
    return 0
