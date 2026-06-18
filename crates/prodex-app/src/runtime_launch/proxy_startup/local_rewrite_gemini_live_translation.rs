use super::GEMINI_LIVE_DEFAULT_MODEL;
use super::local_rewrite_gemini_live_audio::{
    GEMINI_LIVE_AUDIO_RATE, RuntimeGeminiLiveAudioConfig, RuntimeGeminiLiveAudioFormat,
    RuntimeGeminiLiveAudioPayload, runtime_gemini_live_audio_rate_from_mime,
    runtime_gemini_live_decode_alaw, runtime_gemini_live_decode_ulaw,
    runtime_gemini_live_legacy_session_audio_config, runtime_gemini_live_session_audio_config,
};
use anyhow::{Context, Result};
use base64::Engine;
use std::collections::HashMap;

pub(super) struct RuntimeGeminiLiveClientTranslation {
    pub(super) upstream_messages: Vec<serde_json::Value>,
    pub(super) local_events: Vec<serde_json::Value>,
    pub(super) wait_for_setup: bool,
    pub(super) wait_for_turn: bool,
}

pub(super) struct RuntimeGeminiLiveServerTranslation {
    pub(super) events: Vec<serde_json::Value>,
    pub(super) setup_complete: bool,
    pub(super) turn_complete: bool,
}

pub(super) struct RuntimeGeminiLiveState {
    request_id: u64,
    turn_sequence: u64,
    response_id: String,
    item_id: String,
    response_created: bool,
    input_audio_format: RuntimeGeminiLiveAudioFormat,
    input_audio_rate: u64,
    output_audio_rate: u64,
    input_transcript: String,
    output_transcript: String,
    output_text: String,
    tool_names_by_call_id: HashMap<String, String>,
    suppress_current_turn: bool,
}

impl RuntimeGeminiLiveState {
    pub(super) fn new(request_id: u64) -> Self {
        Self {
            request_id,
            turn_sequence: 1,
            response_id: format!("resp_gemini_live_{request_id}"),
            item_id: format!("item_gemini_live_{request_id}"),
            response_created: false,
            input_audio_format: RuntimeGeminiLiveAudioFormat::Pcm16,
            input_audio_rate: GEMINI_LIVE_AUDIO_RATE,
            output_audio_rate: GEMINI_LIVE_AUDIO_RATE,
            input_transcript: String::new(),
            output_transcript: String::new(),
            output_text: String::new(),
            tool_names_by_call_id: HashMap::new(),
            suppress_current_turn: false,
        }
    }

    pub(super) fn translate_client_message(
        &mut self,
        text: &str,
    ) -> Result<RuntimeGeminiLiveClientTranslation> {
        let value: serde_json::Value =
            serde_json::from_str(text).context("failed to parse Codex realtime JSON")?;
        let message_type = value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let mut upstream_messages = Vec::new();
        let mut local_events = Vec::new();
        let mut wait_for_setup = false;
        let mut wait_for_turn = false;
        match message_type {
            "session.update" => {
                let session = value
                    .get("session")
                    .and_then(serde_json::Value::as_object)
                    .context("Codex realtime session.update is missing session")?;
                let input_audio = runtime_gemini_live_session_audio_config(session, "input")
                    .or_else(|| {
                        runtime_gemini_live_legacy_session_audio_config(
                            session,
                            "input_audio_format",
                            "inputAudioFormat",
                        )
                    })
                    .unwrap_or(RuntimeGeminiLiveAudioConfig {
                        format: RuntimeGeminiLiveAudioFormat::Pcm16,
                        rate: GEMINI_LIVE_AUDIO_RATE,
                    });
                self.input_audio_format = input_audio.format;
                self.input_audio_rate = input_audio.rate;
                if let Some(output_audio) =
                    runtime_gemini_live_session_audio_config(session, "output").or_else(|| {
                        runtime_gemini_live_legacy_session_audio_config(
                            session,
                            "output_audio_format",
                            "outputAudioFormat",
                        )
                    })
                {
                    self.output_audio_rate = output_audio.rate;
                }
                upstream_messages.push(runtime_gemini_live_setup_message(session));
                wait_for_setup = true;
            }
            "input_audio_buffer.append" => {
                let audio = value
                    .get("audio")
                    .and_then(serde_json::Value::as_str)
                    .context("Codex realtime audio append is missing audio")?;
                let audio = self.translate_input_audio(audio)?;
                upstream_messages.push(serde_json::json!({
                    "realtime_input": {
                        "audio": {
                            "data": audio.data,
                            "mime_type": audio.mime_type,
                        }
                    }
                }));
            }
            "input_audio_buffer.commit" => {
                upstream_messages.push(serde_json::json!({
                    "realtime_input": {
                        "audio_stream_end": true,
                    }
                }));
                wait_for_turn = true;
            }
            "input_audio_buffer.clear" => {
                self.input_transcript.clear();
                local_events.push(serde_json::json!({
                    "type": "input_audio_buffer.cleared",
                }));
            }
            "response.create" => {
                local_events.extend(self.ensure_response_created());
                wait_for_turn = true;
            }
            "response.cancel" => {
                self.suppress_current_turn = true;
                self.response_created = false;
                self.input_transcript.clear();
                self.output_transcript.clear();
                self.output_text.clear();
                local_events.push(serde_json::json!({
                    "type": "response.cancelled",
                    "response": {"id": self.response_id},
                }));
            }
            "conversation.item.truncate" => {
                local_events.push(serde_json::json!({
                    "type": "conversation.item.truncated",
                    "item_id": value.get("item_id").cloned().unwrap_or(serde_json::Value::Null),
                    "content_index": value.get("content_index").cloned().unwrap_or(serde_json::Value::Null),
                    "audio_end_ms": value.get("audio_end_ms").cloned().unwrap_or(serde_json::Value::Null),
                }));
            }
            "conversation.item.delete" => {
                local_events.push(serde_json::json!({
                    "type": "conversation.item.deleted",
                    "item_id": value.get("item_id").cloned().unwrap_or(serde_json::Value::Null),
                }));
            }
            "conversation.item.create" | "conversation.handoff.append" => {
                if let Some(message) = runtime_gemini_live_client_content_message(&value) {
                    upstream_messages.push(message);
                    wait_for_turn = true;
                }
                if let Some((call_id, message)) =
                    runtime_gemini_live_tool_response_message(&value, &self.tool_names_by_call_id)
                {
                    upstream_messages.push(message);
                    self.tool_names_by_call_id.remove(&call_id);
                    wait_for_turn = true;
                }
            }
            "response.processed" => {}
            _ => {
                local_events.push(serde_json::json!({
                    "type": "error",
                    "error": {
                        "type": "invalid_request_error",
                        "message": format!("Unsupported Codex realtime event type: {message_type}"),
                    }
                }));
            }
        }
        Ok(RuntimeGeminiLiveClientTranslation {
            upstream_messages,
            local_events,
            wait_for_setup,
            wait_for_turn,
        })
    }

    pub(super) fn translate_server_message(
        &mut self,
        text: &str,
    ) -> Result<RuntimeGeminiLiveServerTranslation> {
        let value: serde_json::Value =
            serde_json::from_str(text).context("failed to parse Gemini Live JSON")?;
        let setup_complete =
            runtime_gemini_live_field(&value, "setupComplete", "setup_complete").is_some();
        let mut events = Vec::new();
        if setup_complete {
            events.push(serde_json::json!({
                "type": "session.updated",
                "session": {
                    "id": "sess_gemini_live",
                    "type": "realtime",
                }
            }));
        }
        if let Some(error) = value.get("error") {
            events.push(serde_json::json!({
                "type": "error",
            "error": error,
            }));
        }
        if self.suppress_current_turn {
            let turn_complete = runtime_gemini_live_server_turn_complete(&value);
            if turn_complete {
                self.suppress_current_turn = false;
                self.response_created = false;
                self.input_transcript.clear();
                self.output_transcript.clear();
                self.output_text.clear();
                self.advance_turn();
            }
            return Ok(RuntimeGeminiLiveServerTranslation {
                events,
                setup_complete,
                turn_complete,
            });
        }
        if let Some(tool_call) = runtime_gemini_live_field(&value, "toolCall", "tool_call") {
            events.extend(self.ensure_response_created());
            if let Some(function_calls) =
                runtime_gemini_live_field(tool_call, "functionCalls", "function_calls")
                    .and_then(serde_json::Value::as_array)
            {
                for function_call in function_calls {
                    let call_id = function_call
                        .get("id")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("call_gemini_live");
                    let name = function_call
                        .get("name")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("tool_call");
                    self.tool_names_by_call_id
                        .insert(call_id.to_string(), name.to_string());
                    let args = function_call
                        .get("args")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!({}));
                    events.push(serde_json::json!({
                        "type": "conversation.item.done",
                        "item": {
                            "id": call_id,
                            "type": "function_call",
                            "name": name,
                            "call_id": call_id,
                            "arguments": serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string()),
                        }
                    }));
                }
            }
        }
        let mut turn_complete = false;
        if let Some(content) = runtime_gemini_live_field(&value, "serverContent", "server_content")
        {
            if runtime_gemini_live_field(content, "interrupted", "interrupted")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                events.push(serde_json::json!({
                    "type": "response.cancelled",
                    "response": {"id": self.response_id},
                }));
            }
            if let Some(text) = runtime_gemini_live_transcription_text(
                content,
                "inputTranscription",
                "input_transcription",
            ) {
                let delta = runtime_gemini_live_transcript_delta(&self.input_transcript, text);
                self.input_transcript = text.to_string();
                if !delta.is_empty() {
                    events.push(serde_json::json!({
                        "type": "conversation.item.input_audio_transcription.delta",
                        "item_id": self.item_id,
                        "delta": delta,
                    }));
                }
            }
            if let Some(text) = runtime_gemini_live_transcription_text(
                content,
                "outputTranscription",
                "output_transcription",
            ) {
                events.extend(self.ensure_response_created());
                let delta = runtime_gemini_live_transcript_delta(&self.output_transcript, text);
                self.output_transcript = text.to_string();
                if !delta.is_empty() {
                    events.push(serde_json::json!({
                        "type": "response.output_audio_transcript.delta",
                        "item_id": self.item_id,
                        "response_id": self.response_id,
                        "delta": delta,
                    }));
                }
            }
            if let Some(parts) = runtime_gemini_live_field(content, "modelTurn", "model_turn")
                .and_then(|turn| turn.get("parts"))
                .and_then(serde_json::Value::as_array)
            {
                events.extend(self.ensure_response_created());
                for part in parts {
                    if let Some(text) = part.get("text").and_then(serde_json::Value::as_str) {
                        self.output_text.push_str(text);
                        events.push(serde_json::json!({
                            "type": "response.output_text.delta",
                            "item_id": self.item_id,
                            "response_id": self.response_id,
                            "delta": text,
                        }));
                    }
                    if let Some(inline) =
                        runtime_gemini_live_field(part, "inlineData", "inline_data")
                        && let Some(data) = inline.get("data").and_then(serde_json::Value::as_str)
                    {
                        let sample_rate = inline
                            .get("mimeType")
                            .or_else(|| inline.get("mime_type"))
                            .and_then(serde_json::Value::as_str)
                            .and_then(runtime_gemini_live_audio_rate_from_mime)
                            .unwrap_or(self.output_audio_rate);
                        self.output_audio_rate = sample_rate;
                        events.push(serde_json::json!({
                            "type": "response.output_audio.delta",
                            "item_id": self.item_id,
                            "response_id": self.response_id,
                            "delta": data,
                            "sample_rate": sample_rate,
                            "channels": 1,
                        }));
                    }
                }
            }
            turn_complete = runtime_gemini_live_server_turn_complete(&value);
            if turn_complete {
                if !self.input_transcript.is_empty() {
                    events.push(serde_json::json!({
                        "type": "conversation.item.input_audio_transcription.completed",
                        "item_id": self.item_id,
                        "transcript": self.input_transcript,
                    }));
                }
                if !self.output_transcript.is_empty() {
                    events.push(serde_json::json!({
                        "type": "response.output_audio_transcript.done",
                        "item_id": self.item_id,
                        "response_id": self.response_id,
                        "transcript": self.output_transcript,
                    }));
                }
                if !self.output_text.is_empty() {
                    events.push(serde_json::json!({
                        "type": "response.output_text.done",
                        "item_id": self.item_id,
                        "response_id": self.response_id,
                        "text": self.output_text,
                    }));
                }
                events.push(serde_json::json!({
                    "type": "response.done",
                    "response": {"id": self.response_id},
                }));
                self.response_created = false;
                self.input_transcript.clear();
                self.output_transcript.clear();
                self.output_text.clear();
                self.advance_turn();
            }
        }
        Ok(RuntimeGeminiLiveServerTranslation {
            events,
            setup_complete,
            turn_complete,
        })
    }

    fn ensure_response_created(&mut self) -> Vec<serde_json::Value> {
        if self.response_created {
            return Vec::new();
        }
        self.response_created = true;
        vec![serde_json::json!({
            "type": "response.created",
            "response": {"id": self.response_id},
        })]
    }

    fn advance_turn(&mut self) {
        self.turn_sequence += 1;
        self.response_id = format!(
            "resp_gemini_live_{}_{}",
            self.request_id, self.turn_sequence
        );
        self.item_id = format!(
            "item_gemini_live_{}_{}",
            self.request_id, self.turn_sequence
        );
    }

    fn translate_input_audio(&self, audio: &str) -> Result<RuntimeGeminiLiveAudioPayload> {
        match self.input_audio_format {
            RuntimeGeminiLiveAudioFormat::Pcm16 => Ok(RuntimeGeminiLiveAudioPayload {
                data: audio.to_string(),
                mime_type: format!("audio/pcm;rate={}", self.input_audio_rate),
            }),
            RuntimeGeminiLiveAudioFormat::G711Ulaw | RuntimeGeminiLiveAudioFormat::G711Alaw => {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(audio)
                    .context("failed to decode Codex realtime G.711 audio")?;
                let mut pcm = Vec::with_capacity(bytes.len().saturating_mul(2));
                for byte in bytes {
                    let sample = match self.input_audio_format {
                        RuntimeGeminiLiveAudioFormat::G711Ulaw => {
                            runtime_gemini_live_decode_ulaw(byte)
                        }
                        RuntimeGeminiLiveAudioFormat::G711Alaw => {
                            runtime_gemini_live_decode_alaw(byte)
                        }
                        RuntimeGeminiLiveAudioFormat::Pcm16 => unreachable!(),
                    };
                    pcm.extend_from_slice(&sample.to_le_bytes());
                }
                Ok(RuntimeGeminiLiveAudioPayload {
                    data: base64::engine::general_purpose::STANDARD.encode(pcm),
                    mime_type: format!("audio/pcm;rate={}", self.input_audio_rate),
                })
            }
        }
    }
}

fn runtime_gemini_live_setup_message(
    session: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let configured_model = std::env::var("PRODEX_GEMINI_LIVE_MODEL").ok();
    let requested_model = session
        .get("model")
        .and_then(serde_json::Value::as_str)
        .filter(|model| model.starts_with("gemini-"));
    let model = configured_model
        .as_deref()
        .filter(|model| !model.trim().is_empty())
        .or(requested_model)
        .unwrap_or(GEMINI_LIVE_DEFAULT_MODEL);
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
                .filter_map(runtime_gemini_live_function_declaration)
                .collect::<Vec<_>>()
        })
        .filter(|tools| !tools.is_empty())
    {
        setup["tools"] = serde_json::json!([{"function_declarations": tools}]);
    }
    serde_json::json!({"setup": setup})
}

fn runtime_gemini_live_function_declaration(tool: &serde_json::Value) -> Option<serde_json::Value> {
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

fn runtime_gemini_live_client_content_message(
    value: &serde_json::Value,
) -> Option<serde_json::Value> {
    let item = value.get("item")?;
    if item.get("type").and_then(serde_json::Value::as_str) != Some("message") {
        return None;
    }
    let text = item
        .get("content")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .filter_map(|content| content.get("text").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then(|| {
        serde_json::json!({
            "client_content": {
                "turns": [{"role": "user", "parts": [{"text": text}]}],
                "turn_complete": true,
            }
        })
    })
}

fn runtime_gemini_live_tool_response_message(
    value: &serde_json::Value,
    tool_names_by_call_id: &HashMap<String, String>,
) -> Option<(String, serde_json::Value)> {
    let item = value.get("item")?;
    if item.get("type").and_then(serde_json::Value::as_str) != Some("function_call_output") {
        return None;
    }
    let call_id = item.get("call_id").and_then(serde_json::Value::as_str)?;
    let output = item
        .get("output")
        .cloned()
        .unwrap_or_else(|| serde_json::Value::String(String::new()));
    let name = tool_names_by_call_id
        .get(call_id)
        .map(String::as_str)
        .unwrap_or(call_id);
    Some((
        call_id.to_string(),
        serde_json::json!({
            "tool_response": {
                "function_responses": [{
                    "id": call_id,
                    "name": name,
                    "response": {"output": output},
                }]
            }
        }),
    ))
}

fn runtime_gemini_live_field<'a>(
    value: &'a serde_json::Value,
    camel: &str,
    snake: &str,
) -> Option<&'a serde_json::Value> {
    value.get(camel).or_else(|| value.get(snake))
}

fn runtime_gemini_live_server_turn_complete(value: &serde_json::Value) -> bool {
    runtime_gemini_live_field(value, "serverContent", "server_content")
        .and_then(|content| runtime_gemini_live_field(content, "turnComplete", "turn_complete"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn runtime_gemini_live_transcription_text<'a>(
    content: &'a serde_json::Value,
    camel: &str,
    snake: &str,
) -> Option<&'a str> {
    runtime_gemini_live_field(content, camel, snake)?
        .get("text")
        .and_then(serde_json::Value::as_str)
}

fn runtime_gemini_live_transcript_delta<'a>(previous: &str, current: &'a str) -> &'a str {
    current.strip_prefix(previous).unwrap_or(current)
}
