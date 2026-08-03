use std::net::IpAddr;

use base64::{Engine, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};

pub const LOCAL_BRIDGE_HEALTH_PATH: &str = "/health";
pub const LOCAL_BRIDGE_MODELS_PATH: &str = "/v1/models";
pub const LOCAL_BRIDGE_RESPONSES_PATH: &str = "/v1/responses";
pub const LOCAL_BRIDGE_CHAT_COMPLETIONS_PATH: &str = "/v1/chat/completions";
pub const LOCAL_BRIDGE_EMBEDDINGS_PATH: &str = "/v1/embeddings";
pub const LOCAL_BRIDGE_IMAGES_GENERATIONS_PATH: &str = "/v1/images/generations";
pub const LOCAL_BRIDGE_IMAGES_EDITS_PATH: &str = "/v1/images/edits";
pub const LOCAL_BRIDGE_IMAGES_VARIATIONS_PATH: &str = "/v1/images/variations";
pub const LOCAL_BRIDGE_AUDIO_SPEECH_PATH: &str = "/v1/audio/speech";
pub const LOCAL_BRIDGE_AUDIO_TRANSCRIPTIONS_PATH: &str = "/v1/audio/transcriptions";
pub const LOCAL_BRIDGE_AUDIO_TRANSLATIONS_PATH: &str = "/v1/audio/translations";
pub const LOCAL_BRIDGE_BATCHES_PATH: &str = "/v1/batches";
pub const LOCAL_BRIDGE_RERANK_PATH: &str = "/v1/rerank";
pub const LOCAL_BRIDGE_A2A_PATH: &str = "/v1/a2a";
pub const LOCAL_BRIDGE_MESSAGES_PATH: &str = "/v1/messages";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalBridgeRoute {
    Health,
    Models,
    Responses,
    ChatCompletions,
    Embeddings,
    ImagesGenerations,
    ImagesEdits,
    ImagesVariations,
    AudioSpeech,
    AudioTranscriptions,
    AudioTranslations,
    Batches,
    Batch,
    Rerank,
    A2a,
    Messages,
}

impl LocalBridgeRoute {
    pub fn as_path(self) -> &'static str {
        match self {
            Self::Health => LOCAL_BRIDGE_HEALTH_PATH,
            Self::Models => LOCAL_BRIDGE_MODELS_PATH,
            Self::Responses => LOCAL_BRIDGE_RESPONSES_PATH,
            Self::ChatCompletions => LOCAL_BRIDGE_CHAT_COMPLETIONS_PATH,
            Self::Embeddings => LOCAL_BRIDGE_EMBEDDINGS_PATH,
            Self::ImagesGenerations => LOCAL_BRIDGE_IMAGES_GENERATIONS_PATH,
            Self::ImagesEdits => LOCAL_BRIDGE_IMAGES_EDITS_PATH,
            Self::ImagesVariations => LOCAL_BRIDGE_IMAGES_VARIATIONS_PATH,
            Self::AudioSpeech => LOCAL_BRIDGE_AUDIO_SPEECH_PATH,
            Self::AudioTranscriptions => LOCAL_BRIDGE_AUDIO_TRANSCRIPTIONS_PATH,
            Self::AudioTranslations => LOCAL_BRIDGE_AUDIO_TRANSLATIONS_PATH,
            Self::Batches | Self::Batch => LOCAL_BRIDGE_BATCHES_PATH,
            Self::Rerank => LOCAL_BRIDGE_RERANK_PATH,
            Self::A2a => LOCAL_BRIDGE_A2A_PATH,
            Self::Messages => LOCAL_BRIDGE_MESSAGES_PATH,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalBridgeRequestClass {
    pub route: LocalBridgeRoute,
    pub method: String,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalBridgeRequestRejection {
    MethodNotAllowed,
    PathNotFound,
}

pub fn local_bridge_classify_request(
    method: &str,
    path_and_query: &str,
) -> Result<LocalBridgeRequestClass, LocalBridgeRequestRejection> {
    let method = method.trim();
    let path = path_and_query
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(path_and_query);

    let route = local_bridge_route(path).ok_or(LocalBridgeRequestRejection::PathNotFound)?;
    if !local_bridge_method_is_allowed(route, method) {
        return Err(LocalBridgeRequestRejection::MethodNotAllowed);
    }

    Ok(LocalBridgeRequestClass {
        route,
        method: method.to_ascii_uppercase(),
        path: path.to_string(),
    })
}

fn local_bridge_route(path: &str) -> Option<LocalBridgeRoute> {
    match path {
        LOCAL_BRIDGE_HEALTH_PATH => Some(LocalBridgeRoute::Health),
        path if path == LOCAL_BRIDGE_MODELS_PATH
            || path
                .strip_prefix("/v1/models/")
                .is_some_and(|id| !id.is_empty()) =>
        {
            Some(LocalBridgeRoute::Models)
        }
        LOCAL_BRIDGE_RESPONSES_PATH => Some(LocalBridgeRoute::Responses),
        LOCAL_BRIDGE_CHAT_COMPLETIONS_PATH => Some(LocalBridgeRoute::ChatCompletions),
        LOCAL_BRIDGE_EMBEDDINGS_PATH => Some(LocalBridgeRoute::Embeddings),
        LOCAL_BRIDGE_IMAGES_GENERATIONS_PATH => Some(LocalBridgeRoute::ImagesGenerations),
        LOCAL_BRIDGE_IMAGES_EDITS_PATH => Some(LocalBridgeRoute::ImagesEdits),
        LOCAL_BRIDGE_IMAGES_VARIATIONS_PATH => Some(LocalBridgeRoute::ImagesVariations),
        LOCAL_BRIDGE_AUDIO_SPEECH_PATH => Some(LocalBridgeRoute::AudioSpeech),
        LOCAL_BRIDGE_AUDIO_TRANSCRIPTIONS_PATH => Some(LocalBridgeRoute::AudioTranscriptions),
        LOCAL_BRIDGE_AUDIO_TRANSLATIONS_PATH => Some(LocalBridgeRoute::AudioTranslations),
        LOCAL_BRIDGE_BATCHES_PATH => Some(LocalBridgeRoute::Batches),
        path if path
            .strip_prefix("/v1/batches/")
            .is_some_and(|suffix| !suffix.is_empty()) =>
        {
            Some(LocalBridgeRoute::Batch)
        }
        LOCAL_BRIDGE_RERANK_PATH => Some(LocalBridgeRoute::Rerank),
        LOCAL_BRIDGE_A2A_PATH => Some(LocalBridgeRoute::A2a),
        LOCAL_BRIDGE_MESSAGES_PATH => Some(LocalBridgeRoute::Messages),
        _ => None,
    }
}

fn local_bridge_method_is_allowed(route: LocalBridgeRoute, method: &str) -> bool {
    let allowed = match route {
        LocalBridgeRoute::Health | LocalBridgeRoute::Models => &["GET", "HEAD"][..],
        LocalBridgeRoute::Batches => &["POST", "GET"][..],
        LocalBridgeRoute::Batch => &["GET", "POST", "DELETE"][..],
        _ => &["POST"][..],
    };
    allowed
        .iter()
        .any(|allowed| method.eq_ignore_ascii_case(allowed))
}

pub fn local_bridge_is_allowed_request(method: &str, path_and_query: &str) -> bool {
    local_bridge_classify_request(method, path_and_query).is_ok()
}

pub fn local_bridge_host_is_loopback(host_authority: &str) -> bool {
    local_bridge_host_name(host_authority).is_some_and(|host| {
        host.eq_ignore_ascii_case("localhost")
            || host.parse::<IpAddr>().is_ok_and(|addr| addr.is_loopback())
    })
}

pub fn local_bridge_host_name(host_authority: &str) -> Option<&str> {
    let value = host_authority.trim();
    if value.is_empty() || value.contains('/') || value.contains('@') {
        return None;
    }

    if let Some(rest) = value.strip_prefix('[') {
        let (host, rest) = rest.split_once(']')?;
        if rest.is_empty()
            || rest
                .strip_prefix(':')
                .is_some_and(|port| !port.is_empty() && port.chars().all(|ch| ch.is_ascii_digit()))
        {
            return Some(host);
        }
        return None;
    }

    match value.rsplit_once(':') {
        Some((host, port))
            if !host.contains(':')
                && !host.is_empty()
                && !port.is_empty()
                && port.chars().all(|ch| ch.is_ascii_digit()) =>
        {
            Some(host)
        }
        Some(_) if value.contains(':') => Some(value),
        _ => Some(value),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalBridgeHeaderFilter {
    pub strip_decoded_content_encoding: bool,
}

impl LocalBridgeHeaderFilter {
    pub const FOR_ENCODED_BODY: Self = Self {
        strip_decoded_content_encoding: false,
    };

    pub const FOR_DECODED_BODY: Self = Self {
        strip_decoded_content_encoding: true,
    };
}

pub fn local_bridge_should_skip_response_header(
    name: &str,
    filter: LocalBridgeHeaderFilter,
) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    ) || (filter.strip_decoded_content_encoding
        && matches!(lower.as_str(), "content-encoding" | "content-length"))
}

pub fn local_bridge_filter_text_response_header(
    name: &str,
    value: &str,
    filter: LocalBridgeHeaderFilter,
) -> Option<(String, String)> {
    (!local_bridge_should_skip_response_header(name, filter))
        .then(|| (name.to_string(), value.to_string()))
}

pub fn local_bridge_filter_text_response_headers<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a str)>,
    filter: LocalBridgeHeaderFilter,
) -> Vec<(String, String)> {
    let headers = headers.into_iter().collect::<Vec<_>>();
    let connection_headers = crate::runtime_connection_header_tokens(headers.iter().copied());
    headers
        .into_iter()
        .filter(|(name, _)| {
            !crate::runtime_header_name_matches_connection_token(name, &connection_headers)
        })
        .filter_map(|(name, value)| local_bridge_filter_text_response_header(name, value, filter))
        .collect()
}

pub fn local_bridge_authorization_bearer_token(value: &str) -> Option<&str> {
    let value = value.trim();
    let (scheme, token) = value.split_once(char::is_whitespace)?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = token.trim();
    (!token.is_empty() && !token.chars().any(char::is_whitespace)).then_some(token)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalBridgeBearerTokenHash {
    algorithm: &'static str,
    hash: [u8; 32],
}

impl LocalBridgeBearerTokenHash {
    pub fn from_token(token: &str) -> Self {
        Self {
            algorithm: "sha256",
            hash: sha256(token.as_bytes()),
        }
    }

    pub fn algorithm(&self) -> &'static str {
        self.algorithm
    }

    pub fn hash_bytes(&self) -> &[u8; 32] {
        &self.hash
    }

    pub fn hash_base64(&self) -> String {
        STANDARD.encode(self.hash)
    }

    pub fn from_hash_base64(value: &str) -> Option<Self> {
        let bytes = STANDARD.decode(value).ok()?;
        let hash: [u8; 32] = bytes.try_into().ok()?;
        Some(Self {
            algorithm: "sha256",
            hash,
        })
    }

    pub fn verify_bearer_token(&self, token: &str) -> bool {
        constant_time_eq(
            self.hash.as_slice(),
            Self::from_token(token).hash.as_slice(),
        )
    }

    pub fn verify_authorization_header(&self, authorization: &str) -> bool {
        local_bridge_authorization_bearer_token(authorization)
            .is_some_and(|token| self.verify_bearer_token(token))
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let max_len = left.len().max(right.len());
    let mut diff = left.len() ^ right.len();
    for index in 0..max_len {
        let left_byte = left.get(index).copied().unwrap_or(0);
        let right_byte = right.get(index).copied().unwrap_or(0);
        diff |= usize::from(left_byte ^ right_byte);
    }
    diff == 0
}

fn sha256(input: &[u8]) -> [u8; 32] {
    Sha256::digest(input).into()
}

#[cfg(test)]
mod tests {
    use super::LocalBridgeBearerTokenHash;

    #[test]
    fn bearer_token_hash_base64_decode_is_exact() {
        let encoded = LocalBridgeBearerTokenHash::from_token("secret").hash_base64();

        assert!(LocalBridgeBearerTokenHash::from_hash_base64(&encoded).is_some());
        assert!(LocalBridgeBearerTokenHash::from_hash_base64(&format!(" {encoded}")).is_none());
        assert!(LocalBridgeBearerTokenHash::from_hash_base64(&format!("{encoded}\n")).is_none());
    }
}
