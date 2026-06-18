use super::*;
use base64::Engine;

#[test]
fn gemini_live_translates_codex_session_audio_and_text() {
    let mut state = RuntimeGeminiLiveState::new(7);
    let setup = state
        .translate_client_message(
            &serde_json::json!({
                "type": "session.update",
                "session": {
                    "instructions": "Be concise",
                    "output_modalities": ["audio"],
                    "audio": {"input": {"format": {"rate": 16000}}},
                    "tools": [{
                        "type": "function",
                        "name": "background_agent",
                        "description": "Delegate work",
                        "parameters": {"type": "object"}
                    }]
                }
            })
            .to_string(),
        )
        .unwrap();
    assert!(setup.wait_for_setup);
    assert_eq!(
        setup.upstream_messages[0]["setup"]["generation_config"]["response_modalities"][0],
        "AUDIO"
    );
    assert_eq!(
        setup.upstream_messages[0]["setup"]["tools"][0]["function_declarations"][0]["name"],
        "background_agent"
    );

    let audio = state
        .translate_client_message(
            &serde_json::json!({
                "type": "input_audio_buffer.append",
                "audio": "AAAA"
            })
            .to_string(),
        )
        .unwrap();
    assert_eq!(
        audio.upstream_messages[0]["realtime_input"]["audio"]["mime_type"],
        "audio/pcm;rate=16000"
    );
}

#[test]
fn gemini_live_transcodes_g711_input_audio_to_pcm() {
    let mut state = RuntimeGeminiLiveState::new(8);
    state
        .translate_client_message(
            &serde_json::json!({
                "type": "session.update",
                "session": {
                    "input_audio_format": "g711_ulaw"
                }
            })
            .to_string(),
        )
        .unwrap();
    let audio = base64::engine::general_purpose::STANDARD.encode([0xff_u8, 0x7f_u8]);
    let translated = state
        .translate_client_message(
            &serde_json::json!({
                "type": "input_audio_buffer.append",
                "audio": audio
            })
            .to_string(),
        )
        .unwrap();
    let upstream_audio = &translated.upstream_messages[0]["realtime_input"]["audio"];
    assert_eq!(upstream_audio["mime_type"], "audio/pcm;rate=8000");
    let pcm = base64::engine::general_purpose::STANDARD
        .decode(upstream_audio["data"].as_str().unwrap())
        .unwrap();
    assert_eq!(pcm.len(), 4);
}

#[test]
fn gemini_live_translates_server_audio_transcripts_tools_and_turn_completion() {
    let mut state = RuntimeGeminiLiveState::new(9);
    let translated = state
        .translate_server_message(
            &serde_json::json!({
                "serverContent": {
                    "inputTranscription": {"text": "hello"},
                    "outputTranscription": {"text": "hi"},
                    "modelTurn": {"parts": [
                        {"text": "hi"},
                        {"inlineData": {"data": "AAAA", "mimeType": "audio/pcm;rate=24000"}}
                    ]},
                    "turnComplete": true
                },
                "toolCall": {
                    "functionCalls": [{
                        "id": "call_bg",
                        "name": "background_agent",
                        "args": {"prompt": "work"}
                    }]
                }
            })
            .to_string(),
        )
        .unwrap();
    assert!(translated.turn_complete);
    let serialized = serde_json::to_string(&translated.events).unwrap();
    assert!(serialized.contains("conversation.item.input_audio_transcription.delta"));
    assert!(serialized.contains("response.output_audio.delta"));
    assert!(serialized.contains("conversation.item.done"));
    assert!(serialized.contains("response.done"));
}

#[test]
fn gemini_live_uses_server_audio_mime_rate_for_output_events() {
    let mut state = RuntimeGeminiLiveState::new(10);
    let translated = state
        .translate_server_message(
            &serde_json::json!({
                "serverContent": {
                    "modelTurn": {"parts": [
                        {"inlineData": {"data": "AAAA", "mimeType": "audio/pcm;rate=16000"}}
                    ]}
                }
            })
            .to_string(),
        )
        .unwrap();
    let audio = translated
        .events
        .iter()
        .find(|event| event["type"] == "response.output_audio.delta")
        .expect("audio delta event");
    assert_eq!(audio["sample_rate"], 16000);
}

#[test]
fn gemini_live_translates_function_output_to_tool_response() {
    let mut state = RuntimeGeminiLiveState::new(11);
    state
        .translate_server_message(
            &serde_json::json!({
                "toolCall": {
                    "functionCalls": [{
                        "id": "call_bg",
                        "name": "background_agent",
                        "args": {"prompt": "work"}
                    }]
                }
            })
            .to_string(),
        )
        .unwrap();
    let translated = state
        .translate_client_message(
            &serde_json::json!({
                "type": "conversation.item.create",
                "item": {
                    "type": "function_call_output",
                    "call_id": "call_bg",
                    "output": "done"
                }
            })
            .to_string(),
        )
        .unwrap();
    assert_eq!(
        translated.upstream_messages[0]["tool_response"]["function_responses"][0]["id"],
        "call_bg"
    );
    assert_eq!(
        translated.upstream_messages[0]["tool_response"]["function_responses"][0]["name"],
        "background_agent"
    );
    assert!(translated.wait_for_turn);
}

#[test]
fn gemini_live_accepts_codex_housekeeping_events_without_errors() {
    let mut state = RuntimeGeminiLiveState::new(12);
    for (event, expected) in [
        (
            serde_json::json!({"type": "input_audio_buffer.clear"}),
            "input_audio_buffer.cleared",
        ),
        (
            serde_json::json!({
                "type": "conversation.item.truncate",
                "item_id": "item_1",
                "content_index": 0,
                "audio_end_ms": 42
            }),
            "conversation.item.truncated",
        ),
        (
            serde_json::json!({
                "type": "conversation.item.delete",
                "item_id": "item_1"
            }),
            "conversation.item.deleted",
        ),
    ] {
        let translated = state
            .translate_client_message(&event.to_string())
            .expect("housekeeping event should translate");
        assert_eq!(translated.local_events[0]["type"], expected);
        assert!(translated.upstream_messages.is_empty());
    }
}

#[test]
fn gemini_live_suppresses_late_server_output_after_client_cancel() {
    let mut state = RuntimeGeminiLiveState::new(14);
    let created = state
        .translate_client_message(&serde_json::json!({"type": "response.create"}).to_string())
        .unwrap();
    assert_eq!(created.local_events[0]["type"], "response.created");
    let cancelled = state
        .translate_client_message(&serde_json::json!({"type": "response.cancel"}).to_string())
        .unwrap();
    assert_eq!(cancelled.local_events[0]["type"], "response.cancelled");

    let late_delta = state
        .translate_server_message(
            &serde_json::json!({
                "serverContent": {
                    "modelTurn": {"parts": [{"text": "late"}]},
                    "turnComplete": false
                }
            })
            .to_string(),
        )
        .unwrap();
    assert!(late_delta.events.is_empty());

    let late_done = state
        .translate_server_message(
            &serde_json::json!({
                "serverContent": {
                    "modelTurn": {"parts": [{"text": "ignored"}]},
                    "turnComplete": true
                }
            })
            .to_string(),
        )
        .unwrap();
    assert!(late_done.turn_complete);
    assert!(late_done.events.is_empty());

    let next = state
        .translate_server_message(
            &serde_json::json!({
                "serverContent": {
                    "modelTurn": {"parts": [{"text": "next"}]},
                    "turnComplete": true
                }
            })
            .to_string(),
        )
        .unwrap();
    let serialized = serde_json::to_string(&next.events).unwrap();
    assert!(serialized.contains("resp_gemini_live_14_2"));
    assert!(serialized.contains("next"));
}

#[test]
fn gemini_live_server_interruption_cancels_current_turn_and_recovers() {
    let mut state = RuntimeGeminiLiveState::new(15);
    let first = state
        .translate_server_message(
            &serde_json::json!({
                "serverContent": {
                    "modelTurn": {"parts": [{"text": "partial"}]},
                    "interrupted": true,
                    "turnComplete": true
                }
            })
            .to_string(),
        )
        .unwrap();
    let serialized = serde_json::to_string(&first.events).unwrap();
    assert!(serialized.contains("response.created"));
    assert!(serialized.contains("response.cancelled"));
    assert!(serialized.contains("response.done"));
    assert!(first.turn_complete);

    let next = state
        .translate_server_message(
            &serde_json::json!({
                "serverContent": {
                    "modelTurn": {"parts": [{"text": "recovered"}]},
                    "turnComplete": true
                }
            })
            .to_string(),
        )
        .unwrap();
    let next_serialized = serde_json::to_string(&next.events).unwrap();
    assert!(next_serialized.contains("resp_gemini_live_15_2"));
    assert!(next_serialized.contains("recovered"));
}

#[test]
fn gemini_live_uses_distinct_response_ids_per_turn() {
    let mut state = RuntimeGeminiLiveState::new(13);
    let first = state
        .translate_server_message(
            &serde_json::json!({
                "serverContent": {
                    "modelTurn": {"parts": [{"text": "one"}]},
                    "turnComplete": true
                }
            })
            .to_string(),
        )
        .unwrap();
    let second = state
        .translate_server_message(
            &serde_json::json!({
                "serverContent": {
                    "modelTurn": {"parts": [{"text": "two"}]},
                    "turnComplete": true
                }
            })
            .to_string(),
        )
        .unwrap();
    let first_serialized = serde_json::to_string(&first.events).unwrap();
    let second_serialized = serde_json::to_string(&second.events).unwrap();
    assert!(first_serialized.contains("resp_gemini_live_13"));
    assert!(second_serialized.contains("resp_gemini_live_13_2"));
}
