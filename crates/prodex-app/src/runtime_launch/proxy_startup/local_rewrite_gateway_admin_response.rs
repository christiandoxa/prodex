use super::*;

pub(super) fn runtime_gateway_http_plan_error_response(
    error: &prodex_gateway_http::GatewayHttpPlanError,
) -> tiny_http::ResponseBox {
    let response = prodex_gateway_http::plan_gateway_http_error_response(error);
    let status = match response.status {
        prodex_gateway_http::GatewayHttpErrorStatus::BadRequest => 400,
        prodex_gateway_http::GatewayHttpErrorStatus::MethodNotAllowed => 405,
        prodex_gateway_http::GatewayHttpErrorStatus::PayloadTooLarge => 413,
        prodex_gateway_http::GatewayHttpErrorStatus::RequestHeaderFieldsTooLarge => 431,
        prodex_gateway_http::GatewayHttpErrorStatus::InternalServerError => 500,
    };
    build_runtime_proxy_json_error_response(status, response.code, response.message)
}

pub(super) fn runtime_gateway_admin_page_window(
    page_request: &prodex_domain::PageRequest,
    total: usize,
) -> Result<(usize, usize, Option<String>), tiny_http::ResponseBox> {
    let start = page_request
        .cursor
        .as_ref()
        .map_or(Ok(0), |cursor| cursor.as_str().parse::<usize>())
        .map_err(|_| {
            build_runtime_proxy_json_error_response(
                400,
                "pagination_cursor_invalid",
                "pagination cursor is invalid",
            )
        })?
        .min(total);
    let end = start
        .saturating_add(usize::from(page_request.limit))
        .min(total);
    let next_cursor = (end < total).then(|| end.to_string());
    Ok((start, end, next_cursor))
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn admin_page_window_is_bounded_and_returns_a_cursor() {
        let page = prodex_domain::PageRequest::new(
            Some(2),
            Some(prodex_domain::Cursor::new("2").unwrap()),
        );
        let window = match runtime_gateway_admin_page_window(&page, 5) {
            Ok(window) => window,
            Err(_) => panic!("valid page cursor should produce a window"),
        };
        assert_eq!(window, (2, 4, Some("4".to_string())));
    }

    #[test]
    fn admin_http_plan_maps_duplicate_affinity_to_a_safe_client_error() {
        let mut policy = prodex_gateway_http::GatewayHttpPolicy::production_default();
        policy.require_trace_context = false;
        let request = prodex_gateway_http::GatewayHttpRequestMeta {
            method: prodex_gateway_http::GatewayHttpMethod::Get,
            path: "/v1/prodex/gateway/keys".to_string(),
            body_len: 0,
            headers: vec![
                prodex_gateway_http::GatewayHttpHeader::new("session_id", "one"),
                prodex_gateway_http::GatewayHttpHeader::new("session_id", "two"),
            ],
        };
        let error = prodex_gateway_http::plan_gateway_http_request(policy, request).unwrap_err();
        let response = runtime_gateway_http_plan_error_response(&error);
        assert_eq!(response.status_code().0, 400);
        let mut body = String::new();
        response.into_reader().read_to_string(&mut body).unwrap();
        assert!(body.contains("affinity_header_invalid"));
    }
}
