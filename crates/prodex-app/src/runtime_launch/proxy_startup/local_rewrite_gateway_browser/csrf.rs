use super::{RuntimeGatewayBrowserRoute, RuntimeGatewayBrowserSession, cookie_from_headers};
use crate::{
    RuntimeHeapTrimmedBufferedResponseParts, RuntimeProxyRequest,
    build_runtime_proxy_response_from_parts,
};
use sha2::{Digest, Sha256};

pub(super) const CSRF_COOKIE: &str = "prodex_gateway_csrf";

pub(super) fn browser_logout_method_allowed(method: &str) -> bool {
    method.eq_ignore_ascii_case("POST")
}

pub(super) fn browser_method_not_allowed(
    route: RuntimeGatewayBrowserRoute,
) -> tiny_http::ResponseBox {
    let allow: &[u8] = match route {
        RuntimeGatewayBrowserRoute::Login | RuntimeGatewayBrowserRoute::Callback => b"GET",
        RuntimeGatewayBrowserRoute::Logout | RuntimeGatewayBrowserRoute::BackchannelLogout => {
            b"POST"
        }
    };
    build_runtime_proxy_response_from_parts(RuntimeHeapTrimmedBufferedResponseParts {
        status: 405,
        headers: vec![
            ("allow".to_string(), allow.to_vec()),
            ("cache-control".to_string(), b"no-store".to_vec()),
        ],
        body: Vec::new().into(),
    })
}

pub(super) fn browser_logout_csrf_valid(
    request: &RuntimeProxyRequest,
    session: &RuntimeGatewayBrowserSession,
) -> bool {
    browser_logout_method_allowed(&request.method) && browser_session_csrf_valid(request, session)
}

pub(super) fn browser_session_csrf_valid(
    request: &RuntimeProxyRequest,
    session: &RuntimeGatewayBrowserSession,
) -> bool {
    // Sessions written before CSRF binding existed must reauthenticate.
    if session.csrf_digest == [0; 32] {
        return false;
    }
    if ["GET", "HEAD", "OPTIONS"]
        .iter()
        .any(|method| request.method.eq_ignore_ascii_case(method))
    {
        return true;
    }
    let Some(header) = unique_browser_token_header(&request.headers, "x-csrf-token") else {
        return false;
    };
    let Some(cookie) = cookie_from_headers(&request.headers, CSRF_COOKIE) else {
        return false;
    };
    let header_digest: [u8; 32] = Sha256::digest(header.as_bytes()).into();
    let cookie_digest: [u8; 32] = Sha256::digest(cookie.as_bytes()).into();
    constant_time_digest_eq(&session.csrf_digest, &header_digest)
        && constant_time_digest_eq(&session.csrf_digest, &cookie_digest)
}

fn unique_browser_token_header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    let mut values = headers
        .iter()
        .filter(|(header, _)| header.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str());
    let value = values.next()?;
    (values.next().is_none()
        && !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')))
    .then_some(value)
}

fn constant_time_digest_eq(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (*left ^ *right)
        })
        == 0
}

#[cfg(test)]
mod tests {
    use super::{
        CSRF_COOKIE, browser_logout_csrf_valid, browser_logout_method_allowed,
        browser_session_csrf_valid,
    };
    use crate::RuntimeProxyRequest;
    use sha2::{Digest, Sha256};

    use super::super::RuntimeGatewayBrowserSession;

    #[test]
    fn browser_logout_requires_post_and_session_bound_double_submit_csrf() {
        let csrf = "synthetic-csrf-token";
        let session = RuntimeGatewayBrowserSession {
            id_token: "fixture.id.token".to_string(),
            csrf_digest: Sha256::digest(csrf.as_bytes()).into(),
            logout_keys: Vec::new(),
            expires_at_unix_ms: 456,
        };
        let request = |method: &str, token: Option<&str>| RuntimeProxyRequest {
            method: method.to_string(),
            path_and_query: "/v1/prodex/gateway/keys".to_string(),
            headers: token.map_or_else(Vec::new, |token| {
                vec![
                    ("X-CSRF-Token".to_string(), token.to_string()),
                    ("Cookie".to_string(), format!("{CSRF_COOKIE}={token}")),
                ]
            }),
            body: Vec::new(),
        };

        assert!(browser_session_csrf_valid(&request("GET", None), &session));
        assert!(!browser_logout_method_allowed("GET"));
        assert!(browser_logout_method_allowed("POST"));
        for (route, allow) in [
            (super::super::RuntimeGatewayBrowserRoute::Login, "GET"),
            (super::super::RuntimeGatewayBrowserRoute::Logout, "POST"),
        ] {
            let rejected = super::browser_method_not_allowed(route);
            assert_eq!(rejected.status_code().0, 405);
            assert!(rejected.headers().iter().any(|header| {
                header.field.equiv("allow") && header.value.as_str().eq_ignore_ascii_case(allow)
            }));
        }
        assert!(!browser_logout_csrf_valid(
            &request("GET", Some(csrf)),
            &session
        ));
        assert!(browser_logout_csrf_valid(
            &request("POST", Some(csrf)),
            &session
        ));
        assert!(!browser_logout_csrf_valid(&request("POST", None), &session));
        assert!(!browser_logout_csrf_valid(
            &request("POST", Some("wrong-token")),
            &session
        ));

        let mut legacy_session = session.clone();
        legacy_session.csrf_digest = [0; 32];
        assert!(!browser_session_csrf_valid(
            &request("GET", None),
            &legacy_session
        ));

        let mut duplicate_header = request("POST", Some(csrf));
        duplicate_header
            .headers
            .push(("x-csrf-token".to_string(), csrf.to_string()));
        assert!(!browser_session_csrf_valid(&duplicate_header, &session));
    }
}
