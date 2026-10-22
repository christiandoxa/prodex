from std.memory import Pointer
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_NULL, JSON_NUMBER, JSON_STRING, JSON_ARRAY,
    JSON_OBJECT, pj_child, pj_next, pj_field, pj_kind, pj_text, pj_valid,
)
from rich_types import ProdexRichStringView
from log_number import log_parse_u64_text
from json_sink import (
    JsonSink, js_byte, js_literal, js_view, js_string, js_escaped, js_raw, js_raw_view,
)


def ltp_unsigned(tree: ParsedJson, index: Int64) -> Bool:
    if pj_kind(tree, index) != JSON_NUMBER:
        return False
    var view = js_raw_view(tree, index)
    return log_parse_u64_text(UInt64(view.ptr), UInt64(view.len))[0]


def ltp_valid(tree: ParsedJson) -> Bool:
    var event = pj_child(tree, 0)
    var rate = pj_next(tree, event)
    var missing_rate = pj_next(tree, rate)
    if (pj_kind(tree, 0) != JSON_ARRAY or pj_kind(tree, event) != JSON_OBJECT
        or (pj_kind(tree, rate) != JSON_NULL and pj_kind(tree, rate) != JSON_STRING)
        or pj_kind(tree, missing_rate) != JSON_STRING or pj_next(tree, missing_rate) >= 0):
        return False
    for key in [StringSlice("timestamp"), StringSlice("profile"), StringSlice("transport"), StringSlice("source")]:
        if pj_kind(tree, pj_field(tree, event, key)) != JSON_STRING:
            return False
    for key in [StringSlice("input_tokens"), StringSlice("cached_input_tokens"), StringSlice("output_tokens"), StringSlice("reasoning_tokens")]:
        if not ltp_unsigned(tree, pj_field(tree, event, key)):
            return False
    var field = pj_child(tree, event)
    while field >= 0:
        var known = False
        for key in [StringSlice("timestamp"), StringSlice("profile"), StringSlice("transport"), StringSlice("source"), StringSlice("request"), StringSlice("input_tokens"), StringSlice("cached_input_tokens"), StringSlice("output_tokens"), StringSlice("reasoning_tokens"), StringSlice("generation_ms"), StringSlice("output_tokens_per_second")]:
            if pj_field(tree, event, key) == field:
                known = True
                break
        if not known:
            return False
        field = pj_next(tree, field)
    var request = pj_field(tree, event, StringSlice("request"))
    var generation = pj_field(tree, event, StringSlice("generation_ms"))
    var reported_rate = pj_field(tree, event, StringSlice("output_tokens_per_second"))
    if pj_kind(tree, request) != JSON_NULL and not ltp_unsigned(tree, request):
        return False
    if generation >= 0 and not ltp_unsigned(tree, generation):
        return False
    if reported_rate < 0:
        return pj_kind(tree, rate) == JSON_NULL
    return (pj_kind(tree, reported_rate) == JSON_NUMBER or pj_kind(tree, reported_rate) == JSON_NULL) and pj_kind(tree, rate) == JSON_STRING


def ltp_value(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, event: Int64, key: StringSlice, missing: StringSlice):
    var field = pj_field(tree, event, key)
    if pj_kind(tree, field) == JSON_STRING:
        js_escaped(sink, pj_text(tree, field))
    elif pj_kind(tree, field) == JSON_NUMBER:
        js_view(sink, js_raw_view(tree, field))
        if key == StringSlice("generation_ms"):
            js_literal(sink, StringSlice("ms"))
    else:
        js_literal(sink, missing)


def ltp_has_average(tree: ParsedJson, event: Int64) -> Bool:
    var generation = pj_field(tree, event, StringSlice("generation_ms"))
    if generation < 0 or pj_kind(tree, pj_next(tree, event)) != JSON_STRING:
        return False
    var view = js_raw_view(tree, generation)
    return log_parse_u64_text(UInt64(view.ptr), UInt64(view.len))[1] > 0


def ltp_envelope(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, event: Int64):
    js_literal(sink, StringSlice('{"event":"token_usage","fields":{'))
    var comma = False
    for key in [StringSlice("request"), StringSlice("transport"), StringSlice("profile"), StringSlice("source"), StringSlice("input_tokens"), StringSlice("cached_input_tokens"), StringSlice("output_tokens"), StringSlice("reasoning_tokens"), StringSlice("generation_ms"), StringSlice("output_tokens_per_second")]:
        var field = pj_field(tree, event, key)
        if field < 0 or (key == StringSlice("request") and pj_kind(tree, field) == JSON_NULL):
            continue
        if comma:
            js_byte(sink, 44)
        comma = True
        js_literal(sink, StringSlice('"'))
        js_literal(sink, key)
        js_literal(sink, StringSlice('":'))
        if key == StringSlice("request"):
            js_string(sink, js_raw_view(tree, field))
        else:
            js_raw(sink, tree, field)
    js_byte(sink, 125)
    var field = pj_child(tree, event)
    while field >= 0:
        js_byte(sink, 44)
        js_string(sink, tree.nodes[unsafe_offset=field].key)
        js_byte(sink, 58)
        js_raw(sink, tree, field)
        field = pj_next(tree, field)
    js_byte(sink, 125)


def ltp_metadata(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, event: Int64):
    js_byte(sink, 91)
    var comma = False
    for item in [
        (StringSlice("profile"), StringSlice("profile")),
        (StringSlice("request"), StringSlice("request")),
        (StringSlice("transport"), StringSlice("transport")),
        (StringSlice("source"), StringSlice("source")),
        (StringSlice("input"), StringSlice("input_tokens")),
        (StringSlice("cache"), StringSlice("cached_input_tokens")),
        (StringSlice("output"), StringSlice("output_tokens")),
        (StringSlice("reasoning"), StringSlice("reasoning_tokens")),
        (StringSlice("generation"), StringSlice("generation_ms")),
    ]:
        if comma:
            js_byte(sink, 44)
        comma = True
        js_literal(sink, StringSlice('["'))
        js_literal(sink, item[0])
        js_literal(sink, StringSlice('","'))
        ltp_value(sink, tree, event, item[1], StringSlice("unavailable") if item[1] == StringSlice("generation_ms") else StringSlice("-"))
        js_literal(sink, StringSlice('"]'))
    js_literal(sink, StringSlice(',["avg_output",'))
    var rate = pj_next(tree, event)
    js_string(sink, pj_text(tree, rate if ltp_has_average(tree, event) else pj_next(tree, rate)))
    js_literal(sink, StringSlice(']]'))


def ltp_span(sink: Pointer[mut=True, JsonSink, _], tone: StringSlice, text: StringSlice):
    js_literal(sink, StringSlice('["'))
    js_literal(sink, tone)
    js_literal(sink, StringSlice('","'))
    js_literal(sink, text)
    js_literal(sink, StringSlice('"]'))


def ltp_field_span(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, event: Int64, tone: StringSlice, key: StringSlice):
    js_literal(sink, StringSlice(',["'))
    js_literal(sink, tone)
    js_literal(sink, StringSlice('","'))
    ltp_value(sink, tree, event, key, StringSlice("unavailable") if key == StringSlice("generation_ms") else StringSlice("-"))
    js_literal(sink, StringSlice('"]'))


def ltp_tui(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, event: Int64):
    js_literal(sink, StringSlice('[[["Muted",'))
    js_string(sink, pj_text(tree, pj_field(tree, event, StringSlice("timestamp"))))
    js_literal(sink, StringSlice('],'))
    ltp_span(sink, StringSlice("Raw"), StringSlice(" "))
    js_byte(sink, 44)
    ltp_span(sink, StringSlice("Token"), StringSlice("TOKENS"))
    js_literal(sink, StringSlice('],['))
    var comma = False
    for key in [StringSlice("profile"), StringSlice("request"), StringSlice("transport"), StringSlice("source")]:
        if comma:
            js_literal(sink, StringSlice(',["Muted"," '))
        else:
            js_literal(sink, StringSlice('["Muted","'))
        comma = True
        js_literal(sink, key)
        js_literal(sink, StringSlice(' "]'))
        ltp_field_span(sink, tree, event, StringSlice("Primary"), key)
    js_literal(sink, StringSlice('],['))
    comma = False
    for item in [
        (StringSlice("input"), StringSlice("input_tokens"), StringSlice("Metric")),
        (StringSlice("cache"), StringSlice("cached_input_tokens"), StringSlice("Accent")),
        (StringSlice("output"), StringSlice("output_tokens"), StringSlice("Metric")),
        (StringSlice("reasoning"), StringSlice("reasoning_tokens"), StringSlice("Tool")),
        (StringSlice("generation"), StringSlice("generation_ms"), StringSlice("Metric")),
    ]:
        if comma:
            js_literal(sink, StringSlice(',["Muted"," '))
        else:
            js_literal(sink, StringSlice('["Muted","'))
        comma = True
        js_literal(sink, item[0])
        js_literal(sink, StringSlice(' "]'))
        ltp_field_span(sink, tree, event, item[2], item[1])
    js_byte(sink, 93)
    if ltp_has_average(tree, event):
        js_byte(sink, 44)
        js_byte(sink, 91)
        ltp_span(sink, StringSlice("Muted"), StringSlice("average "))
        js_literal(sink, StringSlice(',["Metric",'))
        js_string(sink, pj_text(tree, pj_next(tree, event)))
        js_literal(sink, StringSlice(']]'))
    js_byte(sink, 93)


def ltp_emit(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, operation: Int64):
    var event = pj_child(tree, 0)
    if operation == 0:
        ltp_envelope(sink, tree, event)
    elif operation == 1:
        ltp_metadata(sink, tree, event)
    elif operation == 2:
        ltp_tui(sink, tree, event)
    else:
        js_byte(sink, 34)
        var comma = False
        for key in [StringSlice("timestamp"), StringSlice("request"), StringSlice("transport"), StringSlice("profile"), StringSlice("source"), StringSlice("input_tokens"), StringSlice("cached_input_tokens"), StringSlice("output_tokens"), StringSlice("reasoning_tokens")]:
            if comma:
                js_byte(sink, 32)
            comma = True
            ltp_value(sink, tree, event, key, StringSlice(""))
        js_byte(sink, 34)


@export("prodex_log_token_presentation_v1")
def prodex_log_token_presentation_v1(
    abi: Int64, operation: Int64, flag: Int64,
    nodes_address: UInt64, nodes_count: Int64, raw_address: UInt64, raw_length: Int64,
    scratch_address: UInt64, scratch_count: Int64,
    measuring: Int64, output_address: UInt64, capacity: Int64, metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (operation < 0 or operation > 3 or flag != 0 or nodes_count <= 0 or nodes_count > 1_048_576
        or nodes_address == 0 or nodes_address % 8 != 0 or raw_length < 0
        or scratch_address == 0 or scratch_address % 8 != 0 or scratch_count != nodes_count
        or metadata_address == 0 or metadata_address % 8 != 0 or capacity < 0
        or measuring < 0 or measuring > 1
        or (measuring == 1 and (output_address != 0 or capacity != 0))
        or (measuring == 0 and output_address == 0)):
        return 1
    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](unsafe_from_address=Int(nodes_address)),
        nodes_count, ProdexRichStringView(UInt(raw_address), UInt(raw_length)),
    )
    if not pj_valid(tree) or not ltp_valid(tree):
        return 1
    var sink = JsonSink(Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)), capacity, 0, True, False)
    ltp_emit(Pointer(to=sink), tree, operation)
    if sink.failed:
        return 3
    var required = sink.written
    if measuring == 0:
        if capacity < required:
            return 3
        sink.measuring = False
        sink.written = 0
        ltp_emit(Pointer(to=sink), tree, operation)
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = required
    return 0
