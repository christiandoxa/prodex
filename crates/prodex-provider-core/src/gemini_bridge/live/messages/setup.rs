//! Gemini Live setup message builders.

#[derive(Clone, Debug, PartialEq)]
pub struct GeminiProviderCoreLiveSessionUpdate {
    pub setup: serde_json::Value,
    pub applied_session: serde_json::Map<String, serde_json::Value>,
}

pub fn gemini_provider_core_live_function_declaration(
    tool: &serde_json::Value,
) -> Option<serde_json::Value> {
    if tool.get("type").and_then(serde_json::Value::as_str) != Some("function") {
        return None;
    }
    let name = tool.get("name").and_then(serde_json::Value::as_str)?;
    let mut declaration = serde_json::json!({
        "name": name,
        "parameters": tool
            .get("parameters")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({"type": "object"})),
    });
    if let Some(description) = tool.get("description").filter(|value| !value.is_null()) {
        declaration["description"] = description.clone();
    }
    Some(declaration)
}

pub fn gemini_provider_core_live_setup_message(
    session: &serde_json::Map<String, serde_json::Value>,
    default_model: &str,
    configured_model: Option<&str>,
) -> serde_json::Value {
    let requested_model = session
        .get("model")
        .and_then(serde_json::Value::as_str)
        .map(|model| model.strip_prefix("models/").unwrap_or(model))
        .filter(|model| model.starts_with("gemini-"));
    let model = configured_model
        .filter(|model| !model.trim().is_empty())
        .or(requested_model)
        .unwrap_or(default_model);
    let model = model.strip_prefix("models/").unwrap_or(model);
    let output_modalities = session
        .get("output_modalities")
        .and_then(serde_json::Value::as_array)
        .map(|modalities| {
            modalities
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(|modality| modality.to_ascii_uppercase())
                .collect::<Vec<_>>()
        })
        .filter(|modalities| !modalities.is_empty())
        .unwrap_or_else(|| vec!["AUDIO".to_string()]);
    let mut setup = serde_json::json!({
        "model": format!("models/{model}"),
        "generation_config": {
            "response_modalities": output_modalities,
        },
        "input_audio_transcription": {},
        "output_audio_transcription": {},
    });
    if let Some(instructions) = session
        .get("instructions")
        .and_then(serde_json::Value::as_str)
        .filter(|instructions| !instructions.trim().is_empty())
    {
        setup["system_instruction"] = serde_json::json!({
            "parts": [{"text": instructions}],
        });
    }
    if let Some(tools) = session
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(gemini_provider_core_live_function_declaration)
                .collect::<Vec<_>>()
        })
        .filter(|tools| !tools.is_empty())
    {
        setup["tools"] = serde_json::json!([{"function_declarations": tools}]);
    }
    serde_json::json!({"setup": setup})
}

pub fn gemini_provider_core_live_session_update(
    session: &serde_json::Map<String, serde_json::Value>,
    default_model: &str,
    configured_model: Option<&str>,
) -> Result<GeminiProviderCoreLiveSessionUpdate, String> {
    validate_live_session_update(session)?;
    let setup = gemini_provider_core_live_setup_message(session, default_model, configured_model);
    let setup_object = setup
        .get("setup")
        .and_then(serde_json::Value::as_object)
        .expect("Gemini Live setup is an object");
    let mut applied_session = serde_json::Map::new();
    if let Some(model) = setup_object.get("model") {
        applied_session.insert("model".to_string(), model.clone());
    }
    if let Some(modalities) = setup_object
        .get("generation_config")
        .and_then(|config| config.get("response_modalities"))
    {
        applied_session.insert("output_modalities".to_string(), modalities.clone());
    }
    if let Some(instructions) = session
        .get("instructions")
        .and_then(serde_json::Value::as_str)
        .filter(|instructions| !instructions.trim().is_empty())
    {
        applied_session.insert(
            "instructions".to_string(),
            serde_json::Value::String(instructions.to_string()),
        );
    }
    if let Some(tools) = session.get("tools") {
        applied_session.insert("tools".to_string(), tools.clone());
    }
    let mut audio = serde_json::Map::new();
    for direction in ["input", "output"] {
        let config =
            super::super::audio::gemini_provider_core_live_session_audio_config(session, direction)
                .or_else(|| {
                    let (snake, camel) = if direction == "input" {
                        ("input_audio_format", "inputAudioFormat")
                    } else {
                        ("output_audio_format", "outputAudioFormat")
                    };
                    super::super::audio::gemini_provider_core_live_legacy_session_audio_config(
                        session, snake, camel,
                    )
                });
        if let Some(config) = config {
            audio.insert(
                direction.to_string(),
                serde_json::json!({
                    "format": live_audio_format_name(config.format),
                    "rate": config.rate,
                }),
            );
        }
    }
    if !audio.is_empty() {
        applied_session.insert("audio".to_string(), serde_json::Value::Object(audio));
    }
    Ok(GeminiProviderCoreLiveSessionUpdate {
        setup,
        applied_session,
    })
}

fn validate_live_session_update(
    session: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    validate_live_session_fields(session)?;
    validate_live_model(session)?;
    validate_live_output_modalities(session)?;
    validate_live_instructions(session)?;
    validate_live_tools(session)?;
    validate_live_audio(session)
}

fn validate_live_session_fields(
    session: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    for field in session.keys() {
        if !matches!(
            field.as_str(),
            "model"
                | "output_modalities"
                | "instructions"
                | "tools"
                | "audio"
                | "input_audio_format"
                | "inputAudioFormat"
                | "output_audio_format"
                | "outputAudioFormat"
        ) {
            return Err(format!(
                "Gemini Live session.update field `{field}` is unsupported"
            ));
        }
    }
    Ok(())
}

fn validate_live_model(session: &serde_json::Map<String, serde_json::Value>) -> Result<(), String> {
    if let Some(model) = session.get("model")
        && model
            .as_str()
            .is_none_or(|model| !model.trim_start_matches("models/").starts_with("gemini-"))
    {
        return Err(
            "Gemini Live session.update field `model` must name a Gemini model".to_string(),
        );
    }
    Ok(())
}

fn validate_live_output_modalities(
    session: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    let Some(modalities) = session.get("output_modalities") else {
        return Ok(());
    };
    let Some(modalities) = modalities.as_array() else {
        return Err(
            "Gemini Live session.update field `output_modalities` must be an array".to_string(),
        );
    };
    if modalities.iter().any(|modality| {
        modality.as_str().is_none_or(|modality| {
            !matches!(
                modality.trim().to_ascii_uppercase().as_str(),
                "AUDIO" | "TEXT"
            )
        })
    }) {
        return Err(
            "Gemini Live session.update field `output_modalities[]` must be `audio` or `text`"
                .to_string(),
        );
    }
    if modalities.is_empty() {
        return Err(
            "Gemini Live session.update field `output_modalities` must not be empty".to_string(),
        );
    }
    Ok(())
}

fn validate_live_instructions(
    session: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    if let Some(instructions) = session.get("instructions")
        && !instructions.is_string()
    {
        return Err("Gemini Live session.update field `instructions` must be a string".to_string());
    }
    Ok(())
}

fn validate_live_tools(session: &serde_json::Map<String, serde_json::Value>) -> Result<(), String> {
    let Some(tools) = session.get("tools") else {
        return Ok(());
    };
    let Some(tools) = tools.as_array() else {
        return Err("Gemini Live session.update field `tools` must be an array".to_string());
    };
    for (index, tool) in tools.iter().enumerate() {
        validate_live_tool(tool, index)?;
    }
    Ok(())
}

fn validate_live_tool(tool: &serde_json::Value, index: usize) -> Result<(), String> {
    let Some(tool) = tool.as_object() else {
        return Err(format!(
            "Gemini Live session.update field `tools[{index}]` must be an object"
        ));
    };
    if tool.get("type").and_then(serde_json::Value::as_str) != Some("function") {
        return Err(format!(
            "Gemini Live session.update field `tools[{index}].type` must be `function`"
        ));
    }
    if tool
        .get("name")
        .and_then(serde_json::Value::as_str)
        .is_none_or(|name| name.trim().is_empty())
    {
        return Err(format!(
            "Gemini Live session.update field `tools[{index}].name` must be a non-empty string"
        ));
    }
    let Some(parameters) = tool.get("parameters") else {
        return Err(format!(
            "Gemini Live session.update field `tools[{index}].parameters` is required"
        ));
    };
    if !parameters.is_object() {
        return Err(format!(
            "Gemini Live session.update field `tools[{index}].parameters` must be an object"
        ));
    }
    if let Some(description) = tool.get("description").filter(|value| !value.is_null())
        && !description.is_string()
    {
        return Err(format!(
            "Gemini Live session.update field `tools[{index}].description` must be a string"
        ));
    }
    Ok(())
}

fn validate_live_audio(session: &serde_json::Map<String, serde_json::Value>) -> Result<(), String> {
    validate_live_audio_object(session)?;
    for (snake, camel, direction) in [
        ("input_audio_format", "inputAudioFormat", "input"),
        ("output_audio_format", "outputAudioFormat", "output"),
    ] {
        validate_live_legacy_audio(session, snake, camel, direction)?;
    }
    Ok(())
}

fn validate_live_audio_object(
    session: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    let Some(audio) = session.get("audio") else {
        return Ok(());
    };
    let Some(audio) = audio.as_object() else {
        return Err("Gemini Live session.update field `audio` must be an object".to_string());
    };
    for (direction, value) in audio {
        validate_live_audio_direction(direction, value)?;
    }
    Ok(())
}

fn validate_live_audio_direction(direction: &str, value: &serde_json::Value) -> Result<(), String> {
    if !matches!(direction, "input" | "output") {
        return Err(format!(
            "Gemini Live session.update field `audio.{direction}` is unsupported"
        ));
    }
    let Some(value) = value.as_object() else {
        return Err(format!(
            "Gemini Live session.update field `audio.{direction}` must be an object"
        ));
    };
    if let Some(field) = value.keys().find(|field| field.as_str() != "format") {
        return Err(format!(
            "Gemini Live session.update field `audio.{direction}.{field}` is unsupported"
        ));
    }
    let Some(format) = value.get("format") else {
        return Err(format!(
            "Gemini Live session.update field `audio.{direction}.format` is required"
        ));
    };
    validate_live_audio_format_object(format, &format!("audio.{direction}.format"))?;
    if super::super::audio::gemini_provider_core_live_audio_config_from_value(format).is_none() {
        return Err(format!(
            "Gemini Live session.update field `audio.{direction}.format` is invalid"
        ));
    }
    Ok(())
}

fn validate_live_legacy_audio(
    session: &serde_json::Map<String, serde_json::Value>,
    snake: &str,
    camel: &str,
    direction: &str,
) -> Result<(), String> {
    let snake_value = session.get(snake);
    let camel_value = session.get(camel);
    if session
        .get("audio")
        .and_then(|audio| audio.get(direction))
        .is_some()
        && (snake_value.is_some() || camel_value.is_some())
    {
        return Err(format!(
            "Gemini Live session.update audio fields for `{direction}` conflict"
        ));
    }
    if let (Some(left), Some(right)) = (snake_value, camel_value)
        && left != right
    {
        return Err(format!(
            "Gemini Live session.update fields `{snake}` and `{camel}` conflict"
        ));
    }
    let Some(value) = snake_value.or(camel_value) else {
        return Ok(());
    };
    validate_live_audio_format_object(value, snake)?;
    if super::super::audio::gemini_provider_core_live_audio_config_from_value(value).is_none() {
        return Err(format!(
            "Gemini Live session.update field `{snake}` is invalid"
        ));
    }
    Ok(())
}

fn validate_live_audio_format_object(value: &serde_json::Value, path: &str) -> Result<(), String> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    if let Some(field) = object.keys().find(|field| {
        !matches!(
            field.as_str(),
            "type" | "name" | "format" | "rate" | "sample_rate" | "sampleRate"
        )
    }) {
        return Err(format!(
            "Gemini Live session.update field `{path}.{field}` is unsupported"
        ));
    }
    if let Some(name) = object
        .get("type")
        .or_else(|| object.get("name"))
        .or_else(|| object.get("format"))
        && name
            .as_str()
            .and_then(super::super::audio::GeminiProviderCoreLiveAudioFormat::from_name)
            .is_none()
    {
        return Err(format!(
            "Gemini Live session.update field `{path}.format` is invalid"
        ));
    }
    if ["rate", "sample_rate", "sampleRate"]
        .into_iter()
        .filter_map(|field| object.get(field))
        .any(|rate| rate.as_u64().is_none_or(|rate| rate == 0))
    {
        return Err(format!(
            "Gemini Live session.update field `{path}.rate` must be a positive integer"
        ));
    }
    Ok(())
}

fn live_audio_format_name(
    format: super::super::audio::GeminiProviderCoreLiveAudioFormat,
) -> &'static str {
    match format {
        super::super::audio::GeminiProviderCoreLiveAudioFormat::Pcm16 => "pcm16",
        super::super::audio::GeminiProviderCoreLiveAudioFormat::G711Ulaw => "g711_ulaw",
        super::super::audio::GeminiProviderCoreLiveAudioFormat::G711Alaw => "g711_alaw",
    }
}
