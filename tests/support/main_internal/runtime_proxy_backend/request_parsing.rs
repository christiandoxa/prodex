use super::*;

pub(crate) fn read_http_request(stream: &mut TcpStream) -> Option<String> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let mut buffer = [0_u8; 1024];
    let mut request = Vec::new();

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                request.extend_from_slice(&buffer[..read]);
                if let Some(header_end) =
                    request.windows(4).position(|window| window == b"\r\n\r\n")
                {
                    let header_len = header_end + 4;
                    let header_text = String::from_utf8_lossy(&request[..header_len]);
                    let content_length = request_header(&header_text, "Content-Length")
                        .and_then(|value| value.parse::<usize>().ok())
                        .unwrap_or(0);
                    while request.len() < header_len + content_length {
                        match stream.read(&mut buffer) {
                            Ok(0) => break,
                            Ok(read) => request.extend_from_slice(&buffer[..read]),
                            Err(err)
                                if matches!(
                                    err.kind(),
                                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                                ) =>
                            {
                                break;
                            }
                            Err(_) => return None,
                        }
                    }
                    break;
                }
            }
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                break;
            }
            Err(_) => return None,
        }
    }

    (!request.is_empty()).then(|| String::from_utf8_lossy(&request).into_owned())
}

pub(crate) fn request_header(request: &str, header_name: &str) -> Option<String> {
    request.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case(header_name) {
            Some(value.trim().to_string())
        } else {
            None
        }
    })
}

pub(crate) fn request_headers_map(request: &str) -> BTreeMap<String, String> {
    request
        .lines()
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect()
}

pub(crate) fn request_previous_response_id(request: &str) -> Option<String> {
    let body = request
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or_default();
    runtime_request_previous_response_id_from_text(body)
}

pub(crate) fn request_body_contains_only_function_call_output(request_body: &str) -> bool {
    let Ok(body) = serde_json::from_str::<serde_json::Value>(request_body) else {
        return false;
    };
    let Some(input) = body.get("input").and_then(serde_json::Value::as_array) else {
        return false;
    };
    !input.is_empty()
        && input.iter().all(|item| {
            let Some(object) = item.as_object() else {
                return false;
            };
            let item_type = object
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let call_id = object
                .get("call_id")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            call_id.is_some() && item_type.ends_with("_call_output")
        })
}

pub(crate) fn request_body_contains_session_id(request_body: &str) -> bool {
    let Ok(body) = serde_json::from_str::<serde_json::Value>(request_body) else {
        return false;
    };
    body.get("session_id")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            body.get("client_metadata")
                .and_then(|metadata| metadata.get("session_id"))
                .and_then(serde_json::Value::as_str)
        })
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}
