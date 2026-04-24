use super::*;

mod auth;
mod render;
mod watch;

pub(super) use self::auth::*;
pub(super) use self::render::*;
pub(super) use self::watch::*;

#[derive(Debug)]
pub(crate) struct BlockedLimit {
    pub(crate) message: String,
}

#[derive(Debug, Clone)]
pub(crate) struct AuthSummary {
    pub(crate) label: String,
    pub(crate) quota_compatible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UsageAuth {
    pub(crate) access_token: String,
    pub(crate) account_id: Option<String>,
    pub(crate) refresh_token: Option<String>,
    pub(crate) expires_at: Option<i64>,
    pub(crate) last_refresh: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UsageAuthSyncSource {
    Reloaded,
    Refreshed,
}

#[derive(Debug, Clone)]
pub(crate) struct UsageAuthSyncOutcome {
    pub(crate) auth: UsageAuth,
    pub(crate) source: UsageAuthSyncSource,
    pub(crate) auth_changed: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum ProviderQuotaSnapshot {
    OpenAi(UsageResponse),
    Copilot(CopilotUserInfo),
}

#[derive(Debug)]
pub(crate) struct QuotaReport {
    pub(crate) name: String,
    pub(crate) active: bool,
    pub(crate) auth: AuthSummary,
    pub(crate) result: std::result::Result<ProviderQuotaSnapshot, String>,
    pub(crate) fetched_at: i64,
}

#[derive(Debug)]
struct QuotaFetchJob {
    name: String,
    active: bool,
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
    let jobs = state
        .profiles
        .iter()
        .map(|(name, profile)| QuotaFetchJob {
            name: name.clone(),
            active: state.active_profile.as_deref() == Some(name.as_str()),
            provider: profile.provider.clone(),
            codex_home: profile.codex_home.clone(),
        })
        .collect();
    let base_url = base_url.map(str::to_owned);

    map_parallel(jobs, |job| {
        let auth = job.provider.auth_summary(&job.codex_home);
        let result = fetch_profile_quota(&job.provider, &job.codex_home, base_url.as_deref())
            .map_err(|err| err.to_string());
        QuotaReport {
            name: job.name,
            active: job.active,
            auth,
            result,
            fetched_at: Local::now().timestamp(),
        }
    })
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
    ensure_profile_supports_quota(provider, codex_home)?;
    match provider {
        ProfileProvider::Openai => Ok(ProviderQuotaSnapshot::OpenAi(fetch_usage(
            codex_home, base_url,
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
    ensure_profile_supports_quota(provider, codex_home)?;
    match provider {
        ProfileProvider::Openai => fetch_usage_json(codex_home, base_url),
        ProfileProvider::Copilot { host, login, .. } => {
            fetch_copilot_user_info_json_for_account(host, login)
        }
    }
}

pub(crate) fn fetch_usage(codex_home: &Path, base_url: Option<&str>) -> Result<UsageResponse> {
    let usage: UsageResponse = serde_json::from_value(fetch_usage_json(codex_home, base_url)?)
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

fn ensure_profile_supports_quota(provider: &ProfileProvider, codex_home: &Path) -> Result<()> {
    if matches!(provider, ProfileProvider::Openai)
        && let Some(model_provider) = codex_non_openai_model_provider(codex_home, None)
    {
        bail!(
            "quota is unavailable for model_provider '{}'; prodex quota only supports the default OpenAI/Codex provider",
            model_provider.provider_id,
        );
    }
    Ok(())
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
    let base_url = base_url.trim_end_matches('/');
    if base_url.contains("/backend-api") {
        format!("{base_url}/wham/usage")
    } else {
        format!("{base_url}/api/codex/usage")
    }
}

pub(crate) fn format_response_body(body: &[u8]) -> String {
    if body.is_empty() {
        return String::new();
    }

    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
        return serde_json::to_string_pretty(&value)
            .unwrap_or_else(|_| String::from_utf8_lossy(body).trim().to_string());
    }

    String::from_utf8_lossy(body).trim().to_string()
}

pub(crate) fn format_binary_resolution(binary: &OsString) -> String {
    let configured = binary.to_string_lossy();
    match resolve_binary_path(binary) {
        Some(path) => format!("{configured} ({})", path.display()),
        None => format!("{configured} (not found)"),
    }
}

pub(crate) fn resolve_binary_path(binary: &OsString) -> Option<PathBuf> {
    let candidate = PathBuf::from(binary);
    if candidate.components().count() > 1 {
        if candidate.is_file() {
            return Some(fs::canonicalize(&candidate).unwrap_or(candidate));
        }
        return None;
    }

    let path_var = env::var_os("PATH")?;
    for directory in env::split_paths(&path_var) {
        let full_path = directory.join(&candidate);
        if full_path.is_file() {
            return Some(full_path);
        }
    }

    None
}
