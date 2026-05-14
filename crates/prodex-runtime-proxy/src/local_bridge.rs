use std::net::IpAddr;

use base64::{Engine, engine::general_purpose::STANDARD};

pub const LOCAL_BRIDGE_HEALTH_PATH: &str = "/health";
pub const LOCAL_BRIDGE_MODELS_PATH: &str = "/v1/models";
pub const LOCAL_BRIDGE_RESPONSES_PATH: &str = "/v1/responses";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalBridgeRoute {
    Health,
    Models,
    Responses,
}

impl LocalBridgeRoute {
    pub fn as_path(self) -> &'static str {
        match self {
            Self::Health => LOCAL_BRIDGE_HEALTH_PATH,
            Self::Models => LOCAL_BRIDGE_MODELS_PATH,
            Self::Responses => LOCAL_BRIDGE_RESPONSES_PATH,
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

    let route = match path {
        LOCAL_BRIDGE_HEALTH_PATH => {
            if !method.eq_ignore_ascii_case("GET") && !method.eq_ignore_ascii_case("HEAD") {
                return Err(LocalBridgeRequestRejection::MethodNotAllowed);
            }
            LocalBridgeRoute::Health
        }
        LOCAL_BRIDGE_MODELS_PATH => {
            if !method.eq_ignore_ascii_case("GET") && !method.eq_ignore_ascii_case("HEAD") {
                return Err(LocalBridgeRequestRejection::MethodNotAllowed);
            }
            LocalBridgeRoute::Models
        }
        LOCAL_BRIDGE_RESPONSES_PATH => {
            if !method.eq_ignore_ascii_case("POST") {
                return Err(LocalBridgeRequestRejection::MethodNotAllowed);
            }
            LocalBridgeRoute::Responses
        }
        _ => return Err(LocalBridgeRequestRejection::PathNotFound),
    };

    Ok(LocalBridgeRequestClass {
        route,
        method: method.to_ascii_uppercase(),
        path: path.to_string(),
    })
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
    let connection_headers = local_bridge_connection_header_tokens(headers.iter().copied());
    headers
        .into_iter()
        .filter(|(name, _)| {
            !connection_headers
                .iter()
                .any(|header_name| header_name.eq_ignore_ascii_case(name.trim()))
        })
        .filter_map(|(name, value)| local_bridge_filter_text_response_header(name, value, filter))
        .collect()
}

fn local_bridge_connection_header_tokens<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Vec<String> {
    headers
        .into_iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("connection"))
        .flat_map(|(_, value)| value.split(','))
        .map(str::trim)
        .filter(|token| {
            !token.is_empty()
                && token.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric()
                        || matches!(
                            byte,
                            b'!' | b'#'
                                | b'$'
                                | b'%'
                                | b'&'
                                | b'\''
                                | b'*'
                                | b'+'
                                | b'-'
                                | b'.'
                                | b'^'
                                | b'_'
                                | b'`'
                                | b'|'
                                | b'~'
                        )
                })
        })
        .map(str::to_string)
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
    const INITIAL_STATE: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut state = INITIAL_STATE;
    let bit_len = (input.len() as u64) * 8;
    let mut padded = Vec::with_capacity(input.len() + 1 + 8 + 64);
    padded.extend_from_slice(input);
    padded.push(0x80);
    while (padded.len() + 8) % 64 != 0 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut w = [0_u32; 64];
        for (index, word) in w.iter_mut().take(16).enumerate() {
            let offset = index * 4;
            *word = u32::from_be_bytes([
                chunk[offset],
                chunk[offset + 1],
                chunk[offset + 2],
                chunk[offset + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = w[index - 15].rotate_right(7)
                ^ w[index - 15].rotate_right(18)
                ^ (w[index - 15] >> 3);
            let s1 = w[index - 2].rotate_right(17)
                ^ w[index - 2].rotate_right(19)
                ^ (w[index - 2] >> 10);
            w[index] = w[index - 16]
                .wrapping_add(s0)
                .wrapping_add(w[index - 7])
                .wrapping_add(s1);
        }

        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];
        let mut e = state[4];
        let mut f = state[5];
        let mut g = state[6];
        let mut h = state[7];

        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[index])
                .wrapping_add(w[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }

    let mut output = [0_u8; 32];
    for (index, word) in state.iter().enumerate() {
        output[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    output
}
