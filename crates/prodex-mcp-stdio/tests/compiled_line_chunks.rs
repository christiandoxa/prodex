use prodex_mcp_stdio::{McpMessageFraming, read_mcp_message};
use serde_json::json;
use std::io::BufReader;

#[test]
fn compiled_line_planner_preserves_utf8_split_and_leaves_following_message_buffered() {
    let source = " \t{\"text\":\"猫🦀\"}\n{\"next\":true}\n";
    for capacity in [1, 2, 3, 7, 128] {
        let mut reader = BufReader::with_capacity(capacity, source.as_bytes());
        assert_eq!(
            read_mcp_message(&mut reader).unwrap(),
            Some((json!({"text":"猫🦀"}), McpMessageFraming::JsonLine))
        );
        assert_eq!(
            read_mcp_message(&mut reader).unwrap(),
            Some((json!({"next":true}), McpMessageFraming::JsonLine))
        );
        assert!(read_mcp_message(&mut reader).unwrap().is_none());
    }
}

#[test]
fn compiled_line_planner_retains_split_content_length_and_header_limits() {
    let source = b"\r\nContent-Length: 2\r\nX-Extra: yes\r\n\r\n{}";
    for capacity in [1, 2, 5, 128] {
        let mut reader = BufReader::with_capacity(capacity, source.as_slice());
        assert_eq!(
            read_mcp_message(&mut reader).unwrap(),
            Some((json!({}), McpMessageFraming::ContentLength))
        );
    }
    for prefix in [" ", "\x0b"] {
        let raw = format!("{prefix}{}\n", "x".repeat(4096));
        let mut reader = BufReader::with_capacity(3, raw.as_bytes());
        assert_eq!(
            read_mcp_message(&mut reader).unwrap_err().to_string(),
            "MCP first line exceeds safe size limit (4096 bytes)"
        );
    }
}
