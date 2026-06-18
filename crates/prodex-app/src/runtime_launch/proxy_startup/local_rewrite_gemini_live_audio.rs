pub(super) const GEMINI_LIVE_AUDIO_RATE: u64 = 24_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RuntimeGeminiLiveAudioFormat {
    Pcm16,
    G711Ulaw,
    G711Alaw,
}

pub(super) struct RuntimeGeminiLiveAudioConfig {
    pub(super) format: RuntimeGeminiLiveAudioFormat,
    pub(super) rate: u64,
}

pub(super) struct RuntimeGeminiLiveAudioPayload {
    pub(super) data: String,
    pub(super) mime_type: String,
}

pub(super) fn runtime_gemini_live_session_audio_config(
    session: &serde_json::Map<String, serde_json::Value>,
    direction: &str,
) -> Option<RuntimeGeminiLiveAudioConfig> {
    let fallback_key = format!("{direction}_audio_format");
    let format = session
        .get("audio")?
        .get(direction)?
        .get("format")
        .or_else(|| session.get(fallback_key.as_str()))?;
    runtime_gemini_live_audio_config_from_value(format)
}

pub(super) fn runtime_gemini_live_legacy_session_audio_config(
    session: &serde_json::Map<String, serde_json::Value>,
    snake_key: &str,
    camel_key: &str,
) -> Option<RuntimeGeminiLiveAudioConfig> {
    session
        .get(snake_key)
        .or_else(|| session.get(camel_key))
        .and_then(runtime_gemini_live_audio_config_from_value)
}

fn runtime_gemini_live_audio_config_from_value(
    value: &serde_json::Value,
) -> Option<RuntimeGeminiLiveAudioConfig> {
    match value {
        serde_json::Value::String(value) => {
            let format = RuntimeGeminiLiveAudioFormat::from_name(value)?;
            Some(RuntimeGeminiLiveAudioConfig {
                format,
                rate: runtime_gemini_live_audio_rate_from_mime(value)
                    .unwrap_or_else(|| format.default_rate()),
            })
        }
        serde_json::Value::Object(object) => {
            let format = object
                .get("type")
                .or_else(|| object.get("name"))
                .or_else(|| object.get("format"))
                .and_then(serde_json::Value::as_str)
                .and_then(RuntimeGeminiLiveAudioFormat::from_name)
                .unwrap_or(RuntimeGeminiLiveAudioFormat::Pcm16);
            let rate = object
                .get("rate")
                .or_else(|| object.get("sample_rate"))
                .or_else(|| object.get("sampleRate"))
                .and_then(serde_json::Value::as_u64)
                .filter(|rate| *rate > 0)
                .unwrap_or_else(|| format.default_rate());
            Some(RuntimeGeminiLiveAudioConfig { format, rate })
        }
        _ => None,
    }
}

impl RuntimeGeminiLiveAudioFormat {
    fn from_name(value: &str) -> Option<Self> {
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

    fn default_rate(self) -> u64 {
        match self {
            Self::Pcm16 => GEMINI_LIVE_AUDIO_RATE,
            Self::G711Ulaw | Self::G711Alaw => 8_000,
        }
    }
}

pub(super) fn runtime_gemini_live_audio_rate_from_mime(mime_type: &str) -> Option<u64> {
    mime_type.split(';').find_map(|part| {
        let part = part.trim();
        let value = part.strip_prefix("rate=")?;
        value.parse::<u64>().ok().filter(|rate| *rate > 0)
    })
}

pub(super) fn runtime_gemini_live_decode_ulaw(byte: u8) -> i16 {
    let value = !byte;
    let sign = value & 0x80;
    let exponent = (value >> 4) & 0x07;
    let mantissa = value & 0x0f;
    let sample = ((((mantissa as i32) << 3) + 0x84) << exponent) - 0x84;
    if sign != 0 {
        -(sample as i16)
    } else {
        sample as i16
    }
}

pub(super) fn runtime_gemini_live_decode_alaw(byte: u8) -> i16 {
    let value = byte ^ 0x55;
    let sign = value & 0x80;
    let exponent = (value & 0x70) >> 4;
    let mantissa = value & 0x0f;
    let sample = if exponent == 0 {
        ((mantissa as i32) << 4) + 8
    } else {
        (((mantissa as i32) << 4) + 0x108) << (exponent - 1)
    };
    if sign != 0 {
        sample as i16
    } else {
        -(sample as i16)
    }
}
