use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use prodex_authn::{OidcBrowserFlowCapability, OidcBrowserFlowRequirement, OidcPkceMethod};
use runtime_proxy_crate::path_without_query;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_admin_auth::{
    RuntimeGatewayAdminAuthentication, runtime_gateway_oidc_admin_auth_from_verified,
    runtime_gateway_oidc_browser_endpoint, runtime_gateway_oidc_post_form,
    runtime_gateway_verify_oidc_token,
};
use super::local_rewrite_gateway_config::RuntimeGatewayBrowserConfig;
use super::local_rewrite_gateway_util::runtime_gateway_unix_epoch_millis;
use super::local_rewrite_request::RuntimeLocalRewriteRequest;
use super::local_rewrite_transport::runtime_gateway_with_outbound_secret;
use crate::{
    RuntimeHeapTrimmedBufferedResponseParts, RuntimeProxyRequest,
    build_runtime_proxy_json_error_response, build_runtime_proxy_response_from_parts,
    read_blocking_response_body_with_limit,
};

const BROWSER_TRANSACTION_TTL_MS: u64 = 5 * 60 * 1_000;
const BROWSER_SESSION_TTL_MS: u64 = 8 * 60 * 60 * 1_000;
const MAX_BROWSER_TRANSACTIONS: usize = 1_024;
const MAX_BROWSER_SESSIONS: usize = 4_096;
const MAX_TOKEN_RESPONSE_BYTES: usize = 64 * 1_024;
const SESSION_COOKIE: &str = "prodex_gateway_session";
const STATE_COOKIE: &str = "prodex_gateway_oidc_state";

#[derive(Default)]
pub(super) struct RuntimeGatewayBrowserState {
    transactions: Arc<Mutex<BTreeMap<String, RuntimeGatewayBrowserTransaction>>>,
    sessions: Arc<Mutex<BTreeMap<String, RuntimeGatewayBrowserSession>>>,
}

struct RuntimeGatewayBrowserTransaction {
    nonce: String,
    code_verifier: String,
    expires_at_unix_ms: u64,
}

#[derive(Clone)]
struct RuntimeGatewayBrowserSession {
    authentication: RuntimeGatewayAdminAuthentication,
    expires_at_unix_ms: u64,
}

#[derive(Clone, Copy)]
enum RuntimeGatewayBrowserRoute {
    Login,
    Callback,
    Logout,
}

pub(super) fn runtime_gateway_browser_auth_response(
    request: &RuntimeLocalRewriteRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<tiny_http::ResponseBox> {
    let route = browser_route(request.url())?;
    let Some(config) = shared.gateway_sso.browser.as_ref() else {
        return Some(build_runtime_proxy_json_error_response(
            404,
            "browser_auth_not_configured",
            "browser authentication is not configured",
        ));
    };
    let response = match route {
        RuntimeGatewayBrowserRoute::Login if request.method().eq_ignore_ascii_case("GET") => {
            runtime_gateway_browser_login(shared, config)
        }
        RuntimeGatewayBrowserRoute::Callback if request.method().eq_ignore_ascii_case("GET") => {
            runtime_gateway_browser_callback(request, shared, config)
        }
        RuntimeGatewayBrowserRoute::Logout
            if request.method().eq_ignore_ascii_case("GET")
                || request.method().eq_ignore_ascii_case("POST") =>
        {
            runtime_gateway_browser_logout(request, shared)
        }
        _ => Ok(method_not_allowed()),
    };
    Some(response.unwrap_or_else(|failure| match failure {
        RuntimeGatewayBrowserFailure::Unauthorized => build_runtime_proxy_json_error_response(
            401,
            "browser_authentication_failed",
            "browser authentication failed",
        ),
        RuntimeGatewayBrowserFailure::Unavailable => build_runtime_proxy_json_error_response(
            503,
            "browser_authentication_unavailable",
            "browser authentication is temporarily unavailable",
        ),
    }))
}

pub(super) fn runtime_gateway_browser_session_auth(
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<RuntimeGatewayAdminAuthentication> {
    shared.gateway_sso.browser.as_ref()?;
    let session_id = cookie_from_headers(&request.headers, SESSION_COOKIE)?;
    let now = runtime_gateway_unix_epoch_millis();
    let mut sessions = shared.gateway_browser.sessions.lock().ok()?;
    sessions.retain(|_, session| session.expires_at_unix_ms > now);
    sessions
        .get(session_id)
        .map(|session| session.authentication.clone())
}

#[derive(Clone, Copy)]
enum RuntimeGatewayBrowserFailure {
    Unauthorized,
    Unavailable,
}

type BrowserResult<T> = Result<T, RuntimeGatewayBrowserFailure>;

fn runtime_gateway_browser_login(
    shared: &RuntimeLocalRewriteProxyShared,
    config: &RuntimeGatewayBrowserConfig,
) -> BrowserResult<tiny_http::ResponseBox> {
    prodex_authn::require_oidc_browser_flow_capability(
        OidcBrowserFlowCapability {
            authorization_code: true,
            pkce_s256: true,
        },
        OidcBrowserFlowRequirement {
            pkce_method: OidcPkceMethod::S256,
        },
    )
    .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    let state = random_token().map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    let nonce = random_token().map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    let code_verifier = random_token().map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    let code_challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(code_verifier.as_bytes()));
    let now = runtime_gateway_unix_epoch_millis();
    let mut transactions = shared
        .gateway_browser
        .transactions
        .lock()
        .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    transactions.retain(|_, transaction| transaction.expires_at_unix_ms > now);
    if transactions.len() >= MAX_BROWSER_TRANSACTIONS {
        return Err(RuntimeGatewayBrowserFailure::Unavailable);
    }
    transactions.insert(
        state.clone(),
        RuntimeGatewayBrowserTransaction {
            nonce: nonce.clone(),
            code_verifier,
            expires_at_unix_ms: now.saturating_add(BROWSER_TRANSACTION_TTL_MS),
        },
    );
    drop(transactions);

    let mut authorization_url = reqwest::Url::parse(&config.authorization_url)
        .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    authorization_url
        .query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &config.client_id)
        .append_pair("redirect_uri", &config.redirect_uri)
        .append_pair("scope", "openid profile email")
        .append_pair("state", &state)
        .append_pair("nonce", &nonce)
        .append_pair("code_challenge", &code_challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(redirect_response(
        302,
        authorization_url.as_str(),
        Some(format!(
            "{STATE_COOKIE}={state}; Secure; HttpOnly; SameSite=Lax; Path=/; Max-Age=300"
        )),
        None,
    ))
}

fn runtime_gateway_browser_callback(
    request: &RuntimeLocalRewriteRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    config: &RuntimeGatewayBrowserConfig,
) -> BrowserResult<tiny_http::ResponseBox> {
    let query = request
        .url()
        .split_once('?')
        .map(|(_, query)| query)
        .ok_or(RuntimeGatewayBrowserFailure::Unauthorized)?;
    let parameters = parse_query(query)?;
    if parameters.contains_key("error") {
        return Err(RuntimeGatewayBrowserFailure::Unauthorized);
    }
    let state = parameters
        .get("state")
        .ok_or(RuntimeGatewayBrowserFailure::Unauthorized)?;
    let code = parameters
        .get("code")
        .ok_or(RuntimeGatewayBrowserFailure::Unauthorized)?;
    if cookie_from_headers(request.headers(), STATE_COOKIE) != Some(state.as_str()) {
        return Err(RuntimeGatewayBrowserFailure::Unauthorized);
    }
    let now = runtime_gateway_unix_epoch_millis();
    let transaction = {
        let mut transactions = shared
            .gateway_browser
            .transactions
            .lock()
            .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
        transactions.retain(|_, transaction| transaction.expires_at_unix_ms > now);
        transactions
            .remove(state)
            .ok_or(RuntimeGatewayBrowserFailure::Unauthorized)?
    };
    if transaction.expires_at_unix_ms <= now {
        return Err(RuntimeGatewayBrowserFailure::Unauthorized);
    }
    let oidc = shared
        .gateway_sso
        .oidc
        .as_ref()
        .ok_or(RuntimeGatewayBrowserFailure::Unavailable)?;
    let endpoint = runtime_gateway_oidc_browser_endpoint(oidc, &config.token_url)
        .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    let mut form = vec![
        ("grant_type".to_string(), "authorization_code".to_string()),
        ("code".to_string(), code.clone()),
        ("redirect_uri".to_string(), config.redirect_uri.clone()),
        ("client_id".to_string(), config.client_id.clone()),
        ("code_verifier".to_string(), transaction.code_verifier),
    ];
    let exchange = |form: &[(String, String)]| {
        runtime_gateway_oidc_post_form(&endpoint, form, &shared.runtime_shared.runtime_config.oidc)
    };
    let response = match config.client_secret.as_ref() {
        Some(secret) => runtime_gateway_with_outbound_secret(secret, |secret| {
            form.push(("client_secret".to_string(), secret.to_string()));
            exchange(&form)
        }),
        None => exchange(&form),
    }
    .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    let body = read_blocking_response_body_with_limit(
        response,
        MAX_TOKEN_RESPONSE_BYTES,
        "failed to read gateway OIDC token response",
    )
    .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    let token_response: serde_json::Value =
        serde_json::from_slice(&body).map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    let id_token = token_response
        .get("id_token")
        .and_then(serde_json::Value::as_str)
        .filter(|token| !token.is_empty() && token.len() <= MAX_TOKEN_RESPONSE_BYTES)
        .ok_or(RuntimeGatewayBrowserFailure::Unauthorized)?;
    let verified = runtime_gateway_verify_oidc_token(id_token, oidc, shared)
        .map_err(|_| RuntimeGatewayBrowserFailure::Unauthorized)?;
    if verified
        .claims
        .get("nonce")
        .and_then(serde_json::Value::as_str)
        != Some(transaction.nonce.as_str())
    {
        return Err(RuntimeGatewayBrowserFailure::Unauthorized);
    }
    let authentication = runtime_gateway_oidc_admin_auth_from_verified(verified, oidc, shared)
        .ok_or(RuntimeGatewayBrowserFailure::Unauthorized)?;
    let session_id = random_token().map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    let mut sessions = shared
        .gateway_browser
        .sessions
        .lock()
        .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    sessions.retain(|_, session| session.expires_at_unix_ms > now);
    if sessions.len() >= MAX_BROWSER_SESSIONS
        && let Some(oldest) = sessions
            .iter()
            .min_by_key(|(_, session)| session.expires_at_unix_ms)
            .map(|(id, _)| id.clone())
    {
        sessions.remove(&oldest);
    }
    sessions.insert(
        session_id.clone(),
        RuntimeGatewayBrowserSession {
            authentication,
            expires_at_unix_ms: now.saturating_add(BROWSER_SESSION_TTL_MS),
        },
    );
    Ok(redirect_response(
        303,
        "/v1/prodex/gateway",
        Some(format!(
            "{SESSION_COOKIE}={session_id}; Secure; HttpOnly; SameSite=Strict; Path=/; Max-Age=28800"
        )),
        Some(format!(
            "{STATE_COOKIE}=; Secure; HttpOnly; SameSite=Lax; Path=/; Max-Age=0"
        )),
    ))
}

fn runtime_gateway_browser_logout(
    request: &RuntimeLocalRewriteRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> BrowserResult<tiny_http::ResponseBox> {
    if let Some(session_id) = cookie_from_headers(request.headers(), SESSION_COOKIE) {
        shared
            .gateway_browser
            .sessions
            .lock()
            .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?
            .remove(session_id);
    }
    Ok(redirect_response(
        303,
        "/v1/prodex/gateway/auth/login",
        Some(format!(
            "{SESSION_COOKIE}=; Secure; HttpOnly; SameSite=Strict; Path=/; Max-Age=0"
        )),
        None,
    ))
}

fn browser_route(path: &str) -> Option<RuntimeGatewayBrowserRoute> {
    let path = path_without_query(path);
    let suffix = path
        .strip_prefix("/v1/prodex/gateway/auth/")
        .or_else(|| path.strip_prefix("/prodex/gateway/auth/"))?;
    match suffix {
        "login" => Some(RuntimeGatewayBrowserRoute::Login),
        "callback" => Some(RuntimeGatewayBrowserRoute::Callback),
        "logout" => Some(RuntimeGatewayBrowserRoute::Logout),
        _ => None,
    }
}

fn parse_query(query: &str) -> BrowserResult<BTreeMap<String, String>> {
    if query.len() > 8 * 1_024 {
        return Err(RuntimeGatewayBrowserFailure::Unauthorized);
    }
    let url = reqwest::Url::parse(&format!("https://gateway.invalid/?{query}"))
        .map_err(|_| RuntimeGatewayBrowserFailure::Unauthorized)?;
    let mut parameters = BTreeMap::new();
    for (name, value) in url.query_pairs().take(17) {
        if parameters.len() >= 16
            || parameters
                .insert(name.into_owned(), value.into_owned())
                .is_some()
        {
            return Err(RuntimeGatewayBrowserFailure::Unauthorized);
        }
    }
    Ok(parameters)
}

fn cookie_from_headers<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    let mut found = None;
    for value in headers
        .iter()
        .filter(|(header, _)| header.eq_ignore_ascii_case("cookie"))
        .map(|(_, value)| value.as_str())
    {
        if value.len() > 8 * 1_024 {
            return None;
        }
        for cookie in value.split(';') {
            let Some((cookie_name, cookie_value)) = cookie.trim().split_once('=') else {
                continue;
            };
            if cookie_name == name {
                if found.is_some()
                    || cookie_value.is_empty()
                    || cookie_value.len() > 128
                    || !cookie_value
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
                {
                    return None;
                }
                found = Some(cookie_value);
            }
        }
    }
    found
}

fn random_token() -> anyhow::Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn redirect_response(
    status: u16,
    location: &str,
    cookie: Option<String>,
    second_cookie: Option<String>,
) -> tiny_http::ResponseBox {
    let mut headers = vec![
        ("location".to_string(), location.as_bytes().to_vec()),
        ("cache-control".to_string(), b"no-store".to_vec()),
        (
            "content-type".to_string(),
            b"text/plain; charset=utf-8".to_vec(),
        ),
        ("x-content-type-options".to_string(), b"nosniff".to_vec()),
        ("referrer-policy".to_string(), b"no-referrer".to_vec()),
    ];
    for cookie in [cookie, second_cookie].into_iter().flatten() {
        headers.push(("set-cookie".to_string(), cookie.into_bytes()));
    }
    build_runtime_proxy_response_from_parts(RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers,
        body: Vec::new().into(),
    })
}

fn method_not_allowed() -> tiny_http::ResponseBox {
    build_runtime_proxy_response_from_parts(RuntimeHeapTrimmedBufferedResponseParts {
        status: 405,
        headers: vec![
            ("allow".to_string(), b"GET, POST".to_vec()),
            ("cache-control".to_string(), b"no-store".to_vec()),
        ],
        body: Vec::new().into(),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeGatewayBrowserRoute, SESSION_COOKIE, browser_route, cookie_from_headers, parse_query,
    };

    #[test]
    fn browser_routes_queries_and_cookies_are_exact() {
        assert!(matches!(
            browser_route("/v1/prodex/gateway/auth/login"),
            Some(RuntimeGatewayBrowserRoute::Login)
        ));
        assert!(browser_route("/v1/prodex/gateway/auth/login/extra").is_none());
        assert!(parse_query("code=a&state=b").is_ok());
        assert!(parse_query("state=a&state=b").is_err());
        let headers = vec![(
            "Cookie".to_string(),
            "other=x; prodex_gateway_session=session_1".to_string(),
        )];
        assert_eq!(
            cookie_from_headers(&headers, SESSION_COOKIE),
            Some("session_1")
        );
    }
}
