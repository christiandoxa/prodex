//! Gemini Live audio payload conversion helpers.

use base64::Engine;

use super::GeminiProviderCoreLiveAudioFormat;
use super::codec::{gemini_provider_core_live_decode_alaw, gemini_provider_core_live_decode_ulaw};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeminiProviderCoreLiveAudioPayload {
    pub data: String,
    pub mime_type: String,
}

pub fn gemini_provider_core_live_input_audio_payload(
    audio: &str,
    format: GeminiProviderCoreLiveAudioFormat,
    rate: u64,
) -> Result<GeminiProviderCoreLiveAudioPayload, String> {
    match format {
        GeminiProviderCoreLiveAudioFormat::Pcm16 => Ok(GeminiProviderCoreLiveAudioPayload {
            data: audio.to_string(),
            mime_type: format!("audio/pcm;rate={rate}"),
        }),
        GeminiProviderCoreLiveAudioFormat::G711Ulaw
        | GeminiProviderCoreLiveAudioFormat::G711Alaw => {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(audio)
                .map_err(|err| format!("failed to decode Codex realtime G.711 audio: {err}"))?;
            let mut pcm = Vec::with_capacity(bytes.len().saturating_mul(2));
            for byte in bytes {
                let sample = match format {
                    GeminiProviderCoreLiveAudioFormat::G711Ulaw => {
                        gemini_provider_core_live_decode_ulaw(byte)
                    }
                    GeminiProviderCoreLiveAudioFormat::G711Alaw => {
                        gemini_provider_core_live_decode_alaw(byte)
                    }
                    GeminiProviderCoreLiveAudioFormat::Pcm16 => unreachable!(),
                };
                pcm.extend_from_slice(&sample.to_le_bytes());
            }
            Ok(GeminiProviderCoreLiveAudioPayload {
                data: base64::engine::general_purpose::STANDARD.encode(pcm),
                mime_type: format!("audio/pcm;rate={rate}"),
            })
        }
    }
}
