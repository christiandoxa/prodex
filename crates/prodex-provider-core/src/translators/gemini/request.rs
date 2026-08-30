#[cfg(feature = "mojo")]
use serde_json::Map;
use serde_json::Value;

#[path = "request/continuation.rs"]
mod continuation;
#[path = "request/generation_config.rs"]
mod generation_config;
#[path = "request/optional_fields.rs"]
mod optional_fields;
#[path = "request/response_format.rs"]
mod response_format;
#[path = "request/schema.rs"]
mod schema;
#[path = "request/tool_signatures.rs"]
mod tool_signatures;
#[path = "request/tools.rs"]
mod tools;

pub(super) use self::continuation::gemini_continuation_metadata;
pub use self::generation_config::gemini_provider_core_model_uses_thinking_level;
pub(super) use self::generation_config::{
    gemini_apply_text_format, gemini_insert_basic_generation_config,
    gemini_insert_extended_generation_config, gemini_thinking_config_from_request,
};
pub(crate) use self::generation_config::{
    gemini_generation_config_from_request, gemini_validate_candidate_count,
};
pub(super) use self::optional_fields::gemini_apply_optional_request_fields;
pub(super) use self::response_format::gemini_apply_response_format;
pub(crate) use self::schema::sanitize_function_schema;
pub(crate) use self::tool_signatures::gemini_preserve_tool_call_signatures;
pub(super) use self::tools::gemini_tool_from_openai_tool;
pub(crate) use self::tools::{
    gemini_builtin_tools_from_request, gemini_function_declaration_from_openai_tool,
    gemini_is_supported_builtin_tool, gemini_tool_config_from_request,
    gemini_validate_openai_tools,
};

#[cfg(feature = "mojo")]
#[derive(Clone, Copy)]
pub(super) enum GeminiRequestFieldScope {
    BasicGeneration,
    ExtendedGeneration,
    OptionalRequest,
}

#[cfg(feature = "mojo")]
pub(super) fn gemini_request_field_plan(
    source: &Map<String, Value>,
    scope: GeminiRequestFieldScope,
) -> Vec<prodex_mojo_core::provider_constraints::GeminiRequestField> {
    let (basic_fields, extended_fields, optional_fields) = match scope {
        GeminiRequestFieldScope::BasicGeneration => (gemini_basic_field_mask(source), 0, 0),
        GeminiRequestFieldScope::ExtendedGeneration => (0, gemini_extended_field_mask(source), 0),
        GeminiRequestFieldScope::OptionalRequest => (0, 0, gemini_optional_field_mask(source)),
    };
    prodex_mojo_core::provider_constraints::gemini_request_field_plan(
        basic_fields,
        extended_fields,
        optional_fields,
    )
    .expect("Gemini request field plan returned invalid output")
}

#[cfg(feature = "mojo")]
pub(super) fn gemini_request_source_value(
    source: &Map<String, Value>,
    field: prodex_mojo_core::provider_constraints::GeminiRequestField,
) -> Option<&Value> {
    GEMINI_REQUEST_SOURCE_KEYS
        .get(field.source_index)
        .and_then(|key| source.get(*key))
}

#[cfg(feature = "mojo")]
pub(super) const fn gemini_request_target_name(
    target: prodex_mojo_core::provider_constraints::GeminiRequestFieldTarget,
) -> &'static str {
    use prodex_mojo_core::provider_constraints::GeminiRequestFieldTarget as Target;

    match target {
        Target::Temperature => "temperature",
        Target::TopP => "topP",
        Target::MaxOutputTokens => "maxOutputTokens",
        Target::StopSequences => "stopSequences",
        Target::TopK => "topK",
        Target::Seed => "seed",
        Target::PresencePenalty => "presencePenalty",
        Target::FrequencyPenalty => "frequencyPenalty",
        Target::ResponseMimeType => "responseMimeType",
        Target::ResponseSchema => "responseSchema",
        Target::ResponseJsonSchema => "responseJsonSchema",
        Target::ResponseModalities => "responseModalities",
        Target::MediaResolution => "mediaResolution",
        Target::AudioTimestamp => "audioTimestamp",
        Target::SpeechConfig => "speechConfig",
        Target::CandidateCount => "candidateCount",
        Target::SafetySettings => "safetySettings",
        Target::CachedContent => "cachedContent",
        Target::Labels => "labels",
    }
}

#[cfg(feature = "mojo")]
const GEMINI_REQUEST_SOURCE_KEYS: [&str; 34] = [
    "temperature",
    "top_p",
    "max_tokens",
    "stop",
    "stop_sequences",
    "stopSequences",
    "top_k",
    "topK",
    "seed",
    "presence_penalty",
    "presencePenalty",
    "frequency_penalty",
    "frequencyPenalty",
    "response_mime_type",
    "responseMimeType",
    "response_schema",
    "responseSchema",
    "response_json_schema",
    "responseJsonSchema",
    "response_modalities",
    "responseModalities",
    "media_resolution",
    "mediaResolution",
    "audio_timestamp",
    "audioTimestamp",
    "speech_config",
    "speechConfig",
    "candidateCount",
    "candidate_count",
    "safety_settings",
    "safetySettings",
    "cached_content",
    "cachedContent",
    "labels",
];

#[cfg(feature = "mojo")]
fn gemini_basic_field_mask(source: &Map<String, Value>) -> u64 {
    let mut mask = 0;
    for (bit, key) in [(0, "temperature"), (1, "top_p"), (2, "max_tokens")] {
        if source.get(key).is_some_and(|value| !value.is_null()) {
            mask |= 1 << bit;
        }
    }
    for (bit, key) in [(3, "stop"), (4, "stop_sequences"), (5, "stopSequences")] {
        if source.contains_key(key) {
            mask |= 1 << bit;
        }
    }
    mask
}

#[cfg(feature = "mojo")]
fn gemini_extended_field_mask(source: &Map<String, Value>) -> u64 {
    let mut mask = 0;
    for (bit, key) in [
        (0, "top_k"),
        (1, "topK"),
        (2, "seed"),
        (3, "presence_penalty"),
        (4, "presencePenalty"),
        (5, "frequency_penalty"),
        (6, "frequencyPenalty"),
        (7, "response_mime_type"),
        (8, "responseMimeType"),
        (9, "response_schema"),
        (10, "responseSchema"),
        (11, "response_json_schema"),
        (12, "responseJsonSchema"),
        (13, "response_modalities"),
        (14, "responseModalities"),
        (15, "media_resolution"),
        (16, "mediaResolution"),
        (17, "audio_timestamp"),
        (18, "audioTimestamp"),
        (19, "speech_config"),
        (20, "speechConfig"),
    ] {
        if source.get(key).is_some_and(|value| !value.is_null()) {
            mask |= 1 << bit;
        }
    }
    for (bit, key) in [(21, "candidateCount"), (22, "candidate_count")] {
        if source.contains_key(key) {
            mask |= 1 << bit;
        }
    }
    mask
}

#[cfg(feature = "mojo")]
fn gemini_optional_field_mask(source: &Map<String, Value>) -> u64 {
    let mut mask = 0;
    for (bit, key) in [(0, "safety_settings"), (1, "safetySettings")] {
        if source.contains_key(key) {
            mask |= 1 << bit;
        }
    }
    for (bit, key) in [(2, "cached_content"), (3, "cachedContent")] {
        if source.contains_key(key) {
            mask |= 1 << bit;
        }
    }
    if source.get("labels").is_some_and(|value| !value.is_null()) {
        mask |= 1 << 4;
    }
    mask
}

pub(crate) fn gemini_request_body_without_tool(body: &[u8], tool_name: &str) -> Option<Vec<u8>> {
    let mut value: Value = serde_json::from_slice(body).ok()?;
    let request = gemini_request_object_mut(&mut value)?;
    let tools = request.get_mut("tools")?.as_array_mut()?;
    let original_len = tools.len();
    tools.retain(|tool| {
        !tool
            .as_object()
            .map(|object| object.contains_key(tool_name))
            .unwrap_or(false)
    });
    if tools.len() == original_len {
        return None;
    }
    if tools.is_empty() {
        request.remove("tools");
    }
    serde_json::to_vec(&value).ok()
}

fn gemini_request_object_mut(value: &mut Value) -> Option<&mut serde_json::Map<String, Value>> {
    if value.get("request").is_some() {
        value.get_mut("request")?.as_object_mut()
    } else {
        value.as_object_mut()
    }
}
