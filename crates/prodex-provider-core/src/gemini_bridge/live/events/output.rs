//! Gemini Live output and transcript event builders.

pub fn gemini_provider_core_live_input_audio_transcription_delta_event(
    item_id: &str,
    delta: &str,
) -> serde_json::Value {
    serde_json::json!({
        "type": "conversation.item.input_audio_transcription.delta",
        "item_id": item_id,
        "delta": delta,
    })
}

pub fn gemini_provider_core_live_input_audio_transcription_completed_event(
    item_id: &str,
    transcript: &str,
) -> serde_json::Value {
    serde_json::json!({
        "type": "conversation.item.input_audio_transcription.completed",
        "item_id": item_id,
        "transcript": transcript,
    })
}

pub fn gemini_provider_core_live_output_audio_transcript_delta_event(
    item_id: &str,
    response_id: &str,
    delta: &str,
) -> serde_json::Value {
    serde_json::json!({
        "type": "response.output_audio_transcript.delta",
        "item_id": item_id,
        "response_id": response_id,
        "delta": delta,
    })
}

pub fn gemini_provider_core_live_output_audio_transcript_done_event(
    item_id: &str,
    response_id: &str,
    transcript: &str,
) -> serde_json::Value {
    serde_json::json!({
        "type": "response.output_audio_transcript.done",
        "item_id": item_id,
        "response_id": response_id,
        "transcript": transcript,
    })
}

pub fn gemini_provider_core_live_output_text_delta_event(
    item_id: &str,
    response_id: &str,
    delta: &str,
) -> serde_json::Value {
    serde_json::json!({
        "type": "response.output_text.delta",
        "item_id": item_id,
        "response_id": response_id,
        "delta": delta,
    })
}

pub fn gemini_provider_core_live_output_text_done_event(
    item_id: &str,
    response_id: &str,
    text: &str,
) -> serde_json::Value {
    serde_json::json!({
        "type": "response.output_text.done",
        "item_id": item_id,
        "response_id": response_id,
        "text": text,
    })
}

pub fn gemini_provider_core_live_output_audio_delta_event(
    item_id: &str,
    response_id: &str,
    data: &str,
    sample_rate: u64,
) -> serde_json::Value {
    serde_json::json!({
        "type": "response.output_audio.delta",
        "item_id": item_id,
        "response_id": response_id,
        "delta": data,
        "sample_rate": sample_rate,
        "channels": 1,
    })
}
