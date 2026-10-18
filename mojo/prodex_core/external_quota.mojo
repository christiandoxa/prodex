# External quota JSON selection and presentation; host acquisition retains numbers.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_ARRAY, JSON_OBJECT, JSON_STRING, JSON_NUMBER, JSON_NULL, JSON_TRUE,
    pj_kind, pj_child, pj_next, pj_field, pj_text, pj_equal, pj_is, pj_valid,
)
from json_sink import (
    JsonSink, js_byte, js_literal, js_string, js_escaped, js_raw_view, js_u64,
)


def eq_optional_string(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, index: Int64):
    if pj_kind(tree, index) == JSON_STRING:
        js_string(sink, pj_text(tree, index))
    else:
        js_literal(sink, StringSlice("null"))


def eq_scalar(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, index: Int64):
    js_escaped(sink, pj_text(tree, index) if pj_kind(tree, index) == JSON_STRING else js_raw_view(tree, index))


@fieldwise_init
struct AgyAccount(Copyable):
    var node: Int64
    var rank: Int64


def eq_agy_account(tree: ParsedJson, body: Int64, preferred: Int64) -> AgyAccount:
    if pj_kind(tree, body) != JSON_ARRAY:
        return AgyAccount(body, -1)
    var first = pj_child(tree, body)
    var current = first
    var rank: Int64 = 0
    if pj_kind(tree, preferred) == JSON_STRING:
        while current >= 0:
            for key in [StringSlice("account"), StringSlice("email")]:
                var field = pj_field(tree, current, key)
                if pj_kind(tree, field) == JSON_STRING and pj_equal(pj_text(tree, field), pj_text(tree, preferred)):
                    return AgyAccount(current, rank)
            current = pj_next(tree, current)
            rank += 1
    return AgyAccount(first, 0)


def eq_agy_detail(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, value: Int64, container: Int64, rank: Int64, count: Pointer[mut=True, Int64, _]):
    if count[] > 0:
        js_byte(sink, 44)
    count[] += 1
    js_literal(sink, StringSlice('{"label":"'))
    if container < 0:
        js_escaped(sink, tree.nodes[unsafe_offset=value].key)
    else:
        js_escaped(sink, tree.nodes[unsafe_offset=container].key)
        if pj_kind(tree, container) == JSON_ARRAY:
            js_byte(sink, 91)
            js_u64(sink, UInt64(rank))
            js_byte(sink, 93)
        else:
            js_byte(sink, 58)
            js_escaped(sink, tree.nodes[unsafe_offset=value].key)
    js_literal(sink, StringSlice('","value":"'))
    eq_scalar(sink, tree, value)
    js_literal(sink, StringSlice('"}'))


def eq_agy_details(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, account: Int64):
    if pj_kind(tree, account) != JSON_OBJECT:
        return
    var count: Int64 = 0
    var field = pj_child(tree, account)
    while field >= 0:
        var key = tree.nodes[unsafe_offset=field].key.copy()
        var nested = False
        var skipped = False
        for name in [StringSlice("details"), StringSlice("models"), StringSlice("quotas")]:
            nested |= pj_equal(key, ProdexRichStringView(UInt(Int(name.unsafe_ptr())), UInt(name.byte_length())))
        for name in [StringSlice("account"), StringSlice("email"), StringSlice("plan"), StringSlice("status"), StringSlice("credits"), StringSlice("usage")]:
            skipped |= pj_equal(key, ProdexRichStringView(UInt(Int(name.unsafe_ptr())), UInt(name.byte_length())))
        if nested:
            var child = pj_child(tree, field)
            var rank: Int64 = 0
            while child >= 0:
                eq_agy_detail(sink, tree, child, field, rank, Pointer(to=count))
                rank += 1
                child = pj_next(tree, child)
        elif not skipped:
            eq_agy_detail(sink, tree, field, -1, 0, Pointer(to=count))
        field = pj_next(tree, field)


def eq_agy(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson) -> Bool:
    var body = pj_field(tree, 0, StringSlice("value"))
    var selected = eq_agy_account(tree, body, pj_field(tree, 0, StringSlice("preferred")))
    if selected.node < 0:
        return False
    var account = pj_field(tree, selected.node, StringSlice("account"))
    if account < 0:
        account = pj_field(tree, selected.node, StringSlice("email"))
    var amount = pj_field(tree, selected.node, StringSlice("credits"))
    var credits = pj_kind(tree, amount) == JSON_NUMBER
    if not credits:
        amount = pj_field(tree, selected.node, StringSlice("usage"))
    var numeric = pj_kind(tree, amount) == JSON_NUMBER
    js_literal(sink, StringSlice('{"info":{"provider":"Anti-Gravity","account":'))
    eq_optional_string(sink, tree, account)
    js_literal(sink, StringSlice(',"plan":'))
    eq_optional_string(sink, tree, pj_field(tree, selected.node, StringSlice("plan")))
    js_literal(sink, StringSlice(',"status":'))
    var status = pj_field(tree, selected.node, StringSlice("status"))
    if pj_kind(tree, status) == JSON_STRING:
        js_string(sink, pj_text(tree, status))
    else:
        js_literal(sink, StringSlice('"Ready"'))
    js_literal(sink, StringSlice(',"main":"'))
    if not numeric:
        js_literal(sink, StringSlice("quota information available"))
    js_literal(sink, StringSlice('","reset":null,"available":true,"details":['))
    eq_agy_details(sink, tree, selected.node)
    js_literal(sink, StringSlice(']},"amount":'))
    if numeric:
        js_literal(sink, StringSlice('{"pointer":"'))
        if selected.rank >= 0:
            js_byte(sink, 47)
            js_u64(sink, UInt64(selected.rank))
        js_literal(sink, StringSlice('/credits') if credits else StringSlice('/usage'))
        js_literal(sink, StringSlice('","suffix":'))
        js_literal(sink, StringSlice('" credits available"') if credits else StringSlice('" usage"'))
        js_byte(sink, 125)
    else:
        js_literal(sink, StringSlice("null"))
    js_byte(sink, 125)
    return True


def eq_anthropic_detail(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, entry: Int64):
    var limits = pj_field(tree, entry, StringSlice("limits"))
    if pj_kind(tree, limits) != JSON_ARRAY:
        return
    var group = pj_field(tree, entry, StringSlice("group_type"))
    var label = group
    if pj_is["model_group"](tree, group):
        var models = pj_field(tree, entry, StringSlice("models"))
        var first = pj_child(tree, models) if pj_kind(tree, models) == JSON_ARRAY else -1
        if pj_kind(tree, first) == JSON_STRING:
            label = first
    js_literal(sink, StringSlice(',{"label":'))
    if pj_kind(tree, label) == JSON_STRING:
        js_string(sink, pj_text(tree, label))
    else:
        js_literal(sink, StringSlice('"rate_limit"'))
    js_literal(sink, StringSlice(',"value":"'))
    var child = pj_child(tree, limits)
    var count: Int64 = 0
    while child >= 0:
        var kind = pj_field(tree, child, StringSlice("type"))
        var value = pj_field(tree, child, StringSlice("value"))
        if pj_kind(tree, kind) == JSON_STRING and value >= 0:
            if count > 0:
                js_literal(sink, StringSlice(", "))
            count += 1
            js_escaped(sink, pj_text(tree, kind))
            js_byte(sink, 61)
            eq_scalar(sink, tree, value)
        child = pj_next(tree, child)
    js_literal(sink, StringSlice('"}'))


def eq_anthropic(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson):
    var data = pj_field(tree, 0, StringSlice("data"))
    var total: Int64 = 0
    var model_groups: Int64 = 0
    var first = pj_child(tree, data) if pj_kind(tree, data) == JSON_ARRAY else -1
    var child = first
    while child >= 0:
        total += 1
        model_groups += Int64(pj_is["model_group"](tree, pj_field(tree, child, StringSlice("group_type"))))
        child = pj_next(tree, child)
    js_literal(sink, StringSlice('{"main":"'))
    js_u64(sink, UInt64(model_groups if model_groups > 0 else total))
    js_literal(sink, StringSlice(' model rate group(s)') if model_groups > 0 else StringSlice(' rate group(s)'))
    js_literal(sink, StringSlice('","details":[{"label":"Rate groups","value":"'))
    js_u64(sink, UInt64(total))
    js_literal(sink, StringSlice('"},{"label":"Model groups","value":"'))
    js_u64(sink, UInt64(model_groups))
    js_literal(sink, StringSlice('"}'))
    child = first
    for _ in range(3):
        if child < 0:
            break
        eq_anthropic_detail(sink, tree, child)
        child = pj_next(tree, child)
    js_literal(sink, StringSlice("]}"))


def eq_balance_field(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, balance: Int64, key: StringSlice, fallback: StringSlice):
    var field = pj_field(tree, balance, key)
    if pj_kind(tree, field) == JSON_STRING:
        js_escaped(sink, pj_text(tree, field))
    else:
        js_literal(sink, fallback)


def eq_deepseek(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson):
    var available = pj_kind(tree, pj_field(tree, 0, StringSlice("is_available"))) == JSON_TRUE
    var balances = pj_field(tree, 0, StringSlice("balance_infos"))
    var first = pj_child(tree, balances) if pj_kind(tree, balances) == JSON_ARRAY else -1
    js_literal(sink, StringSlice('{"provider":"DeepSeek","account":null,"plan":"api-key","status":'))
    js_literal(sink, StringSlice('"Ready"') if available else StringSlice('"Blocked"'))
    js_literal(sink, StringSlice(',"main":"'))
    if first < 0:
        js_literal(sink, StringSlice("balance unavailable"))
    var balance = first
    while balance >= 0:
        if balance != first:
            js_literal(sink, StringSlice(" | "))
        eq_balance_field(sink, tree, balance, StringSlice("currency"), StringSlice("balance"))
        js_byte(sink, 32)
        eq_balance_field(sink, tree, balance, StringSlice("total_balance"), StringSlice("-"))
        balance = pj_next(tree, balance)
    js_literal(sink, StringSlice('","reset":null,"available":'))
    js_literal(sink, StringSlice("true") if available else StringSlice("false"))
    js_literal(sink, StringSlice(',"details":['))
    balance = first
    while balance >= 0:
        if balance != first:
            js_byte(sink, 44)
        js_literal(sink, StringSlice('{"label":"'))
        eq_balance_field(sink, tree, balance, StringSlice("currency"), StringSlice("balance"))
        js_literal(sink, StringSlice(' balance","value":"total '))
        eq_balance_field(sink, tree, balance, StringSlice("total_balance"), StringSlice("-"))
        js_literal(sink, StringSlice("; granted "))
        eq_balance_field(sink, tree, balance, StringSlice("granted_balance"), StringSlice("-"))
        js_literal(sink, StringSlice("; topped up "))
        eq_balance_field(sink, tree, balance, StringSlice("topped_up_balance"), StringSlice("-"))
        js_literal(sink, StringSlice('"}'))
        balance = pj_next(tree, balance)
    js_literal(sink, StringSlice("]}"))


def eq_local_models(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson):
    var body = pj_field(tree, 0, StringSlice("value"))
    var data = pj_field(tree, body, StringSlice("data"))
    var count: UInt64 = 0
    var child = pj_child(tree, data) if pj_kind(tree, data) == JSON_ARRAY else -1
    while child >= 0:
        count += 1
        child = pj_next(tree, child)
    js_literal(sink, StringSlice('{"provider":"Local OpenAI-compatible","account":'))
    js_string(sink, pj_text(tree, pj_field(tree, 0, StringSlice("account"))))
    js_literal(sink, StringSlice(',"plan":"local","status":"Reachable","main":"'))
    if pj_kind(tree, data) == JSON_ARRAY:
        js_u64(sink, count)
        js_literal(sink, StringSlice(" model(s)"))
    else:
        js_literal(sink, StringSlice("models endpoint reachable"))
    js_literal(sink, StringSlice('","reset":null,"available":true,"details":[{"label":"Models URL","value":'))
    js_string(sink, pj_text(tree, pj_field(tree, 0, StringSlice("models_url"))))
    js_literal(sink, StringSlice("}]}"))


def eq_render(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, operation: Int64) -> Bool:
    if operation == 0:
        return eq_agy(sink, tree)
    if operation == 1:
        eq_anthropic(sink, tree)
    elif operation == 2:
        eq_deepseek(sink, tree)
    else:
        eq_local_models(sink, tree)
    return True


@export("prodex_external_quota_projection_v1")
def prodex_external_quota_projection_v1(
    abi: Int64, operation: Int64, flag: Int64,
    nodes: UInt64, count: Int64, raw: UInt64, length: Int64,
    scratch: UInt64, scratch_count: Int64, measuring: Int64,
    output: UInt64, capacity: Int64, metadata: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if operation < 0 or operation > 3 or flag != 0 or nodes == 0 or count <= 0 or count > 1048576 or raw == 0 or length <= 0 or length > 67108864 or scratch == 0 or scratch_count != count or metadata == 0 or capacity < 0 or measuring < 0 or measuring > 1 or measuring == 1 and (output != 0 or capacity != 0) or measuring == 0 and output == 0:
        return 1
    var tree = ParsedJson(Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](unsafe_from_address=Int(nodes)), count, ProdexRichStringView(UInt(raw), UInt(length)))
    var decoded: UInt64 = 0
    for index in range(count):
        var node = tree.nodes[unsafe_offset=index].copy()
        if node.key.len > 67108864 or node.text.len > 67108864 or node.raw_length <= 0:
            return 1
        decoded += UInt64(node.key.len) + UInt64(node.text.len)
        if decoded > 67108864:
            return 1
    if not pj_valid(tree) or (operation == 0 or operation == 3) and pj_field(tree, 0, StringSlice("value")) < 0:
        return 1
    if operation == 0:
        var preferred = pj_kind(tree, pj_field(tree, 0, StringSlice("preferred")))
        if preferred != -1 and preferred != JSON_NULL and preferred != JSON_STRING:
            return 1
    if operation == 3:
        for key in [StringSlice("account"), StringSlice("models_url")]:
            if pj_kind(tree, pj_field(tree, 0, key)) != JSON_STRING:
                return 1
    var sink = JsonSink(Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output)), capacity, 0, True, False)
    var present = eq_render(Pointer(to=sink), tree, operation)
    if sink.failed or measuring == 0 and sink.written > capacity:
        return 3
    if measuring == 0 and present:
        sink.written = 0
        sink.measuring = False
        _ = eq_render(Pointer(to=sink), tree, operation)
    var meta = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata))
    meta[unsafe_offset=0] = Int64(present)
    meta[unsafe_offset=1] = sink.written
    return 0
