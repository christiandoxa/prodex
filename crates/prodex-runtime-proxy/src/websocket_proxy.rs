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
}
