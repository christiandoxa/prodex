use super::*;

mod auth;
mod external_provider;
mod render;
mod virtual_provider;
mod watch;

pub(super) use self::auth::*;
use self::external_provider::{custom_model_provider_quota_info, fetch_anthropic_quota_info};
pub(super) use self::render::*;
use self::virtual_provider::collect_virtual_quota_reports;
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
