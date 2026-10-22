from std.memory import Pointer
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_NULL, JSON_ARRAY, JSON_OBJECT, JSON_STRING, JSON_NUMBER,
    pj_child, pj_next, pj_field, pj_kind, pj_text, pj_valid, pj_is, pj_literal, pj_trim,
)
from rich_types import ProdexRichStringView
from rich_text import rich_view_matches_literal
from json_sink import JsonSink, js_byte, js_literal, js_string, js_raw, js_raw_view, js_u64
from kiro_model_catalog import kiro_catalog_positive_u64


def runtime_reasoning_description(effort: ProdexRichStringView) -> ProdexRichStringView:
    if rich_view_matches_literal["none"](effort, False):
        return pj_literal(StringSlice("No reasoning effort"))
    if rich_view_matches_literal["minimal"](effort, False):
        return pj_literal(StringSlice("Minimal reasoning effort"))
    if rich_view_matches_literal["low"](effort, False):
        return pj_literal(StringSlice("Low reasoning effort"))
    if rich_view_matches_literal["medium"](effort, False):
        return pj_literal(StringSlice("Medium reasoning effort"))
    if rich_view_matches_literal["high"](effort, False):
        return pj_literal(StringSlice("High reasoning effort"))
    if rich_view_matches_literal["xhigh"](effort, False):
        return pj_literal(StringSlice("Extra-high reasoning effort"))
    if rich_view_matches_literal["max"](effort, False):
        return pj_literal(StringSlice("Max reasoning effort"))
    if rich_view_matches_literal["ultra"](effort, False):
        return pj_literal(StringSlice("Ultra reasoning effort"))
    return pj_literal(StringSlice(""))


def runtime_put_reasoning(sink: Pointer[mut=True, JsonSink, _], effort: ProdexRichStringView):
    js_literal(sink, StringSlice('{"effort":'))
    js_string(sink, effort)
    js_literal(sink, StringSlice(',"description":'))
    js_string(sink, runtime_reasoning_description(effort))
    js_byte(sink, 125)


def runtime_catalog_unsigned(tree: ParsedJson, index: Int64) -> Bool:
    return pj_kind(tree, index) == JSON_NUMBER and (
        kiro_catalog_positive_u64(tree, index) > 0
        or rich_view_matches_literal["0"](js_raw_view(tree, index), False)
    )


def runtime_dynamic_catalog_emit(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, limit: Int64):
    var models = pj_field(tree, 0, StringSlice("models"))
    if pj_kind(tree, models) != JSON_ARRAY:
        js_literal(sink, StringSlice('{"Err":"provider model catalog is missing models array"}'))
        return
    var count: Int64 = 0
    var child = pj_child(tree, models)
    while child >= 0:
        if count >= limit:
            js_literal(sink, StringSlice('{"Err":"provider model catalog exceeds the hard limit of '))
            js_u64(sink, UInt64(limit))
            js_literal(sink, StringSlice(' entries"}'))
            return
        count += 1
        child = pj_next(tree, child)
    js_literal(sink, StringSlice('{"Ok":['))
    var first = True
    child = pj_child(tree, models)
    while child >= 0:
        var slug = pj_field(tree, child, StringSlice("id"))
        if slug < 0:
            slug = pj_field(tree, child, StringSlice("slug"))
        if slug < 0:
            slug = pj_field(tree, child, StringSlice("model"))
        var identifier = pj_trim(pj_text(tree, slug))
        if identifier.len > 0:
            var name = pj_field(tree, child, StringSlice("name"))
            if name < 0:
                name = pj_field(tree, child, StringSlice("model_name"))
            var description = pj_field(tree, child, StringSlice("description"))
            var prompt = pj_field(tree, child, StringSlice("max_prompt_tokens"))
            if prompt < 0:
                var capabilities = pj_field(tree, child, StringSlice("capabilities"))
                var limits = pj_field(tree, capabilities, StringSlice("limits"))
                prompt = pj_field(tree, limits, StringSlice("max_prompt_tokens"))
            var context = kiro_catalog_positive_u64(tree, prompt)
            if context <= 1:
                # Unsigned 0/1 token limits block the context alias before final validation.
                var tokens = pj_field(tree, child, StringSlice("context_window_tokens"))
                if not runtime_catalog_unsigned(tree, tokens):
                    tokens = pj_field(tree, child, StringSlice("context_window"))
                context = kiro_catalog_positive_u64(tree, tokens)
            if not first:
                js_byte(sink, 44)
            first = False
            js_literal(sink, StringSlice('{"slug":'))
            js_string(sink, identifier)
            js_literal(sink, StringSlice(',"display_name":'))
            if pj_kind(tree, name) == JSON_STRING:
                js_raw(sink, tree, name)
            else:
                js_literal(sink, StringSlice("null"))
            js_literal(sink, StringSlice(',"description":'))
            if pj_kind(tree, description) == JSON_STRING:
                js_raw(sink, tree, description)
            else:
                js_literal(sink, StringSlice("null"))
            js_literal(sink, StringSlice(',"context_window":'))
            if context > 1:
                js_u64(sink, context)
            else:
                js_literal(sink, StringSlice("null"))
            js_byte(sink, 125)
        child = pj_next(tree, child)
    js_literal(sink, StringSlice(']}'))


def runtime_catalog_emit(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, limit: Int64, stored_catalog: Int64) -> Bool:
    if stored_catalog == 1:
        runtime_dynamic_catalog_emit(sink, tree, limit)
        return True
    if pj_kind(tree, 0) != JSON_OBJECT:
        return False
    var provider = pj_field(tree, 0, StringSlice("provider"))
    var deepseek = pj_is["deepseek"](tree, provider)
    var gemini = pj_is["gemini"](tree, provider)
    var local = pj_is["local"](tree, provider)
    var kiro = pj_is["kiro"](tree, provider)
    var capable = gemini or pj_is["anthropic"](tree, provider) or pj_is["copilot"](tree, provider)
    if not deepseek and not kiro and not capable and not local:
        return False
    var slug = pj_field(tree, 0, StringSlice("slug"))
    var display = pj_field(tree, 0, StringSlice("display_name"))
    var description = pj_field(tree, 0, StringSlice("description"))
    if pj_kind(tree, slug) != JSON_STRING or (not local and (pj_kind(tree, display) != JSON_STRING or pj_kind(tree, description) != JSON_STRING)):
        return False
    var priority = pj_field(tree, 0, StringSlice("priority"))
    var context = pj_field(tree, 0, StringSlice("context_window"))
    var compact = pj_field(tree, 0, StringSlice("auto_compact_token_limit"))
    if (not local and not runtime_catalog_unsigned(tree, priority)) or not runtime_catalog_unsigned(tree, context) or not runtime_catalog_unsigned(tree, compact):
        return False
    var default = pj_field(tree, 0, StringSlice("default_reasoning_effort"))
    var efforts = pj_field(tree, 0, StringSlice("supported_reasoning_efforts"))
    if not local and pj_kind(tree, default) != JSON_NULL and not pj_is["unknown"](tree, default):
        if pj_kind(tree, default) != JSON_STRING or runtime_reasoning_description(pj_text(tree, default)).len == 0:
            return False
    if not local and pj_kind(tree, efforts) != JSON_NULL:
        if pj_kind(tree, efforts) != JSON_ARRAY:
            return False
        var child = pj_child(tree, efforts)
        while child >= 0:
            if not pj_is["unknown"](tree, child):
                if pj_kind(tree, child) != JSON_STRING or runtime_reasoning_description(pj_text(tree, child)).len == 0:
                    return False
            child = pj_next(tree, child)
    js_literal(sink, StringSlice('{"slug":'))
    js_raw(sink, tree, slug)
    js_literal(sink, StringSlice(',"display_name":'))
    js_raw(sink, tree, slug if local else display)
    js_literal(sink, StringSlice(',"description":'))
    if local:
        js_string(sink, pj_literal(StringSlice("Local OpenAI-compatible model routed through Prodex.")))
    else:
        js_raw(sink, tree, description)
    js_literal(sink, StringSlice(',"default_reasoning_level":'))
    if deepseek or gemini or local or pj_kind(tree, default) == JSON_NULL or pj_is["unknown"](tree, default):
        js_literal(sink, StringSlice('"high"'))
    else:
        js_raw(sink, tree, default)
    js_literal(sink, StringSlice(',"supported_reasoning_levels":['))
    if deepseek:
        js_literal(sink, StringSlice(
            '{"effort":"low","description":"DeepSeek low reasoning effort"},'
            '{"effort":"medium","description":"DeepSeek medium reasoning effort"},'
            '{"effort":"high","description":"DeepSeek high reasoning effort"},'
            '{"effort":"xhigh","description":"DeepSeek max reasoning effort"}'
        ))
    elif gemini:
        js_literal(sink, StringSlice(
            '{"effort":"minimal","description":"Disable Gemini thinking budget where supported"},'
            '{"effort":"low","description":"Small Gemini thinking budget"},'
            '{"effort":"medium","description":"Default Gemini thinking budget"},'
            '{"effort":"high","description":"High Gemini thinking budget"},'
            '{"effort":"xhigh","description":"Maximum Gemini thinking effort"}'
        ))
    elif local or pj_kind(tree, efforts) == JSON_NULL:
        runtime_put_reasoning(sink, pj_literal(StringSlice("low")))
        js_byte(sink, 44)
        runtime_put_reasoning(sink, pj_literal(StringSlice("medium")))
        js_byte(sink, 44)
        runtime_put_reasoning(sink, pj_literal(StringSlice("high")))
        js_byte(sink, 44)
        if local:
            js_literal(sink, StringSlice('{"effort":"xhigh","description":"Max reasoning effort"}'))
        else:
            runtime_put_reasoning(sink, pj_literal(StringSlice("xhigh")))
    else:
        var child = pj_child(tree, efforts)
        var first = True
        while child >= 0:
            if not pj_is["unknown"](tree, child):
                if not first:
                    js_byte(sink, 44)
                first = False
                runtime_put_reasoning(sink, pj_text(tree, child))
            child = pj_next(tree, child)
    js_literal(sink, StringSlice(
        '],"shell_type":"shell_command","visibility":"list","supported_in_api":true,"priority":'
    ))
    if local:
        js_u64(sink, UInt64(1))
    else:
        js_raw(sink, tree, priority)
    js_literal(sink, StringSlice(
        ',"additional_speed_tiers":[],"service_tiers":[],"default_service_tier":null,'
        '"availability_nux":null,"upgrade":null,"base_instructions":'
    ))
    if deepseek or gemini:
        js_string(sink, pj_literal(StringSlice(
            "You are Codex, a coding agent. You and the user share the same workspace.\n\n"
            "Focus on the user's software task. Inspect the codebase before changing behavior, make narrow edits, preserve user changes, and verify with relevant tests or commands when feasible.\n\n"
            "Use tools deliberately. For shell work, prefer fast focused commands. For file edits, keep changes minimal and explain non-obvious logic in short comments only when useful."
        )))
    else:
        # Local preserves an explicit empty instruction source for persisted-session prompts.
        js_literal(sink, StringSlice('""'))
    var capability = StringSlice("true") if capable else StringSlice("false")
    js_literal(sink, StringSlice(',"supports_reasoning_summaries":'))
    js_literal(sink, capability)
    js_literal(sink, StringSlice(',"supports_reasoning_summary_parameter":'))
    js_literal(sink, capability)
    js_literal(sink, StringSlice(
        ',"default_reasoning_summary":"none","support_verbosity":false,"default_verbosity":null,'
        '"apply_patch_tool_type":"freeform","web_search_tool_type":"text",'
        '"truncation_policy":{"mode":"tokens","limit":10000},"supports_parallel_tool_calls":true,'
        '"supports_image_detail_original":false,"context_window":'
    ))
    js_raw(sink, tree, context)
    js_literal(sink, StringSlice(',"max_context_window":'))
    js_raw(sink, tree, context)
    js_literal(sink, StringSlice(',"auto_compact_token_limit":'))
    js_raw(sink, tree, compact)
    js_literal(sink, StringSlice(',"effective_context_window_percent":95,"experimental_supported_tools":[],"input_modalities":'))
    js_literal(sink, StringSlice('["text","image"]') if capable else StringSlice('["text"]'))
    js_literal(sink, StringSlice(',"supports_search_tool":'))
    js_literal(sink, StringSlice("true") if local else capability)
    js_byte(sink, 125)
    return True


# Mode 0 renders one schema; mode 1 projects stored entries with a raw-entry bound.
@export("prodex_mojo_runtime_model_catalog_v1")
def prodex_mojo_runtime_model_catalog_v1(
    abi: Int64, max_entries: Int64, stored_catalog: Int64,
    nodes_address: UInt64, nodes_count: Int64, raw_address: UInt64, raw_length: Int64,
    scratch_address: UInt64, scratch_count: Int64,
    measuring: Int64, output_address: UInt64, capacity: Int64, metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (
        (stored_catalog == 0 and max_entries != 0) or (stored_catalog == 1 and max_entries < 0)
        or stored_catalog < 0 or stored_catalog > 1 or nodes_address == 0 or nodes_address % 8 != 0
        or nodes_count <= 0 or nodes_count > 0x7FFFFFFFFFFFFFFF // 80
        or raw_address == 0 or raw_length <= 0 or scratch_address == 0 or scratch_address % 8 != 0
        or scratch_count < nodes_count or measuring < 0 or measuring > 1 or capacity < 0
        or metadata_address == 0 or metadata_address % 8 != 0
    ):
        return 1
    if measuring == 1:
        if output_address != 0 or capacity != 0:
            return 1
    elif output_address == 0:
        return 1
    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](unsafe_from_address=Int(nodes_address)),
        nodes_count, ProdexRichStringView(UInt(raw_address), UInt(raw_length)),
    )
    if not pj_valid(tree):
        return 1
    var sink = JsonSink(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(scratch_address)),
        0, 0, True, False,
    )
    if not runtime_catalog_emit(Pointer(to=sink), tree, max_entries, stored_catalog):
        return 1
    if sink.failed:
        return 3
    var required = sink.written
    if measuring == 0:
        if capacity < required:
            return 3
        sink = JsonSink(
            Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)),
            capacity, 0, False, False,
        )
        if not runtime_catalog_emit(Pointer(to=sink), tree, max_entries, stored_catalog) or sink.failed or sink.written != required:
            return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = required
    return 0
