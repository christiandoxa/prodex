# Gemini 3 declaration descriptions share the existing name/model policy.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_ARRAY, JSON_NULL, JSON_OBJECT, JSON_STRING,
    pj_valid, pj_kind, pj_field, pj_child, pj_next, pj_text, pj_equal, pj_literal,
)
from json_sink import JsonSink, js_byte, js_literal, js_string, js_escaped, js_raw, js_u64
from gemini_tooling_policy import (
    tooling_name_policy, tooling_model_uses_gemini3,
    tooling_output_range_equals,
    TOOL_ALIAS_READ_FILE, TOOL_ALIAS_READ_MANY_FILES, TOOL_ALIAS_GREP,
    TOOL_ALIAS_EXEC_COMMAND, TOOL_ALIAS_WRITE_FILE, TOOL_ALIAS_APPLY_PATCH,
)


def description_kind(name: ProdexRichStringView, scratch: UInt64, capacity: Int64) -> Int64:
    var normalized = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(scratch))
    var plan = tooling_name_policy(name, normalized, capacity)
    if plan.status != 0:
        return -1
    var written = plan.written
    var suffix = plan.suffix_start
    var mask = plan.alias_mask
    if mask & TOOL_ALIAS_READ_FILE != 0 or tooling_output_range_equals["read_file"](normalized, suffix, written):
        return 1
    if mask & TOOL_ALIAS_READ_MANY_FILES != 0 or tooling_output_range_equals["read_many_files"](normalized, suffix, written):
        return 2
    if mask & TOOL_ALIAS_GREP != 0 or tooling_output_range_equals["grep"](normalized, suffix, written) or tooling_output_range_equals["rg"](normalized, suffix, written) or tooling_output_range_equals["rip_grep"](normalized, suffix, written):
        return 3
    if mask & TOOL_ALIAS_EXEC_COMMAND != 0 or tooling_output_range_equals["exec_command"](normalized, suffix, written) or tooling_output_range_equals["run_shell_command"](normalized, suffix, written):
        return 4
    if mask & TOOL_ALIAS_WRITE_FILE != 0 or tooling_output_range_equals["write_file"](normalized, suffix, written):
        return 5
    if mask & TOOL_ALIAS_APPLY_PATCH != 0 or tooling_output_range_equals["apply_patch"](normalized, suffix, written) or tooling_output_range_equals["replace"](normalized, suffix, written) or tooling_output_range_equals["edit"](normalized, suffix, written):
        return 6
    return 0


def description_text(kind: Int64) -> ProdexRichStringView:
    if kind == 1:
        return pj_literal(StringSlice("Read a file with targeted, surgical ranges. Prefer start_line and end_line when only part of the file is needed."))
    if kind == 2:
        return pj_literal(StringSlice("Read multiple files by paths or globs; honor git, Gemini, custom ignore rules, and default heavy-directory excludes."))
    if kind == 3:
        return pj_literal(StringSlice("Search text fast with ripgrep-style behavior. Use this before broad reads when locating symbols or literals."))
    if kind == 4:
        return pj_literal(StringSlice("Run a shell command. Prefer non-interactive commands and continue background sessions until the needed output is available."))
    if kind == 5:
        return pj_literal(StringSlice("Write complete file content. Do not use placeholders; preserve unrelated user changes."))
    return pj_literal(StringSlice("Apply targeted file edits with exact context. Keep edits narrow and use this instead of describing changes. For Add File patches, every content line must start with '+', for example: *** Begin Patch\\n*** Add File: note.txt\\n+hello\\n*** End Patch."))


def parameter_text(kind: Int64, key: ProdexRichStringView) -> ProdexRichStringView:
    if kind == 1:
        if pj_equal(key, pj_literal(StringSlice("start_line"))):
            return pj_literal(StringSlice("1-based first line to read when a targeted range is enough."))
        if pj_equal(key, pj_literal(StringSlice("end_line"))):
            return pj_literal(StringSlice("1-based last line to read, inclusive."))
    if kind == 3 and pj_equal(key, pj_literal(StringSlice("pattern"))):
        return pj_literal(StringSlice("Literal or regex pattern to search for."))
    return ProdexRichStringView(0, 0)


def write_patch_parameters(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, node: Int64, depth: Int64,
) -> Bool:
    if node >= 0 and pj_kind(tree, node) != JSON_NULL and pj_kind(tree, node) != JSON_OBJECT:
        return False
    var key = StringSlice("properties") if depth == 0 else StringSlice("input") if depth == 1 else StringSlice("description")
    var target = pj_field(tree, node, key)
    js_byte(sink, 123)
    var child = pj_child(tree, node)
    var emitted = False
    while child >= 0:
        if child != target:
            if emitted:
                js_byte(sink, 44)
            js_byte(sink, 34)
            js_escaped(sink, tree.nodes[unsafe_offset=child].key)
            js_literal(sink, StringSlice('\":'))
            js_raw(sink, tree, child)
            emitted = True
        child = pj_next(tree, child)
    if emitted:
        js_byte(sink, 44)
    js_string(sink, pj_literal(key))
    js_byte(sink, 58)
    if depth == 2:
        js_string(sink, pj_literal(StringSlice("Full apply_patch grammar string. For Add File, prefix every new file content line with '+'.")))
    elif not write_patch_parameters(sink, tree, target, depth + 1):
        return False
    js_byte(sink, 125)
    return True


def write_parameter_property(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, node: Int64,
    description: ProdexRichStringView,
):
    if pj_kind(tree, node) != JSON_OBJECT or pj_field(tree, node, StringSlice("description")) >= 0:
        js_raw(sink, tree, node)
        return
    js_byte(sink, 123)
    var field = pj_child(tree, node)
    while field >= 0:
        js_byte(sink, 34)
        js_escaped(sink, tree.nodes[unsafe_offset=field].key)
        js_literal(sink, StringSlice('\":'))
        js_raw(sink, tree, field)
        js_byte(sink, 44)
        field = pj_next(tree, field)
    js_literal(sink, StringSlice('\"description\":'))
    js_string(sink, description)
    js_byte(sink, 125)


def write_existing_parameters(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, node: Int64, kind: Int64, depth: Int64,
):
    if pj_kind(tree, node) != JSON_OBJECT:
        js_raw(sink, tree, node)
        return
    js_byte(sink, 123)
    var field = pj_child(tree, node)
    var emitted = False
    while field >= 0:
        if emitted:
            js_byte(sink, 44)
        var key = tree.nodes[unsafe_offset=field].key.copy()
        js_byte(sink, 34)
        js_escaped(sink, key)
        js_literal(sink, StringSlice('\":'))
        var description = parameter_text(kind, key)
        if depth == 0 and pj_equal(key, pj_literal(StringSlice("properties"))):
            write_existing_parameters(sink, tree, field, kind, 1)
        elif depth == 1 and description.len > 0:
            write_parameter_property(sink, tree, field, description)
        else:
            js_raw(sink, tree, field)
        emitted = True
        field = pj_next(tree, field)
    js_byte(sink, 125)


def write_declaration(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, node: Int64,
    scratch: UInt64, capacity: Int64,
) -> Bool:
    var name = pj_field(tree, node, StringSlice("name"))
    var kind: Int64 = 0
    if pj_kind(tree, name) == JSON_STRING:
        kind = description_kind(pj_text(tree, name), scratch, capacity)
    if kind < 0:
        return False
    if kind == 0:
        js_raw(sink, tree, node)
        return True
    js_byte(sink, 123)
    var field = pj_child(tree, node)
    var emitted = False
    var parameters = pj_field(tree, node, StringSlice("parameters"))
    while field >= 0:
        if not pj_equal(tree.nodes[unsafe_offset=field].key, pj_literal(StringSlice("description"))):
            if emitted:
                js_byte(sink, 44)
            js_byte(sink, 34)
            js_escaped(sink, tree.nodes[unsafe_offset=field].key)
            js_literal(sink, StringSlice('\":'))
            if field == parameters and kind == 6:
                if not write_patch_parameters(sink, tree, field, 0):
                    return False
            elif field == parameters and (kind == 1 or kind == 3):
                write_existing_parameters(sink, tree, field, kind, 0)
            else:
                js_raw(sink, tree, field)
            emitted = True
        field = pj_next(tree, field)
    if emitted:
        js_byte(sink, 44)
    if kind == 6 and parameters < 0:
        js_literal(sink, StringSlice('\"parameters\":'))
        if not write_patch_parameters(sink, tree, -1, 0):
            return False
        js_byte(sink, 44)
    js_literal(sink, StringSlice('\"description\":'))
    js_string(sink, description_text(kind))
    js_byte(sink, 125)
    return True


@export("prodex_mojo_gemini_tool_descriptions_v1")
def prodex_mojo_gemini_tool_descriptions_v1(
    abi: Int64, operation: Int64, flag: Int64,
    nodes_address: UInt64, count: Int64, raw_address: UInt64, raw_length: Int64,
    scratch_address: UInt64, scratch_count: Int64,
    measuring: Int64, output_address: UInt64, capacity: Int64, metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if operation < 0 or operation > 2 or flag != 0 or nodes_address == 0 or count <= 0 or raw_length < 0 or scratch_address == 0 or scratch_count <= 0 or scratch_count > 0x7FFFFFFFFFFFFFFF // 16 or raw_length > scratch_count * 16 or metadata_address == 0 or capacity < 0 or measuring < 0 or measuring > 1:
        return 1
    if measuring == 1 and (output_address != 0 or capacity != 0) or measuring == 0 and output_address == 0:
        return 1
    var tree = ParsedJson(Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](unsafe_from_address=Int(nodes_address)), count, ProdexRichStringView(UInt(raw_address), UInt(raw_length)))
    if not pj_valid(tree):
        return 1
    var sink = JsonSink(Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)), capacity, 0, measuring == 1, False)
    if operation == 0:
        js_byte(Pointer(to=sink), 91)
        for kind in range(1, 7):
            if kind > 1:
                js_byte(Pointer(to=sink), 44)
            js_string(Pointer(to=sink), description_text(Int64(kind)))
        js_byte(Pointer(to=sink), 93)
    elif operation == 1:
        if pj_kind(tree, 0) != JSON_STRING:
            return 1
        var kind = description_kind(pj_text(tree, 0), scratch_address, scratch_count * 16)
        if kind < 0:
            return 1
        if kind == 0:
            js_literal(Pointer(to=sink), StringSlice("null"))
        else:
            js_u64(Pointer(to=sink), UInt64(kind - 1))
    else:
        var model = pj_field(tree, 0, StringSlice("model"))
        var declarations = pj_field(tree, 0, StringSlice("declarations"))
        if pj_kind(tree, model) != JSON_STRING or pj_kind(tree, declarations) != JSON_ARRAY:
            return 1
        var model_view = pj_text(tree, model)
        if not tooling_model_uses_gemini3(model_view):
            js_raw(Pointer(to=sink), tree, declarations)
        else:
            js_byte(Pointer(to=sink), 91)
            var declaration = pj_child(tree, declarations)
            var emitted = False
            while declaration >= 0:
                if emitted:
                    js_byte(Pointer(to=sink), 44)
                if not write_declaration(Pointer(to=sink), tree, declaration, scratch_address, scratch_count * 16):
                    return 1
                emitted = True
                declaration = pj_next(tree, declaration)
            js_byte(Pointer(to=sink), 93)
    if sink.failed:
        return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = sink.written
    return 0
