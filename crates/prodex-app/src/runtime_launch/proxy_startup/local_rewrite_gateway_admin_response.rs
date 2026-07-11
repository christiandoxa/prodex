use super::*;

#[derive(Debug)]
pub(super) struct RuntimeGatewayAdminError {
    status: u16,
    code: &'static str,
    message: String,
}

impl RuntimeGatewayAdminError {
    pub(super) fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    pub(super) fn code(&self) -> &'static str {
        self.code
    }

    #[cfg(test)]
    pub(super) fn test_status(&self) -> u16 {
        self.status
    }

    #[cfg(test)]
    pub(super) fn test_code(&self) -> &'static str {
        self.code
    }

    #[cfg(test)]
    pub(super) fn test_message(&self) -> &str {
        &self.message
    }

    pub(super) fn into_response(self) -> tiny_http::ResponseBox {
        build_runtime_proxy_json_error_response(self.status, self.code, &self.message)
    }
}

pub(super) fn runtime_gateway_admin_json_body(
    captured: &RuntimeProxyRequest,
) -> Result<serde_json::Value, tiny_http::ResponseBox> {
    serde_json::from_slice::<serde_json::Value>(&captured.body).map_err(|_err| {
        build_runtime_proxy_json_error_response(
            400,
            "invalid_json",
            "request body is not valid JSON",
        )
    })
}

pub(super) fn runtime_gateway_admin_json_response(
    status: u16,
    value: serde_json::Value,
) -> tiny_http::ResponseBox {
    let body = serde_json::to_vec_pretty(&value).unwrap_or_else(|_| b"{}".to_vec());
    build_runtime_proxy_response_from_parts(RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers: vec![
            (
                "content-type".to_string(),
                b"application/json; charset=utf-8".to_vec(),
            ),
            ("cache-control".to_string(), b"no-store".to_vec()),
            ("x-content-type-options".to_string(), b"nosniff".to_vec()),
        ],
        body: body.into(),
    })
}

pub(super) fn runtime_gateway_admin_csv_response(body: String) -> tiny_http::ResponseBox {
    build_runtime_proxy_response_from_parts(RuntimeHeapTrimmedBufferedResponseParts {
        status: 200,
        headers: vec![
            (
                "content-type".to_string(),
                b"text/csv; charset=utf-8".to_vec(),
            ),
            ("cache-control".to_string(), b"no-store".to_vec()),
            ("x-content-type-options".to_string(), b"nosniff".to_vec()),
        ],
        body: body.into_bytes().into(),
    })
}
