use base64::Engine;
use prodex_mojo_core::websocket_proxy_policy as mojo_websocket_proxy;
use std::collections::VecDeque;
use std::io::{self, Read};
use std::net::SocketAddr;

#[cfg(test)]
const HTTPS_PROXY_KEYS: [&str; 6] = [
    "HTTPS_PROXY",
    "https_proxy",
    "ALL_PROXY",
    "all_proxy",
    "PROXY",
    "proxy",
];
#[cfg(test)]
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
    let port = port.unwrap_or_else(|| {
        mojo_websocket_proxy::default_port(scheme)
            .expect("Mojo websocket proxy default-port policy returned invalid output")
    });
    let authority = runtime_websocket_authority(&host, port);
    RuntimeWebsocketTarget {
        host,
        port,
        authority,
    }
}

#[cfg(test)]
pub(crate) fn runtime_websocket_proxy_env_keys(scheme: &str) -> &'static [&'static str] {
    if matches!(scheme, "wss" | "https") {
        HTTPS_PROXY_KEYS.as_slice()
    } else {
        HTTP_PROXY_KEYS.as_slice()
    }
}

pub fn runtime_websocket_proxy_url_candidate(value: &str) -> Option<String> {
    mojo_websocket_proxy::proxy_url_candidate(value)
        .expect("Mojo websocket proxy URL policy returned invalid output")
}

pub fn runtime_websocket_no_proxy_value_matches(value: &str, host: &str, port: u16) -> bool {
    mojo_websocket_proxy::value_matches(value, host, port)
        .expect("Mojo websocket NO_PROXY value policy returned invalid output")
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
    mojo_websocket_proxy::pattern_matches(pattern, host, port)
        .expect("Mojo websocket NO_PROXY pattern policy returned invalid output")
}

pub fn runtime_websocket_no_proxy_pattern_host_port(pattern: &str) -> (&str, Option<u16>) {
    let plan = mojo_websocket_proxy::pattern_host_port(pattern)
        .expect("Mojo websocket NO_PROXY host/port parser returned invalid output");
    (&pattern[plan.host_start..plan.host_end], plan.port)
}

pub fn runtime_websocket_normalize_host(host: &str) -> String {
    mojo_websocket_proxy::normalize_host(host)
        .expect("Mojo websocket host normalization returned invalid output")
}

pub fn runtime_websocket_authority(host: &str, port: u16) -> String {
    mojo_websocket_proxy::authority(host, port)
        .expect("Mojo websocket authority rendering returned invalid output")
}

#[cfg(test)]
#[path = "../tests/src/websocket_proxy.rs"]
mod tests;
