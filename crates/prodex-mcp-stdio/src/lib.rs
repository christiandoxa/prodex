use anyhow::{Context, Result};
use serde_json::Value;
use std::io::{self, BufRead, Write};

const MCP_MESSAGE_MAX_BYTES: usize = 64 * 1024 * 1024;
const MCP_HEADER_LINE_MAX_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpMessageFraming {
    JsonLine,
    ContentLength,
}

pub fn read_mcp_message<R: BufRead>(reader: &mut R) -> Result<Option<(Value, McpMessageFraming)>> {
    let first = loop {
        let Some(line) = read_limited_line(reader, MCP_MESSAGE_MAX_BYTES)? else {
            return Ok(None);
        };
        if !line.trim().is_empty() {
            break line;
        }
    };
    if first.to_ascii_lowercase().starts_with("content-length:") {
        let mut content_length = parse_content_length(&first)?;
        if content_length > MCP_MESSAGE_MAX_BYTES {
            anyhow::bail!(
                "MCP message exceeds safe size limit ({} bytes)",
                MCP_MESSAGE_MAX_BYTES
            );
        }
        loop {
            let Some(header) = read_limited_line(reader, MCP_HEADER_LINE_MAX_BYTES)? else {
                break;
            };
            let trimmed = header.trim();
            if trimmed.is_empty() {
                break;
            }
            if trimmed.to_ascii_lowercase().starts_with("content-length:") {
                content_length = parse_content_length(trimmed)?;
                if content_length > MCP_MESSAGE_MAX_BYTES {
                    anyhow::bail!(
                        "MCP message exceeds safe size limit ({} bytes)",
                        MCP_MESSAGE_MAX_BYTES
                    );
                }
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

fn read_limited_line<R: BufRead>(reader: &mut R, limit: usize) -> io::Result<Option<String>> {
    let mut bytes = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            if bytes.is_empty() {
                return Ok(None);
            }
            break;
        }
        let take = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|index| index + 1)
            .unwrap_or(available.len());
        if bytes.len().saturating_add(take) > limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("MCP line exceeds safe size limit ({limit} bytes)"),
            ));
        }
        bytes.extend_from_slice(&available[..take]);
        reader.consume(take);
        if bytes.last() == Some(&b'\n') {
            break;
        }
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
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
    fn rejects_oversized_content_length_without_allocating_body() {
        let raw = format!("Content-Length: {}\r\n\r\n", MCP_MESSAGE_MAX_BYTES + 1);
        let mut reader = BufReader::new(raw.as_bytes());

        let err = read_mcp_message(&mut reader).expect_err("oversized MCP frame should fail");

        assert!(err.to_string().contains("safe size limit"));
    }

    #[test]
    fn limited_line_reader_rejects_oversized_json_line() {
        let mut reader = BufReader::new(b"12345\n".as_slice());

        let err = read_limited_line(&mut reader, 4).expect_err("oversized line should fail");

        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
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
