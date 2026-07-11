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
