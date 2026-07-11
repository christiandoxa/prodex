//! Gemini Live audio configuration and codec helpers.

#[path = "audio/codec.rs"]
mod codec;
#[path = "audio/payload.rs"]
mod payload;

pub use self::codec::{
    gemini_provider_core_live_decode_alaw, gemini_provider_core_live_decode_ulaw,
};
pub use self::payload::{
    GeminiProviderCoreLiveAudioPayload, gemini_provider_core_live_input_audio_payload,
};

pub const GEMINI_PROVIDER_CORE_LIVE_AUDIO_RATE: u64 = 24_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeminiProviderCoreLiveAudioFormat {
    Pcm16,
    G711Ulaw,
    G711Alaw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeminiProviderCoreLiveAudioConfig {
    pub format: GeminiProviderCoreLiveAudioFormat,
    pub rate: u64,
}

pub fn gemini_provider_core_live_session_audio_config(
    session: &serde_json::Map<String, serde_json::Value>,
    direction: &str,
) -> Option<GeminiProviderCoreLiveAudioConfig> {
    let fallback_key = format!("{direction}_audio_format");
    let format = session
        .get("audio")?
        .get(direction)?
        .get("format")
        .or_else(|| session.get(fallback_key.as_str()))?;
    gemini_provider_core_live_audio_config_from_value(format)
}

pub fn gemini_provider_core_live_legacy_session_audio_config(
    session: &serde_json::Map<String, serde_json::Value>,
    snake_key: &str,
    camel_key: &str,
) -> Option<GeminiProviderCoreLiveAudioConfig> {
    session
        .get(snake_key)
        .or_else(|| session.get(camel_key))
        .and_then(gemini_provider_core_live_audio_config_from_value)
}

pub fn gemini_provider_core_live_audio_config_from_value(
    value: &serde_json::Value,
) -> Option<GeminiProviderCoreLiveAudioConfig> {
    match value {
        serde_json::Value::String(value) => {
            let format = GeminiProviderCoreLiveAudioFormat::from_name(value)?;
            Some(GeminiProviderCoreLiveAudioConfig {
                format,
                rate: gemini_provider_core_live_audio_rate_from_mime(value)
                    .unwrap_or_else(|| format.default_rate()),
            })
        }
        serde_json::Value::Object(object) => {
            let format = object
                .get("type")
                .or_else(|| object.get("name"))
                .or_else(|| object.get("format"))
                .and_then(serde_json::Value::as_str)
                .and_then(GeminiProviderCoreLiveAudioFormat::from_name)
                .unwrap_or(GeminiProviderCoreLiveAudioFormat::Pcm16);
            let rate = object
                .get("rate")
                .or_else(|| object.get("sample_rate"))
                .or_else(|| object.get("sampleRate"))
                .and_then(serde_json::Value::as_u64)
                .filter(|rate| *rate > 0)
                .unwrap_or_else(|| format.default_rate());
            Some(GeminiProviderCoreLiveAudioConfig { format, rate })
        }
        _ => None,
    }
}

impl GeminiProviderCoreLiveAudioFormat {
    pub fn from_name(value: &str) -> Option<Self> {
        let normalized = value
            .trim()
            .split(';')
            .next()
            .unwrap_or(value)
            .to_ascii_lowercase()
            .replace(['-', '/'], "_");
        match normalized.as_str() {
            "pcm16" | "pcm" | "audio_pcm" | "linear16" => Some(Self::Pcm16),
            "g711_ulaw" | "mulaw" | "ulaw" | "pcmu" | "audio_pcmu" => Some(Self::G711Ulaw),
            "g711_alaw" | "alaw" | "pcma" | "audio_pcma" => Some(Self::G711Alaw),
            _ => None,
        }
    }

    pub fn default_rate(self) -> u64 {
        match self {
            Self::Pcm16 => GEMINI_PROVIDER_CORE_LIVE_AUDIO_RATE,
            Self::G711Ulaw | Self::G711Alaw => 8_000,
        }
    }
}

pub fn gemini_provider_core_live_audio_rate_from_mime(mime_type: &str) -> Option<u64> {
    mime_type.split(';').find_map(|part| {
        let part = part.trim();
        let value = part.strip_prefix("rate=")?;
        value.parse::<u64>().ok().filter(|rate| *rate > 0)
    })
}
