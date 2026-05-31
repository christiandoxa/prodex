use super::*;

mod auth;
mod render;
mod watch;

pub(super) use self::auth::*;
pub(super) use self::render::*;
pub(super) use self::watch::*;
pub(crate) use prodex_core::format_binary_resolution;
pub(crate) use prodex_quota::{
    AuthSummary, BlockedLimit, ExternalQuotaDetail, ExternalQuotaInfo, GeminiQuotaInfo,
    QuotaAuthFilter, UsageAuth,
};

#[derive(Debug, Clone)]
pub(crate) enum ProviderQuotaSnapshot {
    OpenAi(UsageResponse),
    Copilot(CopilotUserInfo),
    Gemini(GeminiQuotaInfo),
    External(ExternalQuotaInfo),
}

#[derive(Debug, Clone)]
pub(crate) struct QuotaReport {
    pub(crate) name: String,
    pub(crate) active: bool,
    pub(crate) auth: AuthSummary,
    pub(crate) provider: ProfileProvider,
    pub(crate) workspace_id: Option<String>,
    pub(crate) result: std::result::Result<ProviderQuotaSnapshot, String>,
    pub(crate) fetched_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuotaProviderFilter {
    All,
    OpenAi,
    Gemini,
    Anthropic,
    Copilot,
    DeepSeek,
    Local,
}

impl QuotaProviderFilter {
    pub(crate) fn parse(value: &str) -> Result<Self> {
        let normalized = value.trim().to_ascii_lowercase().replace('_', "-");
        match normalized.as_str() {
            "" | "all" => Ok(Self::All),
            "openai" | "chatgpt" | "codex" | "openai-codex" => Ok(Self::OpenAi),
            "gemini" | "google" | "google-gemini" => Ok(Self::Gemini),
            "anthropic" | "claude" | "anthropic-claude" => Ok(Self::Anthropic),
            "copilot" | "github-copilot" | "github" => Ok(Self::Copilot),
            "deepseek" => Ok(Self::DeepSeek),
            "local" | "openai-compatible" | "openai_compatible" => Ok(Self::Local),
            other => bail!(
                "invalid quota provider filter '{other}'; supported values are all, openai, gemini, anthropic, claude, copilot, deepseek, local"
            ),
        }
    }

    pub(crate) fn next(self) -> Self {
        match self {
            Self::All => Self::OpenAi,
            Self::OpenAi => Self::Gemini,
            Self::Gemini => Self::Anthropic,
            Self::Anthropic => Self::Copilot,
            Self::Copilot => Self::DeepSeek,
            Self::DeepSeek => Self::Local,
            Self::Local => Self::All,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::OpenAi => "openai",
            Self::Gemini => "gemini",
            Self::Anthropic => "anthropic",
            Self::Copilot => "copilot",
            Self::DeepSeek => "deepseek",
            Self::Local => "local",
        }
    }

    pub(crate) fn matches(self, provider: &ProfileProvider) -> bool {
        match self {
            Self::All => true,
            Self::OpenAi => matches!(provider, ProfileProvider::Openai),
            Self::Gemini => matches!(provider, ProfileProvider::Gemini { .. }),
            Self::Anthropic => matches!(provider, ProfileProvider::Anthropic { .. }),
            Self::Copilot => matches!(provider, ProfileProvider::Copilot { .. }),
            Self::DeepSeek | Self::Local => false,
        }
    }

    pub(crate) fn matches_profile(self, provider: &ProfileProvider, codex_home: &Path) -> bool {
        if self.matches(provider) {
            return true;
        }
        let Some(model_provider) = codex_non_openai_model_provider(codex_home, None) else {
            return false;
        };
        match self {
            Self::DeepSeek => model_provider
                .provider_id
                .eq_ignore_ascii_case(SUPER_DEEPSEEK_PROVIDER_ID),
            Self::Local => model_provider
                .provider_id
                .eq_ignore_ascii_case(SUPER_LOCAL_PROVIDER_ID),
            Self::All | Self::OpenAi | Self::Gemini | Self::Anthropic | Self::Copilot => false,
        }
    }

    pub(crate) fn matches_report(self, report: &QuotaReport) -> bool {
        if self.matches(&report.provider) {
            return true;
        }
        let Ok(ProviderQuotaSnapshot::External(info)) = &report.result else {
            return false;
        };
        match self {
            Self::DeepSeek => info.provider.eq_ignore_ascii_case("DeepSeek"),
            Self::Local => info
                .provider
                .eq_ignore_ascii_case("Local OpenAI-compatible"),
            Self::All | Self::OpenAi | Self::Gemini | Self::Anthropic | Self::Copilot => false,
        }
    }
}

#[derive(Debug)]
struct QuotaFetchJob {
    name: String,
    active: bool,
    auth: AuthSummary,
    provider: ProfileProvider,
    codex_home: PathBuf,
}

#[derive(Debug)]
struct ProfileSummaryJob {
    name: String,
    active: bool,
    managed: bool,
    email: Option<String>,
    provider: ProfileProvider,
    codex_home: PathBuf,
}

#[derive(Debug)]
pub(crate) struct ProfileSummaryReport {
    pub(crate) name: String,
    pub(crate) active: bool,
    pub(crate) managed: bool,
    pub(crate) auth: AuthSummary,
    pub(crate) email: Option<String>,
    pub(crate) provider: ProfileProvider,
    pub(crate) codex_home: PathBuf,
}

#[derive(Debug)]
pub(crate) struct DoctorProfileReport {
    pub(crate) summary: ProfileSummaryReport,
    pub(crate) quota: Option<std::result::Result<ProviderQuotaSnapshot, String>>,
}

pub(crate) fn collect_quota_reports(state: &AppState, base_url: Option<&str>) -> Vec<QuotaReport> {
    collect_quota_reports_with_auth_filter(state, base_url, &QuotaAuthFilter::All)
}

pub(crate) fn collect_quota_reports_with_auth_filter(
    state: &AppState,
    base_url: Option<&str>,
    auth_filter: &QuotaAuthFilter,
) -> Vec<QuotaReport> {
    collect_quota_reports_with_filters(state, base_url, auth_filter, QuotaProviderFilter::All)
}

pub(crate) fn collect_quota_reports_with_filters(
    state: &AppState,
    base_url: Option<&str>,
    auth_filter: &QuotaAuthFilter,
    provider_filter: QuotaProviderFilter,
) -> Vec<QuotaReport> {
    let jobs = state
        .profiles
        .iter()
        .filter_map(|(name, profile)| {
            if !provider_filter.matches_profile(&profile.provider, &profile.codex_home) {
                return None;
            }
            let auth = profile.provider.auth_summary(&profile.codex_home);
            auth_filter.matches(&auth).then(|| QuotaFetchJob {
                name: name.clone(),
                active: state.active_profile.as_deref() == Some(name.as_str()),
                auth,
                provider: profile.provider.clone(),
                codex_home: profile.codex_home.clone(),
            })
        })
        .collect();
    let base_url = base_url.map(str::to_owned);

    let mut reports = map_parallel(jobs, |job| {
        let workspace_id = match &job.provider {
            ProfileProvider::Openai => read_profile_account_id_from_auth(&job.codex_home)
                .ok()
                .flatten(),
            ProfileProvider::Gemini { .. }
            | ProfileProvider::Anthropic { .. }
            | ProfileProvider::Copilot { .. } => None,
        };
        let result = fetch_profile_quota(&job.provider, &job.codex_home, base_url.as_deref())
            .map_err(|err| err.to_string());
        QuotaReport {
            name: job.name,
            active: job.active,
            auth: job.auth,
            provider: job.provider,
            workspace_id,
            result,
            fetched_at: Local::now().timestamp(),
        }
    });
    reports.extend(collect_virtual_quota_reports(
        base_url.as_deref(),
        provider_filter,
    ));
    reports
}

pub(crate) fn collect_profile_summaries(state: &AppState) -> Vec<ProfileSummaryReport> {
    let jobs = state
        .profiles
        .iter()
        .map(|(name, profile)| ProfileSummaryJob {
            name: name.clone(),
            active: state.active_profile.as_deref() == Some(name.as_str()),
            managed: profile.managed,
            email: profile.email.clone(),
            provider: profile.provider.clone(),
            codex_home: profile.codex_home.clone(),
        })
        .collect();

    map_parallel(jobs, |job| ProfileSummaryReport {
        name: job.name,
        active: job.active,
        managed: job.managed,
        auth: job.provider.auth_summary(&job.codex_home),
        email: job.email,
        provider: job.provider,
        codex_home: job.codex_home,
    })
}

pub(crate) fn collect_doctor_profile_reports(
    state: &AppState,
    include_quota: bool,
) -> Vec<DoctorProfileReport> {
    map_parallel(collect_profile_summaries(state), |summary| {
        DoctorProfileReport {
            quota: include_quota.then(|| {
                fetch_profile_quota(&summary.provider, &summary.codex_home, None)
                    .map_err(|err| err.to_string())
            }),
            summary,
        }
    })
}

pub(crate) fn fetch_profile_quota(
    provider: &ProfileProvider,
    codex_home: &Path,
    base_url: Option<&str>,
) -> Result<ProviderQuotaSnapshot> {
    if let Some(info) = custom_model_provider_quota_info(provider, codex_home) {
        return Ok(ProviderQuotaSnapshot::External(info));
    }
    match provider {
        ProfileProvider::Openai => Ok(ProviderQuotaSnapshot::OpenAi(fetch_usage(
            codex_home, base_url,
        )?)),
        ProfileProvider::Gemini { project_id, .. } => Ok(ProviderQuotaSnapshot::Gemini(
            fetch_gemini_quota(codex_home, project_id.as_deref())?,
        )),
        ProfileProvider::Anthropic {
            account,
            auth_method,
        } => Ok(ProviderQuotaSnapshot::External(fetch_anthropic_quota_info(
            codex_home,
            account.as_deref(),
            auth_method.as_deref(),
        )?)),
        ProfileProvider::Copilot { host, login, .. } => Ok(ProviderQuotaSnapshot::Copilot(
            fetch_copilot_user_info_for_account(host, login)?,
        )),
    }
}

pub(crate) fn fetch_profile_quota_json(
    provider: &ProfileProvider,
    codex_home: &Path,
    base_url: Option<&str>,
) -> Result<serde_json::Value> {
    if let Some(info) = custom_model_provider_quota_info(provider, codex_home) {
        return serde_json::to_value(info).context("failed to render provider quota JSON");
    }
    match provider {
        ProfileProvider::Openai => fetch_usage_json(codex_home, base_url),
        ProfileProvider::Gemini { project_id, .. } => {
            fetch_gemini_quota_json(codex_home, project_id.as_deref())
        }
        ProfileProvider::Anthropic {
            account,
            auth_method,
        } => serde_json::to_value(fetch_anthropic_quota_info(
            codex_home,
            account.as_deref(),
            auth_method.as_deref(),
        )?)
        .context("failed to render Anthropic quota JSON"),
        ProfileProvider::Copilot { host, login, .. } => {
            fetch_copilot_user_info_json_for_account(host, login)
        }
    }
}

fn custom_model_provider_quota_info(
    provider: &ProfileProvider,
    codex_home: &Path,
) -> Option<ExternalQuotaInfo> {
    if !matches!(provider, ProfileProvider::Openai) {
        return None;
    }
    let model_provider = codex_non_openai_model_provider(codex_home, None)?;
    let provider_name = custom_model_provider_display_name(&model_provider.provider_id);
    Some(ExternalQuotaInfo {
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
    })
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

fn fetch_anthropic_quota_info(
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
                details.push(ExternalQuotaDetail {
                    label: "Admin API".to_string(),
                    value: format!("error: {}", first_line_of_error(&err.to_string())),
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
    let body = response
        .bytes()
        .context("failed to read Anthropic rate limit response")?;
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

fn collect_virtual_quota_reports(
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
            let result = fetch_deepseek_quota_info(&key, base_url).map_err(|err| err.to_string());
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
        .map_err(|err| err.to_string());
    virtual_quota_report("local", "local", result)
}

fn virtual_quota_report(
    name: &str,
    auth_label: &str,
    result: std::result::Result<ProviderQuotaSnapshot, String>,
) -> QuotaReport {
    QuotaReport {
        name: name.to_string(),
        active: false,
        auth: AuthSummary {
            label: auth_label.to_string(),
            quota_compatible: false,
        },
        provider: ProfileProvider::Openai,
        workspace_id: None,
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

pub(crate) fn fetch_usage(codex_home: &Path, base_url: Option<&str>) -> Result<UsageResponse> {
    fetch_usage_with_proxy_policy(codex_home, base_url, false)
}

pub(crate) fn fetch_usage_with_proxy_policy(
    codex_home: &Path,
    base_url: Option<&str>,
    upstream_no_proxy: bool,
) -> Result<UsageResponse> {
    let usage: UsageResponse = serde_json::from_value(fetch_usage_json_with_proxy_policy(
        codex_home,
        base_url,
        upstream_no_proxy,
    )?)
    .with_context(|| {
        format!(
            "invalid JSON returned by quota backend for {}",
            codex_home.display()
        )
    })?;
    Ok(usage)
}

pub(crate) fn fetch_usage_json(
    codex_home: &Path,
    base_url: Option<&str>,
) -> Result<serde_json::Value> {
    UsageFetchFlow::new(codex_home, base_url)?.execute()
}

pub(crate) fn fetch_usage_json_with_proxy_policy(
    codex_home: &Path,
    base_url: Option<&str>,
    upstream_no_proxy: bool,
) -> Result<serde_json::Value> {
    UsageFetchFlow::new_with_proxy_policy(codex_home, base_url, upstream_no_proxy)?.execute()
}

pub(crate) fn print_quota_reports(reports: &[QuotaReport], detail: bool) {
    print_stdout_text(&render_quota_reports(reports, detail));
}

pub(crate) fn quota_base_url(explicit: Option<&str>) -> String {
    explicit
        .map(ToOwned::to_owned)
        .or_else(|| env::var("CODEX_CHATGPT_BASE_URL").ok())
        .unwrap_or_else(|| DEFAULT_CHATGPT_BASE_URL.to_string())
        .trim_end_matches('/')
        .to_string()
}

pub(crate) fn usage_url(base_url: &str) -> String {
    prodex_quota::usage_url(base_url)
}

pub(crate) fn format_response_body(body: &[u8]) -> String {
    prodex_quota::format_response_body(body)
}
