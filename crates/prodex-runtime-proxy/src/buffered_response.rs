use std::ops::Deref;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RuntimeManagedResponseBody {
    bytes: Vec<u8>,
}

impl RuntimeManagedResponseBody {
    pub fn into_vec(mut self) -> Vec<u8> {
        std::mem::take(&mut self.bytes)
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }
}

impl From<Vec<u8>> for RuntimeManagedResponseBody {
    fn from(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
}

impl From<&[u8]> for RuntimeManagedResponseBody {
    fn from(bytes: &[u8]) -> Self {
        bytes.to_vec().into()
    }
}

impl Deref for RuntimeManagedResponseBody {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

impl<'a> IntoIterator for &'a RuntimeManagedResponseBody {
    type Item = &'a u8;
    type IntoIter = std::slice::Iter<'a, u8>;

    fn into_iter(self) -> Self::IntoIter {
        self.bytes.iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBufferedResponseParts {
    pub status: u16,
    pub headers: Vec<(String, Vec<u8>)>,
    pub body: RuntimeManagedResponseBody,
}

pub fn build_runtime_proxy_text_response_parts(
    status: u16,
    message: &str,
) -> RuntimeBufferedResponseParts {
    RuntimeBufferedResponseParts {
        status,
        headers: vec![(
            "Content-Type".to_string(),
            b"text/plain; charset=utf-8".to_vec(),
        )],
        body: message.as_bytes().to_vec().into(),
    }
}

pub fn build_runtime_proxy_json_error_parts(
    status: u16,
    code: &str,
    message: &str,
) -> RuntimeBufferedResponseParts {
    let body = serde_json::json!({
        "error": {
            "code": code,
            "message": message,
        }
    })
    .to_string();

    RuntimeBufferedResponseParts {
        status,
        headers: vec![("Content-Type".to_string(), b"application/json".to_vec())],
        body: body.into_bytes().into(),
    }
}

pub fn runtime_buffered_response_content_type(
    parts: &RuntimeBufferedResponseParts,
) -> Option<&str> {
    parts.headers.iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("content-type")
            .then(|| std::str::from_utf8(value).ok())
            .flatten()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_json_error_parts() {
        let parts = build_runtime_proxy_json_error_parts(503, "service_unavailable", "retry");

        assert_eq!(parts.status, 503);
        assert_eq!(
            runtime_buffered_response_content_type(&parts),
            Some("application/json")
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&parts.body).unwrap(),
            serde_json::json!({
                "error": {
                    "code": "service_unavailable",
                    "message": "retry"
                }
            })
        );
    }

    #[test]
    fn finds_content_type_case_insensitively() {
        let parts = RuntimeBufferedResponseParts {
            status: 200,
            headers: vec![("content-type".to_string(), b" text/event-stream ".to_vec())],
            body: Vec::new().into(),
        };

        assert_eq!(
            runtime_buffered_response_content_type(&parts),
            Some("text/event-stream")
        );
    }
}
