use base64::Engine;
use std::collections::VecDeque;
use std::io::{self, Read};
use std::net::SocketAddr;

const HTTPS_PROXY_KEYS: [&str; 6] = [
    "HTTPS_PROXY",
    "https_proxy",
    "ALL_PROXY",
    "all_proxy",
    "PROXY",
    "proxy",
];
const HTTP_PROXY_KEYS: [&str; 6] = [
    "HTTP_PROXY",
    "http_proxy",
    "ALL_PROXY",
    "all_proxy",
    "PROXY",
    "proxy",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeWebsocketTarget {
    pub host: String,
    pub port: u16,
    pub authority: String,
}

pub fn runtime_interleave_socket_addrs(addrs: Vec<SocketAddr>) -> Vec<SocketAddr> {
    let (mut primary, mut secondary): (VecDeque<_>, VecDeque<_>) =
        addrs.into_iter().partition(|addr| addr.is_ipv6());
    let prefer_ipv6 = primary.front().is_some();
    if !prefer_ipv6 {
        std::mem::swap(&mut primary, &mut secondary);
    }

    let mut ordered = Vec::with_capacity(primary.len().saturating_add(secondary.len()));
    loop {
        let mut progressed = false;
        if let Some(addr) = primary.pop_front() {
            ordered.push(addr);
            progressed = true;
        }
        if let Some(addr) = secondary.pop_front() {
            ordered.push(addr);
            progressed = true;
        }
        if !progressed {
            break;
        }
    }
    ordered
}

pub fn runtime_websocket_target_from_parts(
    host: &str,
    port: Option<u16>,
    scheme: Option<&str>,
) -> RuntimeWebsocketTarget {
    let host = runtime_websocket_normalize_host(host);
    let port = port.unwrap_or(match scheme {
        Some("wss") | Some("https") => 443,
        _ => 80,
    });
    let authority = runtime_websocket_authority(&host, port);
    RuntimeWebsocketTarget {
        host,
        port,
        authority,
    }
}

pub fn runtime_websocket_proxy_env_keys(scheme: &str) -> &'static [&'static str] {
    if matches!(scheme, "wss" | "https") {
        HTTPS_PROXY_KEYS.as_slice()
    } else {
        HTTP_PROXY_KEYS.as_slice()
    }
}

pub fn runtime_websocket_proxy_url_candidate(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.contains("://") {
        Some(trimmed.to_string())
    } else {
        Some(format!("http://{trimmed}"))
    }
}

pub fn runtime_websocket_no_proxy_value_matches(value: &str, host: &str, port: u16) -> bool {
    value
        .split(',')
        .any(|pattern| runtime_websocket_no_proxy_pattern_matches(pattern, host, port))
}

pub fn runtime_websocket_http_connect_request(
    authority: &str,
    proxy_authorization: Option<&str>,
) -> String {
    let mut request = format!(
        "CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\nProxy-Connection: Keep-Alive\r\n",
    );
    if let Some(header) = proxy_authorization {
        request.push_str("Proxy-Authorization: ");
        request.push_str(header);
        request.push_str("\r\n");
    }
    request.push_str("\r\n");
    request
}

pub fn runtime_websocket_proxy_authorization_header(
    username: &str,
    password: Option<&str>,
) -> Option<String> {
    if username.is_empty() {
        return None;
    }
    let credentials = format!("{}:{}", username, password.unwrap_or_default());
    Some(format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(credentials)
    ))
}

pub fn runtime_websocket_read_http_connect_response(
    stream: &mut impl Read,
) -> io::Result<(u16, usize)> {
    const MAX_CONNECT_RESPONSE_BYTES: usize = 8192;
    let mut response = Vec::new();
    let mut buffer = [0u8; 512];
    loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "runtime websocket proxy closed before CONNECT response completed",
            ));
        }
        response.extend_from_slice(&buffer[..read]);
        if response.windows(4).any(|window| window == b"\r\n\r\n")
            || response.windows(2).any(|window| window == b"\n\n")
        {
            break;
        }
        if response.len() >= MAX_CONNECT_RESPONSE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "runtime websocket proxy CONNECT response is too large",
            ));
        }
    }
    let text = std::str::from_utf8(&response).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "runtime websocket proxy CONNECT response is not valid UTF-8",
        )
    })?;
    let status = text
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|status| status.parse::<u16>().ok())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "runtime websocket proxy CONNECT response is missing a status code",
            )
        })?;
    Ok((status, response.len()))
}

pub fn runtime_websocket_no_proxy_pattern_matches(pattern: &str, host: &str, port: u16) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return false;
    }
    if pattern == "*" {
        return true;
    }
    let (pattern_host, pattern_port) = runtime_websocket_no_proxy_pattern_host_port(pattern);
    if pattern_port.is_some_and(|candidate_port| candidate_port != port) {
        return false;
    }
    let pattern_host = runtime_websocket_normalize_host(pattern_host);
    let host = runtime_websocket_normalize_host(host);
    let pattern_host = pattern_host.trim_start_matches('.').to_ascii_lowercase();
    let host = host.to_ascii_lowercase();
    host == pattern_host || host.ends_with(&format!(".{pattern_host}"))
}

pub fn runtime_websocket_no_proxy_pattern_host_port(pattern: &str) -> (&str, Option<u16>) {
    if let Some(stripped) = pattern.strip_prefix('[')
        && let Some((host, rest)) = stripped.split_once(']')
    {
        let port = rest
            .strip_prefix(':')
            .and_then(|value| value.parse::<u16>().ok());
        return (host, port);
    }
    if pattern.matches(':').count() == 1
        && let Some((host, port)) = pattern.rsplit_once(':')
        && let Ok(port) = port.parse::<u16>()
    {
        return (host, Some(port));
    }
    (pattern, None)
}

pub fn runtime_websocket_normalize_host(host: &str) -> String {
    host.trim_matches(|ch| ch == '[' || ch == ']').to_string()
}

pub fn runtime_websocket_authority(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_proxy_pattern_matches_exact_suffix_and_wildcard() {
        assert!(runtime_websocket_no_proxy_pattern_matches(
            "*",
            "api.openai.com",
            443
        ));
        assert!(runtime_websocket_no_proxy_pattern_matches(
            ".openai.com",
            "api.openai.com",
            443
        ));
        assert!(runtime_websocket_no_proxy_pattern_matches(
            "openai.com",
            "openai.com",
            443
        ));
        assert!(!runtime_websocket_no_proxy_pattern_matches(
            "openai.com",
            "notopenai.com",
            443
        ));
    }

    #[test]
    fn no_proxy_pattern_respects_ports_and_ipv6_brackets() {
        assert!(runtime_websocket_no_proxy_pattern_matches(
            "api.openai.com:8443",
            "api.openai.com",
            8443
        ));
        assert!(!runtime_websocket_no_proxy_pattern_matches(
            "api.openai.com:8443",
            "api.openai.com",
            443
        ));
        assert_eq!(
            runtime_websocket_no_proxy_pattern_host_port("[::1]:8080"),
            ("::1", Some(8080))
        );
    }

    #[test]
    fn websocket_authority_brackets_ipv6_hosts() {
        assert_eq!(
            runtime_websocket_authority("api.openai.com", 443),
            "api.openai.com:443"
        );
        assert_eq!(runtime_websocket_authority("::1", 8080), "[::1]:8080");
    }

    #[test]
    fn target_parts_normalize_host_port_and_authority() {
        assert_eq!(
            runtime_websocket_target_from_parts("api.openai.com", None, Some("wss")),
            RuntimeWebsocketTarget {
                host: "api.openai.com".to_string(),
                port: 443,
                authority: "api.openai.com:443".to_string(),
            }
        );
        assert_eq!(
            runtime_websocket_target_from_parts("[::1]", Some(8080), Some("ws")),
            RuntimeWebsocketTarget {
                host: "::1".to_string(),
                port: 8080,
                authority: "[::1]:8080".to_string(),
            }
        );
    }

    #[test]
    fn proxy_env_keys_match_scheme_and_proxy_url_candidates_are_normalized() {
        assert_eq!(
            runtime_websocket_proxy_env_keys("wss").first().copied(),
            Some("HTTPS_PROXY")
        );
        assert_eq!(
            runtime_websocket_proxy_env_keys("ws").first().copied(),
            Some("HTTP_PROXY")
        );
        assert_eq!(
            runtime_websocket_proxy_url_candidate("127.0.0.1:1080").as_deref(),
            Some("http://127.0.0.1:1080")
        );
        assert_eq!(
            runtime_websocket_proxy_url_candidate(" socks5://proxy.test:1080 ").as_deref(),
            Some("socks5://proxy.test:1080")
        );
        assert!(runtime_websocket_proxy_url_candidate("  ").is_none());
    }

    #[test]
    fn no_proxy_value_matches_any_comma_separated_pattern() {
        assert!(runtime_websocket_no_proxy_value_matches(
            ".example.com,api.openai.com:443",
            "api.openai.com",
            443
        ));
        assert!(!runtime_websocket_no_proxy_value_matches(
            ".example.com,api.openai.com:8443",
            "api.openai.com",
            443
        ));
    }

    #[test]
    fn proxy_authorization_header_encodes_basic_credentials() {
        assert_eq!(
            runtime_websocket_proxy_authorization_header("user", Some("pass")).as_deref(),
            Some("Basic dXNlcjpwYXNz")
        );
        assert_eq!(
            runtime_websocket_proxy_authorization_header("", Some("pass")),
            None
        );
    }

    #[test]
    fn http_connect_request_includes_target_and_optional_authorization() {
        assert_eq!(
            runtime_websocket_http_connect_request("chatgpt.com:443", None),
            "CONNECT chatgpt.com:443 HTTP/1.1\r\nHost: chatgpt.com:443\r\nProxy-Connection: Keep-Alive\r\n\r\n"
        );
        assert_eq!(
            runtime_websocket_http_connect_request("chatgpt.com:443", Some("Basic token")),
            "CONNECT chatgpt.com:443 HTTP/1.1\r\nHost: chatgpt.com:443\r\nProxy-Connection: Keep-Alive\r\nProxy-Authorization: Basic token\r\n\r\n"
        );
    }

    #[test]
    fn read_http_connect_response_parses_status_and_byte_count() {
        let mut response = std::io::Cursor::new(b"HTTP/1.1 200 OK\r\nHeader: value\r\n\r\nbody");
        assert_eq!(
            runtime_websocket_read_http_connect_response(&mut response).expect("connect response"),
            (200, "HTTP/1.1 200 OK\r\nHeader: value\r\n\r\nbody".len())
        );
    }

    #[test]
    fn interleaves_socket_addrs_with_ipv6_preferred_when_available() {
        let v4_a: SocketAddr = "127.0.0.1:443".parse().expect("v4");
        let v4_b: SocketAddr = "127.0.0.2:443".parse().expect("v4");
        let v6_a: SocketAddr = "[::1]:443".parse().expect("v6");
        let v6_b: SocketAddr = "[::2]:443".parse().expect("v6");

        assert_eq!(
            runtime_interleave_socket_addrs(vec![v4_a, v4_b, v6_a, v6_b]),
            vec![v6_a, v4_a, v6_b, v4_b]
        );
        assert_eq!(
            runtime_interleave_socket_addrs(vec![v6_a, v6_b, v4_a, v4_b]),
            vec![v6_a, v4_a, v6_b, v4_b]
        );
    }
}
