use std::collections::VecDeque;
use std::net::SocketAddr;

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
