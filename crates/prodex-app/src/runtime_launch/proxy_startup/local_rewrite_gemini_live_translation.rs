use super::GEMINI_LIVE_DEFAULT_MODEL;
use anyhow::{Context, Result};
use prodex_domain::{CallId, RequestId};
use prodex_provider_core::{
    GEMINI_PROVIDER_CORE_LIVE_AUDIO_RATE as GEMINI_LIVE_AUDIO_RATE,
    GeminiProviderCoreLiveAudioConfig as RuntimeGeminiLiveAudioConfig,
    GeminiProviderCoreLiveAudioFormat as RuntimeGeminiLiveAudioFormat,
    GeminiProviderCoreLiveAudioPayload as RuntimeGeminiLiveAudioPayload,
    gemini_provider_core_live_audio_rate_from_mime as runtime_gemini_live_audio_rate_from_mime,
    gemini_provider_core_live_audio_stream_end_message,
    gemini_provider_core_live_client_content_message,
    gemini_provider_core_live_conversation_item_deleted_event,
    gemini_provider_core_live_conversation_item_truncated_event,
    gemini_provider_core_live_error_event, gemini_provider_core_live_field,
    gemini_provider_core_live_function_call_done_event,
    gemini_provider_core_live_input_audio_cleared_event,
    gemini_provider_core_live_input_audio_payload as runtime_gemini_live_input_audio_payload,
    gemini_provider_core_live_input_audio_transcription_completed_event,
    gemini_provider_core_live_input_audio_transcription_delta_event,
    gemini_provider_core_live_legacy_session_audio_config as runtime_gemini_live_legacy_session_audio_config,
    gemini_provider_core_live_output_audio_delta_event,
    gemini_provider_core_live_output_audio_transcript_delta_event,
    gemini_provider_core_live_output_audio_transcript_done_event,
    gemini_provider_core_live_output_text_delta_event,
    gemini_provider_core_live_output_text_done_event,
    gemini_provider_core_live_realtime_audio_message,
    gemini_provider_core_live_response_cancelled_event,
    gemini_provider_core_live_response_created_event,
    gemini_provider_core_live_response_done_event, gemini_provider_core_live_server_turn_complete,
    gemini_provider_core_live_session_audio_config as runtime_gemini_live_session_audio_config,
    gemini_provider_core_live_session_updated_event, gemini_provider_core_live_setup_message,
    gemini_provider_core_live_tool_response_message, gemini_provider_core_live_transcript_delta,
    gemini_provider_core_live_transcription_text,
    gemini_provider_core_live_unsupported_event_error,
};
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
    configured_model: Option<String>,
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
    #[cfg(test)]
    pub(super) fn new(_request_id: u64) -> Self {
        Self::new_with_model(_request_id, None)
    }

    pub(super) fn new_with_model(_request_id: u64, configured_model: Option<String>) -> Self {
        Self {
            configured_model,
            turn_sequence: 1,
            response_id: runtime_gemini_live_response_id(),
            item_id: runtime_gemini_live_item_id(),
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
                upstream_messages.push(runtime_gemini_live_setup_message(
                    session,
                    self.configured_model.as_deref(),
                ));
                wait_for_setup = true;
            }
            "input_audio_buffer.append" => {
                let audio = value
                    .get("audio")
                    .and_then(serde_json::Value::as_str)
                    .context("Codex realtime audio append is missing audio")?;
                let audio = self.translate_input_audio(audio)?;
                upstream_messages.push(gemini_provider_core_live_realtime_audio_message(
                    audio.data,
                    audio.mime_type,
                ));
            }
            "input_audio_buffer.commit" => {
                upstream_messages.push(gemini_provider_core_live_audio_stream_end_message());
                wait_for_turn = true;
            }
            "input_audio_buffer.clear" => {
                self.input_transcript.clear();
                local_events.push(gemini_provider_core_live_input_audio_cleared_event());
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
                local_events.push(gemini_provider_core_live_response_cancelled_event(
                    &self.response_id,
                ));
            }
            "conversation.item.truncate" => {
                local_events.push(gemini_provider_core_live_conversation_item_truncated_event(
                    &value,
                ));
            }
            "conversation.item.delete" => {
                local_events.push(gemini_provider_core_live_conversation_item_deleted_event(
                    &value,
                ));
            }
            "conversation.item.create" | "conversation.handoff.append" => {
                if let Some(message) = gemini_provider_core_live_client_content_message(&value) {
                    upstream_messages.push(message);
                    wait_for_turn = true;
                }
                if let Some((call_id, message)) = gemini_provider_core_live_tool_response_message(
                    &value,
                    &self.tool_names_by_call_id,
                ) {
                    upstream_messages.push(message);
                    self.tool_names_by_call_id.remove(&call_id);
                    wait_for_turn = true;
                }
            }
            "response.processed" => {}
            _ => {
                local_events.push(gemini_provider_core_live_unsupported_event_error(
                    message_type,
                ));
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
            gemini_provider_core_live_field(&value, "setupComplete", "setup_complete").is_some();
        let mut events = Vec::new();
        if setup_complete {
            events.push(gemini_provider_core_live_session_updated_event());
        }
        if let Some(error) = value.get("error") {
            events.push(gemini_provider_core_live_error_event(error));
        }
        if self.suppress_current_turn {
            let turn_complete = gemini_provider_core_live_server_turn_complete(&value);
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
        if let Some(tool_call) = gemini_provider_core_live_field(&value, "toolCall", "tool_call") {
            events.extend(self.ensure_response_created());
            if let Some(function_calls) =
                gemini_provider_core_live_field(tool_call, "functionCalls", "function_calls")
                    .and_then(serde_json::Value::as_array)
            {
                for function_call in function_calls {
                    let call_id = function_call
                        .get("id")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_else(runtime_gemini_live_call_id);
                    let name = function_call
                        .get("name")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("tool_call");
                    self.tool_names_by_call_id
                        .insert(call_id.clone(), name.to_string());
                    let args = function_call
                        .get("args")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!({}));
                    events.push(gemini_provider_core_live_function_call_done_event(
                        &call_id, name, &args,
                    ));
                }
            }
        }
        let mut turn_complete = false;
        if let Some(content) =
            gemini_provider_core_live_field(&value, "serverContent", "server_content")
        {
            if gemini_provider_core_live_field(content, "interrupted", "interrupted")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                events.push(gemini_provider_core_live_response_cancelled_event(
                    &self.response_id,
                ));
            }
            if let Some(text) = gemini_provider_core_live_transcription_text(
                content,
                "inputTranscription",
                "input_transcription",
            ) {
                let delta =
                    gemini_provider_core_live_transcript_delta(&self.input_transcript, text);
                self.input_transcript = text.to_string();
                if !delta.is_empty() {
                    events.push(
                        gemini_provider_core_live_input_audio_transcription_delta_event(
                            &self.item_id,
                            delta,
                        ),
                    );
                }
            }
            if let Some(text) = gemini_provider_core_live_transcription_text(
                content,
                "outputTranscription",
                "output_transcription",
            ) {
                events.extend(self.ensure_response_created());
                let delta =
                    gemini_provider_core_live_transcript_delta(&self.output_transcript, text);
                self.output_transcript = text.to_string();
                if !delta.is_empty() {
                    events.push(
                        gemini_provider_core_live_output_audio_transcript_delta_event(
                            &self.item_id,
                            &self.response_id,
                            delta,
                        ),
                    );
                }
            }
            if let Some(parts) = gemini_provider_core_live_field(content, "modelTurn", "model_turn")
                .and_then(|turn| turn.get("parts"))
                .and_then(serde_json::Value::as_array)
            {
                events.extend(self.ensure_response_created());
                for part in parts {
                    if let Some(text) = part.get("text").and_then(serde_json::Value::as_str) {
                        self.output_text.push_str(text);
                        events.push(gemini_provider_core_live_output_text_delta_event(
                            &self.item_id,
                            &self.response_id,
                            text,
                        ));
                    }
                    if let Some(inline) =
                        gemini_provider_core_live_field(part, "inlineData", "inline_data")
                        && let Some(data) = inline.get("data").and_then(serde_json::Value::as_str)
                    {
                        let sample_rate = inline
                            .get("mimeType")
                            .or_else(|| inline.get("mime_type"))
                            .and_then(serde_json::Value::as_str)
                            .and_then(runtime_gemini_live_audio_rate_from_mime)
                            .unwrap_or(self.output_audio_rate);
                        self.output_audio_rate = sample_rate;
                        events.push(gemini_provider_core_live_output_audio_delta_event(
                            &self.item_id,
                            &self.response_id,
                            data,
                            sample_rate,
                        ));
                    }
                }
            }
            turn_complete = gemini_provider_core_live_server_turn_complete(&value);
            if turn_complete {
                if !self.input_transcript.is_empty() {
                    events.push(
                        gemini_provider_core_live_input_audio_transcription_completed_event(
                            &self.item_id,
                            &self.input_transcript,
                        ),
                    );
                }
                if !self.output_transcript.is_empty() {
                    events.push(
                        gemini_provider_core_live_output_audio_transcript_done_event(
                            &self.item_id,
                            &self.response_id,
                            &self.output_transcript,
                        ),
                    );
                }
                if !self.output_text.is_empty() {
                    events.push(gemini_provider_core_live_output_text_done_event(
                        &self.item_id,
                        &self.response_id,
                        &self.output_text,
                    ));
                }
                events.push(gemini_provider_core_live_response_done_event(
                    &self.response_id,
                ));
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
        vec![gemini_provider_core_live_response_created_event(
            &self.response_id,
        )]
    }

    fn advance_turn(&mut self) {
        self.turn_sequence += 1;
        self.response_id = runtime_gemini_live_response_id();
        self.item_id = runtime_gemini_live_item_id();
    }

    fn translate_input_audio(&self, audio: &str) -> Result<RuntimeGeminiLiveAudioPayload> {
        runtime_gemini_live_input_audio_payload(
            audio,
            self.input_audio_format,
            self.input_audio_rate,
        )
        .map_err(anyhow::Error::msg)
    }
}

fn runtime_gemini_live_response_id() -> String {
    format!("resp_gemini_live_{}", RequestId::new())
}

fn runtime_gemini_live_item_id() -> String {
    format!("item_gemini_live_{}", RequestId::new())
}

fn runtime_gemini_live_call_id() -> String {
    format!("call_gemini_live_{}", CallId::new())
}

fn runtime_gemini_live_setup_message(
    session: &serde_json::Map<String, serde_json::Value>,
    configured_model: Option<&str>,
) -> serde_json::Value {
    gemini_provider_core_live_setup_message(session, GEMINI_LIVE_DEFAULT_MODEL, configured_model)
}
