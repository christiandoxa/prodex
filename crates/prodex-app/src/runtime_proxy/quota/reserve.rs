use super::{RuntimeRotationProxyShared, UsageResponse, runtime_profile_usage_cache_is_fresh};
use anyhow::{Context, Result};
use chrono::Local;

pub(crate) fn rewrite_runtime_luna_reserve_model_if_authorized(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    authenticated_account_id: Option<&str>,
    body: &[u8],
    allow_rewrite: bool,
) -> Result<Option<Vec<u8>>> {
    if !allow_rewrite {
        return Ok(None);
    }
    let value: serde_json::Value = match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    let requested_model = value.get("model").and_then(serde_json::Value::as_str);
    if requested_model != Some(prodex_quota::OPENAI_LUNA_MODEL) {
        return Ok(None);
    }

    let (usage, cached_account_id) = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        let now = Local::now().timestamp();
        let cached_account_id = runtime
            .profile_usage_auth
            .get(profile_name)
            .and_then(|entry| entry.auth.account_id.clone());
        let usage = runtime
            .profile_probe_cache
            .get(profile_name)
            .filter(|entry| runtime_profile_usage_cache_is_fresh(entry, now))
            .and_then(|entry| entry.result.as_ref().ok())
            .cloned();
        (usage, cached_account_id)
    };
    let Some(usage) = usage else { return Ok(None) };
    rewrite_runtime_luna_reserve_model_for_usage(
        &usage,
        authenticated_account_id.or(cached_account_id.as_deref()),
        body,
    )
}

fn rewrite_runtime_luna_reserve_model_for_usage(
    usage: &UsageResponse,
    authenticated_account_id: Option<&str>,
    body: &[u8],
) -> Result<Option<Vec<u8>>> {
    let mut value: serde_json::Value = match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    let requested_model = value.get("model").and_then(serde_json::Value::as_str);
    if prodex_quota::openai_effective_model_for_usage(
        usage,
        requested_model,
        authenticated_account_id,
    ) != Some(prodex_quota::OPENAI_LUNA_RESERVE_MODEL)
    {
        return Ok(None);
    }
    value["model"] = serde_json::Value::String(prodex_quota::OPENAI_LUNA_RESERVE_MODEL.to_string());
    serde_json::to_vec(&value)
        .map(Some)
        .context("failed to encode Luna Reserve upstream request")
}

#[cfg(test)]
#[path = "../../../tests/src/runtime_proxy/quota/reserve.rs"]
mod tests;
