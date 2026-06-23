use super::*;

mod auth;
mod external_provider;
mod render;
mod virtual_provider;
mod watch;

pub(super) use self::auth::*;
use self::external_provider::{
    custom_model_provider_quota_info, fetch_agy_quota_info, fetch_anthropic_quota_info,
};
pub(super) use self::render::*;
use self::virtual_provider::collect_virtual_quota_reports;
pub(super) use self::watch::*;
pub(crate) use prodex_core::format_binary_resolution;
pub(crate) use prodex_quota::{
    AuthSummary, ExternalQuotaDetail, ExternalQuotaInfo, GeminiQuotaInfo, QuotaAuthFilter,
    UsageAuth,
};
use prodex_runtime_doctor::read_runtime_log_tail;

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
    Agy,
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
            "agy" | "anti-gravity" => Ok(Self::Agy),
            other => bail!(
                "invalid quota provider filter '{other}'; supported values are all, openai, gemini, anthropic, claude, copilot, deepseek, local, agy"
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
            Self::Local => Self::Agy,
            Self::Agy => Self::All,
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
            Self::Agy => "agy",
        }
    }

    pub(crate) fn matches(self, provider: &ProfileProvider) -> bool {
        match self {
            Self::All => true,
            Self::OpenAi => matches!(provider, ProfileProvider::Openai),
            Self::Gemini => matches!(provider, ProfileProvider::Gemini { .. }),
            Self::Anthropic => matches!(provider, ProfileProvider::Anthropic { .. }),
            Self::Copilot => matches!(provider, ProfileProvider::Copilot { .. }),
            Self::Agy => matches!(provider, ProfileProvider::Agy { .. }),
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
            Self::All
            | Self::OpenAi
            | Self::Gemini
            | Self::Anthropic
            | Self::Copilot
            | Self::Agy => false,
        }
    }

    pub(crate) fn matches_report(self, report: &QuotaReport) -> bool {
        if self.matches(&report.provider) {
            return true;
        }
        if let Ok(ProviderQuotaSnapshot::External(info)) = &report.result {
            match self {
                Self::DeepSeek => return info.provider.eq_ignore_ascii_case("DeepSeek"),
                Self::Local => {
                    return info
                        .provider
                        .eq_ignore_ascii_case("Local OpenAI-compatible");
                }
                _ => {}
            }
        }
        false
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
    let current_profile = quota_current_profile_name(state);
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
                active: current_profile.as_deref() == Some(name.as_str()),
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
            | ProfileProvider::Copilot { .. }
            | ProfileProvider::Agy { .. } => None,
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

const QUOTA_RUNTIME_LOG_TAIL_BYTES: usize = 1024 * 1024;
const QUOTA_RUNTIME_PROFILE_EVENTS: &[&str] = &["token_usage", "profile_commit"];

fn quota_current_profile_name(state: &AppState) -> Option<String> {
    quota_current_runtime_profile_name(state).or_else(|| quota_state_current_profile_name(state))
}

fn quota_state_current_profile_name(state: &AppState) -> Option<String> {
    state
        .last_run_selected_at
        .iter()
        .filter(|(profile_name, _)| state.profiles.contains_key(*profile_name))
        .max_by(|(left_name, left_at), (right_name, right_at)| {
            left_at
                .cmp(right_at)
                .then_with(|| left_name.cmp(right_name))
        })
        .map(|(profile_name, _)| profile_name.clone())
        .or_else(|| {
            state
                .active_profile
                .clone()
                .filter(|profile_name| state.profiles.contains_key(profile_name))
        })
}

fn quota_current_runtime_profile_name(state: &AppState) -> Option<String> {
    quota_current_runtime_profile_name_from_paths(
        state,
        prodex_runtime_log_paths_in_dir(&runtime_proxy_log_dir()),
    )
}

fn quota_current_runtime_profile_name_from_paths<I>(state: &AppState, paths: I) -> Option<String>
where
    I: IntoIterator<Item = PathBuf>,
{
    let mut paths: Vec<PathBuf> = paths.into_iter().collect();
    paths.sort_by(|left, right| {
        let left_modified = left
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok();
        let right_modified = right
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok();
        left_modified
            .cmp(&right_modified)
            .then_with(|| left.cmp(right))
    });

    let mut current = None;
    for path in paths {
        let Ok(tail) = read_runtime_log_tail(&path, QUOTA_RUNTIME_LOG_TAIL_BYTES) else {
            continue;
        };
        let tail = String::from_utf8_lossy(&tail);
        for line in tail.lines() {
            let Some(profile_name) = quota_runtime_profile_from_line(line) else {
                continue;
            };
            if state.profiles.contains_key(profile_name.as_str()) {
                current = Some(profile_name);
            }
        }
    }
    current
}

fn quota_runtime_profile_from_line(line: &str) -> Option<String> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
        let event = value.get("event").and_then(serde_json::Value::as_str)?;
        if !QUOTA_RUNTIME_PROFILE_EVENTS.contains(&event) {
            return None;
        }
        return value
            .get("fields")
            .and_then(serde_json::Value::as_object)
            .and_then(|fields| fields.get("profile"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
    }

    let message = line
        .strip_prefix('[')
        .and_then(|rest| rest.split_once("] ").map(|(_, message)| message))
        .unwrap_or(line);
    let event = runtime_proxy_crate::runtime_proxy_log_event(message)?;
    if !QUOTA_RUNTIME_PROFILE_EVENTS.contains(&event) {
        return None;
    }
    runtime_proxy_crate::runtime_proxy_log_fields(message)
        .get("profile")
        .cloned()
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
        ProfileProvider::Agy { account } => Ok(ProviderQuotaSnapshot::External(
            fetch_agy_quota_info(account.as_deref())?,
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
        ProfileProvider::Agy { account } => {
            serde_json::to_value(fetch_agy_quota_info(account.as_deref())?)
                .context("failed to render Agy quota JSON")
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

pub(crate) fn rate_limit_reset_credit_consume_url(base_url: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    if base_url.contains("/backend-api") {
        format!("{base_url}/wham/rate-limit-reset-credits/consume")
    } else {
        format!("{base_url}/api/codex/rate-limit-reset-credits/consume")
    }
}

pub(crate) fn format_response_body(body: &[u8]) -> String {
    prodex_quota::format_response_body(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn profile(path: &str) -> ProfileEntry {
        ProfileEntry {
            codex_home: PathBuf::from(path),
            managed: true,
            email: None,
            provider: ProfileProvider::Openai,
        }
    }

    fn test_runtime_log_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = env::temp_dir().join(format!("prodex-quota-test-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn quota_current_profile_prefers_latest_runtime_selection() {
        let state = AppState {
            active_profile: Some("active".to_string()),
            profiles: BTreeMap::from([
                ("active".to_string(), profile("/tmp/active")),
                ("runtime".to_string(), profile("/tmp/runtime")),
            ]),
            last_run_selected_at: BTreeMap::from([
                ("active".to_string(), 10),
                ("runtime".to_string(), 20),
            ]),
            ..AppState::default()
        };

        assert_eq!(
            quota_current_profile_name(&state).as_deref(),
            Some("runtime")
        );
    }

    #[test]
    fn quota_current_profile_falls_back_to_active_profile() {
        let state = AppState {
            active_profile: Some("active".to_string()),
            profiles: BTreeMap::from([("active".to_string(), profile("/tmp/active"))]),
            last_run_selected_at: BTreeMap::from([("deleted".to_string(), 30)]),
            ..AppState::default()
        };

        assert_eq!(
            quota_current_profile_name(&state).as_deref(),
            Some("active")
        );
    }

    #[test]
    fn reset_credit_consume_url_matches_backend_path_style() {
        assert_eq!(
            rate_limit_reset_credit_consume_url("https://chatgpt.com/backend-api"),
            "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits/consume"
        );
        assert_eq!(
            rate_limit_reset_credit_consume_url("http://127.0.0.1:8080"),
            "http://127.0.0.1:8080/api/codex/rate-limit-reset-credits/consume"
        );
    }

    #[test]
    fn quota_current_profile_prefers_runtime_log_usage() {
        let log_path = test_runtime_log_path("prodex-runtime-test.log");
        let mut log = fs::File::create(&log_path).unwrap();
        writeln!(
            log,
            "[2026-06-22 16:00:00.000 +07:00] token_usage request=1 route=websocket transport=websocket profile=runtime source=responses_websocket"
        )
        .unwrap();

        let state = AppState {
            active_profile: Some("active".to_string()),
            profiles: BTreeMap::from([
                ("active".to_string(), profile("/tmp/active")),
                ("runtime".to_string(), profile("/tmp/runtime")),
            ]),
            last_run_selected_at: BTreeMap::from([("active".to_string(), 30)]),
            ..AppState::default()
        };

        assert_eq!(
            quota_current_runtime_profile_name_from_paths(&state, vec![log_path]).as_deref(),
            Some("runtime")
        );
    }

    #[test]
    fn quota_runtime_log_ignores_unknown_profiles() {
        let log_path = test_runtime_log_path("prodex-runtime-test.log");
        let mut log = fs::File::create(&log_path).unwrap();
        writeln!(
            log,
            "[2026-06-22 16:00:00.000 +07:00] token_usage request=1 route=websocket transport=websocket profile=deleted source=responses_websocket"
        )
        .unwrap();

        let state = AppState {
            active_profile: Some("active".to_string()),
            profiles: BTreeMap::from([("active".to_string(), profile("/tmp/active"))]),
            ..AppState::default()
        };

        assert_eq!(
            quota_current_runtime_profile_name_from_paths(&state, vec![log_path]),
            None
        );
    }
}
