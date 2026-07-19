use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use prodex_authn::{OidcBrowserFlowCapability, OidcBrowserFlowRequirement, OidcPkceMethod};
use runtime_proxy_crate::path_without_query;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

mod store;

pub(super) use self::store::RuntimeGatewayBrowserState;
use self::store::{
    BROWSER_TRANSACTION_TTL_MS, RuntimeGatewayBrowserSession, RuntimeGatewayBrowserTransaction,
    browser_store_transaction, browser_take_transaction,
};

use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_admin_auth::{
    RuntimeGatewayAdminAuthentication, runtime_gateway_oidc_admin_auth_from_verified,
    runtime_gateway_oidc_browser_endpoint, runtime_gateway_oidc_post_form,
    runtime_gateway_verify_oidc_logout_token, runtime_gateway_verify_oidc_token,
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

const MAX_TOKEN_RESPONSE_BYTES: usize = 64 * 1_024;
const BROWSER_SESSION_TTL_MS: u64 = 8 * 60 * 60 * 1_000;
const MAX_BROWSER_SESSIONS: usize = 4_096;
const MAX_BACKCHANNEL_LOGOUT_BYTES: u64 = 8 * 1_024;
const BACKCHANNEL_LOGOUT_MAX_AGE_SECONDS: u64 = 5 * 60;
const BACKCHANNEL_LOGOUT_EVENT: &str = "http://schemas.openid.net/event/backchannel-logout";
const SESSION_COOKIE: &str = "prodex_gateway_session";
const STATE_COOKIE: &str = "prodex_gateway_oidc_state";
const LOGOUT_INDEX_KEY_PREFIX: &str = "prodex:gateway:browser:logout:";
const SESSION_KEY_PREFIX: &str = "prodex:gateway:browser:session:";

#[derive(Clone, Copy)]
enum RuntimeGatewayBrowserRoute {
    Login,
    Callback,
    Logout,
    BackchannelLogout,
}

pub(super) fn runtime_gateway_browser_auth_response(
    request: &mut RuntimeLocalRewriteRequest,
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
        RuntimeGatewayBrowserRoute::BackchannelLogout
            if request.method().eq_ignore_ascii_case("POST") =>
        {
            runtime_gateway_browser_backchannel_logout(request, shared)
        }
        _ => Ok(method_not_allowed()),
    };
    Some(response.unwrap_or_else(|failure| match failure {
        RuntimeGatewayBrowserFailure::Unauthorized => build_runtime_proxy_json_error_response(
            401,
            "browser_authentication_failed",
            "browser authentication failed",
        ),
        RuntimeGatewayBrowserFailure::InvalidRequest => build_runtime_proxy_json_error_response(
            400,
            "invalid_request",
            "back-channel logout request is invalid",
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
    let oidc = shared.gateway_sso.oidc.as_ref()?;
    let session_id = cookie_from_headers(&request.headers, SESSION_COOKIE)?;
    let now = runtime_gateway_unix_epoch_millis();
    let session = browser_load_session(shared, session_id).ok().flatten()?;
    if session.expires_at_unix_ms <= now {
        let _ = browser_delete_session_record(shared, session_id, Some(&session));
        return None;
    }
    let verified = match runtime_gateway_verify_oidc_token(&session.id_token, oidc, shared) {
        Ok(verified) => verified,
        Err(_) => {
            let _ = browser_delete_session_record(shared, session_id, Some(&session));
            return None;
        }
    };
    match runtime_gateway_oidc_admin_auth_from_verified(verified, oidc, shared) {
        Some(authentication) => Some(authentication),
        None => {
            let _ = browser_delete_session_record(shared, session_id, Some(&session));
            None
        }
    }
}

#[derive(Clone, Copy)]
enum RuntimeGatewayBrowserFailure {
    Unauthorized,
    InvalidRequest,
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
    browser_store_transaction(
        shared,
        state.clone(),
        RuntimeGatewayBrowserTransaction {
            nonce: nonce.clone(),
            code_verifier,
            expires_at_unix_ms: now.saturating_add(BROWSER_TRANSACTION_TTL_MS),
        },
    )?;

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
    let transaction = browser_take_transaction(shared, state)?
        .ok_or(RuntimeGatewayBrowserFailure::Unauthorized)?;
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
    let logout_keys = browser_logout_keys(&verified.claims, &oidc.issuer, &oidc.audience);
    let _authentication = runtime_gateway_oidc_admin_auth_from_verified(verified, oidc, shared)
        .ok_or(RuntimeGatewayBrowserFailure::Unauthorized)?;
    let session_id = random_token().map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    browser_store_session(
        shared,
        session_id.clone(),
        RuntimeGatewayBrowserSession {
            id_token: id_token.to_string(),
            logout_keys,
            expires_at_unix_ms: now.saturating_add(BROWSER_SESSION_TTL_MS),
        },
    )?;
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
        browser_delete_session(shared, session_id)?;
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

fn runtime_gateway_browser_backchannel_logout(
    request: &mut RuntimeLocalRewriteRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> BrowserResult<tiny_http::ResponseBox> {
    let content_type_valid = request.headers().iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("content-type")
            && value
                .split(';')
                .next()
                .is_some_and(|value| value.trim() == "application/x-www-form-urlencoded")
    });
    if !content_type_valid {
        return Err(RuntimeGatewayBrowserFailure::InvalidRequest);
    }
    let captured = request
        .capture_with_limit(
            shared.runtime_shared.runtime_config.max_request_body_bytes,
            MAX_BACKCHANNEL_LOGOUT_BYTES,
        )
        .map_err(|_| RuntimeGatewayBrowserFailure::InvalidRequest)?;
    let form = std::str::from_utf8(&captured.body)
        .map_err(|_| RuntimeGatewayBrowserFailure::InvalidRequest)?;
    let parameters = parse_query(form).map_err(|_| RuntimeGatewayBrowserFailure::InvalidRequest)?;
    let logout_token = parameters
        .get("logout_token")
        .filter(|token| !token.is_empty())
        .ok_or(RuntimeGatewayBrowserFailure::InvalidRequest)?;
    let oidc = shared
        .gateway_sso
        .oidc
        .as_ref()
        .ok_or(RuntimeGatewayBrowserFailure::Unavailable)?;
    let verified = runtime_gateway_verify_oidc_logout_token(logout_token, oidc, shared)
        .map_err(|_| RuntimeGatewayBrowserFailure::InvalidRequest)?;
    let now_seconds = runtime_gateway_unix_epoch_millis() / 1_000;
    let logout_keys = browser_backchannel_logout_keys(
        &verified.claims,
        &oidc.issuer,
        &oidc.audience,
        now_seconds,
    )?;
    for logout_key in logout_keys {
        browser_revoke_logout_key(shared, &logout_key)?;
    }
    Ok(build_runtime_proxy_response_from_parts(
        RuntimeHeapTrimmedBufferedResponseParts {
            status: 204,
            headers: vec![("cache-control".to_string(), b"no-store".to_vec())],
            body: Vec::new().into(),
        },
    ))
}

fn browser_backchannel_logout_keys(
    claims: &BTreeMap<String, serde_json::Value>,
    issuer: &str,
    audience: &str,
    now_seconds: u64,
) -> BrowserResult<Vec<String>> {
    if claims.get("nonce").is_some()
        || !claims
            .get("events")
            .and_then(serde_json::Value::as_object)
            .is_some_and(|events| {
                events
                    .get(BACKCHANNEL_LOGOUT_EVENT)
                    .is_some_and(serde_json::Value::is_object)
            })
    {
        return Err(RuntimeGatewayBrowserFailure::InvalidRequest);
    }
    claims
        .get("jti")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 256)
        .ok_or(RuntimeGatewayBrowserFailure::InvalidRequest)?;
    let issued_at = claims
        .get("iat")
        .and_then(serde_json::Value::as_u64)
        .ok_or(RuntimeGatewayBrowserFailure::InvalidRequest)?;
    if issued_at > now_seconds.saturating_add(60)
        || now_seconds.saturating_sub(issued_at) > BACKCHANNEL_LOGOUT_MAX_AGE_SECONDS
    {
        return Err(RuntimeGatewayBrowserFailure::InvalidRequest);
    }
    let keys = browser_logout_keys(claims, issuer, audience);
    if keys.is_empty() {
        return Err(RuntimeGatewayBrowserFailure::InvalidRequest);
    }
    Ok(keys)
}

fn browser_logout_keys(
    claims: &BTreeMap<String, serde_json::Value>,
    issuer: &str,
    audience: &str,
) -> Vec<String> {
    ["sid", "sub"]
        .into_iter()
        .filter_map(|claim| {
            claims
                .get(claim)
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty() && value.len() <= 256)
                .map(|value| browser_logout_index_key(issuer, audience, claim, value))
        })
        .collect()
}

fn browser_logout_index_key(issuer: &str, audience: &str, claim: &str, value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"prodex:gateway:browser:logout:v1");
    for part in [
        issuer.as_bytes(),
        audience.as_bytes(),
        claim.as_bytes(),
        value.as_bytes(),
    ] {
        hasher.update([0]);
        hasher.update(part);
    }
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    format!("{LOGOUT_INDEX_KEY_PREFIX}{encoded}")
}

fn browser_revoke_logout_key(
    shared: &RuntimeLocalRewriteProxyShared,
    logout_key: &str,
) -> BrowserResult<()> {
    if let Some(executor) = shared.gateway_redis_rate_limit_executor.as_ref() {
        let session_ids = shared
            .runtime_shared
            .async_runtime
            .handle()
            .block_on(executor.take_ephemeral_members(logout_key))
            .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
        for session_id in session_ids {
            browser_delete_session(shared, &session_id)?;
        }
        return Ok(());
    }
    let session_ids = shared
        .gateway_browser
        .sessions
        .lock()
        .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?
        .iter()
        .filter(|(_, session)| {
            session
                .logout_keys
                .iter()
                .any(|candidate| candidate == logout_key)
        })
        .map(|(session_id, _)| session_id.clone())
        .collect::<Vec<_>>();
    for session_id in session_ids {
        browser_delete_session(shared, &session_id)?;
    }
    Ok(())
}

fn browser_store_session(
    shared: &RuntimeLocalRewriteProxyShared,
    session_id: String,
    session: RuntimeGatewayBrowserSession,
) -> BrowserResult<()> {
    let now = runtime_gateway_unix_epoch_millis();
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
    sessions.insert(session_id.clone(), session.clone());
    drop(sessions);
    let Some(executor) = shared.gateway_redis_rate_limit_executor.as_ref() else {
        return Ok(());
    };
    let value =
        serde_json::to_string(&session).map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    let stored = shared
        .runtime_shared
        .async_runtime
        .handle()
        .block_on(executor.put_ephemeral(
            &format!("{SESSION_KEY_PREFIX}{session_id}"),
            &value,
            std::time::Duration::from_millis(BROWSER_SESSION_TTL_MS),
        ))
        .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    if !stored {
        shared
            .gateway_browser
            .sessions
            .lock()
            .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?
            .remove(&session_id);
        return Err(RuntimeGatewayBrowserFailure::Unavailable);
    }
    for logout_key in &session.logout_keys {
        if shared
            .runtime_shared
            .async_runtime
            .handle()
            .block_on(executor.add_ephemeral_member(
                logout_key,
                &session_id,
                std::time::Duration::from_millis(BROWSER_SESSION_TTL_MS),
            ))
            .is_err()
        {
            let _ = browser_delete_session_record(shared, &session_id, Some(&session));
            return Err(RuntimeGatewayBrowserFailure::Unavailable);
        }
    }
    Ok(())
}

fn browser_load_session(
    shared: &RuntimeLocalRewriteProxyShared,
    session_id: &str,
) -> BrowserResult<Option<RuntimeGatewayBrowserSession>> {
    if let Some(executor) = shared.gateway_redis_rate_limit_executor.as_ref() {
        let value = shared
            .runtime_shared
            .async_runtime
            .handle()
            .block_on(executor.get_ephemeral(&format!("{SESSION_KEY_PREFIX}{session_id}")))
            .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
        return value
            .map(|value| {
                serde_json::from_str(&value).map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)
            })
            .transpose();
    }
    let now = runtime_gateway_unix_epoch_millis();
    let mut sessions = shared
        .gateway_browser
        .sessions
        .lock()
        .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    sessions.retain(|_, session| session.expires_at_unix_ms > now);
    Ok(sessions.get(session_id).cloned())
}

fn browser_delete_session(
    shared: &RuntimeLocalRewriteProxyShared,
    session_id: &str,
) -> BrowserResult<()> {
    let session = browser_load_session(shared, session_id)?;
    browser_delete_session_record(shared, session_id, session.as_ref())
}

fn browser_delete_session_record(
    shared: &RuntimeLocalRewriteProxyShared,
    session_id: &str,
    session: Option<&RuntimeGatewayBrowserSession>,
) -> BrowserResult<()> {
    if let Some(executor) = shared.gateway_redis_rate_limit_executor.as_ref() {
        shared
            .runtime_shared
            .async_runtime
            .handle()
            .block_on(executor.delete_ephemeral(&format!("{SESSION_KEY_PREFIX}{session_id}")))
            .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
        if let Some(session) = session {
            for logout_key in &session.logout_keys {
                shared
                    .runtime_shared
                    .async_runtime
                    .handle()
                    .block_on(executor.remove_ephemeral_member(logout_key, session_id))
                    .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
            }
        }
    }
    shared
        .gateway_browser
        .sessions
        .lock()
        .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?
        .remove(session_id);
    Ok(())
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
        "backchannel-logout" => Some(RuntimeGatewayBrowserRoute::BackchannelLogout),
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
    use std::collections::BTreeMap;

    use super::{
        BACKCHANNEL_LOGOUT_EVENT, LOGOUT_INDEX_KEY_PREFIX, RuntimeGatewayBrowserRoute,
        RuntimeGatewayBrowserSession, RuntimeGatewayBrowserTransaction, SESSION_COOKIE,
        browser_backchannel_logout_keys, browser_route, cookie_from_headers, parse_query,
    };

    #[test]
    fn browser_routes_queries_and_cookies_are_exact() {
        assert!(matches!(
            browser_route("/v1/prodex/gateway/auth/login"),
            Some(RuntimeGatewayBrowserRoute::Login)
        ));
        assert!(browser_route("/v1/prodex/gateway/auth/login/extra").is_none());
        assert!(matches!(
            browser_route("/v1/prodex/gateway/auth/backchannel-logout"),
            Some(RuntimeGatewayBrowserRoute::BackchannelLogout)
        ));
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

    #[test]
    fn browser_shared_records_round_trip_without_authentication_state() {
        let transaction = RuntimeGatewayBrowserTransaction {
            nonce: "nonce".to_string(),
            code_verifier: "verifier".to_string(),
            expires_at_unix_ms: 123,
        };
        let transaction: RuntimeGatewayBrowserTransaction =
            serde_json::from_str(&serde_json::to_string(&transaction).unwrap()).unwrap();
        assert_eq!(transaction.nonce, "nonce");

        let session = RuntimeGatewayBrowserSession {
            id_token: "fixture.id.token".to_string(),
            logout_keys: vec![format!("{LOGOUT_INDEX_KEY_PREFIX}fixture")],
            expires_at_unix_ms: 456,
        };
        let session: RuntimeGatewayBrowserSession =
            serde_json::from_str(&serde_json::to_string(&session).unwrap()).unwrap();
        assert_eq!(session.id_token, "fixture.id.token");
    }

    #[test]
    fn backchannel_logout_claims_are_recent_event_bound_and_hashed() {
        let claims = BTreeMap::from([
            ("iat".to_string(), serde_json::json!(1_000)),
            (
                "jti".to_string(),
                serde_json::json!("logout-token-identifier"),
            ),
            (
                "sid".to_string(),
                serde_json::json!("private-session-identifier"),
            ),
            (
                "sub".to_string(),
                serde_json::json!("private-subject-identifier"),
            ),
            (
                "events".to_string(),
                serde_json::json!({BACKCHANNEL_LOGOUT_EVENT: {}}),
            ),
        ]);
        let keys = browser_backchannel_logout_keys(
            &claims,
            "https://identity.example.com",
            "prodex-gateway",
            1_001,
        )
        .ok()
        .unwrap();
        assert_eq!(keys.len(), 2);
        assert!(
            keys.iter()
                .all(|key| key.starts_with(LOGOUT_INDEX_KEY_PREFIX))
        );
        assert!(keys.iter().all(|key| {
            !key.contains("private-session-identifier")
                && !key.contains("private-subject-identifier")
        }));

        let mut invalid = claims;
        invalid.insert("nonce".to_string(), serde_json::json!("not-allowed"));
        assert!(
            browser_backchannel_logout_keys(
                &invalid,
                "https://identity.example.com",
                "prodex-gateway",
                1_001,
            )
            .is_err()
        );
    }
}
