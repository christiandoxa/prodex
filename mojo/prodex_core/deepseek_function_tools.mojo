# Function-tool validation, strict normalization and stable duplicate selection.
from std.memory import Pointer
from std.collections import Array
from rich_types import ProdexRichStringView
from rich_text import rich_view_ptr, rich_view_order_sort, rich_view_compare
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_ARRAY, JSON_OBJECT, JSON_STRING, JSON_TRUE,
    pj_kind, pj_child, pj_next, pj_field, pj_text, pj_nonblank, pj_equal, pj_valid, pj_literal,
)
from json_sink import JsonSink, js_byte, js_literal, js_raw, js_raw_view, js_view, js_escaped, js_u64
from deepseek import (
    DeepSeekResponseWriter, deepseek_schema_validate, deepseek_schema_write,
    deepseek_raw_u64, DEEPSEEK_KERNEL_MAX_BYTES,
)
from json_view import deepseek_json_object_member


def function_name(tree: ParsedJson, tool: Int64) -> Int64:
    var name = pj_field(tree, pj_field(tree, tool, StringSlice("function")), StringSlice("name"))
    return name if pj_kind(tree, name) == JSON_STRING else -1


def function_name_valid(name: ProdexRichStringView, limit: UInt64) -> Bool:
    if name.len == 0 or UInt64(name.len) > limit:
        return False
    var ptr = rich_view_ptr(name)
    for index in range(Int64(name.len)):
        var byte = ptr[unsafe_offset=index]
        if not (byte >= 65 and byte <= 90 or byte >= 97 and byte <= 122 or byte >= 48 and byte <= 57 or byte == 95 or byte == 45):
            return False
    return True


def generic_wire(tool: ProdexRichStringView) -> Bool:
    var function = deepseek_json_object_member(tool, 0, Int64(tool.len), StringSlice("function"))
    if function[0] < 0:
        return False
    var parameters = deepseek_json_object_member(tool, function[0], function[1], StringSlice("parameters"))
    if parameters[0] < 0:
        return False
    var additional = deepseek_json_object_member(tool, parameters[0], parameters[1], StringSlice("additionalProperties"))
    return (
        additional[0] >= 0 and additional[1] - additional[0] == 4
        and rich_view_compare(ProdexRichStringView(tool.ptr + UInt(additional[0]), 4), pj_literal(StringSlice("true"))) == 0
        and deepseek_json_object_member(tool, parameters[0], parameters[1], StringSlice("properties"))[0] < 0
        and deepseek_json_object_member(tool, parameters[0], parameters[1], StringSlice("required"))[0] < 0
    )


def function_issue(tree: ParsedJson, tool: Int64, limit: UInt64, strict: Bool) -> Int64:
    var name = function_name(tree, tool)
    if name < 0:
        return 0
    if not function_name_valid(pj_text(tree, name), limit):
        return 1
    var function = pj_field(tree, tool, StringSlice("function"))
    var parameters = pj_field(tree, function, StringSlice("parameters"))
    if parameters >= 0 and pj_kind(tree, parameters) != JSON_OBJECT:
        return 2
    if not strict and pj_kind(tree, pj_field(tree, function, StringSlice("strict"))) == JSON_TRUE:
        return 3
    return 0


def schema_view(tree: ParsedJson, tool: Int64) -> ProdexRichStringView:
    var parameters = pj_field(tree, pj_field(tree, tool, StringSlice("function")), StringSlice("parameters"))
    if parameters >= 0:
        return js_raw_view(tree, parameters)
    return pj_literal(StringSlice('{"type":"object"}'))


def write_normalized_tool(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, tool: Int64, strict: Bool,
) -> Bool:
    var function = pj_field(tree, tool, StringSlice("function"))
    if not strict or pj_kind(tree, function) != JSON_OBJECT:
        js_raw(sink, tree, tool)
        return True
    var schema = schema_view(tree, tool)
    if schema.len > UInt(DEEPSEEK_KERNEL_MAX_BYTES) or not deepseek_schema_validate(schema, 0, Int64(schema.len), 0):
        return False
    js_byte(sink, 123)
    var field = pj_child(tree, tool)
    var emitted = False
    while field >= 0:
        if emitted:
            js_byte(sink, 44)
        js_byte(sink, 34)
        js_escaped(sink, tree.nodes[unsafe_offset=field].key)
        js_literal(sink, StringSlice('":'))
        if field != function:
            js_raw(sink, tree, field)
        else:
            js_byte(sink, 123)
            var member = pj_child(tree, function)
            var member_emitted = False
            while member >= 0:
                if member != pj_field(tree, function, StringSlice("parameters")) and member != pj_field(tree, function, StringSlice("strict")):
                    if member_emitted:
                        js_byte(sink, 44)
                    js_byte(sink, 34)
                    js_escaped(sink, tree.nodes[unsafe_offset=member].key)
                    js_literal(sink, StringSlice('":'))
                    js_raw(sink, tree, member)
                    member_emitted = True
                member = pj_next(tree, member)
            if member_emitted:
                js_byte(sink, 44)
            js_literal(sink, StringSlice('"parameters":'))
            var writer = DeepSeekResponseWriter(sink[].output, sink[].capacity, sink[].written, sink[].measuring)
            if not deepseek_schema_write(schema, 0, Int64(schema.len), Pointer(to=writer), 0):
                return False
            sink[].written = writer.written
            js_literal(sink, StringSlice(',"strict":true}'))
        emitted = True
        field = pj_next(tree, field)
    js_byte(sink, 125)
    return True


def function_error(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson,
    issue: Int64, name: ProdexRichStringView, limit: UInt64,
):
    js_literal(sink, StringSlice('{"error":"'))
    js_escaped(sink, pj_text(tree, pj_field(tree, 0, StringSlice("provider_label"))))
    if issue == 1:
        js_literal(sink, StringSlice(" function tool names must use only letters, numbers, underscores, or dashes and be at most "))
        js_u64(sink, limit)
        js_literal(sink, StringSlice(" bytes"))
    else:
        if issue == 3:
            js_literal(sink, StringSlice(" strict function tool `"))
        elif issue == 4:
            js_literal(sink, StringSlice(" strict tool schema `"))
        elif issue == 5:
            js_literal(sink, StringSlice(" function tool name `"))
        elif issue == 6:
            js_literal(sink, StringSlice(" named tool_choice `"))
        else:
            js_literal(sink, StringSlice(" function tool `"))
        js_escaped(sink, name)
        if issue == 2:
            js_literal(sink, StringSlice("` parameters must be an object"))
        elif issue == 3:
            js_literal(sink, StringSlice("` requires deepseek.strict_tools=true"))
        elif issue == 4:
            js_literal(sink, StringSlice("` could not be normalized"))
        elif issue == 5:
            js_literal(sink, StringSlice("` is duplicated after translation"))
        else:
            js_literal(sink, StringSlice("` does not match any translated function tool"))
    js_literal(sink, StringSlice('"}'))


def context_valid(tree: ParsedJson, operation: Int64) -> Bool:
    if not pj_valid(tree) or pj_kind(tree, 0) != JSON_OBJECT or operation < 0 or operation > 8:
        return False
    var limit = pj_field(tree, 0, StringSlice("max_name_bytes"))
    if limit < 0 or pj_kind(tree, pj_field(tree, 0, StringSlice("provider_label"))) != JSON_STRING:
        return False
    var node = tree.nodes[unsafe_offset=limit].copy()
    var bounds = Array[Int64, 2](fill=0)
    bounds[0] = node.raw_start
    bounds[1] = node.raw_start + node.raw_length
    return deepseek_raw_u64(tree.raw, bounds)[0] == 1 and pj_field(tree, 0, StringSlice("input")) >= 0


def name_limit(tree: ParsedJson) -> UInt64:
    var node = tree.nodes[unsafe_offset=pj_field(tree, 0, StringSlice("max_name_bytes"))].copy()
    var bounds = Array[Int64, 2](fill=0)
    bounds[0] = node.raw_start
    bounds[1] = node.raw_start + node.raw_length
    return deepseek_raw_u64(tree.raw, bounds)[1]


# Serde canonicalizes object keys and number text. Equal signed floating zero
# has two canonical spellings; quoted text is compared byte-for-byte.
def canonical_tools_equal(left: ProdexRichStringView, right: ProdexRichStringView) -> Bool:
    var a = rich_view_ptr(left)
    var b = rich_view_ptr(right)
    var x: Int64 = 0
    var y: Int64 = 0
    var quoted = False
    var escaped = False
    while x < Int64(left.len) and y < Int64(right.len):
        if not quoted:
            if a[unsafe_offset=x] == 45 and x + 4 <= Int64(left.len):
                if a[unsafe_offset=x + 1] == 48 and a[unsafe_offset=x + 2] == 46 and a[unsafe_offset=x + 3] == 48 and (x + 4 == Int64(left.len) or a[unsafe_offset=x + 4] == 44 or a[unsafe_offset=x + 4] == 93 or a[unsafe_offset=x + 4] == 125):
                    x += 1
            if b[unsafe_offset=y] == 45 and y + 4 <= Int64(right.len):
                if b[unsafe_offset=y + 1] == 48 and b[unsafe_offset=y + 2] == 46 and b[unsafe_offset=y + 3] == 48 and (y + 4 == Int64(right.len) or b[unsafe_offset=y + 4] == 44 or b[unsafe_offset=y + 4] == 93 or b[unsafe_offset=y + 4] == 125):
                    y += 1
        var byte = a[unsafe_offset=x]
        if byte != b[unsafe_offset=y]:
            return False
        if quoted:
            if escaped:
                escaped = False
            elif byte == 92:
                escaped = True
            elif byte == 34:
                quoted = False
        elif byte == 34:
            quoted = True
        x += 1
        y += 1
    return x == Int64(left.len) and y == Int64(right.len)


def sorted_name_slot(
    names: Pointer[mut=True, ProdexRichStringView, MutUntrackedOrigin],
    order: Pointer[mut=True, Int64, MutUntrackedOrigin], count: Int64,
    name: ProdexRichStringView,
) -> Int64:
    var start: Int64 = 0
    var end = count
    while start < end:
        var middle = start + (end - start) // 2
        if rich_view_compare(names[unsafe_offset=order[unsafe_offset=middle]], name) < 0:
            start = middle + 1
        else:
            end = middle
    return start


def deduplicate_function_tools(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, strict: Bool,
    scratch_address: UInt64, scratch_units: Int64,
) -> Int64:
    var tools = pj_field(tree, 0, StringSlice("input"))
    if pj_kind(tree, tools) != JSON_ARRAY:
        return 1
    var names = Pointer[mut=True, ProdexRichStringView, MutUntrackedOrigin](unsafe_from_address=Int(scratch_address))
    var order = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(scratch_address + UInt64(tree.count * 16)))
    var previous = order.unsafe_offset(Int(tree.count))
    var active = previous.unsafe_offset(Int(tree.count))
    var starts = active.unsafe_offset(Int(tree.count))
    var ends = starts.unsafe_offset(Int(tree.count))
    var buffer_address = scratch_address + UInt64(tree.count * 64)
    var buffer = JsonSink(Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(buffer_address)), (scratch_units - tree.count * 4) * 16, 0, False, False)
    var count: Int64 = 0
    for index in range(tree.count):
        previous[unsafe_offset=index] = -1
        active[unsafe_offset=index] = 0
    var tool = pj_child(tree, tools)
    while tool >= 0:
        var name = function_name(tree, tool)
        if name >= 0:
            names[unsafe_offset=tool] = pj_text(tree, name)
            order[unsafe_offset=count] = tool
            count += 1
        tool = pj_next(tree, tool)
    rich_view_order_sort(names, order, count)
    tool = pj_child(tree, tools)
    var limit = name_limit(tree)
    while tool >= 0:
        var name = function_name(tree, tool)
        var issue = function_issue(tree, tool, limit, strict)
        if issue != 0:
            function_error(sink, tree, issue, pj_text(tree, name), limit)
            return 0
        starts[unsafe_offset=tool] = buffer.written
        if not write_normalized_tool(Pointer(to=buffer), tree, tool, strict and name >= 0):
            function_error(sink, tree, 4, pj_text(tree, name), limit)
            return 0
        if buffer.failed:
            return 3
        ends[unsafe_offset=tool] = buffer.written
        var keep = True
        if name >= 0:
            var slot = sorted_name_slot(names, order, count, pj_text(tree, name))
            var prior = previous[unsafe_offset=slot]
            if prior >= 0:
                var current_view = ProdexRichStringView(UInt(buffer_address) + UInt(starts[unsafe_offset=tool]), UInt(ends[unsafe_offset=tool] - starts[unsafe_offset=tool]))
                var prior_view = ProdexRichStringView(UInt(buffer_address) + UInt(starts[unsafe_offset=prior]), UInt(ends[unsafe_offset=prior] - starts[unsafe_offset=prior]))
                if canonical_tools_equal(current_view, prior_view):
                    keep = False
                else:
                    var current_generic = generic_wire(current_view)
                    var prior_generic = generic_wire(prior_view)
                    if prior_generic and not current_generic:
                        active[unsafe_offset=prior] = 0
                    elif current_generic:
                        keep = False
                    else:
                        function_error(sink, tree, 5, pj_text(tree, name), limit)
                        return 0
            if keep:
                previous[unsafe_offset=slot] = tool
        active[unsafe_offset=tool] = Int64(keep)
        tool = pj_next(tree, tool)
    js_literal(sink, StringSlice('{"value":['))
    var emitted = False
    tool = pj_child(tree, tools)
    while tool >= 0:
        if active[unsafe_offset=tool] == 1:
            if emitted:
                js_byte(sink, 44)
            js_view(sink, ProdexRichStringView(UInt(buffer_address) + UInt(starts[unsafe_offset=tool]), UInt(ends[unsafe_offset=tool] - starts[unsafe_offset=tool])))
            emitted = True
        tool = pj_next(tree, tool)
    js_literal(sink, StringSlice("]}"))
    return 0


def function_tool_operation(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson,
    operation: Int64, strict: Bool, scratch_address: UInt64, scratch_count: Int64,
) -> Int64:
    if operation == 0:
        return deduplicate_function_tools(sink, tree, strict, scratch_address, scratch_count)
    var input = pj_field(tree, 0, StringSlice("input"))
    var name = function_name(tree, input)
    var limit = name_limit(tree)
    var issue: Int64 = 0
    if operation == 5:
        name = input
        if pj_kind(tree, name) != JSON_STRING:
            return 1
        if not function_name_valid(pj_text(tree, name), limit):
            issue = 1
    elif operation == 6:
        name = pj_field(tree, 0, StringSlice("name"))
        var parameters = pj_field(tree, pj_field(tree, input, StringSlice("function")), StringSlice("parameters"))
        if parameters >= 0 and pj_kind(tree, parameters) != JSON_OBJECT:
            issue = 2
    elif operation == 7:
        if name >= 0 and not function_name_valid(pj_text(tree, name), limit):
            issue = 1
    elif operation == 8:
        name = function_name(tree, pj_field(tree, input, StringSlice("tool_choice")))
        if name >= 0:
            var candidate = pj_child(tree, pj_field(tree, input, StringSlice("names")))
            var found = False
            while candidate >= 0:
                if pj_kind(tree, candidate) == JSON_STRING and pj_equal(pj_text(tree, candidate), pj_text(tree, name)):
                    found = True
                    break
                candidate = pj_next(tree, candidate)
            if not found:
                issue = 6
    if issue != 0:
        function_error(sink, tree, issue, pj_text(tree, name), limit)
        return 0
    if operation == 1:
        var measure = JsonSink(sink[].output, 0, 0, True, False)
        if not write_normalized_tool(Pointer(to=measure), tree, input, True):
            var display_name = pj_text(tree, name)
            if name < 0:
                display_name = pj_literal(StringSlice("function"))
            function_error(sink, tree, 4, display_name, limit)
            return 0
    js_literal(sink, StringSlice('{"value":'))
    if operation == 1:
        if not write_normalized_tool(sink, tree, input, True):
            return 3
    elif operation == 2 or operation == 4:
        if operation == 4:
            var direct = pj_field(tree, input, StringSlice("name"))
            if pj_kind(tree, direct) == JSON_STRING:
                name = direct
            if name >= 0 and not pj_nonblank(tree, name):
                name = -1
        if name >= 0:
            js_raw(sink, tree, name)
        else:
            js_literal(sink, StringSlice("null"))
    elif operation == 3:
        js_literal(sink, StringSlice("true") if generic_wire(js_raw_view(tree, input)) else StringSlice("false"))
    else:
        js_literal(sink, StringSlice("null"))
    js_byte(sink, 125)
    return 0


@export("prodex_mojo_deepseek_function_tools_scratch_v1")
def prodex_mojo_deepseek_function_tools_scratch_v1(
    abi: Int64, operation: Int64, flag: Int64,
    nodes_address: UInt64, count: Int64, raw_address: UInt64, raw_length: Int64,
    required_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if flag < 0 or flag > 1 or nodes_address == 0 or required_address == 0 or raw_length < 0 or count <= 0 or count > 0x7FFFFFFFFFFFFFFF // 128:
        return 1
    var tree = ParsedJson(Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](unsafe_from_address=Int(nodes_address)), count, ProdexRichStringView(UInt(raw_address), UInt(raw_length)))
    if not context_valid(tree, operation):
        return 1
    var measure = JsonSink(Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(required_address)), 0, 0, True, False)
    if operation == 0:
        var input = pj_field(tree, 0, StringSlice("input"))
        if pj_kind(tree, input) != JSON_ARRAY:
            return 1
        var tool = pj_child(tree, input)
        while tool >= 0:
            if function_issue(tree, tool, name_limit(tree), flag == 1) != 0:
                break
            if not write_normalized_tool(Pointer(to=measure), tree, tool, flag == 1 and function_name(tree, tool) >= 0):
                break
            tool = pj_next(tree, tool)
    if measure.failed or measure.written > 0x7FFFFFFFFFFFFFFF - count * 64 - 15:
        return 3
    var required = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(required_address))
    required[] = count * 4 + (measure.written + 15) // 16
    return 0


@export("prodex_mojo_deepseek_function_tools_v1")
def prodex_mojo_deepseek_function_tools_v1(
    abi: Int64, operation: Int64, flag: Int64,
    nodes_address: UInt64, count: Int64, raw_address: UInt64, raw_length: Int64,
    scratch_address: UInt64, scratch_count: Int64,
    measuring: Int64, output_address: UInt64, capacity: Int64, metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if flag < 0 or flag > 1 or nodes_address == 0 or count <= 0 or count > 0x7FFFFFFFFFFFFFFF // 128 or raw_length < 0 or scratch_address == 0 or scratch_count < count * 4 or scratch_count > 0x7FFFFFFFFFFFFFFF // 16 or metadata_address == 0 or capacity < 0 or measuring < 0 or measuring > 1:
        return 1
    if measuring == 1 and (output_address != 0 or capacity != 0) or measuring == 0 and output_address == 0:
        return 1
    var tree = ParsedJson(Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](unsafe_from_address=Int(nodes_address)), count, ProdexRichStringView(UInt(raw_address), UInt(raw_length)))
    if not context_valid(tree, operation):
        return 1
    var required: Int64 = -1
    var sizing_status = prodex_mojo_deepseek_function_tools_scratch_v1(
        abi, operation, flag, nodes_address, count, raw_address, raw_length,
        UInt64(Int(Pointer(to=required))),
    )
    if sizing_status != 0:
        return sizing_status
    if scratch_count < required:
        return 3
    var sink = JsonSink(Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)), capacity, 0, measuring == 1, False)
    var status = function_tool_operation(Pointer(to=sink), tree, operation, flag == 1, scratch_address, scratch_count)
    if status != 0:
        return status
    if sink.failed:
        return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = sink.written
    return 0
