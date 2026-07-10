use super::local_rewrite::{RuntimeGatewayOidcHttpCacheEntry, RuntimeLocalRewriteProxyShared};
use super::local_rewrite_gateway_scope::RuntimeGatewayGovernanceScope;
use super::local_rewrite_gateway_store_types::{
    RuntimeGatewayScimUser, RuntimeGatewayVirtualKeyEntry,
    runtime_gateway_scim_user_auth_entry_from_stored,
};
use super::*;
use crate::{RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES, read_blocking_response_body_with_limit};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use prodex_observability::{
    JwksRefreshOutcome, OidcRefreshOperation, OidcRefreshResult, plan_jwks_cache_age_metric,
    plan_jwks_refresh_outcome_metric, plan_oidc_refresh_metric,
};
use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};

const RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV: &str = "PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS";
const DEFAULT_RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS: u64 = 2_000;
const RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV: &str = "PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS";
const DEFAULT_RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS: u64 = 300;
const RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_ENV: &str =
    "PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS";
const DEFAULT_RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS: u64 = 30_000;
const RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV: &str =
    "PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS";
const DEFAULT_RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS: u64 = 86_400;
const MIN_RUNTIME_GATEWAY_OIDC_BACKGROUND_REFRESH_MS: u64 = 50;

pub(super) struct RuntimeGatewayAdminAuth {
    pub(super) name: String,
    pub(super) role: RuntimeGatewayAdminRole,
    pub(super) tenant_id: Option<String>,
    pub(super) team_id: Option<String>,
    pub(super) project_id: Option<String>,
    pub(super) user_id: Option<String>,
    pub(super) budget_id: Option<String>,
    pub(super) allowed_key_prefixes: Vec<String>,
}

pub(super) fn runtime_gateway_admin_auth(
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<RuntimeGatewayAdminAuth> {
    if let Some(auth) = runtime_gateway_oidc_admin_auth(captured, shared) {
        return Some(auth);
    }
    if let Some(auth) = runtime_gateway_sso_admin_auth(captured, shared) {
        return Some(auth);
    }
    let authorization = captured
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))
        .map(|(_, value)| value.as_str())?;
    for token in &shared.gateway_admin_tokens {
        if token.token_hash.verify_authorization_header(authorization) {
            return Some(RuntimeGatewayAdminAuth {
                name: token.name.clone(),
                role: token.role,
                tenant_id: token.tenant_id.clone(),
                team_id: token.team_id.clone(),
                project_id: token.project_id.clone(),
                user_id: token.user_id.clone(),
                budget_id: token.budget_id.clone(),
                allowed_key_prefixes: token.allowed_key_prefixes.clone(),
            });
        }
    }
    None
}

fn runtime_gateway_oidc_admin_auth(
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<RuntimeGatewayAdminAuth> {
    let config = shared.gateway_sso.oidc.as_ref()?;
    let authorization = runtime_gateway_header(captured, "authorization")?;
    let token = runtime_gateway_authorization_bearer_token(authorization)?;
    let claims = runtime_gateway_verify_oidc_token(token, config, shared).ok()?;
    let name = runtime_gateway_oidc_admin_name(&claims, config)?;
    let claimed_tenant_id =
        runtime_gateway_oidc_claim_scope_string(&claims, &config.tenant_claim).ok()?;
    let scim_user = runtime_gateway_scim_user_for_claimed_tenant(
        runtime_gateway_scim_user_by_name(shared, &name).ok()?,
        claimed_tenant_id.as_deref(),
    );
    if scim_user.as_ref().is_some_and(|user| !user.active) {
        return None;
    }
    let role = runtime_gateway_sso_resolved_role(
        runtime_gateway_oidc_role_claim_string(&claims, &config.role_claim).as_deref(),
        scim_user.as_ref(),
    );
    let tenant_id =
        claimed_tenant_id.or_else(|| scim_user.as_ref().and_then(|user| user.tenant_id.clone()));
    if shared.gateway_sso.require_tenant && tenant_id.is_none() {
        return None;
    }
    let team_id = scim_user.as_ref().and_then(|user| user.team_id.clone());
    let project_id = scim_user.as_ref().and_then(|user| user.project_id.clone());
    let user_id = scim_user.as_ref().and_then(|user| user.user_id.clone());
    let budget_id = scim_user.as_ref().and_then(|user| user.budget_id.clone());
    let allowed_key_prefixes =
        runtime_gateway_oidc_claim_string_vec(&claims, &config.key_prefixes_claim)
            .ok()?
            .or_else(|| {
                scim_user
                    .as_ref()
                    .map(|user| user.allowed_key_prefixes.clone())
            })
            .unwrap_or_default();
    Some(RuntimeGatewayAdminAuth {
        name: format!("oidc:{name}"),
        role,
        tenant_id,
        team_id,
        project_id,
        user_id,
        budget_id,
        allowed_key_prefixes,
    })
}

fn runtime_gateway_verify_oidc_token(
    token: &str,
    config: &RuntimeGatewayOidcConfig,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<BTreeMap<String, serde_json::Value>> {
    let header = decode_header(token).context("failed to decode gateway OIDC JWT header")?;
    let alg = runtime_gateway_oidc_algorithm(header.alg)?;
    let jwks_url = runtime_gateway_oidc_cached_jwks_url(config, shared)?;
    let jwks_value = runtime_gateway_oidc_cached_json(shared, &jwks_url, "JWKS")?;
    let jwks: JwkSet =
        serde_json::from_value(jwks_value).context("failed to parse gateway OIDC JWKS")?;
    let kid = header
        .kid
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("gateway OIDC JWT is missing kid"))?;
    let key = jwks
        .find(kid)
        .ok_or_else(|| anyhow::anyhow!("gateway OIDC JWKS missing key for kid"))?;
    let decoding_key = DecodingKey::from_jwk(key).context("failed to build gateway OIDC key")?;
    let mut validation = Validation::new(alg);
    validation.set_audience(&[config.audience.as_str()]);
    validation.set_issuer(&[config.issuer.as_str()]);
    let data = decode::<BTreeMap<String, serde_json::Value>>(token, &decoding_key, &validation)
        .context("gateway OIDC token validation failed")?;
    Ok(data.claims)
}

pub(super) fn runtime_gateway_prefetch_oidc_cache(
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<()> {
    let Some(config) = shared.gateway_sso.oidc.as_ref() else {
        return Ok(());
    };
    let jwks_url = runtime_gateway_oidc_fetch_jwks_url(config, shared)?;
    runtime_gateway_oidc_fetch_json(shared, &jwks_url, "JWKS")?;
    Ok(())
}

pub(super) fn runtime_gateway_run_oidc_background_refresh_loop(
    shared: RuntimeLocalRewriteProxyShared,
    shutdown: Arc<AtomicBool>,
) {
    let ttl = runtime_gateway_oidc_http_cache_ttl();
    let mut initial_prefetch_pending = true;
    let mut last_refresh_failed = false;
    loop {
        if runtime_gateway_oidc_cache_refresh_due(&shared, ttl) {
            match runtime_gateway_prefetch_oidc_cache(&shared) {
                Ok(()) => last_refresh_failed = false,
                Err(_err) => {
                    last_refresh_failed = true;
                    runtime_proxy_log_to_path(
                        &shared.runtime_shared.log_path,
                        &runtime_proxy_structured_log_message(
                            "gateway_oidc_prefetch_failed",
                            [runtime_proxy_log_field(
                                "error_kind",
                                "gateway_oidc_prefetch_failed",
                            )],
                        ),
                    );
                }
            }
        }
        if initial_prefetch_pending {
            shared
                .gateway_oidc_prefetch_in_flight
                .store(false, Ordering::SeqCst);
            initial_prefetch_pending = false;
        }
        let poll_interval = if last_refresh_failed {
            runtime_gateway_log_oidc_refresh_metric(
                &shared,
                OidcRefreshOperation::FetchJwks,
                OidcRefreshResult::Backoff,
            );
            runtime_gateway_oidc_refresh_failure_backoff()
        } else {
            runtime_gateway_oidc_background_refresh_sleep(&shared, ttl)
        };
        if runtime_gateway_oidc_wait_for_shutdown(&shutdown, poll_interval) {
            break;
        }
    }
    if initial_prefetch_pending {
        shared
            .gateway_oidc_prefetch_in_flight
            .store(false, Ordering::SeqCst);
    }
}

fn runtime_gateway_oidc_cached_json(
    shared: &RuntimeLocalRewriteProxyShared,
    url: &str,
    description: &str,
) -> Result<serde_json::Value> {
    let fallback_ttl = runtime_gateway_oidc_http_cache_ttl();
    let fallback_lkg = runtime_gateway_oidc_last_known_good_window();
    if let Some(entry) =
        runtime_gateway_oidc_cached_entry(shared, url, description, fallback_ttl, fallback_lkg)
    {
        return serde_json::from_str(&entry.payload)
            .with_context(|| format!("failed to parse cached gateway OIDC {description}"));
    }
    runtime_gateway_wait_for_oidc_prefetch(shared);
    if let Some(entry) =
        runtime_gateway_oidc_cached_entry(shared, url, description, fallback_ttl, fallback_lkg)
    {
        return serde_json::from_str(&entry.payload)
            .with_context(|| format!("failed to parse cached gateway OIDC {description}"));
    }
    bail!("gateway OIDC {description} is not cached")
}

fn runtime_gateway_oidc_cached_entry(
    shared: &RuntimeLocalRewriteProxyShared,
    url: &str,
    description: &str,
    fallback_ttl: Duration,
    fallback_lkg: Duration,
) -> Option<RuntimeGatewayOidcHttpCacheEntry> {
    let entry = shared
        .gateway_oidc_http_cache
        .lock()
        .ok()
        .and_then(|cache| cache.get(url).cloned());
    if let Some(entry) = entry.as_ref() {
        runtime_gateway_log_jwks_cache_age_metric(shared, description, entry);
    }
    entry.filter(|entry| {
        runtime_gateway_oidc_cache_entry_usable(entry, fallback_ttl, fallback_lkg, Instant::now())
    })
}

fn runtime_gateway_wait_for_oidc_prefetch(shared: &RuntimeLocalRewriteProxyShared) {
    if !shared
        .gateway_oidc_prefetch_in_flight
        .load(Ordering::SeqCst)
    {
        return;
    }
    let deadline = Instant::now() + runtime_gateway_oidc_prefetch_timeout();
    while shared
        .gateway_oidc_prefetch_in_flight
        .load(Ordering::SeqCst)
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn runtime_gateway_oidc_fetch_json(
    shared: &RuntimeLocalRewriteProxyShared,
    url: &str,
    description: &str,
) -> Result<serde_json::Value> {
    let now = Instant::now();
    let fetched_payload = shared
        .client
        .get(url)
        .timeout(runtime_gateway_oidc_prefetch_timeout())
        .send()
        .with_context(|| format!("failed to fetch gateway OIDC {description}"))
        .and_then(|response| {
            response
                .error_for_status()
                .with_context(|| format!("gateway OIDC {description} endpoint returned an error"))
        });
    match fetched_payload {
        Ok(response) => {
            let operation = runtime_gateway_oidc_refresh_operation(description);
            let cache_control = response
                .headers()
                .get(reqwest::header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok());
            let max_age = runtime_gateway_oidc_cache_control_max_age(cache_control);
            let stale_while_revalidate =
                runtime_gateway_oidc_cache_control_stale_while_revalidate(cache_control);
            let payload = match read_blocking_response_body_with_limit(
                response,
                RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
                &format!("failed to read gateway OIDC {description}"),
            ) {
                Ok(payload) => payload,
                Err(err) => {
                    runtime_gateway_log_oidc_refresh_metric(
                        shared,
                        operation,
                        OidcRefreshResult::Failed,
                    );
                    runtime_gateway_log_jwks_refresh_metric(
                        shared,
                        description,
                        JwksRefreshOutcome::Failure,
                    );
                    return Err(err);
                }
            };
            let parsed: serde_json::Value = match serde_json::from_slice(&payload)
                .with_context(|| format!("failed to parse gateway OIDC {description}"))
            {
                Ok(parsed) => parsed,
                Err(err) => {
                    runtime_gateway_log_oidc_refresh_metric(
                        shared,
                        operation,
                        OidcRefreshResult::Failed,
                    );
                    runtime_gateway_log_jwks_refresh_metric(
                        shared,
                        description,
                        JwksRefreshOutcome::Failure,
                    );
                    return Err(err);
                }
            };
            let payload = String::from_utf8(payload)
                .with_context(|| format!("gateway OIDC {description} is not UTF-8"))?;
            runtime_gateway_log_oidc_refresh_metric(shared, operation, OidcRefreshResult::Success);
            runtime_gateway_log_jwks_refresh_metric(
                shared,
                description,
                JwksRefreshOutcome::Success,
            );
            if let Ok(mut cache) = shared.gateway_oidc_http_cache.lock() {
                cache.insert(
                    url.to_string(),
                    RuntimeGatewayOidcHttpCacheEntry {
                        fetched_at: now,
                        payload,
                        max_age,
                        stale_while_revalidate,
                    },
                );
            }
            Ok(parsed)
        }
        Err(err) => {
            runtime_gateway_log_oidc_refresh_metric(
                shared,
                runtime_gateway_oidc_refresh_operation(description),
                OidcRefreshResult::Failed,
            );
            runtime_gateway_log_jwks_refresh_metric(
                shared,
                description,
                JwksRefreshOutcome::Failure,
            );
            Err(err)
        }
    }
}

fn runtime_gateway_oidc_refresh_operation(description: &str) -> OidcRefreshOperation {
    if description == "JWKS" {
        OidcRefreshOperation::FetchJwks
    } else {
        OidcRefreshOperation::DiscoverIssuer
    }
}

fn runtime_gateway_log_oidc_refresh_metric(
    shared: &RuntimeLocalRewriteProxyShared,
    operation: OidcRefreshOperation,
    result: OidcRefreshResult,
) {
    let Ok(metric) = plan_oidc_refresh_metric(operation, result) else {
        return;
    };
    let Ok((operation_key, operation_value)) = metric.operation_label.as_metric_label() else {
        return;
    };
    let Ok((result_key, result_value)) = metric.result_label.as_metric_label() else {
        return;
    };
    runtime_proxy_log_to_path(
        &shared.runtime_shared.log_path,
        &runtime_proxy_structured_log_message(
            "gateway_oidc_refresh_metric",
            [
                runtime_proxy_log_field("metric_name", metric.metric_name),
                runtime_proxy_log_field("increment", metric.increment.to_string()),
                runtime_proxy_log_field(operation_key, operation_value),
                runtime_proxy_log_field(result_key, result_value),
            ],
        ),
    );
}

fn runtime_gateway_log_jwks_refresh_metric(
    shared: &RuntimeLocalRewriteProxyShared,
    description: &str,
    outcome: JwksRefreshOutcome,
) {
    if description != "JWKS" {
        return;
    }
    let Ok(metric) = plan_jwks_refresh_outcome_metric(outcome) else {
        return;
    };
    let Ok((result_key, result_value)) = metric.result_label.as_metric_label() else {
        return;
    };
    runtime_proxy_log_to_path(
        &shared.runtime_shared.log_path,
        &runtime_proxy_structured_log_message(
            "gateway_jwks_refresh_metric",
            [
                runtime_proxy_log_field("metric_name", metric.metric_name),
                runtime_proxy_log_field("increment", metric.increment.to_string()),
                runtime_proxy_log_field(result_key, result_value),
            ],
        ),
    );
}

fn runtime_gateway_log_jwks_cache_age_metric(
    shared: &RuntimeLocalRewriteProxyShared,
    description: &str,
    entry: &RuntimeGatewayOidcHttpCacheEntry,
) {
    if description != "JWKS" {
        return;
    }
    let age_ms = Instant::now()
        .saturating_duration_since(entry.fetched_at)
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    let ttl_ms = runtime_gateway_oidc_cache_entry_ttl(entry, runtime_gateway_oidc_http_cache_ttl())
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    let stale_ms = runtime_gateway_oidc_cache_entry_stale_window(
        entry,
        runtime_gateway_oidc_last_known_good_window(),
    )
    .as_millis()
    .min(u128::from(u64::MAX)) as u64;
    let snapshot = prodex_domain::JwksCacheSnapshot {
        fetched_at_unix_ms: 0,
        expires_at_unix_ms: ttl_ms,
        stale_until_unix_ms: ttl_ms.saturating_add(stale_ms),
        key_count: 1,
        last_refresh_error_at_unix_ms: None,
        retry_after_unix_ms: None,
    };
    let Ok(metric) = plan_jwks_cache_age_metric(Some(&snapshot), age_ms) else {
        return;
    };
    let Ok((state_key, state_value)) = metric.state_label.as_metric_label() else {
        return;
    };
    runtime_proxy_log_to_path(
        &shared.runtime_shared.log_path,
        &runtime_proxy_structured_log_message(
            "gateway_jwks_cache_age_metric",
            [
                runtime_proxy_log_field("metric_name", metric.metric_name),
                runtime_proxy_log_field(
                    "age_ms",
                    metric.age_ms.map(|age| age.to_string()).unwrap_or_default(),
                ),
                runtime_proxy_log_field(state_key, state_value),
            ],
        ),
    );
}

fn runtime_gateway_oidc_cache_control_max_age(cache_control: Option<&str>) -> Option<Duration> {
    runtime_gateway_oidc_cache_control_duration(cache_control, "max-age")
}

fn runtime_gateway_oidc_cache_control_stale_while_revalidate(
    cache_control: Option<&str>,
) -> Option<Duration> {
    runtime_gateway_oidc_cache_control_duration(cache_control, "stale-while-revalidate")
}

fn runtime_gateway_oidc_cache_control_duration(
    cache_control: Option<&str>,
    directive_name: &str,
) -> Option<Duration> {
    for directive in cache_control?.split(',') {
        let Some((name, value)) = directive.trim().split_once('=') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case(directive_name) {
            return value
                .trim()
                .trim_matches('"')
                .parse::<u64>()
                .ok()
                .map(Duration::from_secs);
        }
    }
    None
}

fn runtime_gateway_oidc_prefetch_timeout() -> Duration {
    Duration::from_millis(
        std::env::var(RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV)
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS),
    )
}

fn runtime_gateway_oidc_http_cache_ttl() -> Duration {
    match std::env::var(RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV) {
        Ok(value) => match value.trim().parse::<u64>() {
            Ok(seconds) => Duration::from_secs(seconds),
            Err(_) => Duration::from_secs(DEFAULT_RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS),
        },
        Err(_) => Duration::from_secs(DEFAULT_RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS),
    }
}

fn runtime_gateway_oidc_refresh_failure_backoff() -> Duration {
    Duration::from_millis(
        std::env::var(RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_ENV)
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS),
    )
}

fn runtime_gateway_oidc_last_known_good_window() -> Duration {
    match std::env::var(RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV) {
        Ok(value) => match value.trim().parse::<u64>() {
            Ok(seconds) => Duration::from_secs(seconds),
            Err(_) => Duration::from_secs(DEFAULT_RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS),
        },
        Err(_) => Duration::from_secs(DEFAULT_RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS),
    }
}

fn runtime_gateway_oidc_background_refresh_interval(ttl: Duration) -> Duration {
    if ttl.is_zero() {
        Duration::from_millis(MIN_RUNTIME_GATEWAY_OIDC_BACKGROUND_REFRESH_MS)
    } else {
        ttl
    }
}

fn runtime_gateway_oidc_background_refresh_sleep(
    shared: &RuntimeLocalRewriteProxyShared,
    fallback: Duration,
) -> Duration {
    let ttl = shared
        .gateway_oidc_http_cache
        .lock()
        .ok()
        .and_then(|cache| {
            cache
                .values()
                .map(|entry| runtime_gateway_oidc_cache_entry_ttl(entry, fallback))
                .min()
        })
        .unwrap_or(fallback);
    runtime_gateway_oidc_background_refresh_interval(ttl)
}

fn runtime_gateway_oidc_cache_refresh_due(
    shared: &RuntimeLocalRewriteProxyShared,
    ttl: Duration,
) -> bool {
    let Ok(cache) = shared.gateway_oidc_http_cache.lock() else {
        return true;
    };
    if cache.is_empty() || ttl.is_zero() {
        return true;
    }
    let now = Instant::now();
    cache.values().any(|entry| {
        now.duration_since(entry.fetched_at) >= runtime_gateway_oidc_cache_entry_ttl(entry, ttl)
    })
}

fn runtime_gateway_oidc_cache_entry_ttl(
    entry: &RuntimeGatewayOidcHttpCacheEntry,
    fallback: Duration,
) -> Duration {
    if fallback.is_zero() {
        Duration::ZERO
    } else {
        entry.max_age.unwrap_or(fallback)
    }
}

fn runtime_gateway_oidc_cache_entry_stale_window(
    entry: &RuntimeGatewayOidcHttpCacheEntry,
    fallback: Duration,
) -> Duration {
    entry.stale_while_revalidate.unwrap_or(fallback)
}

fn runtime_gateway_oidc_cache_entry_usable(
    entry: &RuntimeGatewayOidcHttpCacheEntry,
    fallback_ttl: Duration,
    fallback_lkg: Duration,
    now: Instant,
) -> bool {
    let usable_for = runtime_gateway_oidc_cache_entry_ttl(entry, fallback_ttl).saturating_add(
        runtime_gateway_oidc_cache_entry_stale_window(entry, fallback_lkg),
    );
    now.saturating_duration_since(entry.fetched_at) < usable_for
}

fn runtime_gateway_oidc_wait_for_shutdown(shutdown: &AtomicBool, duration: Duration) -> bool {
    let deadline = Instant::now() + duration;
    loop {
        if shutdown.load(Ordering::SeqCst) {
            return true;
        }
        let now = Instant::now();
        if now >= deadline {
            return false;
        }
        std::thread::sleep((deadline - now).min(Duration::from_millis(50)));
    }
}

fn runtime_gateway_oidc_cached_jwks_url(
    config: &RuntimeGatewayOidcConfig,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<String> {
    if let Some(jwks_url) = config.jwks_url.as_deref() {
        return Ok(jwks_url.to_string());
    }
    let discovery_url = format!(
        "{}/.well-known/openid-configuration",
        config.issuer.trim_end_matches('/')
    );
    let discovery = runtime_gateway_oidc_cached_json(shared, &discovery_url, "discovery document")?;
    discovery
        .get("jwks_uri")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("gateway OIDC discovery document is missing jwks_uri"))
}

fn runtime_gateway_oidc_fetch_jwks_url(
    config: &RuntimeGatewayOidcConfig,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<String> {
    if let Some(jwks_url) = config.jwks_url.as_deref() {
        return Ok(jwks_url.to_string());
    }
    let discovery_url = format!(
        "{}/.well-known/openid-configuration",
        config.issuer.trim_end_matches('/')
    );
    let discovery = runtime_gateway_oidc_fetch_json(shared, &discovery_url, "discovery document")?;
    discovery
        .get("jwks_uri")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("gateway OIDC discovery document is missing jwks_uri"))
}

fn runtime_gateway_oidc_algorithm(alg: Algorithm) -> Result<Algorithm> {
    match alg {
        Algorithm::RS256
        | Algorithm::RS384
        | Algorithm::RS512
        | Algorithm::ES256
        | Algorithm::ES384 => Ok(alg),
        _ => bail!("gateway OIDC JWT algorithm is not allowed"),
    }
}

fn runtime_gateway_authorization_bearer_token(value: &str) -> Option<&str> {
    let mut parts = value.split_whitespace();
    let scheme = parts.next()?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    Some(token)
}

fn runtime_gateway_oidc_claim_string(
    claims: &BTreeMap<String, serde_json::Value>,
    field: &str,
) -> Option<String> {
    claims
        .get(field)
        .and_then(serde_json::Value::as_str)
        .and_then(runtime_gateway_exact_scope_string)
}

fn runtime_gateway_oidc_admin_name(
    claims: &BTreeMap<String, serde_json::Value>,
    config: &RuntimeGatewayOidcConfig,
) -> Option<String> {
    let configured_claim =
        runtime_gateway_oidc_claim_scope_string(claims, &config.user_claim).ok()?;
    if let Some(name) = configured_claim {
        return Some(name);
    }
    for claim in ["email", "preferred_username", "sub"] {
        if claim == config.user_claim {
            continue;
        }
        if let Some(name) = runtime_gateway_oidc_claim_scope_string(claims, claim).ok()? {
            return Some(name);
        }
    }
    None
}

fn runtime_gateway_oidc_claim_scope_string(
    claims: &BTreeMap<String, serde_json::Value>,
    field: &str,
) -> Result<Option<String>, ()> {
    let Some(value) = claims.get(field) else {
        return Ok(None);
    };
    value
        .as_str()
        .and_then(runtime_gateway_exact_scope_string)
        .map(Some)
        .ok_or(())
}

fn runtime_gateway_oidc_role_claim_string(
    claims: &BTreeMap<String, serde_json::Value>,
    field: &str,
) -> Option<String> {
    runtime_gateway_oidc_claim_scope_string(claims, field).unwrap_or_else(|()| Some(String::new()))
}

fn runtime_gateway_oidc_claim_string_vec(
    claims: &BTreeMap<String, serde_json::Value>,
    field: &str,
) -> Result<Option<Vec<String>>, ()> {
    let Some(value) = claims.get(field) else {
        return Ok(None);
    };
    if let Some(value) = value.as_str() {
        return runtime_gateway_parse_sso_prefixes(value)
            .map(Some)
            .ok_or(());
    }
    let values = value.as_array().ok_or(())?;
    values
        .iter()
        .map(|value| runtime_gateway_exact_scope_string(value.as_str()?))
        .collect::<Option<Vec<_>>>()
        .map(Some)
        .ok_or(())
}

fn runtime_gateway_sso_admin_auth(
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<RuntimeGatewayAdminAuth> {
    let config = &shared.gateway_sso;
    let proxy_token_hash = config.proxy_token_hash.as_ref()?;
    let proxy_token = runtime_gateway_header(captured, &config.token_header)?;
    if !runtime_gateway_sso_proxy_token_matches(proxy_token_hash, proxy_token) {
        return None;
    }
    let name =
        runtime_gateway_sso_user_name(runtime_gateway_header(captured, &config.user_header))?;
    let claimed_tenant_id = match runtime_gateway_header(captured, &config.tenant_header) {
        Some(value) => Some(runtime_gateway_exact_scope_string(value)?),
        None => None,
    };
    let scim_user = runtime_gateway_scim_user_for_claimed_tenant(
        runtime_gateway_scim_user_by_name(shared, name).ok()?,
        claimed_tenant_id.as_deref(),
    );
    if scim_user.as_ref().is_some_and(|user| !user.active) {
        return None;
    }
    let role = runtime_gateway_sso_resolved_role(
        runtime_gateway_header(captured, &config.role_header),
        scim_user.as_ref(),
    );
    let tenant_id =
        claimed_tenant_id.or_else(|| scim_user.as_ref().and_then(|user| user.tenant_id.clone()));
    if config.require_tenant && tenant_id.is_none() {
        return None;
    }
    let team_id = scim_user.as_ref().and_then(|user| user.team_id.clone());
    let project_id = scim_user.as_ref().and_then(|user| user.project_id.clone());
    let user_id = scim_user.as_ref().and_then(|user| user.user_id.clone());
    let budget_id = scim_user.as_ref().and_then(|user| user.budget_id.clone());
    let allowed_key_prefixes =
        match runtime_gateway_header(captured, &config.key_prefixes_header) {
            Some(value) => Some(runtime_gateway_parse_sso_prefixes(value)?),
            None => None,
        }
        .or_else(|| {
            scim_user
                .as_ref()
                .map(|user| user.allowed_key_prefixes.clone())
        })
        .unwrap_or_default();
    Some(RuntimeGatewayAdminAuth {
        name: format!("sso:{name}"),
        role,
        tenant_id,
        team_id,
        project_id,
        user_id,
        budget_id,
        allowed_key_prefixes,
    })
}

fn runtime_gateway_sso_proxy_token_matches(
    proxy_token_hash: &runtime_proxy_crate::LocalBridgeBearerTokenHash,
    proxy_token: &str,
) -> bool {
    proxy_token_hash.verify_bearer_token(proxy_token)
}

fn runtime_gateway_sso_user_name(value: Option<&str>) -> Option<&str> {
    match value {
        Some(value) if !value.is_empty() && !value.chars().any(char::is_whitespace) => Some(value),
        Some(_) => None,
        None => None,
    }
}

fn runtime_gateway_sso_resolved_role(
    role_claim: Option<&str>,
    scim_user: Option<&RuntimeGatewayScimUser>,
) -> RuntimeGatewayAdminRole {
    if let Some(role_claim) = role_claim {
        return RuntimeGatewayAdminRole::parse(role_claim)
            .unwrap_or(RuntimeGatewayAdminRole::Viewer);
    }
    scim_user
        .and_then(|user| user.role.as_deref())
        .and_then(RuntimeGatewayAdminRole::parse)
        .unwrap_or(RuntimeGatewayAdminRole::Viewer)
}

fn runtime_gateway_header<'a>(
    captured: &'a RuntimeProxyRequest,
    header_name: &str,
) -> Option<&'a str> {
    captured
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(header_name))
        .map(|(_, value)| value.as_str())
}

fn runtime_gateway_parse_sso_prefixes(value: &str) -> Option<Vec<String>> {
    value
        .split([',', ';', '\n'])
        .map(runtime_gateway_exact_scope_string)
        .collect()
}

fn runtime_gateway_exact_scope_string(value: &str) -> Option<String> {
    (!value.is_empty() && !value.chars().any(char::is_whitespace)).then(|| value.to_string())
}

fn runtime_gateway_scim_user_by_name(
    shared: &RuntimeLocalRewriteProxyShared,
    name: &str,
) -> Result<Option<RuntimeGatewayScimUser>, ()> {
    if name.is_empty() || name.chars().any(char::is_whitespace) {
        return Ok(None);
    }
    let store = match super::local_rewrite::runtime_gateway_virtual_key_store_load_strict(
        &shared.gateway_state_store,
        &shared.runtime_shared.log_path,
    ) {
        Ok(store) => store,
        Err(err) => {
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_admin_scim_state_unavailable",
                    [runtime_proxy_log_field("error", err.to_string())],
                ),
            );
            return Err(());
        }
    };
    Ok(store
        .scim_users
        .into_iter()
        .filter_map(|user| runtime_gateway_scim_user_auth_entry_from_stored(&user))
        .find(|user| user.user_name.eq_ignore_ascii_case(name)))
}

fn runtime_gateway_scim_user_for_claimed_tenant(
    scim_user: Option<RuntimeGatewayScimUser>,
    claimed_tenant_id: Option<&str>,
) -> Option<RuntimeGatewayScimUser> {
    let user = scim_user?;
    if claimed_tenant_id.is_some_and(|tenant_id| user.tenant_id.as_deref() != Some(tenant_id)) {
        return None;
    }
    Some(user)
}

impl RuntimeGatewayAdminAuth {
    pub(super) fn governance_scope(&self) -> RuntimeGatewayGovernanceScope {
        RuntimeGatewayGovernanceScope::new(
            self.tenant_id.clone(),
            self.team_id.clone(),
            self.project_id.clone(),
            self.user_id.clone(),
            self.budget_id.clone(),
        )
    }

    pub(super) fn can_access_tenant(&self, tenant_id: Option<&str>) -> bool {
        self.governance_scope().matches_tenant(tenant_id)
    }

    pub(super) fn can_access_dimensions(
        &self,
        team_id: Option<&str>,
        project_id: Option<&str>,
        user_id: Option<&str>,
        budget_id: Option<&str>,
    ) -> bool {
        self.governance_scope()
            .matches_dimensions(team_id, project_id, user_id, budget_id)
    }

    pub(super) fn can_access_key(&self, key_name: &str) -> bool {
        self.allowed_key_prefixes.is_empty()
            || self
                .allowed_key_prefixes
                .iter()
                .any(|prefix| key_name.starts_with(prefix))
    }

    pub(super) fn can_access_entry(&self, entry: &RuntimeGatewayVirtualKeyEntry) -> bool {
        self.governance_scope().matches(
            entry.tenant_id.as_deref(),
            entry.key.team_id.as_deref(),
            entry.key.project_id.as_deref(),
            entry.key.user_id.as_deref(),
            entry.key.budget_id.as_deref(),
        ) && self.can_access_key(&entry.key.name)
    }

    pub(super) fn can_access_scim_user(&self, user: &RuntimeGatewayScimUser) -> bool {
        self.governance_scope().matches(
            user.tenant_id.as_deref(),
            user.team_id.as_deref(),
            user.project_id.as_deref(),
            user.user_id.as_deref(),
            user.budget_id.as_deref(),
        )
    }
}

pub(super) fn runtime_gateway_admin_auth_is_unscoped(admin_auth: &RuntimeGatewayAdminAuth) -> bool {
    admin_auth.governance_scope().is_unscoped()
}

pub(super) fn runtime_gateway_admin_auth_matches_entry(
    admin_auth: &RuntimeGatewayAdminAuth,
    entry: &RuntimeGatewayVirtualKeyEntry,
) -> bool {
    admin_auth.governance_scope().matches(
        entry.tenant_id.as_deref(),
        entry.key.team_id.as_deref(),
        entry.key.project_id.as_deref(),
        entry.key.user_id.as_deref(),
        entry.key.budget_id.as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oidc_prefetch_timeout_uses_positive_env_override() {
        let _guard = crate::TestEnvVarGuard::set(RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV, "25");

        assert_eq!(
            runtime_gateway_oidc_prefetch_timeout(),
            Duration::from_millis(25)
        );
    }

    #[test]
    fn oidc_prefetch_timeout_rejects_zero_and_invalid_values() {
        for value in ["0", "not-a-number"] {
            let _guard =
                crate::TestEnvVarGuard::set(RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV, value);

            assert_eq!(
                runtime_gateway_oidc_prefetch_timeout(),
                Duration::from_millis(DEFAULT_RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS)
            );
        }
    }

    #[test]
    fn oidc_http_cache_ttl_uses_positive_env_override() {
        let _guard = crate::TestEnvVarGuard::set(RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV, "25");

        assert_eq!(
            runtime_gateway_oidc_http_cache_ttl(),
            Duration::from_secs(25)
        );
        assert_eq!(
            runtime_gateway_oidc_background_refresh_interval(runtime_gateway_oidc_http_cache_ttl()),
            Duration::from_secs(25)
        );
    }

    #[test]
    fn oidc_http_cache_ttl_allows_zero_and_rejects_invalid_values() {
        let _zero = crate::TestEnvVarGuard::set(RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV, "0");
        assert_eq!(runtime_gateway_oidc_http_cache_ttl(), Duration::ZERO);
        assert_eq!(
            runtime_gateway_oidc_background_refresh_interval(Duration::ZERO),
            Duration::from_millis(MIN_RUNTIME_GATEWAY_OIDC_BACKGROUND_REFRESH_MS)
        );

        drop(_zero);

        let _invalid =
            crate::TestEnvVarGuard::set(RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV, "not-a-number");
        assert_eq!(
            runtime_gateway_oidc_http_cache_ttl(),
            Duration::from_secs(DEFAULT_RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS)
        );
    }

    #[test]
    fn oidc_refresh_failure_backoff_uses_positive_env_override() {
        let _guard =
            crate::TestEnvVarGuard::set(RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_ENV, "75");

        assert_eq!(
            runtime_gateway_oidc_refresh_failure_backoff(),
            Duration::from_millis(75)
        );
    }

    #[test]
    fn oidc_refresh_failure_backoff_rejects_zero_and_invalid_values() {
        for value in ["0", "not-a-number"] {
            let _guard = crate::TestEnvVarGuard::set(
                RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_ENV,
                value,
            );

            assert_eq!(
                runtime_gateway_oidc_refresh_failure_backoff(),
                Duration::from_millis(DEFAULT_RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS)
            );
        }
    }

    #[test]
    fn oidc_last_known_good_window_allows_zero_and_rejects_invalid_values() {
        let _zero = crate::TestEnvVarGuard::set(RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV, "0");
        assert_eq!(
            runtime_gateway_oidc_last_known_good_window(),
            Duration::ZERO
        );

        drop(_zero);

        let _valid = crate::TestEnvVarGuard::set(RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV, "25");
        assert_eq!(
            runtime_gateway_oidc_last_known_good_window(),
            Duration::from_secs(25)
        );

        drop(_valid);

        let _invalid =
            crate::TestEnvVarGuard::set(RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV, "not-a-number");
        assert_eq!(
            runtime_gateway_oidc_last_known_good_window(),
            Duration::from_secs(DEFAULT_RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS)
        );
    }

    #[test]
    fn oidc_cache_control_max_age_is_used_when_present() {
        assert_eq!(
            runtime_gateway_oidc_cache_control_max_age(Some("public, max-age=7")),
            Some(Duration::from_secs(7))
        );
        assert_eq!(
            runtime_gateway_oidc_cache_control_max_age(Some("max-age=\"8\", must-revalidate")),
            Some(Duration::from_secs(8))
        );
        assert_eq!(
            runtime_gateway_oidc_cache_control_max_age(Some("no-store")),
            None
        );
        assert_eq!(runtime_gateway_oidc_cache_control_max_age(None), None);
    }

    #[test]
    fn oidc_cache_control_stale_while_revalidate_is_used_when_present() {
        assert_eq!(
            runtime_gateway_oidc_cache_control_stale_while_revalidate(Some(
                "public, max-age=7, stale-while-revalidate=9"
            )),
            Some(Duration::from_secs(9))
        );
        assert_eq!(
            runtime_gateway_oidc_cache_control_stale_while_revalidate(Some(
                "stale-while-revalidate=\"10\", must-revalidate"
            )),
            Some(Duration::from_secs(10))
        );
        assert_eq!(
            runtime_gateway_oidc_cache_control_stale_while_revalidate(Some("max-age=7")),
            None
        );
        assert_eq!(
            runtime_gateway_oidc_cache_control_stale_while_revalidate(None),
            None
        );
    }

    #[test]
    fn oidc_cache_entry_ttl_prefers_http_max_age_except_zero_override() {
        let entry = RuntimeGatewayOidcHttpCacheEntry {
            fetched_at: Instant::now(),
            payload: "{}".to_string(),
            max_age: Some(Duration::from_secs(7)),
            stale_while_revalidate: None,
        };
        assert_eq!(
            runtime_gateway_oidc_cache_entry_ttl(&entry, Duration::from_secs(300)),
            Duration::from_secs(7)
        );
        assert_eq!(
            runtime_gateway_oidc_cache_entry_ttl(&entry, Duration::ZERO),
            Duration::ZERO
        );

        let without_header = RuntimeGatewayOidcHttpCacheEntry {
            fetched_at: Instant::now(),
            payload: "{}".to_string(),
            max_age: None,
            stale_while_revalidate: None,
        };
        assert_eq!(
            runtime_gateway_oidc_cache_entry_ttl(&without_header, Duration::from_secs(300)),
            Duration::from_secs(300)
        );
    }

    #[test]
    fn oidc_cache_entry_usable_is_bounded_by_lkg_window() {
        let now = Instant::now();
        let fresh = RuntimeGatewayOidcHttpCacheEntry {
            fetched_at: now - Duration::from_secs(9),
            payload: "{}".to_string(),
            max_age: None,
            stale_while_revalidate: None,
        };
        assert!(runtime_gateway_oidc_cache_entry_usable(
            &fresh,
            Duration::from_secs(10),
            Duration::from_secs(5),
            now
        ));

        let lkg = RuntimeGatewayOidcHttpCacheEntry {
            fetched_at: now - Duration::from_secs(12),
            payload: "{}".to_string(),
            max_age: None,
            stale_while_revalidate: None,
        };
        assert!(runtime_gateway_oidc_cache_entry_usable(
            &lkg,
            Duration::from_secs(10),
            Duration::from_secs(5),
            now
        ));

        let expired = RuntimeGatewayOidcHttpCacheEntry {
            fetched_at: now - Duration::from_secs(16),
            payload: "{}".to_string(),
            max_age: None,
            stale_while_revalidate: None,
        };
        assert!(!runtime_gateway_oidc_cache_entry_usable(
            &expired,
            Duration::from_secs(10),
            Duration::from_secs(5),
            now
        ));

        let response_stale_window = RuntimeGatewayOidcHttpCacheEntry {
            fetched_at: now - Duration::from_secs(12),
            payload: "{}".to_string(),
            max_age: Some(Duration::from_secs(10)),
            stale_while_revalidate: Some(Duration::from_secs(1)),
        };
        assert!(!runtime_gateway_oidc_cache_entry_usable(
            &response_stale_window,
            Duration::from_secs(10),
            Duration::from_secs(60),
            now
        ));
    }

    #[test]
    fn sso_role_resolution_never_defaults_missing_or_unknown_to_admin() {
        let scim_admin = RuntimeGatewayScimUser {
            id: "user-1".to_string(),
            user_name: "alice@example.com".to_string(),
            external_id: None,
            display_name: None,
            active: true,
            role: Some("admin".to_string()),
            tenant_id: Some("tenant-a".to_string()),
            team_id: None,
            project_id: None,
            user_id: Some("user-1".to_string()),
            budget_id: None,
            allowed_key_prefixes: Vec::new(),
            created_at_epoch: 1,
            updated_at_epoch: 1,
        };

        assert_eq!(
            runtime_gateway_sso_resolved_role(None, None),
            RuntimeGatewayAdminRole::Viewer
        );
        assert_eq!(
            runtime_gateway_sso_resolved_role(None, Some(&scim_admin)),
            RuntimeGatewayAdminRole::Admin
        );
        assert_eq!(
            runtime_gateway_sso_resolved_role(Some("not-admin"), Some(&scim_admin)),
            RuntimeGatewayAdminRole::Viewer
        );
        assert_eq!(
            runtime_gateway_sso_resolved_role(Some(" admin "), Some(&scim_admin)),
            RuntimeGatewayAdminRole::Viewer
        );
    }

    #[test]
    fn oidc_malformed_role_claim_does_not_fall_back_to_scim_admin() {
        let scim_admin = RuntimeGatewayScimUser {
            id: "user-1".to_string(),
            user_name: "alice@example.com".to_string(),
            external_id: None,
            display_name: None,
            active: true,
            role: Some("admin".to_string()),
            tenant_id: Some("tenant-a".to_string()),
            team_id: None,
            project_id: None,
            user_id: Some("user-1".to_string()),
            budget_id: None,
            allowed_key_prefixes: Vec::new(),
            created_at_epoch: 1,
            updated_at_epoch: 1,
        };
        let malformed_claims =
            BTreeMap::from([("prodex_role".to_string(), serde_json::json!(" admin "))]);

        assert_eq!(
            runtime_gateway_sso_resolved_role(
                runtime_gateway_oidc_role_claim_string(&malformed_claims, "prodex_role").as_deref(),
                Some(&scim_admin),
            ),
            RuntimeGatewayAdminRole::Viewer
        );

        assert_eq!(
            runtime_gateway_sso_resolved_role(
                runtime_gateway_oidc_role_claim_string(&BTreeMap::new(), "prodex_role").as_deref(),
                Some(&scim_admin),
            ),
            RuntimeGatewayAdminRole::Admin
        );
    }

    #[test]
    fn claimed_tenant_mismatch_does_not_reuse_scim_admin_role() {
        let user = RuntimeGatewayScimUser {
            id: "user-1".to_string(),
            user_name: "alice@example.com".to_string(),
            external_id: None,
            display_name: None,
            active: true,
            role: Some("admin".to_string()),
            tenant_id: Some("tenant-a".to_string()),
            team_id: None,
            project_id: None,
            user_id: Some("user-1".to_string()),
            budget_id: None,
            allowed_key_prefixes: vec!["tenant-a-".to_string()],
            created_at_epoch: 1,
            updated_at_epoch: 1,
        };

        let matched =
            runtime_gateway_scim_user_for_claimed_tenant(Some(user.clone()), Some("tenant-a"));
        assert_eq!(
            runtime_gateway_sso_resolved_role(None, matched.as_ref()),
            RuntimeGatewayAdminRole::Admin
        );

        let mismatched = runtime_gateway_scim_user_for_claimed_tenant(Some(user), Some("tenant-b"));
        assert!(mismatched.is_none());
        assert_eq!(
            runtime_gateway_sso_resolved_role(None, mismatched.as_ref()),
            RuntimeGatewayAdminRole::Viewer
        );
    }

    #[test]
    fn missing_tenant_claim_may_still_use_scim_tenant_fallback() {
        let user = RuntimeGatewayScimUser {
            id: "user-1".to_string(),
            user_name: "alice@example.com".to_string(),
            external_id: None,
            display_name: None,
            active: true,
            role: Some("viewer".to_string()),
            tenant_id: Some("tenant-a".to_string()),
            team_id: None,
            project_id: None,
            user_id: Some("user-1".to_string()),
            budget_id: None,
            allowed_key_prefixes: Vec::new(),
            created_at_epoch: 1,
            updated_at_epoch: 1,
        };

        assert!(
            runtime_gateway_scim_user_for_claimed_tenant(Some(user), None)
                .is_some_and(|user| user.tenant_id.as_deref() == Some("tenant-a"))
        );
    }

    #[test]
    fn sso_proxy_token_matches_exactly_without_trimming() {
        let token_hash =
            runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("sso-proxy-token");

        assert!(runtime_gateway_sso_proxy_token_matches(
            &token_hash,
            "sso-proxy-token"
        ));
        assert!(!runtime_gateway_sso_proxy_token_matches(
            &token_hash,
            " sso-proxy-token "
        ));
    }

    #[test]
    fn sso_user_names_match_exactly_without_trimming() {
        assert_eq!(runtime_gateway_sso_user_name(None), None);
        assert_eq!(
            runtime_gateway_sso_user_name(Some("alice@example.com")),
            Some("alice@example.com")
        );
        assert_eq!(
            runtime_gateway_sso_user_name(Some(" alice@example.com ")),
            None
        );
        assert_eq!(runtime_gateway_sso_user_name(Some("")), None);
    }

    #[test]
    fn oidc_principal_claims_match_exactly_without_trimming() {
        let claims = BTreeMap::from([
            (
                "email".to_string(),
                serde_json::json!(" alice@example.com "),
            ),
            (
                "preferred_username".to_string(),
                serde_json::json!("bob@example.com"),
            ),
        ]);

        assert_eq!(runtime_gateway_oidc_claim_string(&claims, "email"), None);
        assert_eq!(
            runtime_gateway_oidc_claim_string(&claims, "preferred_username"),
            Some("bob@example.com".to_string())
        );
    }

    #[test]
    fn oidc_configured_user_claim_must_be_exact_when_present() {
        let config = RuntimeGatewayOidcConfig {
            issuer: "https://idp.example".to_string(),
            audience: "prodex-gateway".to_string(),
            jwks_url: None,
            user_claim: "prodex_user".to_string(),
            role_claim: "prodex_role".to_string(),
            tenant_claim: "prodex_tenant".to_string(),
            key_prefixes_claim: "prodex_key_prefixes".to_string(),
        };
        let malformed_configured_claim = BTreeMap::from([
            (
                "prodex_user".to_string(),
                serde_json::json!(" alice@example.com "),
            ),
            (
                "email".to_string(),
                serde_json::json!("fallback@example.com"),
            ),
        ]);

        assert_eq!(
            runtime_gateway_oidc_admin_name(&malformed_configured_claim, &config),
            None
        );

        let missing_configured_claim = BTreeMap::from([(
            "email".to_string(),
            serde_json::json!("fallback@example.com"),
        )]);
        assert_eq!(
            runtime_gateway_oidc_admin_name(&missing_configured_claim, &config),
            Some("fallback@example.com".to_string())
        );

        let malformed_first_fallback = BTreeMap::from([
            (
                "email".to_string(),
                serde_json::json!(" fallback@example.com "),
            ),
            (
                "preferred_username".to_string(),
                serde_json::json!("preferred@example.com"),
            ),
        ]);
        assert_eq!(
            runtime_gateway_oidc_admin_name(&malformed_first_fallback, &config),
            None
        );
    }

    #[test]
    fn sso_key_prefixes_reject_empty_or_whitespace_scope_parts() {
        assert_eq!(
            runtime_gateway_parse_sso_prefixes("team-a-,team-b-"),
            Some(vec!["team-a-".to_string(), "team-b-".to_string()])
        );
        assert_eq!(runtime_gateway_parse_sso_prefixes(""), None);
        assert_eq!(runtime_gateway_parse_sso_prefixes("team-a-, team-b-"), None);
        assert_eq!(runtime_gateway_parse_sso_prefixes("team-a-,"), None);
    }

    #[test]
    fn oidc_key_prefix_claim_rejects_whitespace_scope_parts() {
        let claims = BTreeMap::from([(
            "prodex_key_prefixes".to_string(),
            serde_json::json!(["team-a-", " team-b-"]),
        )]);

        assert!(runtime_gateway_oidc_claim_string_vec(&claims, "prodex_key_prefixes").is_err());
    }
}
