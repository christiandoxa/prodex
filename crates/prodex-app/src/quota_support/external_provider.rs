use super::{
    ExternalQuotaDetail, ExternalQuotaInfo, ProfileProvider, build_upstream_blocking_http_client,
    first_line_of_error, format_response_body, quota_error_message,
    refresh_claude_oauth_secret_if_needed,
};
use crate::{
    KIRO_MODEL_CATALOG_FILE, parse_kiro_model_catalog_text, read_kiro_auth_secret,
    read_provider_model_catalog_text,
};
use crate::{RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES, read_blocking_response_body_with_limit};
use anyhow::{Context, Result, bail};
use codex_config::codex_non_openai_model_provider;
use prodex_mojo_core::rich::ascii_casefold_equal_exact;
use prodex_mojo_core::super_provider_config::{
    RuntimeModelProviderClass, runtime_model_provider_class,
};
use std::env;
use std::path::Path;
use std::process::Command;

pub(super) fn fetch_agy_quota_info(account: Option<&str>) -> Result<ExternalQuotaInfo> {
    let mut command = Command::new("agy");
    command.args(["auth", "quota", "--format=json", "--detail"]);
    if account.is_none() {
        command.arg("--all-accounts");
    }

    let output = crate::command_probe_output(&mut command, "agy quota command")
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
    let model_count = match read_provider_model_catalog_text(&catalog_path)? {
        Some(text) => Some(
            parse_kiro_model_catalog_text(&text)
                .context("failed to parse Kiro model catalog")?
                .len(),
        ),
        None => None,
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

#[derive(serde::Deserialize)]
struct ExternalQuotaAmount {
    pointer: String,
    suffix: String,
}

#[derive(serde::Deserialize)]
struct AgyQuotaProjection {
    info: ExternalQuotaInfo,
    amount: Option<ExternalQuotaAmount>,
}

pub(super) fn project_external_quota<T: serde::de::DeserializeOwned>(
    value: &serde_json::Value,
    operation: prodex_mojo_core::json::ExternalQuotaProjection,
) -> Result<Option<T>> {
    let tree = crate::json_boundary::app_json_tree(value)?;
    let raw = std::str::from_utf8(&tree.raw).context("external quota wire was not UTF-8")?;
    prodex_mojo_core::json::external_quota_projection(&tree.nodes, raw, operation)
        .map_err(|error| anyhow::anyhow!("compiled external quota projection failed: {error:?}"))?
        .map(|output| {
            serde_json::from_slice(&output).context("invalid compiled external quota projection")
        })
        .transpose()
}

fn parse_agy_quota_json(
    value: serde_json::Value,
    preferred_account: Option<&str>,
) -> Result<ExternalQuotaInfo> {
    let mut input = serde_json::json!({"preferred": preferred_account});
    input["value"] = value;
    let mut projection: AgyQuotaProjection =
        project_external_quota(&input, prodex_mojo_core::json::ExternalQuotaProjection::Agy)?
            .context("no account found in agy output")?;
    if let Some(amount) = projection.amount {
        let number = input["value"]
            .pointer(&amount.pointer)
            .and_then(serde_json::Value::as_f64)
            .context("compiled quota amount did not resolve a number")?;
        // Mojo 1.1 float formatting imports the unsupported KGEN runtime ABI.
        projection.info.main = format!("{number:.2}{}", amount.suffix);
    }
    Ok(projection.info)
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
    match runtime_model_provider_class(provider_id)
        .expect("runtime model-provider classification should accept Rust strings")
    {
        Some(RuntimeModelProviderClass::Local) => "Local OpenAI-compatible".to_string(),
        Some(RuntimeModelProviderClass::DeepSeek) => "DeepSeek".to_string(),
        Some(RuntimeModelProviderClass::Anthropic) => "Anthropic Claude".to_string(),
        _ if ascii_casefold_equal_exact(provider_id, "amazon-bedrock")
            .expect("Mojo Bedrock provider comparison failed")
            || ascii_casefold_equal_exact(provider_id, "amazon-bedrock-runtime")
                .expect("Mojo Bedrock provider comparison failed")
            || ascii_casefold_equal_exact(provider_id, "bedrock")
                .expect("Mojo Bedrock provider comparison failed") =>
        {
            "Amazon Bedrock".to_string()
        }
        _ => format!("Custom provider ({provider_id})"),
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
                let (main, mut rate_details) = anthropic_rate_limits_summary(&value)?;
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

fn anthropic_rate_limits_summary(
    value: &serde_json::Value,
) -> Result<(String, Vec<ExternalQuotaDetail>)> {
    #[derive(serde::Deserialize)]
    struct Projection {
        main: String,
        details: Vec<ExternalQuotaDetail>,
    }
    let projection: Projection = project_external_quota(
        value,
        prodex_mojo_core::json::ExternalQuotaProjection::Anthropic,
    )?
    .context("compiled Anthropic quota projection was absent")?;
    Ok((projection.main, projection.details))
}

#[cfg(test)]
#[path = "external_provider_tests.rs"]
mod tests;
