use super::{
    ExternalQuotaDetail, ExternalQuotaInfo, ProfileProvider, SUPER_ANTHROPIC_PROVIDER_ID,
    SUPER_DEEPSEEK_PROVIDER_ID, SUPER_LOCAL_PROVIDER_ID, build_upstream_blocking_http_client,
    first_line_of_error, format_response_body, quota_error_message,
    refresh_claude_oauth_secret_if_needed,
};
use crate::{KIRO_MODEL_CATALOG_FILE, parse_kiro_model_catalog_text, read_kiro_auth_secret};
use crate::{RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES, read_blocking_response_body_with_limit};
use anyhow::{Context, Result, bail};
use codex_config::codex_non_openai_model_provider;
use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;

pub(super) fn fetch_agy_quota_info(account: Option<&str>) -> Result<ExternalQuotaInfo> {
    let mut command = Command::new("agy");
    command.args(["auth", "quota", "--format=json", "--detail"]);
    if account.is_none() {
        command.arg("--all-accounts");
    }

    let output = command
        .output()
        .context("failed to execute 'agy' CLI. Ensure it is installed and in your PATH.")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("agy quota command failed: {}", stderr.trim());
    }

    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("failed to parse agy quota JSON output")?;

    parse_agy_quota_json(value, account)
}

pub(super) fn fetch_kiro_quota_info(codex_home: &Path) -> Result<ExternalQuotaInfo> {
    let secret = read_kiro_auth_secret(codex_home)?;
    let catalog_path = codex_home.join(KIRO_MODEL_CATALOG_FILE);
    let model_count = match fs::read_to_string(&catalog_path) {
        Ok(text) => Some(
            parse_kiro_model_catalog_text(&text)
                .with_context(|| format!("failed to parse {}", catalog_path.display()))?
                .len(),
        ),
        Err(err) if err.kind() == ErrorKind::NotFound => None,
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read {}", catalog_path.display()));
        }
    };
    let account = secret
        .email
        .clone()
        .or_else(|| secret.profile_name.clone())
        .or_else(|| secret.profile_arn.clone());
    let mut details = vec![ExternalQuotaDetail {
        label: "Auth method".to_string(),
        value: secret.auth_kind.clone(),
    }];
    for (label, value) in [("Profile", secret.profile_name), ("Region", secret.region)] {
        if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
            details.push(ExternalQuotaDetail {
                label: label.to_string(),
                value,
            });
        }
    }
    details.push(ExternalQuotaDetail {
        label: "Models".to_string(),
        value: model_count
            .map(|count| count.to_string())
            .unwrap_or_else(|| "catalog unavailable".to_string()),
    });

    Ok(ExternalQuotaInfo {
        provider: "Kiro CLI".to_string(),
        account,
        plan: Some(secret.auth_kind),
        status: "Ready (imported)".to_string(),
        main: model_count.map_or_else(
            || "credential snapshot available".to_string(),
            |count| format!("{count} imported models"),
        ),
        reset: None,
        available: Some(true),
        details,
    })
}

fn parse_agy_quota_json(
    value: serde_json::Value,
    preferred_account: Option<&str>,
) -> Result<ExternalQuotaInfo> {
    let accounts = agy_accounts(value);
    let data = agy_selected_account(&accounts, preferred_account)?;
    let (account, plan, status) = agy_identity(data);
    let main = agy_main(data);
    let details = agy_details(data);

    Ok(ExternalQuotaInfo {
        provider: "Anti-Gravity".to_string(),
        account,
        plan,
        status,
        main,
        reset: None,
        available: Some(true),
        details,
    })
}

fn agy_accounts(value: serde_json::Value) -> Vec<serde_json::Value> {
    value.as_array().cloned().unwrap_or_else(|| vec![value])
}

fn agy_selected_account<'a>(
    accounts: &'a [serde_json::Value],
    preferred_account: Option<&str>,
) -> Result<&'a serde_json::Value> {
    preferred_account
        .and_then(|preferred| {
            accounts.iter().find(|account| {
                account.get("account").and_then(|value| value.as_str()) == Some(preferred)
                    || account.get("email").and_then(|value| value.as_str()) == Some(preferred)
            })
        })
        .or_else(|| accounts.first())
        .context("no account found in agy output")
}

fn agy_identity(data: &serde_json::Value) -> (Option<String>, Option<String>, String) {
    let account = data
        .get("account")
        .or_else(|| data.get("email"))
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let plan = data
        .get("plan")
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let status = data
        .get("status")
        .and_then(|value| value.as_str())
        .unwrap_or("Ready")
        .to_string();
    (account, plan, status)
}

fn agy_main(data: &serde_json::Value) -> String {
    data.get("credits")
        .and_then(|value| value.as_f64())
        .map(|credits| format!("{credits:.2} credits available"))
        .or_else(|| {
            data.get("usage")
                .and_then(|value| value.as_f64())
                .map(|usage| format!("{usage:.2} usage"))
        })
        .unwrap_or_else(|| "quota information available".to_string())
}

fn agy_details(data: &serde_json::Value) -> Vec<ExternalQuotaDetail> {
    data.as_object()
        .into_iter()
        .flat_map(|object| object.iter())
        .filter_map(|(key, value)| agy_detail_entries(key, value))
        .flatten()
        .collect()
}

fn agy_detail_entries(key: &str, value: &serde_json::Value) -> Option<Vec<ExternalQuotaDetail>> {
    if matches!(key, "details" | "models" | "quotas") {
        return Some(
            value
                .as_object()
                .into_iter()
                .flat_map(|object| {
                    object
                        .iter()
                        .map(|(inner_key, inner_value)| ExternalQuotaDetail {
                            label: format!("{key}:{inner_key}"),
                            value: quota_json_scalar(inner_value),
                        })
                })
                .chain(value.as_array().into_iter().flat_map(|array| {
                    array
                        .iter()
                        .enumerate()
                        .map(|(index, inner_value)| ExternalQuotaDetail {
                            label: format!("{key}[{index}]"),
                            value: quota_json_scalar(inner_value),
                        })
                }))
                .collect(),
        );
    }
    if matches!(
        key,
        "account" | "email" | "plan" | "status" | "credits" | "usage"
    ) {
        return None;
    }
    Some(vec![ExternalQuotaDetail {
        label: key.to_string(),
        value: quota_json_scalar(value),
    }])
}

pub(super) fn custom_model_provider_quota_info(
    provider: &ProfileProvider,
    codex_home: &Path,
) -> anyhow::Result<Option<ExternalQuotaInfo>> {
    if !matches!(provider, ProfileProvider::Openai) {
        return Ok(None);
    }
    let Some(model_provider) = codex_non_openai_model_provider(codex_home, None)? else {
        return Ok(None);
    };
    let provider_name = custom_model_provider_display_name(&model_provider.provider_id);
    Ok(Some(ExternalQuotaInfo {
        provider: provider_name,
        account: Some(model_provider.provider_id.clone()),
        plan: Some(model_provider.source.display_name().to_string()),
        status: "Configured".to_string(),
        main: "quota handled by provider/Codex".to_string(),
        reset: None,
        available: None,
        details: vec![
            ExternalQuotaDetail {
                label: "Model provider".to_string(),
                value: model_provider.provider_id,
            },
            ExternalQuotaDetail {
                label: "Source".to_string(),
                value: model_provider.source.display_name().to_string(),
            },
        ],
    }))
}

fn custom_model_provider_display_name(provider_id: &str) -> String {
    if provider_id.eq_ignore_ascii_case(SUPER_LOCAL_PROVIDER_ID) {
        "Local OpenAI-compatible".to_string()
    } else if provider_id.eq_ignore_ascii_case(SUPER_DEEPSEEK_PROVIDER_ID) {
        "DeepSeek".to_string()
    } else if provider_id.eq_ignore_ascii_case(SUPER_ANTHROPIC_PROVIDER_ID) {
        "Anthropic Claude".to_string()
    } else if provider_id.eq_ignore_ascii_case("amazon-bedrock")
        || provider_id.eq_ignore_ascii_case("bedrock")
    {
        "Amazon Bedrock".to_string()
    } else {
        format!("Custom provider ({provider_id})")
    }
}

pub(super) fn fetch_anthropic_quota_info(
    codex_home: &Path,
    provider_account: Option<&str>,
    provider_auth_method: Option<&str>,
) -> Result<ExternalQuotaInfo> {
    let secret = refresh_claude_oauth_secret_if_needed(codex_home)?;
    let account = secret
        .account
        .as_deref()
        .or(provider_account)
        .map(str::to_string);
    let auth_method = secret
        .auth_method
        .as_deref()
        .or(provider_auth_method)
        .map(str::to_string);
    let mut details = Vec::new();
    if let Some(expires_at) = secret.expires_at {
        details.push(ExternalQuotaDetail {
            label: "OAuth expires".to_string(),
            value: format_claude_oauth_expiry(expires_at),
        });
    }

    if let Some(admin_key) = anthropic_admin_api_key_from_env() {
        match fetch_anthropic_rate_limits_json(&admin_key) {
            Ok(value) => {
                let (main, mut rate_details) = anthropic_rate_limits_summary(&value);
                details.append(&mut rate_details);
                return Ok(ExternalQuotaInfo {
                    provider: "Anthropic Claude".to_string(),
                    account,
                    plan: auth_method,
                    status: "Ready".to_string(),
                    main,
                    reset: None,
                    available: Some(true),
                    details,
                });
            }
            Err(err) => {
                let error = quota_error_message(&err);
                details.push(ExternalQuotaDetail {
                    label: "Admin API".to_string(),
                    value: format!("error: {}", first_line_of_error(&error)),
                });
            }
        }
    } else {
        details.push(ExternalQuotaDetail {
            label: "Admin API".to_string(),
            value: "set ANTHROPIC_ADMIN_KEY for configured rate limits".to_string(),
        });
    }

    Ok(ExternalQuotaInfo {
        provider: "Anthropic Claude".to_string(),
        account,
        plan: auth_method,
        status: "Ready (OAuth)".to_string(),
        main: "rate limits require admin key".to_string(),
        reset: None,
        available: Some(true),
        details,
    })
}

fn format_claude_oauth_expiry(expires_at_ms: i64) -> String {
    if expires_at_ms <= 0 {
        return "unknown".to_string();
    }
    prodex_quota::format_precise_reset_time(Some(expires_at_ms / 1000))
}

fn anthropic_admin_api_key_from_env() -> Option<String> {
    ["ANTHROPIC_ADMIN_KEY", "ANTHROPIC_ADMIN_API_KEY"]
        .into_iter()
        .find_map(|key| {
            env::var(key)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}

fn anthropic_admin_base_url() -> String {
    env::var("PRODEX_ANTHROPIC_ADMIN_BASE_URL")
        .or_else(|_| env::var("ANTHROPIC_BASE_URL"))
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "https://api.anthropic.com".to_string())
}

fn fetch_anthropic_rate_limits_json(api_key: &str) -> Result<serde_json::Value> {
    let client = build_upstream_blocking_http_client("Anthropic quota HTTP", false)?;
    let url = format!(
        "{}/v1/organizations/rate_limits",
        anthropic_admin_base_url()
    );
    let response = client
        .get(&url)
        .header("anthropic-version", "2023-06-01")
        .header("accept", "application/json")
        .header("x-api-key", api_key)
        .send()
        .context("failed to fetch Anthropic rate limits")?;
    let status = response.status();
    let body = read_blocking_response_body_with_limit(
        response,
        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
        "failed to read Anthropic rate limit response",
    )?;
    if !status.is_success() {
        bail!(
            "Anthropic rate limit request failed (HTTP {}): {}",
            status.as_u16(),
            format_response_body(&body)
        );
    }
    serde_json::from_slice(&body).context("failed to parse Anthropic rate limit response")
}

fn anthropic_rate_limits_summary(value: &serde_json::Value) -> (String, Vec<ExternalQuotaDetail>) {
    let data = value
        .get("data")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let total_groups = data.len();
    let model_groups = data
        .iter()
        .filter(|entry| {
            entry.get("group_type").and_then(serde_json::Value::as_str) == Some("model_group")
        })
        .count();
    let mut details = vec![
        ExternalQuotaDetail {
            label: "Rate groups".to_string(),
            value: total_groups.to_string(),
        },
        ExternalQuotaDetail {
            label: "Model groups".to_string(),
            value: model_groups.to_string(),
        },
    ];
    for entry in data.iter().take(3) {
        if let Some(detail) = anthropic_rate_limit_group_detail(entry) {
            details.push(detail);
        }
    }
    let main = if model_groups > 0 {
        format!("{model_groups} model rate group(s)")
    } else {
        format!("{total_groups} rate group(s)")
    };
    (main, details)
}

fn anthropic_rate_limit_group_detail(entry: &serde_json::Value) -> Option<ExternalQuotaDetail> {
    let group = entry
        .get("group_type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("rate_limit");
    let label = if group == "model_group" {
        entry
            .get("models")
            .and_then(serde_json::Value::as_array)
            .and_then(|models| models.first())
            .and_then(serde_json::Value::as_str)
            .unwrap_or(group)
    } else {
        group
    };
    let limits = entry
        .get("limits")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .filter_map(|limit| {
            let kind = limit.get("type")?.as_str()?;
            let value = limit.get("value")?;
            Some(format!("{kind}={}", quota_json_scalar(value)))
        })
        .collect::<Vec<_>>()
        .join(", ");
    Some(ExternalQuotaDetail {
        label: label.to_string(),
        value: limits,
    })
}

fn quota_json_scalar(value: &serde_json::Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}
