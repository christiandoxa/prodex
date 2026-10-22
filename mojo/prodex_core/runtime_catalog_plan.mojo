from std.memory import Pointer
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_OBJECT, JSON_ARRAY, JSON_STRING, JSON_NUMBER, JSON_NULL,
    pj_field, pj_kind, pj_child, pj_next, pj_text, pj_trim, pj_is, pj_literal, pj_valid,
)
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid
from rich_catalog import (
    catalog_find, catalog_find_exact, catalog_merge_additional, RICH_STATUS_OK, CATALOG_MAX_MODELS, CATALOG_MAX_IDENTIFIER_BYTES,
    CATALOG_MAX_INPUT_MODELS,
)
from provider_constraints import provider_copilot_prompt_token_limit
from super_provider_config import config_saturating_mul_u64
from runtime_model_catalog import runtime_catalog_unsigned
from kiro_model_catalog import kiro_catalog_positive_u64
from json_sink import JsonSink, js_literal, js_byte, js_u64, js_string


@fieldwise_init
struct RuntimeCatalogWindow(Copyable):
    var present: Bool
    var tokens: UInt64


@fieldwise_init
struct RuntimeCatalogPlan(Copyable):
    var ids: Pointer[mut=True, ProdexRichStringView, MutUntrackedOrigin]
    var sources: Pointer[mut=True, Int64, MutUntrackedOrigin]
    var aliases: Pointer[mut=True, ProdexRichStringView, MutUntrackedOrigin]
    var alias_owners: Pointer[mut=True, Int64, MutUntrackedOrigin]
    var dynamic_count: Int64
    var static_count: Int64
    var alias_count: Int64


def runtime_catalog_window_valid(tree: ParsedJson, index: Int64) -> Bool:
    return index < 0 or pj_kind(tree, index) == JSON_NULL or runtime_catalog_unsigned(tree, index)


def runtime_catalog_window(tree: ParsedJson, index: Int64) -> RuntimeCatalogWindow:
    return RuntimeCatalogWindow(pj_kind(tree, index) == JSON_NUMBER, kiro_catalog_positive_u64(tree, index))


def runtime_catalog_static_node(plan: RuntimeCatalogPlan, query: ProdexRichStringView) -> Int64:
    if pj_trim(query).len > UInt(CATALOG_MAX_IDENTIFIER_BYTES):
        return -1
    var static_start = 1 + plan.dynamic_count
    var index = catalog_find(
        Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(plan.ids) + Int(static_start * 16)),
        plan.static_count,
        Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(plan.aliases)),
        Pointer[mut=False, Int64, ImmUntrackedOrigin](unsafe_from_address=Int(plan.alias_owners)),
        plan.alias_count, query,
    )
    return plan.sources[unsafe_offset=static_start + index] if index >= 0 else -1


def runtime_catalog_dynamic_node(plan: RuntimeCatalogPlan, query: ProdexRichStringView) -> Int64:
    if query.len > UInt(CATALOG_MAX_IDENTIFIER_BYTES):
        return -1
    var index = catalog_find_exact(
        Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(plan.ids) + 16),
        plan.dynamic_count,
        Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(plan.aliases)),
        Pointer[mut=False, Int64, ImmUntrackedOrigin](unsafe_from_address=Int(plan.alias_owners)),
        0, query,
    )
    return plan.sources[unsafe_offset=1 + index] if index >= 0 else -1


def runtime_catalog_prompt_window(tree: ParsedJson, plan: RuntimeCatalogPlan, query: ProdexRichStringView, copilot: Bool) -> RuntimeCatalogWindow:
    if not copilot:
        return RuntimeCatalogWindow(False, 0)
    var scalar = provider_copilot_prompt_token_limit(query)
    if scalar > 0:
        return RuntimeCatalogWindow(True, UInt64(scalar))
    return runtime_catalog_window(tree, pj_field(tree, runtime_catalog_static_node(plan, query), StringSlice("context_window_tokens")))


def runtime_catalog_candidate_window(tree: ParsedJson, plan: RuntimeCatalogPlan, position: Int64, copilot: Bool) -> RuntimeCatalogWindow:
    var query = plan.ids[unsafe_offset=position].copy()
    var selected: RuntimeCatalogWindow
    if position == 0:
        selected = runtime_catalog_window(tree, pj_field(tree, runtime_catalog_dynamic_node(plan, query), StringSlice("context_window")))
        if not selected.present:
            selected = runtime_catalog_prompt_window(tree, plan, query, copilot)
        if not selected.present:
            selected = runtime_catalog_window(tree, pj_field(tree, runtime_catalog_static_node(plan, query), StringSlice("context_window_tokens")))
    elif position <= plan.dynamic_count:
        selected = runtime_catalog_window(tree, pj_field(tree, plan.sources[unsafe_offset=position], StringSlice("context_window")))
        if not selected.present:
            selected = runtime_catalog_prompt_window(tree, plan, query, copilot)
    else:
        selected = runtime_catalog_prompt_window(tree, plan, query, copilot)
        if not selected.present:
            selected = runtime_catalog_window(tree, pj_field(tree, plan.sources[unsafe_offset=position], StringSlice("context_window_tokens")))
    return selected^


def runtime_catalog_build_emit(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, arena: UInt64, limit: Int64) -> Bool:
    var provider = pj_field(tree, 0, StringSlice("provider"))
    var copilot = pj_is["copilot"](tree, provider)
    var deepseek = pj_is["deepseek"](tree, provider)
    if not deepseek and not copilot and not pj_is["kiro"](tree, provider) and not pj_is["anthropic"](tree, provider):
        return False
    var launch = pj_field(tree, 0, StringSlice("launch_model"))
    var default_context = pj_field(tree, 0, StringSlice("context_window"))
    var default_compact = pj_field(tree, 0, StringSlice("auto_compact_token_limit"))
    var dynamic = pj_field(tree, 0, StringSlice("dynamic_models"))
    var bundled_root = pj_field(tree, 0, StringSlice("static_models"))
    if (
        pj_kind(tree, launch) != JSON_STRING
        or not runtime_catalog_unsigned(tree, default_context)
        or not runtime_catalog_unsigned(tree, default_compact) or pj_kind(tree, dynamic) != JSON_ARRAY
        or pj_kind(tree, bundled_root) != JSON_ARRAY
    ):
        return False
    # Four 16-byte units per node hold the seven view/index word ranges.
    var ids = Pointer[mut=True, ProdexRichStringView, MutUntrackedOrigin](unsafe_from_address=Int(arena))
    var sources = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(arena) + Int(tree.count * 16))
    var accepted = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(arena) + Int(tree.count * 24))
    var aliases = Pointer[mut=True, ProdexRichStringView, MutUntrackedOrigin](unsafe_from_address=Int(arena) + Int(tree.count * 32))
    var alias_owners = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(arena) + Int(tree.count * 48))
    ids[unsafe_offset=0] = pj_text(tree, launch)
    sources[unsafe_offset=0] = -1
    var count: Int64 = 1
    var child = pj_child(tree, dynamic)
    while child >= 0:
        var slug = pj_field(tree, child, StringSlice("slug"))
        if count >= tree.count or pj_kind(tree, slug) != JSON_STRING:
            return False
        var identifier = pj_text(tree, slug)
        if identifier.len == 0 or not rich_view_valid(identifier, CATALOG_MAX_IDENTIFIER_BYTES):
            return False
        if not runtime_catalog_window_valid(tree, pj_field(tree, child, StringSlice("context_window"))):
            return False
        for key in [StringSlice("display_name"), StringSlice("description")]:
            var field = pj_field(tree, child, key)
            if field >= 0 and pj_kind(tree, field) != JSON_NULL and pj_kind(tree, field) != JSON_STRING:
                return False
        ids[unsafe_offset=count] = identifier.copy()
        sources[unsafe_offset=count] = child
        count += 1
        child = pj_next(tree, child)
    var dynamic_count = count - 1
    if deepseek and dynamic_count != 0:
        return False
    var static_start = count
    var alias_count: Int64 = 0
    child = pj_child(tree, bundled_root)
    while child >= 0:
        var identifier = pj_field(tree, child, StringSlice("id"))
        var display = pj_field(tree, child, StringSlice("display_name"))
        var description = pj_field(tree, child, StringSlice("description"))
        var names = pj_field(tree, child, StringSlice("aliases"))
        if (
            count >= tree.count or pj_kind(tree, identifier) != JSON_STRING
            or pj_kind(tree, display) != JSON_STRING or pj_kind(tree, description) != JSON_STRING
            or pj_kind(tree, names) != JSON_ARRAY
            or not runtime_catalog_window_valid(tree, pj_field(tree, child, StringSlice("context_window_tokens")))
        ):
            return False
        var id = pj_text(tree, identifier)
        if id.len == 0 or not rich_view_valid(id, CATALOG_MAX_IDENTIFIER_BYTES):
            return False
        ids[unsafe_offset=count] = id.copy()
        sources[unsafe_offset=count] = child
        var alias = pj_child(tree, names)
        while alias >= 0:
            if alias_count >= tree.count or pj_kind(tree, alias) != JSON_STRING or not rich_view_valid(pj_text(tree, alias), CATALOG_MAX_IDENTIFIER_BYTES):
                return False
            aliases[unsafe_offset=alias_count] = pj_text(tree, alias)
            alias_owners[unsafe_offset=alias_count] = count - static_start
            alias_count += 1
            alias = pj_next(tree, alias)
        count += 1
        child = pj_next(tree, child)
    var static_count = count - static_start
    if dynamic_count > CATALOG_MAX_MODELS or static_count > CATALOG_MAX_MODELS or alias_count > CATALOG_MAX_INPUT_MODELS or count > CATALOG_MAX_INPUT_MODELS:
        return False
    var merged = catalog_merge_additional(
        Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(ids)), 0,
        Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(aliases)),
        Pointer[mut=False, Int64, ImmUntrackedOrigin](unsafe_from_address=Int(alias_owners)), 0,
        Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(ids)), count,
        accepted, count,
    )
    if merged.status != RICH_STATUS_OK:
        return False
    var accepted_count = merged.count
    if accepted_count > limit:
        js_literal(sink, StringSlice('{"Err":"provider model catalog exceeds the hard limit of '))
        js_u64(sink, UInt64(limit))
        js_literal(sink, StringSlice(' entries"}'))
        return True
    var plan = RuntimeCatalogPlan(ids, sources, aliases, alias_owners, dynamic_count, static_count, alias_count)
    var fallback_context = kiro_catalog_positive_u64(tree, default_context)
    var fallback_compact = kiro_catalog_positive_u64(tree, default_compact)
    js_literal(sink, StringSlice('{"Ok":['))
    for offset in range(accepted_count):
        var position = accepted[unsafe_offset=offset]
        var slug = pj_trim(ids[unsafe_offset=position])
        var window = RuntimeCatalogWindow(False, 0) if deepseek else runtime_catalog_candidate_window(tree, plan, position, copilot)
        var context = window.tokens if window.present else fallback_context
        var compact = config_saturating_mul_u64(window.tokens, UInt64(95)) / UInt64(100) if window.present else fallback_compact
        var ceiling = context - UInt64(1) if context > 0 else UInt64(0)
        if not deepseek and compact > ceiling:
            compact = ceiling
        var observed = runtime_catalog_dynamic_node(plan, slug)
        var bundled = runtime_catalog_static_node(plan, slug)
        var display = pj_field(tree, observed, StringSlice("display_name"))
        if pj_kind(tree, display) != JSON_STRING:
            display = pj_field(tree, bundled, StringSlice("display_name"))
        var name = pj_text(tree, display) if display >= 0 else slug.copy()
        var description = pj_field(tree, observed, StringSlice("description"))
        if pj_kind(tree, description) != JSON_STRING:
            description = pj_field(tree, bundled, StringSlice("description"))
        var fallback = pj_literal(StringSlice("DeepSeek model routed through the Prodex Responses adapter.")) if deepseek else pj_literal(StringSlice("External provider model routed through the Prodex Responses adapter."))
        var detail = pj_text(tree, description) if description >= 0 else fallback.copy()
        if offset > 0:
            js_byte(sink, 44)
        js_literal(sink, StringSlice('{"slug":'))
        js_string(sink, slug)
        js_literal(sink, StringSlice(',"display_name":'))
        js_string(sink, name)
        js_literal(sink, StringSlice(',"description":'))
        js_string(sink, detail)
        js_literal(sink, StringSlice(',"priority":'))
        js_u64(sink, UInt64(offset + 1))
        js_literal(sink, StringSlice(',"context_window":'))
        js_u64(sink, context)
        js_literal(sink, StringSlice(',"auto_compact_token_limit":'))
        js_u64(sink, compact)
        js_byte(sink, 125)

    js_literal(sink, StringSlice(']}'))
    return True


@export("prodex_mojo_runtime_catalog_plan_v1")
def prodex_mojo_runtime_catalog_plan_v1(
    abi: Int64, limit: Int64, flag: Int64,
    nodes_address: UInt64, nodes_count: Int64, raw_address: UInt64, raw_length: Int64,
    scratch_address: UInt64, scratch_count: Int64,
    measuring: Int64, output_address: UInt64, capacity: Int64, metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (
        limit < 0 or flag != 0 or nodes_address == 0 or nodes_address % 8 != 0
        or nodes_count <= 0 or nodes_count > 0x7FFFFFFFFFFFFFFF // 80
        or raw_address == 0 or raw_length <= 0 or scratch_address == 0 or scratch_address % 8 != 0
        or scratch_count < nodes_count * 4 or measuring < 0 or measuring > 1 or capacity < 0
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
        Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(scratch_address)), 0, 0, True, False,
    )
    if not runtime_catalog_build_emit(Pointer(to=sink), tree, scratch_address, limit):
        return 1
    if sink.failed:
        return 3
    var required = sink.written
    if measuring == 0:
        if capacity < required:
            return 3
        sink = JsonSink(
            Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)), capacity, 0, False, False,
        )
        if not runtime_catalog_build_emit(Pointer(to=sink), tree, scratch_address, limit) or sink.failed or sink.written != required:
            return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = required
    return 0
