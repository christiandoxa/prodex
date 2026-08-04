use anyhow::{Context, Result};
use serde_json::Value;
use std::io::{self, BufRead, Write};

const MCP_MESSAGE_MAX_BYTES: usize = 64 * 1024 * 1024;
// Framing headers are bounded independently so an attacker cannot grow the
// first-line buffer to the much larger JSON message limit.
const MCP_FIRST_HEADER_LINE_MAX_BYTES: usize = 4 * 1024;
const MCP_HEADER_LINE_MAX_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpMessageFraming {
    JsonLine,
    ContentLength,
}

pub fn read_mcp_message<R: BufRead>(reader: &mut R) -> Result<Option<(Value, McpMessageFraming)>> {
    let Some(first) = read_mcp_first_line(reader)? else {
        return Ok(None);
    };
    if first.to_ascii_lowercase().starts_with("content-length:") {
        let content_length = parse_content_length(&first)?;
        if content_length > MCP_MESSAGE_MAX_BYTES {
            anyhow::bail!(
                "MCP message exceeds safe size limit ({} bytes)",
                MCP_MESSAGE_MAX_BYTES
            );
        }
        while let Some(header) = read_limited_line(reader, MCP_HEADER_LINE_MAX_BYTES)? {
            let trimmed = header.trim();
            if trimmed.is_empty() {
                break;
            }
            if trimmed.to_ascii_lowercase().starts_with("content-length:") {
                anyhow::bail!("duplicate MCP Content-Length header");
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

fn read_mcp_first_line<R: BufRead>(reader: &mut R) -> io::Result<Option<String>> {
    loop {
        let Some(line) = read_mcp_physical_first_line(reader)? else {
            return Ok(None);
        };
        if !line.trim().is_empty() {
            return Ok(Some(line));
        }
    }
}

fn read_mcp_physical_first_line<R: BufRead>(reader: &mut R) -> io::Result<Option<String>> {
    let mut bytes = Vec::new();
    let mut first_non_whitespace = None;
    loop {
        match append_mcp_first_line_chunk(reader, &mut bytes, &mut first_non_whitespace)? {
            McpFirstLineRead::Eof => return Ok(None),
            McpFirstLineRead::More => {}
            McpFirstLineRead::Complete => break,
        }
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

enum McpFirstLineRead {
    Eof,
    More,
    Complete,
}

fn append_mcp_first_line_chunk<R: BufRead>(
    reader: &mut R,
    bytes: &mut Vec<u8>,
    first_non_whitespace: &mut Option<u8>,
) -> io::Result<McpFirstLineRead> {
    let available = reader.fill_buf()?;
    if available.is_empty() {
        return Ok(if bytes.is_empty() {
            McpFirstLineRead::Eof
        } else {
            McpFirstLineRead::Complete
        });
    }
    let take = available
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|index| index + 1)
        .unwrap_or(available.len());
    let next_first_non_whitespace = available[..take]
        .iter()
        .find(|byte| !byte.is_ascii_whitespace())
        .copied();
    let first = first_non_whitespace.or(next_first_non_whitespace);
    let limit = if first.is_some_and(|byte| !mcp_first_line_looks_like_header(byte)) {
        MCP_MESSAGE_MAX_BYTES
    } else {
        MCP_FIRST_HEADER_LINE_MAX_BYTES
    };
    if bytes.len().saturating_add(take) > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("MCP first line exceeds safe size limit ({limit} bytes)"),
        ));
    }
    bytes.extend_from_slice(&available[..take]);
    reader.consume(take);
    if first_non_whitespace.is_none() {
        *first_non_whitespace = next_first_non_whitespace;
    }
    Ok(if bytes.last() == Some(&b'\n') {
        McpFirstLineRead::Complete
    } else {
        McpFirstLineRead::More
    })
}

fn mcp_first_line_looks_like_header(byte: u8) -> bool {
    // MCP JSON-RPC messages are objects or batches. Everything else is either
    // a framing header or invalid input and must stay under the header bound.
    !matches!(byte, b'{' | b'[')
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
    fn rejects_oversized_first_header_before_growing_header_buffer() {
        let raw = format!(
            "Content-Length: {}\r\n",
            "x".repeat(MCP_FIRST_HEADER_LINE_MAX_BYTES)
        );
        let mut reader = BufReader::new(raw.as_bytes());

        let err = read_mcp_message(&mut reader).expect_err("oversized header should fail");

        assert!(
            err.to_string()
                .contains("first line exceeds safe size limit")
        );
    }

    #[test]
    fn rejects_oversized_header_even_when_its_name_starts_like_a_json_literal() {
        for prefix in ["transfer-encoding: ", "false-header: ", "null-header: "] {
            let raw = format!(
                "{prefix}{}\r\n",
                "x".repeat(MCP_FIRST_HEADER_LINE_MAX_BYTES)
            );
            let mut reader = BufReader::new(raw.as_bytes());

            let err = read_mcp_message(&mut reader).expect_err("oversized header should fail");

            assert!(
                err.to_string()
                    .contains("first line exceeds safe size limit"),
                "unexpected error for {prefix:?}: {err}"
            );
        }
    }

    #[test]
    fn rejects_oversized_whitespace_prefix_before_header_classification() {
        let raw = format!(
            "{}Content-Length: 2\r\n",
            " ".repeat(MCP_FIRST_HEADER_LINE_MAX_BYTES)
        );
        let mut reader = BufReader::new(raw.as_bytes());

        let err = read_mcp_message(&mut reader).expect_err("oversized header prefix should fail");

        assert!(
            err.to_string()
                .contains("first line exceeds safe size limit")
        );
    }

    #[test]
    fn first_header_line_accepts_exact_safe_limit() {
        let raw = format!("{}\n", "x".repeat(MCP_FIRST_HEADER_LINE_MAX_BYTES - 1));
        let mut reader = BufReader::new(raw.as_bytes());

        let line = read_mcp_first_line(&mut reader).unwrap().unwrap();

        assert_eq!(line.len(), MCP_FIRST_HEADER_LINE_MAX_BYTES);
    }

    #[test]
    fn large_json_line_keeps_message_limit_after_first_line_classification() {
        let body = serde_json::to_string(&json!({"payload": "x".repeat(8 * 1024)})).unwrap();
        let mut reader = BufReader::new(body.as_bytes());

        let (value, framing) = read_mcp_message(&mut reader).unwrap().unwrap();

        assert_eq!(framing, McpMessageFraming::JsonLine);
        assert_eq!(value["payload"].as_str().unwrap().len(), 8 * 1024);
    }

    #[test]
    fn rejects_maximum_declared_content_length_without_body_allocation() {
        let raw = format!("Content-Length: {}\r\n\r\n", usize::MAX);
        let mut reader = BufReader::new(raw.as_bytes());

        let err = read_mcp_message(&mut reader).expect_err("malicious declared size should fail");

        assert!(err.to_string().contains("safe size limit"));
    }

    #[test]
    fn rejects_truncated_content_length_frame() {
        let mut reader = BufReader::new(b"Content-Length: 8\r\n\r\n{}".as_slice());

        let err = read_mcp_message(&mut reader).expect_err("truncated MCP frame should fail");

        assert_eq!(
            err.downcast_ref::<std::io::Error>()
                .map(std::io::Error::kind),
            Some(std::io::ErrorKind::UnexpectedEof)
        );
    }

    #[test]
    fn rejects_duplicate_content_length_header() {
        let mut reader =
            BufReader::new(b"Content-Length: 1\r\nContent-Length: 2\r\n\r\n{}".as_slice());

        let err = read_mcp_message(&mut reader).expect_err("ambiguous MCP frame should fail");

        assert!(err.to_string().contains("duplicate MCP Content-Length"));
    }

    #[test]
    fn limited_line_reader_rejects_oversized_json_line() {
        let mut reader = BufReader::new(b"12345\n".as_slice());

        let err = read_limited_line(&mut reader, 4).expect_err("oversized line should fail");

        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn limited_line_reader_accepts_exact_limit() {
        let mut reader = BufReader::new(b"1234\n".as_slice());

        assert_eq!(
            read_limited_line(&mut reader, 5).unwrap().as_deref(),
            Some("1234\n")
        );
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
