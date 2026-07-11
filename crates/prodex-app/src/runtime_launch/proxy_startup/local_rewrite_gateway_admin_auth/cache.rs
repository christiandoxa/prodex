use super::super::local_rewrite::{
    RuntimeGatewayOidcHttpCacheEntry, RuntimeLocalRewriteProxyShared,
};
use super::super::*;
use super::endpoint_policy::{RuntimeGatewayOidcEndpoint, RuntimeGatewayOidcEndpointPolicy};
use super::transport::runtime_gateway_oidc_send;
use crate::read_blocking_response_body_with_limit;
use jsonwebtoken::jwk::JwkSet;
use prodex_observability::{
    JwksRefreshOutcome, OidcRefreshOperation, OidcRefreshResult, plan_jwks_cache_age_metric,
    plan_jwks_refresh_outcome_metric, plan_oidc_refresh_metric,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub(super) const MIN_RUNTIME_GATEWAY_OIDC_BACKGROUND_REFRESH_MS: u64 = 50;
pub(super) const MAX_RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS: u64 = 86_400;
pub(super) const MAX_RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS: u64 = 604_800;
pub(super) const MAX_RUNTIME_GATEWAY_OIDC_BODY_BYTES: usize = 1_048_576;
pub(super) const MAX_RUNTIME_GATEWAY_OIDC_CACHE_ENTRIES: usize = 4;
pub(super) const MAX_RUNTIME_GATEWAY_OIDC_JWKS_KEYS: usize = 128;

pub(in super::super) struct RuntimeGatewayOidcJwksSnapshot {
    pub(super) jwks: JwkSet,
    pub(super) fetched_at: Instant,
    pub(super) fresh_for: Duration,
    pub(super) stale_for: Duration,
}

impl RuntimeGatewayOidcJwksSnapshot {
    pub(super) fn usable_at(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.fetched_at)
            < self.fresh_for.saturating_add(self.stale_for)
    }

    pub(super) fn domain_snapshot_at(
        &self,
        now: Instant,
        now_unix_ms: u64,
    ) -> prodex_domain::JwksCacheSnapshot {
        let age_ms = duration_millis(now.saturating_duration_since(self.fetched_at));
        let fetched_at_unix_ms = now_unix_ms.saturating_sub(age_ms);
        let expires_at_unix_ms = fetched_at_unix_ms.saturating_add(duration_millis(self.fresh_for));
        prodex_domain::JwksCacheSnapshot {
            fetched_at_unix_ms,
            expires_at_unix_ms,
            stale_until_unix_ms: expires_at_unix_ms.saturating_add(duration_millis(self.stale_for)),
            key_count: self.jwks.keys.len().min(usize::from(u16::MAX)) as u16,
            last_refresh_error_at_unix_ms: None,
            retry_after_unix_ms: None,
        }
    }
}

fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

pub(in super::super) fn runtime_gateway_prefetch_oidc_cache(
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<()> {
    let Some(config) = shared.gateway_sso.oidc.as_ref() else {
        return Ok(());
    };
    let jwks_url = runtime_gateway_oidc_fetch_jwks_url(config, shared)?;
    let jwks_value = runtime_gateway_oidc_fetch_json(shared, &jwks_url, "JWKS")?;
    runtime_gateway_oidc_validate_jwks_key_count(&jwks_value)?;
    let jwks: JwkSet =
        serde_json::from_value(jwks_value).context("failed to parse gateway OIDC JWKS")?;
    let entry = shared
        .gateway_oidc_http_cache
        .lock()
        .map_err(|_| anyhow::anyhow!("gateway OIDC cache is unavailable"))?
        .get(&jwks_url.cache_key())
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("gateway OIDC JWKS cache entry is unavailable"))?;
    let snapshot = RuntimeGatewayOidcJwksSnapshot {
        jwks,
        fetched_at: entry.fetched_at,
        fresh_for: runtime_gateway_oidc_cache_entry_ttl(
            &entry,
            shared.runtime_shared.runtime_config.oidc.http_cache_ttl,
        ),
        stale_for: runtime_gateway_oidc_cache_entry_stale_window(
            &entry,
            shared
                .runtime_shared
                .runtime_config
                .oidc
                .last_known_good_window,
        ),
    };
    shared
        .gateway_oidc_jwks_snapshot
        .store(Some(Arc::new(snapshot)));
    Ok(())
}

pub(super) fn runtime_gateway_oidc_validate_jwks_key_count(
    value: &serde_json::Value,
) -> Result<()> {
    let key_count = value
        .get("keys")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .ok_or_else(|| anyhow::anyhow!("gateway OIDC JWKS keys are invalid"))?;
    if key_count == 0 || key_count > MAX_RUNTIME_GATEWAY_OIDC_JWKS_KEYS {
        bail!("gateway OIDC JWKS key count is invalid");
    }
    Ok(())
}

pub(in super::super) fn runtime_gateway_run_oidc_background_refresh_loop(
    shared: RuntimeLocalRewriteProxyShared,
    shutdown: Arc<AtomicBool>,
) {
    let ttl = shared.runtime_shared.runtime_config.oidc.http_cache_ttl;
    let mut last_refresh_failed = false;
    loop {
        if runtime_gateway_oidc_cache_refresh_due(&shared, ttl) {
            match super::runtime_gateway_prefetch_oidc_cache(&shared) {
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
        let poll_interval = if last_refresh_failed {
            runtime_gateway_log_oidc_refresh_metric(
                &shared,
                OidcRefreshOperation::FetchJwks,
                OidcRefreshResult::Backoff,
            );
            shared
                .runtime_shared
                .runtime_config
                .oidc
                .refresh_failure_backoff
        } else {
            runtime_gateway_oidc_background_refresh_sleep(&shared, ttl)
        };
        if runtime_gateway_oidc_wait_for_shutdown(&shutdown, poll_interval) {
            break;
        }
    }
}

pub(super) fn runtime_gateway_oidc_fetch_json(
    shared: &RuntimeLocalRewriteProxyShared,
    endpoint: &RuntimeGatewayOidcEndpoint,
    description: &str,
) -> Result<serde_json::Value> {
    let now = Instant::now();
    let fetched_payload = runtime_gateway_oidc_send(
        endpoint,
        description,
        &shared.runtime_shared.runtime_config.oidc,
    );
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
                MAX_RUNTIME_GATEWAY_OIDC_BODY_BYTES,
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
            runtime_gateway_log_oidc_refresh_metric(shared, operation, OidcRefreshResult::Success);
            runtime_gateway_log_jwks_refresh_metric(
                shared,
                description,
                JwksRefreshOutcome::Success,
            );
            if let Ok(mut cache) = shared.gateway_oidc_http_cache.lock() {
                runtime_gateway_oidc_insert_cache_entry(
                    &mut cache,
                    endpoint.cache_key(),
                    RuntimeGatewayOidcHttpCacheEntry {
                        fetched_at: now,
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

pub(super) fn runtime_gateway_oidc_insert_cache_entry(
    cache: &mut BTreeMap<String, RuntimeGatewayOidcHttpCacheEntry>,
    cache_key: String,
    entry: RuntimeGatewayOidcHttpCacheEntry,
) {
    if !cache.contains_key(&cache_key)
        && cache.len() >= MAX_RUNTIME_GATEWAY_OIDC_CACHE_ENTRIES
        && let Some(oldest_key) = cache
            .iter()
            .min_by_key(|(_, entry)| entry.fetched_at)
            .map(|(key, _)| key.clone())
    {
        cache.remove(&oldest_key);
    }
    cache.insert(cache_key, entry);
}

pub(super) fn runtime_gateway_oidc_refresh_operation(description: &str) -> OidcRefreshOperation {
    if description == "JWKS" {
        OidcRefreshOperation::FetchJwks
    } else {
        OidcRefreshOperation::DiscoverIssuer
    }
}

pub(super) fn runtime_gateway_log_oidc_refresh_metric(
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

pub(super) fn runtime_gateway_log_jwks_refresh_metric(
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

pub(super) fn runtime_gateway_log_jwks_snapshot_age_metric(
    shared: &RuntimeLocalRewriteProxyShared,
    snapshot: &RuntimeGatewayOidcJwksSnapshot,
) {
    let age_ms = Instant::now()
        .saturating_duration_since(snapshot.fetched_at)
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    let ttl_ms = snapshot.fresh_for.as_millis().min(u128::from(u64::MAX)) as u64;
    let stale_ms = snapshot.stale_for.as_millis().min(u128::from(u64::MAX)) as u64;
    let snapshot = prodex_domain::JwksCacheSnapshot {
        fetched_at_unix_ms: 0,
        expires_at_unix_ms: ttl_ms,
        stale_until_unix_ms: ttl_ms.saturating_add(stale_ms),
        key_count: snapshot.jwks.keys.len().min(usize::from(u16::MAX)) as u16,
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

pub(super) fn runtime_gateway_oidc_cache_control_max_age(
    cache_control: Option<&str>,
) -> Option<Duration> {
    runtime_gateway_oidc_cache_control_duration(cache_control, "max-age")
}

pub(super) fn runtime_gateway_oidc_cache_control_stale_while_revalidate(
    cache_control: Option<&str>,
) -> Option<Duration> {
    runtime_gateway_oidc_cache_control_duration(cache_control, "stale-while-revalidate")
}

pub(super) fn runtime_gateway_oidc_cache_control_duration(
    cache_control: Option<&str>,
    directive_name: &str,
) -> Option<Duration> {
    for directive in cache_control?.split(',') {
        let Some((name, value)) = directive.trim().split_once('=') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case(directive_name) {
            let limit = if directive_name.eq_ignore_ascii_case("max-age") {
                MAX_RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS
            } else {
                MAX_RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS
            };
            return value
                .trim()
                .trim_matches('"')
                .parse::<u64>()
                .ok()
                .map(|seconds| seconds.min(limit))
                .map(Duration::from_secs);
        }
    }
    None
}

#[cfg(test)]
pub(super) fn runtime_gateway_oidc_prefetch_timeout() -> Duration {
    RuntimeConfig::compatibility_current().oidc.prefetch_timeout
}

#[cfg(test)]
pub(super) fn runtime_gateway_oidc_http_cache_ttl() -> Duration {
    RuntimeConfig::compatibility_current().oidc.http_cache_ttl
}

#[cfg(test)]
pub(super) fn runtime_gateway_oidc_refresh_failure_backoff() -> Duration {
    RuntimeConfig::compatibility_current()
        .oidc
        .refresh_failure_backoff
}

#[cfg(test)]
pub(super) fn runtime_gateway_oidc_last_known_good_window() -> Duration {
    RuntimeConfig::compatibility_current()
        .oidc
        .last_known_good_window
}

pub(super) fn runtime_gateway_oidc_background_refresh_interval(ttl: Duration) -> Duration {
    if ttl.is_zero() {
        Duration::from_millis(MIN_RUNTIME_GATEWAY_OIDC_BACKGROUND_REFRESH_MS)
    } else {
        ttl
    }
}

pub(super) fn runtime_gateway_oidc_background_refresh_sleep(
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

pub(super) fn runtime_gateway_oidc_cache_refresh_due(
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

pub(super) fn runtime_gateway_oidc_cache_entry_ttl(
    entry: &RuntimeGatewayOidcHttpCacheEntry,
    fallback: Duration,
) -> Duration {
    if fallback.is_zero() {
        Duration::ZERO
    } else {
        entry.max_age.unwrap_or(fallback)
    }
}

pub(super) fn runtime_gateway_oidc_cache_entry_stale_window(
    entry: &RuntimeGatewayOidcHttpCacheEntry,
    fallback: Duration,
) -> Duration {
    entry.stale_while_revalidate.unwrap_or(fallback)
}

#[cfg(test)]
pub(super) fn runtime_gateway_oidc_cache_entry_usable(
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

pub(super) fn runtime_gateway_oidc_wait_for_shutdown(
    shutdown: &AtomicBool,
    duration: Duration,
) -> bool {
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

pub(super) fn runtime_gateway_oidc_fetch_jwks_url(
    config: &RuntimeGatewayOidcConfig,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<RuntimeGatewayOidcEndpoint> {
    let policy = RuntimeGatewayOidcEndpointPolicy::from_config(config)?;
    if let Some(jwks_endpoint) = policy.configured_jwks() {
        return Ok(jwks_endpoint);
    }
    let discovery_endpoint = policy.discovery_endpoint();
    let discovery =
        runtime_gateway_oidc_fetch_json(shared, &discovery_endpoint, "discovery document")?;
    policy.validate_discovery_document(&discovery)
}
