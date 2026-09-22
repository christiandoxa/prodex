from std.memory import Pointer
from rich_types import ProdexRichStringView
from parsed_json import (
    ParsedJson,
    JSON_STRING,
    JSON_ARRAY,
    JSON_OBJECT,
    pj_kind,
    pj_child,
    pj_next,
    pj_text,
    pj_field,
    pj_string_field,
    pj_is,
    pj_nonblank,
)
from json_sink import JsonSink, js_byte, js_literal, js_string, js_escaped, js_raw_view

def ocr_text_defined(tree: ParsedJson, index: Int64) -> Bool:
    if pj_kind(tree, index) == JSON_STRING:
        return True
    if pj_kind(tree, index) == JSON_ARRAY:
        var item = pj_child(tree, index)
        while item >= 0:
            var text = pj_field(tree, item, StringSlice("text"))
            if pj_kind(tree, text) == JSON_STRING and pj_text(tree, text).len > 0:
                return True
            var content = pj_field(tree, item, StringSlice("content"))
            if pj_kind(tree, content) == JSON_STRING and pj_text(tree, content).len > 0:
                return True
            item = pj_next(tree, item)
        return False
    if pj_kind(tree, index) == JSON_OBJECT:
        if pj_string_field(tree, index, StringSlice("text")) >= 0:
            return True
        var content = pj_field(tree, index, StringSlice("content"))
        if content >= 0 and ocr_text_defined(tree, content):
            return True
        return pj_string_field(tree, index, StringSlice("output_text")) >= 0
    return False


def ocr_text_exists(tree: ParsedJson, index: Int64) -> Bool:
    if pj_kind(tree, index) == JSON_STRING:
        return pj_text(tree, index).len > 0
    if pj_kind(tree, index) == JSON_ARRAY:
        return ocr_text_defined(tree, index)
    if pj_kind(tree, index) == JSON_OBJECT:
        var text = pj_string_field(tree, index, StringSlice("text"))
        if text >= 0:
            return pj_text(tree, text).len > 0
        var content = pj_field(tree, index, StringSlice("content"))
        if content >= 0 and ocr_text_defined(tree, content):
            return ocr_text_exists(tree, content)
        var output_text = pj_string_field(tree, index, StringSlice("output_text"))
        return output_text >= 0 and pj_text(tree, output_text).len > 0
    return False


def ocr_write_text(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, index: Int64):
    if pj_kind(tree, index) == JSON_STRING:
        js_escaped(sink, pj_text(tree, index))
        return
    if pj_kind(tree, index) == JSON_ARRAY:
        var item = pj_child(tree, index)
        var wrote = False
        while item >= 0:
            var field = pj_field(tree, item, StringSlice("text"))
            if pj_kind(tree, field) != JSON_STRING:
                field = pj_field(tree, item, StringSlice("content"))
            if pj_kind(tree, field) == JSON_STRING and pj_text(tree, field).len > 0:
                if wrote:
                    js_literal(sink, StringSlice("\\n"))
                js_escaped(sink, pj_text(tree, field))
                wrote = True
            item = pj_next(tree, item)
        return
    if pj_kind(tree, index) == JSON_OBJECT:
        var text = pj_string_field(tree, index, StringSlice("text"))
        if text >= 0:
            js_escaped(sink, pj_text(tree, text))
            return
        var content = pj_field(tree, index, StringSlice("content"))
        if content >= 0 and ocr_text_defined(tree, content):
            ocr_write_text(sink, tree, content)
            return
        var output_text = pj_string_field(tree, index, StringSlice("output_text"))
        if output_text >= 0:
            js_escaped(sink, pj_text(tree, output_text))


def ocr_write_message(
    sink: Pointer[mut=True, JsonSink, _],
    role: ProdexRichStringView,
    tree: ParsedJson,
    text_index: Int64,
):
    js_literal(sink, StringSlice('{"content":"'))
    ocr_write_text(sink, tree, text_index)
    js_literal(sink, StringSlice('","role":'))
    js_string(sink, role)
    js_byte(sink, 125)


def ocr_write_literal_role_message[role: StaticString](
    sink: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson,
    text_index: Int64,
):
    js_literal(sink, StringSlice('{"content":"'))
    ocr_write_text(sink, tree, text_index)
    js_literal(sink, StringSlice('","role":"'))
    js_literal(sink, StringSlice(role))
    js_literal(sink, StringSlice('"}'))


def ocr_first_string_field3(
    tree: ParsedJson, object: Int64,
    first: StringSlice, second: StringSlice, third: StringSlice,
) -> Int64:
    var result = pj_field(tree, object, first)
    if result < 0:
        result = pj_field(tree, object, second)
    if result < 0:
        result = pj_field(tree, object, third)
    return result if pj_kind(tree, result) == JSON_STRING else -1


def ocr_first_field4(
    tree: ParsedJson, object: Int64,
    first: StringSlice, second: StringSlice, third: StringSlice, fourth: StringSlice,
) -> Int64:
    var result = pj_field(tree, object, first)
    if result < 0:
        result = pj_field(tree, object, second)
    if result < 0:
        result = pj_field(tree, object, third)
    if result < 0:
        result = pj_field(tree, object, fourth)
    return result


def ocr_write_json_value_as_string(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, index: Int64
):
    js_byte(sink, 34)
    if pj_kind(tree, index) == JSON_STRING:
        js_escaped(sink, pj_text(tree, index))
    elif index >= 0:
        js_escaped(sink, js_raw_view(tree, index))
    js_byte(sink, 34)


def ocr_function_call_name(tree: ParsedJson, object: Int64) -> Int64:
    var name = pj_field(tree, object, StringSlice("name"))
    if name < 0:
        name = pj_field(tree, object, StringSlice("tool_name"))
    if name < 0:
        name = pj_field(
            tree, pj_field(tree, object, StringSlice("function")), StringSlice("name")
        )
    return name if pj_kind(tree, name) == JSON_STRING else -1


def ocr_write_function_call(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, object: Int64
) -> Bool:
    var call_id = ocr_first_string_field3(
        tree, object, StringSlice("call_id"), StringSlice("tool_call_id"), StringSlice("id")
    )
    var name = ocr_function_call_name(tree, object)
    if not pj_nonblank(tree, name):
        return False
    var namespace = pj_string_field(tree, object, StringSlice("namespace"))
    var arguments = pj_field(tree, object, StringSlice("arguments"))
    if arguments < 0:
        arguments = pj_field(tree, object, StringSlice("input"))
    if arguments < 0:
        arguments = pj_field(
            tree, pj_field(tree, object, StringSlice("function")), StringSlice("arguments")
        )
    js_literal(sink, StringSlice('{"content":"","role":"assistant","tool_calls":[{"function":{"arguments":'))
    if arguments < 0:
        js_literal(sink, StringSlice('"{}"'))
    else:
        ocr_write_json_value_as_string(sink, tree, arguments)
    js_literal(sink, StringSlice(',"name":"'))
    if pj_nonblank(tree, namespace):
        js_escaped(sink, pj_text(tree, namespace))
        js_byte(sink, 46)
    js_escaped(sink, pj_text(tree, name))
    js_literal(sink, StringSlice('"},"id":'))
    if call_id >= 0:
        js_string(sink, pj_text(tree, call_id))
    else:
        js_literal(sink, StringSlice('"call_1"'))
    js_literal(sink, StringSlice(',"type":"function"}]}'))
    return True


def ocr_write_function_output(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, object: Int64
) -> Bool:
    var call_id = ocr_first_string_field3(
        tree, object, StringSlice("call_id"), StringSlice("tool_call_id"), StringSlice("id")
    )
    if not pj_nonblank(tree, call_id):
        return False
    var content = ocr_first_field4(
        tree, object, StringSlice("output"), StringSlice("content"),
        StringSlice("result"), StringSlice("error")
    )
    js_literal(sink, StringSlice('{"content":'))
    if content < 0:
        js_literal(sink, StringSlice('""'))
    else:
        ocr_write_json_value_as_string(sink, tree, content)
    js_literal(sink, StringSlice(',"role":"tool","tool_call_id":'))
    js_string(sink, pj_text(tree, call_id))
    js_byte(sink, 125)
    return True


def ocr_input_item_will_emit(tree: ParsedJson, item: Int64) -> Bool:
    if pj_kind(tree, item) == JSON_STRING:
        return ocr_text_exists(tree, item)
    if pj_kind(tree, item) != JSON_OBJECT:
        return False
    var kind = pj_field(tree, item, StringSlice("type"))
    if pj_is["function_call"](tree, kind):
        return pj_nonblank(tree, ocr_function_call_name(tree, item))
    if pj_is["function_call_output"](tree, kind):
        var call_id = ocr_first_string_field3(
            tree, item, StringSlice("call_id"), StringSlice("tool_call_id"), StringSlice("id")
        )
        return pj_nonblank(tree, call_id)
    if kind < 0 or pj_is["message"](tree, kind):
        var content = pj_field(tree, item, StringSlice("content"))
        if ocr_text_defined(tree, content):
            return ocr_text_exists(tree, content)
        var text = pj_string_field(tree, item, StringSlice("text"))
        return pj_kind(tree, text) == JSON_STRING and pj_text(tree, text).len > 0
    if pj_is["input_text"](tree, kind) or pj_is["output_text"](tree, kind):
        return pj_kind(tree, pj_string_field(tree, item, StringSlice("text"))) == JSON_STRING and pj_text(tree, pj_string_field(tree, item, StringSlice("text"))).len > 0
    return False


def ocr_write_input_item(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, item: Int64
) -> Bool:
    if pj_kind(tree, item) == JSON_STRING:
        if not ocr_text_exists(tree, item):
            return False
        ocr_write_literal_role_message["user"](sink, tree, item)
        return True
    if pj_kind(tree, item) != JSON_OBJECT:
        return False
    var kind = pj_field(tree, item, StringSlice("type"))
    if pj_is["function_call"](tree, kind):
        return ocr_write_function_call(sink, tree, item)
    if pj_is["function_call_output"](tree, kind):
        return ocr_write_function_output(sink, tree, item)
    if kind < 0 or pj_is["message"](tree, kind):
        var role = pj_string_field(tree, item, StringSlice("role"))
        var content = pj_field(tree, item, StringSlice("content"))
        if not ocr_text_defined(tree, content):
            content = pj_string_field(tree, item, StringSlice("text"))
        if not ocr_text_exists(tree, content):
            return False
        if role >= 0:
            ocr_write_message(sink, pj_text(tree, role), tree, content)
        else:
            ocr_write_literal_role_message["user"](sink, tree, content)
        return True
    if pj_is["input_text"](tree, kind):
        var text = pj_string_field(tree, item, StringSlice("text"))
        if pj_kind(tree, text) != JSON_STRING or pj_text(tree, text).len == 0:
            return False
        ocr_write_literal_role_message["user"](sink, tree, text)
        return True
    if pj_is["output_text"](tree, kind):
        var text = pj_string_field(tree, item, StringSlice("text"))
        if pj_kind(tree, text) != JSON_STRING or pj_text(tree, text).len == 0:
            return False
        ocr_write_literal_role_message["assistant"](sink, tree, text)
        return True
    return False


def ocr_message_count(tree: ParsedJson, request: Int64) -> Int64:
    var count: Int64 = 0
    var instructions = pj_field(tree, request, StringSlice("instructions"))
    if ocr_text_exists(tree, instructions):
        count += 1
    var input = pj_field(tree, request, StringSlice("input"))
    if ocr_text_exists(tree, input):
        return count + 1
    if pj_kind(tree, input) == JSON_ARRAY:
        var item = pj_child(tree, input)
        while item >= 0:
            if ocr_input_item_will_emit(tree, item):
                count += 1
            item = pj_next(tree, item)
    elif pj_kind(tree, input) == JSON_OBJECT and ocr_input_item_will_emit(tree, input):
        count += 1
    return count


def ocr_write_messages(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, request: Int64
):
    var count: Int64 = 0
    js_byte(sink, 91)
    var instructions = pj_field(tree, request, StringSlice("instructions"))
    if ocr_text_exists(tree, instructions):
        ocr_write_literal_role_message["system"](sink, tree, instructions)
        count += 1
    var input = pj_field(tree, request, StringSlice("input"))
    if ocr_text_exists(tree, input):
        if count > 0:
            js_byte(sink, 44)
        ocr_write_literal_role_message["user"](sink, tree, input)
        count += 1
    elif pj_kind(tree, input) == JSON_ARRAY:
        var item = pj_child(tree, input)
        while item >= 0:
            if ocr_input_item_will_emit(tree, item):
                if count > 0:
                    js_byte(sink, 44)
                if ocr_write_input_item(sink, tree, item):
                    count += 1
            item = pj_next(tree, item)
    elif pj_kind(tree, input) == JSON_OBJECT and ocr_input_item_will_emit(tree, input):
        if count > 0:
            js_byte(sink, 44)
        if ocr_write_input_item(sink, tree, input):
            count += 1
    js_byte(sink, 93)
