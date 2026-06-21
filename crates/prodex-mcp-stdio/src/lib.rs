use anyhow::{Context, Result};
use serde_json::Value;
use std::io::{BufRead, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpMessageFraming {
    JsonLine,
    ContentLength,
}

pub fn read_mcp_message<R: BufRead>(reader: &mut R) -> Result<Option<(Value, McpMessageFraming)>> {
    let mut first = String::new();
    if reader.read_line(&mut first)? == 0 {
        return Ok(None);
    }
    if first.trim().is_empty() {
        return read_mcp_message(reader);
    }
    if first.to_ascii_lowercase().starts_with("content-length:") {
        let mut content_length = parse_content_length(&first)?;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header)?;
            let trimmed = header.trim();
            if trimmed.is_empty() {
                break;
            }
            if trimmed.to_ascii_lowercase().starts_with("content-length:") {
                content_length = parse_content_length(trimmed)?;
            }
        }
        let mut body = vec![0_u8; content_length];
        reader.read_exact(&mut body)?;
        let value = serde_json::from_slice(&body).context("failed to parse MCP JSON body")?;
        return Ok(Some((value, McpMessageFraming::ContentLength)));
    }
    let value = serde_json::from_str(first.trim()).context("failed to parse MCP JSON line")?;
    Ok(Some((value, McpMessageFraming::JsonLine)))
}

pub fn parse_content_length(line: &str) -> Result<usize> {
    let (_, value) = line
        .split_once(':')
        .ok_or_else(|| anyhow::anyhow!("invalid Content-Length header"))?;
    value
        .trim()
        .parse::<usize>()
        .context("invalid Content-Length value")
}

pub fn write_mcp_message<W: Write>(
    writer: &mut W,
    response: &Value,
    framing: McpMessageFraming,
) -> Result<()> {
    let body = serde_json::to_vec(response).context("failed to serialize MCP response")?;
    match framing {
        McpMessageFraming::JsonLine => {
            writer.write_all(&body)?;
            writer.write_all(b"\n")?;
        }
        McpMessageFraming::ContentLength => {
            write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
            writer.write_all(&body)?;
        }
    }
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::BufReader;

    #[test]
    fn reads_json_line_message() {
        let mut reader = BufReader::new(br#"{"jsonrpc":"2.0","id":1}"#.as_slice());
        let (value, framing) = read_mcp_message(&mut reader).unwrap().unwrap();
        assert_eq!(framing, McpMessageFraming::JsonLine);
        assert_eq!(value["id"], 1);
    }

    #[test]
    fn reads_content_length_message() {
        let body = br#"{"jsonrpc":"2.0","id":1}"#;
        let raw = format!("Content-Length: {}\r\n\r\n", body.len());
        let mut bytes = raw.into_bytes();
        bytes.extend_from_slice(body);
        let mut reader = BufReader::new(bytes.as_slice());
        let (value, framing) = read_mcp_message(&mut reader).unwrap().unwrap();
        assert_eq!(framing, McpMessageFraming::ContentLength);
        assert_eq!(value["id"], 1);
    }

    #[test]
    fn writes_framed_messages() {
        let response = json!({"jsonrpc":"2.0","id":1,"result":{}});
        let mut json_line = Vec::new();
        write_mcp_message(&mut json_line, &response, McpMessageFraming::JsonLine).unwrap();
        assert_eq!(json_line.last(), Some(&b'\n'));

        let mut content_length = Vec::new();
        write_mcp_message(
            &mut content_length,
            &response,
            McpMessageFraming::ContentLength,
        )
        .unwrap();
        assert!(content_length.starts_with(b"Content-Length:"));
    }
}
