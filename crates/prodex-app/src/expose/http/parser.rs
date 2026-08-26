use super::{ExposeHttpParseError, ExposeParsedRequest, expose_parse_error};
use crate::expose::{
    EXPOSE_MAX_HEADER_BYTES, EXPOSE_MAX_HEADERS, EXPOSE_MAX_INPUT_BYTES, EXPOSE_MAX_MCP_BODY_BYTES,
    expose_valid_header_name, expose_valid_method, expose_valid_request_target,
};
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::net::TcpStream;

type ExposeHttpRequestHead = (String, String, BTreeMap<String, Vec<String>>, usize);

pub(crate) fn expose_read_http_request(
    stream: &mut TcpStream,
) -> std::result::Result<ExposeParsedRequest, ExposeHttpParseError> {
    let (received, header_end) = expose_read_http_headers(stream)?;
    let (method, target, headers, content_length) =
        expose_parse_http_request_head(&received, header_end)?;
    let body = expose_read_http_body(stream, &received[header_end + 4..], content_length)?;
    Ok(ExposeParsedRequest {
        method,
        target,
        headers,
        body,
    })
}

fn expose_read_http_headers(
    stream: &mut TcpStream,
) -> std::result::Result<(Vec<u8>, usize), ExposeHttpParseError> {
    let mut received = Vec::with_capacity(4096);
    let header_end = loop {
        if let Some(index) = received.windows(4).position(|window| window == b"\r\n\r\n") {
            break index;
        }
        if received.len() >= EXPOSE_MAX_HEADER_BYTES {
            return Err(expose_parse_error(431, "request headers too large"));
        }
        let mut chunk = [0_u8; 2048];
        let read = stream.read(&mut chunk).map_err(|err| {
            if matches!(
                err.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            ) {
                expose_parse_error(408, "request timeout")
            } else {
                expose_parse_error(400, "invalid request")
            }
        })?;
        if read == 0 {
            return Err(expose_parse_error(400, "invalid request"));
        }
        received.extend_from_slice(&chunk[..read]);
    };
    if header_end > EXPOSE_MAX_HEADER_BYTES {
        return Err(expose_parse_error(431, "request headers too large"));
    }
    Ok((received, header_end))
}

fn expose_parse_http_request_head(
    received: &[u8],
    header_end: usize,
) -> std::result::Result<ExposeHttpRequestHead, ExposeHttpParseError> {
    let head = std::str::from_utf8(&received[..header_end])
        .map_err(|_| expose_parse_error(400, "invalid request"))?;
    let mut lines = head.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| expose_parse_error(400, "invalid request"))?;
    let mut request_parts = request_line.split(' ');
    let method = request_parts.next().unwrap_or_default();
    let target = request_parts.next().unwrap_or_default();
    let version = request_parts.next().unwrap_or_default();
    if request_parts.next().is_some()
        || !expose_valid_method(method)
        || !expose_valid_request_target(target)
        || version != "HTTP/1.1"
    {
        return Err(expose_parse_error(400, "invalid request"));
    }

    let headers = expose_parse_http_headers(lines)?;
    let content_length = match headers.get("content-length").map(Vec::as_slice) {
        None => 0,
        Some([value]) if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) => {
            value
                .parse::<usize>()
                .map_err(|_| expose_parse_error(400, "invalid content length"))?
        }
        Some(_) => return Err(expose_parse_error(400, "invalid content length")),
    };
    let body_limit = if target.starts_with("/pdx/v1/") {
        EXPOSE_MAX_MCP_BODY_BYTES
    } else {
        EXPOSE_MAX_INPUT_BYTES
    };
    if content_length > body_limit {
        return Err(expose_parse_error(413, "request body too large"));
    }
    Ok((
        method.to_string(),
        target.to_string(),
        headers,
        content_length,
    ))
}

fn expose_parse_http_headers<'a>(
    lines: impl Iterator<Item = &'a str>,
) -> std::result::Result<BTreeMap<String, Vec<String>>, ExposeHttpParseError> {
    let mut headers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (index, line) in lines.enumerate() {
        if index >= EXPOSE_MAX_HEADERS
            || line.starts_with([' ', '\t'])
            || line
                .bytes()
                .any(|byte| byte == 0 || byte == b'\n' || byte == b'\r')
        {
            return Err(expose_parse_error(400, "invalid request headers"));
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| expose_parse_error(400, "invalid request headers"))?;
        if !expose_valid_header_name(name) {
            return Err(expose_parse_error(400, "invalid request headers"));
        }
        let value = value.trim_matches([' ', '\t']);
        if value.bytes().any(|byte| byte.is_ascii_control()) {
            return Err(expose_parse_error(400, "invalid request headers"));
        }
        headers
            .entry(name.to_ascii_lowercase())
            .or_default()
            .push(value.to_string());
    }
    if headers.contains_key("transfer-encoding") {
        return Err(expose_parse_error(400, "transfer encoding is unsupported"));
    }
    if headers.contains_key("expect") {
        return Err(expose_parse_error(417, "expectation failed"));
    }
    Ok(headers)
}

fn expose_read_http_body(
    stream: &mut TcpStream,
    initial_body: &[u8],
    content_length: usize,
) -> std::result::Result<Vec<u8>, ExposeHttpParseError> {
    if initial_body.len() > content_length {
        return Err(expose_parse_error(400, "unexpected request bytes"));
    }
    let mut body = Vec::with_capacity(content_length);
    body.extend_from_slice(initial_body);
    while body.len() < content_length {
        let remaining = content_length - body.len();
        let mut chunk = [0_u8; 2048];
        let chunk_len = remaining.min(chunk.len());
        let read = stream.read(&mut chunk[..chunk_len]).map_err(|err| {
            if matches!(
                err.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            ) {
                expose_parse_error(408, "request timeout")
            } else {
                expose_parse_error(400, "invalid request body")
            }
        })?;
        if read == 0 {
            return Err(expose_parse_error(400, "invalid request body"));
        }
        body.extend_from_slice(&chunk[..read]);
    }
    Ok(body)
}
