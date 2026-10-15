from std.memory import Pointer

from rich_text import rich_trim_bounds, rich_view_matches_literal, rich_view_valid
from rich_text import rich_view_ptr
from rich_types import ProdexRichStringView

comptime PROVIDER_REGISTRY_ABI_VERSION: Int64 = 1
comptime PROVIDER_REGISTRY_STATUS_OK: Int64 = 0
comptime PROVIDER_REGISTRY_STATUS_INVALID: Int64 = 1
comptime PROVIDER_REGISTRY_STATUS_UTF8: Int64 = 2
comptime PROVIDER_REGISTRY_STATUS_ABI: Int64 = 4
comptime PROVIDER_REGISTRY_PROVIDER_COUNT: Int64 = 7
comptime PROVIDER_REGISTRY_ENDPOINT_COUNT: Int64 = 11

comptime PROVIDER_RUNTIME_METADATA_ABI_VERSION: Int64 = 1
comptime PROVIDER_RUNTIME_METADATA_FIELD_COUNT: Int64 = 6
comptime PROVIDER_RUNTIME_METADATA_MODEL_PROVIDER_ID: Int64 = 0
comptime PROVIDER_RUNTIME_METADATA_DISPLAY_NAME: Int64 = 1
comptime PROVIDER_RUNTIME_METADATA_CODEX_PROVIDER_NAME: Int64 = 2
comptime PROVIDER_RUNTIME_METADATA_DEFAULT_MODEL: Int64 = 3
comptime PROVIDER_RUNTIME_METADATA_DEFAULT_BASE_URL: Int64 = 4
comptime PROVIDER_RUNTIME_METADATA_WEB_SEARCH_MODE: Int64 = 5

comptime PROVIDER_OPENAI: Int64 = 0
comptime PROVIDER_ANTHROPIC: Int64 = 1
comptime PROVIDER_COPILOT: Int64 = 2
comptime PROVIDER_DEEPSEEK: Int64 = 3
comptime PROVIDER_GEMINI: Int64 = 4
comptime PROVIDER_KIRO: Int64 = 5
comptime PROVIDER_LOCAL: Int64 = 6

comptime WIRE_OPENAI_RESPONSES: Int64 = 0
comptime WIRE_OPENAI_CHAT: Int64 = 1
comptime WIRE_GEMINI: Int64 = 3
comptime WIRE_PASSTHROUGH: Int64 = 4

comptime CAP_NATIVE: UInt64 = 0
comptime CAP_TRANSLATED: UInt64 = 1
comptime CAP_PASSTHROUGH: UInt64 = 2
comptime CAP_EMULATED: UInt64 = 3
comptime CAP_UNSUPPORTED: UInt64 = 5

comptime END_RESPONSES: Int64 = 0
comptime END_RESPONSES_COMPACT: Int64 = 1
comptime END_CHAT: Int64 = 2
comptime END_MESSAGES: Int64 = 3
comptime END_MODELS: Int64 = 4
comptime END_EMBEDDINGS: Int64 = 5
comptime END_IMAGES: Int64 = 6
comptime END_AUDIO: Int64 = 7
comptime END_BATCHES: Int64 = 8
comptime END_RERANK: Int64 = 9
comptime END_A2A: Int64 = 10

@fieldwise_init
struct ProdexProviderRegistryPlan(Copyable):
    var abi_version: Int64
    var provider: Int64
    var client_wire: Int64
    var upstream_wire: Int64
    var response_wire: Int64
    var supports_streaming: Int64
    var supports_model_fallback: Int64
    var endpoint_mask: UInt64
    var passthrough_mask: UInt64
    var capability_bits: UInt64


@fieldwise_init
struct ProdexProviderRuntimeMetadataPlan(Copyable):
    var abi_version: Int64
    var provider: Int64
    var default_context_window: Int64
    var default_auto_compact_token_limit: Int64
    var image_generation_enabled: Int64


def runtime_metadata_plan(provider: Int64) -> ProdexProviderRuntimeMetadataPlan:
    var context_window: Int64 = 0
    var auto_compact_limit: Int64 = 0
    var image_generation: Int64 = 0
    if provider == PROVIDER_LOCAL:
        context_window = 16_384
        auto_compact_limit = 14_000
    elif provider == PROVIDER_DEEPSEEK:
        context_window = 1_048_576
        auto_compact_limit = 900_000
    elif provider == PROVIDER_GEMINI:
        context_window = 1_048_576
        auto_compact_limit = 900_000
        image_generation = 1
    elif provider == PROVIDER_ANTHROPIC:
        context_window = 1_000_000
        auto_compact_limit = 950_000
    elif provider == PROVIDER_COPILOT:
        context_window = 1_050_000
        auto_compact_limit = 997_500
    elif provider == PROVIDER_KIRO:
        context_window = 1_000_000
        auto_compact_limit = 950_000
    return ProdexProviderRuntimeMetadataPlan(
        PROVIDER_RUNTIME_METADATA_ABI_VERSION,
        provider,
        context_window,
        auto_compact_limit,
        image_generation,
    )


def runtime_metadata_label(
    provider: Int64, field: Int64
) -> ProdexRichStringView:
    var label = StringSlice("")
    if provider == PROVIDER_LOCAL:
        if field == PROVIDER_RUNTIME_METADATA_MODEL_PROVIDER_ID:
            label = StringSlice("prodex-local")
        elif (
            field == PROVIDER_RUNTIME_METADATA_DISPLAY_NAME
            or field == PROVIDER_RUNTIME_METADATA_CODEX_PROVIDER_NAME
        ):
            label = StringSlice("Prodex Local")
        elif field == PROVIDER_RUNTIME_METADATA_DEFAULT_MODEL:
            label = StringSlice("unsloth/qwen3.5-35b-a3b")
        elif field == PROVIDER_RUNTIME_METADATA_WEB_SEARCH_MODE:
            label = StringSlice("disabled")
    elif provider == PROVIDER_DEEPSEEK:
        if field == PROVIDER_RUNTIME_METADATA_MODEL_PROVIDER_ID:
            label = StringSlice("prodex-deepseek")
        elif (
            field == PROVIDER_RUNTIME_METADATA_DISPLAY_NAME
            or field == PROVIDER_RUNTIME_METADATA_CODEX_PROVIDER_NAME
        ):
            label = StringSlice("DeepSeek")
        elif field == PROVIDER_RUNTIME_METADATA_DEFAULT_MODEL:
            label = StringSlice("deepseek-v4-pro")
        elif field == PROVIDER_RUNTIME_METADATA_DEFAULT_BASE_URL:
            label = StringSlice("https://api.deepseek.com")
        elif field == PROVIDER_RUNTIME_METADATA_WEB_SEARCH_MODE:
            label = StringSlice("live")
    elif provider == PROVIDER_GEMINI:
        if field == PROVIDER_RUNTIME_METADATA_MODEL_PROVIDER_ID:
            label = StringSlice("prodex-gemini")
        elif field == PROVIDER_RUNTIME_METADATA_DISPLAY_NAME:
            label = StringSlice("Google Gemini")
        elif field == PROVIDER_RUNTIME_METADATA_CODEX_PROVIDER_NAME:
            label = StringSlice("Azure")
        elif field == PROVIDER_RUNTIME_METADATA_DEFAULT_MODEL:
            label = StringSlice("auto")
        elif field == PROVIDER_RUNTIME_METADATA_DEFAULT_BASE_URL:
            label = StringSlice(
                "https://generativelanguage.googleapis.com/v1beta"
            )
        elif field == PROVIDER_RUNTIME_METADATA_WEB_SEARCH_MODE:
            label = StringSlice("live")
    elif provider == PROVIDER_ANTHROPIC:
        if field == PROVIDER_RUNTIME_METADATA_MODEL_PROVIDER_ID:
            label = StringSlice("prodex-anthropic")
        elif (
            field == PROVIDER_RUNTIME_METADATA_DISPLAY_NAME
            or field == PROVIDER_RUNTIME_METADATA_CODEX_PROVIDER_NAME
        ):
            label = StringSlice("Anthropic Claude")
        elif field == PROVIDER_RUNTIME_METADATA_DEFAULT_MODEL:
            label = StringSlice("claude-sonnet-5-5")
        elif field == PROVIDER_RUNTIME_METADATA_DEFAULT_BASE_URL:
            label = StringSlice("https://api.anthropic.com/v1")
        elif field == PROVIDER_RUNTIME_METADATA_WEB_SEARCH_MODE:
            label = StringSlice("live")
    elif provider == PROVIDER_COPILOT:
        if field == PROVIDER_RUNTIME_METADATA_MODEL_PROVIDER_ID:
            label = StringSlice("prodex-copilot")
        elif field == PROVIDER_RUNTIME_METADATA_DISPLAY_NAME:
            label = StringSlice("GitHub Copilot")
        elif field == PROVIDER_RUNTIME_METADATA_CODEX_PROVIDER_NAME:
            label = StringSlice("OpenAI")
        elif field == PROVIDER_RUNTIME_METADATA_DEFAULT_MODEL:
            label = StringSlice("gpt-6-astra")
        elif field == PROVIDER_RUNTIME_METADATA_DEFAULT_BASE_URL:
            label = StringSlice("https://api.githubcopilot.com")
        elif field == PROVIDER_RUNTIME_METADATA_WEB_SEARCH_MODE:
            label = StringSlice("live")
    elif provider == PROVIDER_KIRO:
        if field == PROVIDER_RUNTIME_METADATA_MODEL_PROVIDER_ID:
            label = StringSlice("prodex-kiro")
        elif field == PROVIDER_RUNTIME_METADATA_DISPLAY_NAME:
            label = StringSlice("Kiro")
        elif field == PROVIDER_RUNTIME_METADATA_CODEX_PROVIDER_NAME:
            label = StringSlice("Azure")
        elif field == PROVIDER_RUNTIME_METADATA_DEFAULT_MODEL:
            label = StringSlice("auto")
        elif field == PROVIDER_RUNTIME_METADATA_DEFAULT_BASE_URL:
            label = StringSlice("https://kiro.dev")
        elif field == PROVIDER_RUNTIME_METADATA_WEB_SEARCH_MODE:
            label = StringSlice("live")
    return ProdexRichStringView(
        UInt(Int(label.unsafe_ptr())), UInt(label.byte_length())
    )


def runtime_metadata_copy_label(
    label: ProdexRichStringView,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) -> Int64:
    if output_address == 0 or output_capacity < 0 or written_address == 0:
        return PROVIDER_REGISTRY_STATUS_INVALID
    var length = Int64(label.len)
    if length > output_capacity:
        return 2
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var source = rich_view_ptr(label)
    for index in range(length):
        output[unsafe_offset=index] = source[unsafe_offset=index]
    written[] = length
    return PROVIDER_REGISTRY_STATUS_OK


@export("prodex_mojo_provider_runtime_metadata_plan_v1")
def prodex_mojo_provider_runtime_metadata_plan_v1(
    abi_version: Int64,
    provider: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != PROVIDER_RUNTIME_METADATA_ABI_VERSION:
        return PROVIDER_REGISTRY_STATUS_ABI
    if (
        provider <= PROVIDER_OPENAI
        or provider >= PROVIDER_REGISTRY_PROVIDER_COUNT
        or output_address == 0
    ):
        return PROVIDER_REGISTRY_STATUS_INVALID
    var output = Pointer[
        mut=True,
        ProdexProviderRuntimeMetadataPlan,
        MutUntrackedOrigin,
    ](unsafe_from_address=Int(output_address))
    output[] = runtime_metadata_plan(provider)
    return PROVIDER_REGISTRY_STATUS_OK


@export("prodex_mojo_provider_runtime_metadata_label_v1")
def prodex_mojo_provider_runtime_metadata_label_v1(
    abi_version: Int64,
    provider: Int64,
    field: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PROVIDER_RUNTIME_METADATA_ABI_VERSION:
        return PROVIDER_REGISTRY_STATUS_ABI
    if (
        provider <= PROVIDER_OPENAI
        or provider >= PROVIDER_REGISTRY_PROVIDER_COUNT
        or field < 0
        or field >= PROVIDER_RUNTIME_METADATA_FIELD_COUNT
    ):
        return PROVIDER_REGISTRY_STATUS_INVALID
    return runtime_metadata_copy_label(
        runtime_metadata_label(provider, field),
        output_address,
        output_capacity,
        written_address,
    )


def endpoint_bit(index: Int64) -> UInt64:
    return UInt64(1) << UInt64(index)


def capability_set(bits: UInt64, endpoint: Int64, status: UInt64) -> UInt64:
    var shift = UInt64(endpoint * 3)
    var clear_mask = ~(UInt64(7) << shift)
    return (bits & clear_mask) | ((status & 7) << shift)


def capability_defaults() -> UInt64:
    var bits: UInt64 = 0
    for endpoint in range(PROVIDER_REGISTRY_ENDPOINT_COUNT):
        bits = capability_set(bits, endpoint, CAP_UNSUPPORTED)
    return bits


def registry_plan(provider: Int64) -> ProdexProviderRegistryPlan:
    var bits = capability_defaults()
    var supported: UInt64 = 0
    var passthrough: UInt64 = 0
    var upstream = WIRE_OPENAI_RESPONSES
    var fallback: Int64 = 0
    if provider == PROVIDER_OPENAI:
        supported = endpoint_bit(END_RESPONSES) | endpoint_bit(END_RESPONSES_COMPACT) | endpoint_bit(END_CHAT) | endpoint_bit(END_MODELS) | endpoint_bit(END_EMBEDDINGS) | endpoint_bit(END_IMAGES) | endpoint_bit(END_AUDIO) | endpoint_bit(END_BATCHES)
        passthrough = endpoint_bit(END_RESPONSES) | endpoint_bit(END_CHAT) | endpoint_bit(END_MODELS) | endpoint_bit(END_EMBEDDINGS) | endpoint_bit(END_IMAGES) | endpoint_bit(END_AUDIO) | endpoint_bit(END_BATCHES)
        bits = capability_set(bits, END_RESPONSES, CAP_NATIVE)
        bits = capability_set(bits, END_RESPONSES_COMPACT, CAP_EMULATED)
        bits = capability_set(bits, END_CHAT, CAP_NATIVE)
        bits = capability_set(bits, END_MODELS, CAP_NATIVE)
        bits = capability_set(bits, END_EMBEDDINGS, CAP_NATIVE)
        bits = capability_set(bits, END_IMAGES, CAP_NATIVE)
        bits = capability_set(bits, END_AUDIO, CAP_NATIVE)
        bits = capability_set(bits, END_BATCHES, CAP_NATIVE)
    elif provider == PROVIDER_ANTHROPIC or provider == PROVIDER_DEEPSEEK:
        supported = endpoint_bit(END_RESPONSES) | endpoint_bit(END_RESPONSES_COMPACT) | endpoint_bit(END_CHAT) | endpoint_bit(END_MESSAGES) | endpoint_bit(END_MODELS)
        passthrough = endpoint_bit(END_CHAT) | endpoint_bit(END_MESSAGES)
        upstream = WIRE_OPENAI_CHAT
        fallback = 1
        bits = capability_set(bits, END_RESPONSES, CAP_TRANSLATED)
        bits = capability_set(bits, END_RESPONSES_COMPACT, CAP_EMULATED)
        bits = capability_set(bits, END_CHAT, CAP_PASSTHROUGH)
        bits = capability_set(bits, END_MESSAGES, CAP_PASSTHROUGH)
        bits = capability_set(bits, END_MODELS, CAP_EMULATED)
    elif provider == PROVIDER_COPILOT:
        supported = endpoint_bit(END_RESPONSES) | endpoint_bit(END_RESPONSES_COMPACT) | endpoint_bit(END_CHAT) | endpoint_bit(END_MESSAGES) | endpoint_bit(END_MODELS)
        passthrough = endpoint_bit(END_RESPONSES) | endpoint_bit(END_RESPONSES_COMPACT) | endpoint_bit(END_CHAT) | endpoint_bit(END_MESSAGES)
        fallback = 1
        bits = capability_set(bits, END_RESPONSES, CAP_NATIVE)
        bits = capability_set(bits, END_RESPONSES_COMPACT, CAP_PASSTHROUGH)
        bits = capability_set(bits, END_CHAT, CAP_PASSTHROUGH)
        bits = capability_set(bits, END_MESSAGES, CAP_PASSTHROUGH)
        bits = capability_set(bits, END_MODELS, CAP_EMULATED)
    elif provider == PROVIDER_GEMINI:
        supported = endpoint_bit(END_RESPONSES) | endpoint_bit(END_RESPONSES_COMPACT) | endpoint_bit(END_CHAT) | endpoint_bit(END_MESSAGES) | endpoint_bit(END_MODELS) | endpoint_bit(END_EMBEDDINGS)
        passthrough = endpoint_bit(END_CHAT) | endpoint_bit(END_MESSAGES) | endpoint_bit(END_EMBEDDINGS)
        upstream = WIRE_GEMINI
        fallback = 1
        bits = capability_set(bits, END_RESPONSES, CAP_TRANSLATED)
        bits = capability_set(bits, END_RESPONSES_COMPACT, CAP_EMULATED)
        bits = capability_set(bits, END_CHAT, CAP_PASSTHROUGH)
        bits = capability_set(bits, END_MESSAGES, CAP_PASSTHROUGH)
        bits = capability_set(bits, END_MODELS, CAP_EMULATED)
        bits = capability_set(bits, END_EMBEDDINGS, CAP_PASSTHROUGH)
    elif provider == PROVIDER_KIRO:
        supported = endpoint_bit(END_RESPONSES) | endpoint_bit(END_RESPONSES_COMPACT) | endpoint_bit(END_CHAT) | endpoint_bit(END_MESSAGES) | endpoint_bit(END_MODELS)
        upstream = WIRE_PASSTHROUGH
        bits = capability_set(bits, END_RESPONSES, CAP_TRANSLATED)
        bits = capability_set(bits, END_RESPONSES_COMPACT, CAP_EMULATED)
        bits = capability_set(bits, END_CHAT, CAP_TRANSLATED)
        bits = capability_set(bits, END_MESSAGES, CAP_TRANSLATED)
        bits = capability_set(bits, END_MODELS, CAP_EMULATED)
    elif provider == PROVIDER_LOCAL:
        supported = endpoint_bit(END_RESPONSES) | endpoint_bit(END_RESPONSES_COMPACT) | endpoint_bit(END_CHAT) | endpoint_bit(END_MESSAGES) | endpoint_bit(END_MODELS) | endpoint_bit(END_EMBEDDINGS) | endpoint_bit(END_IMAGES) | endpoint_bit(END_AUDIO) | endpoint_bit(END_BATCHES) | endpoint_bit(END_RERANK) | endpoint_bit(END_A2A)
        passthrough = endpoint_bit(END_RESPONSES) | endpoint_bit(END_CHAT) | endpoint_bit(END_MESSAGES) | endpoint_bit(END_MODELS) | endpoint_bit(END_EMBEDDINGS) | endpoint_bit(END_IMAGES) | endpoint_bit(END_AUDIO) | endpoint_bit(END_BATCHES) | endpoint_bit(END_RERANK) | endpoint_bit(END_A2A)
        for endpoint in range(PROVIDER_REGISTRY_ENDPOINT_COUNT):
            bits = capability_set(bits, endpoint, CAP_PASSTHROUGH)
        bits = capability_set(bits, END_RESPONSES_COMPACT, CAP_EMULATED)
    return ProdexProviderRegistryPlan(PROVIDER_REGISTRY_ABI_VERSION, provider, WIRE_OPENAI_RESPONSES, upstream, WIRE_OPENAI_RESPONSES, 1, fallback, supported, passthrough, bits)


@export("prodex_mojo_provider_registry_plan_v1")
def prodex_mojo_provider_registry_plan_v1(
    abi_version: Int64,
    provider: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != PROVIDER_REGISTRY_ABI_VERSION:
        return PROVIDER_REGISTRY_STATUS_ABI
    if provider < 0 or provider >= PROVIDER_REGISTRY_PROVIDER_COUNT or output_address == 0:
        return PROVIDER_REGISTRY_STATUS_INVALID
    var output = Pointer[mut=True, ProdexProviderRegistryPlan, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    output[] = registry_plan(provider)
    return PROVIDER_REGISTRY_STATUS_OK


def registry_trimmed(view: ProdexRichStringView) -> ProdexRichStringView:
    var bounds = rich_trim_bounds(view)
    return ProdexRichStringView(view.ptr + UInt(bounds[0]), UInt(bounds[1] - bounds[0]))


def registry_alias_provider(view: ProdexRichStringView) -> Int64:
    var value = registry_trimmed(view)
    if rich_view_matches_literal["openai"](value, True) or rich_view_matches_literal["openai-responses"](value, True) or rich_view_matches_literal["openai_compatible"](value, True) or rich_view_matches_literal["openai-compatible"](value, True):
        return PROVIDER_OPENAI
    if rich_view_matches_literal["anthropic"](value, True) or rich_view_matches_literal["claude"](value, True):
        return PROVIDER_ANTHROPIC
    if rich_view_matches_literal["copilot"](value, True) or rich_view_matches_literal["github-copilot"](value, True) or rich_view_matches_literal["github_copilot"](value, True):
        return PROVIDER_COPILOT
    if rich_view_matches_literal["deepseek"](value, True):
        return PROVIDER_DEEPSEEK
    if rich_view_matches_literal["gemini"](value, True) or rich_view_matches_literal["google"](value, True):
        return PROVIDER_GEMINI
    if rich_view_matches_literal["kiro"](value, True):
        return PROVIDER_KIRO
    if rich_view_matches_literal["local"](value, True) or rich_view_matches_literal["local-openai"](value, True) or rich_view_matches_literal["local_openai"](value, True):
        return PROVIDER_LOCAL
    return -1


def registry_model_provider(view: ProdexRichStringView) -> Int64:
    var resolved_alias = registry_alias_provider(view)
    if resolved_alias >= 0:
        return resolved_alias
    var value = registry_trimmed(view)
    if rich_view_matches_literal["prodex-openai-governed-http"](value, True):
        return PROVIDER_OPENAI
    if rich_view_matches_literal["prodex-anthropic"](value, True):
        return PROVIDER_ANTHROPIC
    if rich_view_matches_literal["prodex-copilot"](value, True):
        return PROVIDER_COPILOT
    if rich_view_matches_literal["prodex-deepseek"](value, True):
        return PROVIDER_DEEPSEEK
    if rich_view_matches_literal["prodex-gemini"](value, True):
        return PROVIDER_GEMINI
    if rich_view_matches_literal["prodex-kiro"](value, True):
        return PROVIDER_KIRO
    if rich_view_matches_literal["prodex-local"](value, True):
        return PROVIDER_LOCAL
    return -1


@export("prodex_mojo_provider_registry_resolve_v1")
def prodex_mojo_provider_registry_resolve_v1(
    abi_version: Int64,
    value_address: UInt,
    value_length: Int64,
    include_model_provider_ids: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != PROVIDER_REGISTRY_ABI_VERSION:
        return PROVIDER_REGISTRY_STATUS_ABI
    if value_length < 0 or include_model_provider_ids < 0 or include_model_provider_ids > 1 or output_address == 0 or value_length > 0 and value_address == 0:
        return PROVIDER_REGISTRY_STATUS_INVALID
    var value = ProdexRichStringView(value_address, UInt(value_length))
    if not rich_view_valid(value, 65536):
        return PROVIDER_REGISTRY_STATUS_UTF8
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    if include_model_provider_ids == 1:
        output[] = registry_model_provider(value)
    else:
        output[] = registry_alias_provider(value)
    return PROVIDER_REGISTRY_STATUS_OK
