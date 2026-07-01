use super::external_provider::fetch_agy_quota_info;
use super::{
    AuthSummary, ExternalQuotaDetail, ExternalQuotaInfo, ProviderQuotaSnapshot,
    QuotaProviderFilter, QuotaReport, build_upstream_blocking_http_client, format_response_body,
    quota_error_message,
};
use anyhow::{Context, Result, bail};
use chrono::Local;
use prodex_state::ProfileProvider;
use std::env;

pub(super) fn collect_virtual_quota_reports(
    base_url: Option<&str>,
    provider_filter: QuotaProviderFilter,
) -> Vec<QuotaReport> {
    let mut reports = Vec::new();
    if provider_filter == QuotaProviderFilter::DeepSeek {
        reports.extend(collect_deepseek_quota_reports(base_url));
    }
    if provider_filter == QuotaProviderFilter::Local {
        reports.push(collect_local_quota_report(base_url));
    }
    if provider_filter == QuotaProviderFilter::Agy {
        reports.extend(collect_agy_quota_reports());
    }
    reports
}

fn collect_deepseek_quota_reports(base_url: Option<&str>) -> Vec<QuotaReport> {
    let keys = deepseek_api_keys_from_env();
    let Some(keys) = keys else {
        return vec![virtual_quota_report(
            "deepseek",
            "deepseek-key",
            Err("DeepSeek quota requires DEEPSEEK_API_KEY or DEEPSEEK_API_KEYS".to_string()),
        )];
    };
    keys.into_iter()
        .enumerate()
        .map(|(index, key)| {
            let name = if index == 0 {
                "deepseek".to_string()
            } else {
                format!("deepseek-{}", index + 1)
            };
            let result =
                fetch_deepseek_quota_info(&key, base_url).map_err(|err| quota_error_message(&err));
            virtual_quota_report(
                &name,
                "deepseek-key",
                result.map(ProviderQuotaSnapshot::External),
            )
        })
        .collect()
}

fn collect_local_quota_report(base_url: Option<&str>) -> QuotaReport {
    let result = base_url
        .map(fetch_local_openai_compatible_quota_info)
        .unwrap_or_else(|| {
            Err(anyhow::anyhow!(
                "local quota view requires --base-url pointing at the OpenAI-compatible server"
            ))
        })
        .map(ProviderQuotaSnapshot::External)
        .map_err(|err| quota_error_message(&err));
    virtual_quota_report("local", "local", result)
}

fn collect_agy_quota_reports() -> Vec<QuotaReport> {
    match fetch_agy_quota_info(None) {
        Ok(info) => vec![virtual_quota_report_with_provider(
            &info
                .account
                .as_deref()
                .map(|a| format!("agy:{a}"))
                .unwrap_or_else(|| "agy".to_string()),
            "agy",
            Ok(ProviderQuotaSnapshot::External(info)),
            ProfileProvider::Agy { account: None },
        )],
        Err(err) => vec![virtual_quota_report_with_provider(
            "agy",
            "agy",
            Err(quota_error_message(&err)),
            ProfileProvider::Agy { account: None },
        )],
    }
}

fn virtual_quota_report(
    name: &str,
    auth_label: &str,
    result: std::result::Result<ProviderQuotaSnapshot, String>,
) -> QuotaReport {
    virtual_quota_report_with_provider(name, auth_label, result, ProfileProvider::Openai)
}

fn virtual_quota_report_with_provider(
    name: &str,
    auth_label: &str,
    result: std::result::Result<ProviderQuotaSnapshot, String>,
    provider: ProfileProvider,
) -> QuotaReport {
    QuotaReport {
        name: name.to_string(),
        active: false,
        auth: AuthSummary {
            label: auth_label.to_string(),
            quota_compatible: false,
        },
        provider,
        workspace_id: None,
        workspace_name: None,
        result,
        fetched_at: Local::now().timestamp(),
    }
}

fn deepseek_api_keys_from_env() -> Option<Vec<String>> {
    env::var("DEEPSEEK_API_KEYS")
        .ok()
        .and_then(|value| provider_api_keys_from_list(&value))
        .or_else(|| {
            env::var("DEEPSEEK_API_KEY")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .map(|value| vec![value])
        })
}

fn provider_api_keys_from_list(value: &str) -> Option<Vec<String>> {
    let keys = value
        .split([',', ';', '\n'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    (!keys.is_empty()).then_some(keys)
}

fn fetch_deepseek_quota_info(api_key: &str, base_url: Option<&str>) -> Result<ExternalQuotaInfo> {
    let value = fetch_deepseek_balance_json(api_key, base_url)?;
    let available = value
        .get("is_available")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let balances = value
        .get("balance_infos")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut details = Vec::new();
    let mut parts = Vec::new();
    for balance in balances {
        let currency = balance
            .get("currency")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("balance");
        let total = balance
            .get("total_balance")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("-");
        parts.push(format!("{currency} {total}"));
        let granted = balance
            .get("granted_balance")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("-");
        let topped_up = balance
            .get("topped_up_balance")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("-");
        details.push(ExternalQuotaDetail {
            label: format!("{currency} balance"),
            value: format!("total {total}; granted {granted}; topped up {topped_up}"),
        });
    }
    Ok(ExternalQuotaInfo {
        provider: "DeepSeek".to_string(),
        account: None,
        plan: Some("api-key".to_string()),
        status: if available { "Ready" } else { "Blocked" }.to_string(),
        main: if parts.is_empty() {
            "balance unavailable".to_string()
        } else {
            parts.join(" | ")
        },
        reset: None,
        available: Some(available),
        details,
    })
}

fn fetch_deepseek_balance_json(api_key: &str, base_url: Option<&str>) -> Result<serde_json::Value> {
    let client = build_upstream_blocking_http_client("DeepSeek quota HTTP", false)?;
    let url = format!("{}/user/balance", deepseek_base_url(base_url));
    let response = client
        .get(&url)
        .header("accept", "application/json")
        .bearer_auth(api_key)
        .send()
        .context("failed to fetch DeepSeek balance")?;
    let status = response.status();
    let body = response
        .bytes()
        .context("failed to read DeepSeek balance response")?;
    if !status.is_success() {
        bail!(
            "DeepSeek balance request failed (HTTP {}): {}",
            status.as_u16(),
            format_response_body(&body)
        );
    }
    serde_json::from_slice(&body).context("failed to parse DeepSeek balance response")
}

fn deepseek_base_url(explicit: Option<&str>) -> String {
    let base = explicit
        .map(str::to_string)
        .or_else(|| env::var("PRODEX_DEEPSEEK_BASE_URL").ok())
        .or_else(|| env::var("DEEPSEEK_BASE_URL").ok())
        .unwrap_or_else(|| "https://api.deepseek.com".to_string());
    base.trim()
        .trim_end_matches('/')
        .trim_end_matches("/v1")
        .to_string()
}

fn fetch_local_openai_compatible_quota_info(base_url: &str) -> Result<ExternalQuotaInfo> {
    let client = build_upstream_blocking_http_client("local OpenAI-compatible quota HTTP", false)?;
    let url = openai_compatible_models_url(base_url)?;
    let mut request = client.get(&url).header("accept", "application/json");
    if let Some(api_key) = local_openai_compatible_api_key_from_env() {
        request = request.bearer_auth(api_key);
    }
    let response = request
        .send()
        .context("failed to fetch local OpenAI-compatible models")?;
    let status = response.status();
    let body = response
        .bytes()
        .context("failed to read local OpenAI-compatible response")?;
    if !status.is_success() {
        bail!(
            "local OpenAI-compatible models request failed (HTTP {}): {}",
            status.as_u16(),
            format_response_body(&body)
        );
    }
    let value: serde_json::Value =
        serde_json::from_slice(&body).context("failed to parse local models response")?;
    let model_count = value
        .get("data")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len);
    Ok(ExternalQuotaInfo {
        provider: "Local OpenAI-compatible".to_string(),
        account: Some(base_url.trim().to_string()),
        plan: Some("local".to_string()),
        status: "Reachable".to_string(),
        main: model_count.map_or_else(
            || "models endpoint reachable".to_string(),
            |count| format!("{count} model(s)"),
        ),
        reset: None,
        available: Some(true),
        details: vec![ExternalQuotaDetail {
            label: "Models URL".to_string(),
            value: url,
        }],
    })
}

fn openai_compatible_models_url(base_url: &str) -> Result<String> {
    let mut url = reqwest::Url::parse(base_url.trim())
        .with_context(|| format!("invalid local base URL '{}'", base_url.trim()))?;
    let path = url.path().trim_end_matches('/');
    if path.is_empty() {
        url.set_path("/v1/models");
    } else if !path.ends_with("/models") {
        url.set_path(&format!("{path}/models"));
    }
    Ok(url.to_string())
}

fn local_openai_compatible_api_key_from_env() -> Option<String> {
    ["PRODEX_LOCAL_API_KEY", "OPENAI_API_KEY"]
        .into_iter()
        .find_map(|key| {
            env::var(key)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}
