use anyhow::{Context, Result, anyhow};
use prodex_provider_core::{
    ProviderId, provider_adapter_contract_matrix, provider_model_catalog_json,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::Read;
use std::net::SocketAddr;
use terminal_ui::print_panel;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::dashboard_html::DASHBOARD_HTML;
use crate::{
    AppPaths, AppState, AppStateIoExt, DashboardArgs, ProfileEntry, ProfileProvider,
    ProviderQuotaSnapshot, QuotaProviderFilter, collect_profile_summaries,
    collect_quota_reports_with_filters, create_codex_home_if_missing, ensure_path_is_unique,
    format_copilot_main_quota, format_copilot_quota_status, format_copilot_reset_summary,
    format_gemini_main_quota, format_gemini_quota_status, format_gemini_reset_summary,
    format_main_windows, managed_profile_home_path, prepare_managed_codex_home,
    runtime_proxy_latest_log_path_from_pointer, runtime_proxy_log_dir,
};

const DASHBOARD_PROVIDER_IDS: &[ProviderId] = &[
    ProviderId::OpenAi,
    ProviderId::Gemini,
    ProviderId::Anthropic,
    ProviderId::Copilot,
    ProviderId::DeepSeek,
    ProviderId::Kiro,
    ProviderId::Local,
];
const DASHBOARD_MAX_JSON_BODY_BYTES: usize = 64 * 1024;

#[derive(Debug)]
struct DashboardJsonBodyTooLarge {
    limit: usize,
}

impl std::fmt::Display for DashboardJsonBodyTooLarge {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "dashboard request body exceeds {} bytes",
            self.limit
        )
    }
}

impl std::error::Error for DashboardJsonBodyTooLarge {}

#[derive(Debug)]
struct DashboardServer {
    paths: AppPaths,
    base_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ActiveProfileRequest {
    profile: String,
}

#[derive(Debug, Deserialize)]
struct AddProfileRequest {
    name: String,
    #[serde(default)]
    activate: bool,
}

pub(crate) fn serve_dashboard(paths: AppPaths, args: DashboardArgs) -> Result<()> {
    let bind = format!("{}:{}", args.host.trim(), args.port);
    let server = Server::http(&bind).map_err(|err| anyhow!("failed to bind {bind}: {err}"))?;
    let addr = server.server_addr();
    let url = dashboard_url(addr);
    let warning = (!bind.starts_with("127.0.0.1:")
        && !bind.starts_with("localhost:")
        && !bind.starts_with("[::1]:"))
    .then_some("dashboard has no password auth; bind localhost unless the network is trusted");
    print_dashboard_status(&url, warning)?;

    let dashboard = DashboardServer {
        paths,
        base_url: args.base_url,
    };
    for request in server.incoming_requests() {
        if let Err(err) = dashboard.handle(request) {
            eprintln!("dashboard request failed: {err:#}");
        }
    }
    Ok(())
}

fn print_dashboard_status(url: &str, warning: Option<&str>) -> Result<()> {
    print_panel("Dashboard", &dashboard_status_fields(url, warning));
    Ok(())
}

fn dashboard_status_fields(url: &str, warning: Option<&str>) -> Vec<(String, String)> {
    let mut fields = vec![("URL".to_string(), url.to_string())];
    if let Some(warning) = warning {
        fields.push(("Warning".to_string(), warning.to_string()));
    }
    fields
}

fn dashboard_url(addr: tiny_http::ListenAddr) -> String {
    match addr.to_ip() {
        Some(SocketAddr::V4(addr)) => format!("http://{}:{}", addr.ip(), addr.port()),
        Some(SocketAddr::V6(addr)) => format!("http://[{}]:{}", addr.ip(), addr.port()),
        None => "http://127.0.0.1:8765".to_string(),
    }
}

impl DashboardServer {
    fn handle(&self, request: Request) -> Result<()> {
        let method = request.method().clone();
        let path = request.url().split('?').next().unwrap_or("/").to_string();
        match (method, path.as_str()) {
            (Method::Get, "/") | (Method::Get, "/dashboard") => {
                respond_html(request, DASHBOARD_HTML)
            }
            (Method::Get, "/api/state") => respond_json_result(request, self.state_json()),
            (Method::Get, "/api/accounts") => respond_json_result(request, self.accounts_json()),
            (Method::Get, "/api/usage") => respond_json_result(request, self.usage_json()),
            (Method::Get, "/api/providers") => respond_json_result(request, self.providers_json()),
            (Method::Get, "/api/provider-presets") => {
                respond_json_result(request, self.provider_presets_json())
            }
            (Method::Get, "/api/models") => respond_json_result(request, self.models_json()),
            (Method::Get, "/api/runtime-status") => {
                respond_json_result(request, self.runtime_status_json())
            }
            (Method::Post, "/api/profile") => self.handle_add_profile(request),
            (Method::Post, "/api/profile/active") => self.handle_set_active(request),
            (Method::Delete, path) if path.starts_with("/api/profile/") => {
                let name = path.trim_start_matches("/api/profile/").to_string();
                self.handle_remove_profile(request, name)
            }
            _ => respond_status(
                request,
                StatusCode(404),
                "application/json",
                br#"{"error":"not_found"}"#.to_vec(),
            ),
        }
    }

    fn state_json(&self) -> Result<Value> {
        let state = AppState::load(&self.paths)?;
        Ok(json!({
            "activeProfile": state.active_profile,
            "profileCount": state.profiles.len(),
            "paths": {
                "stateFile": self.paths.state_file.display().to_string(),
                "managedProfilesRoot": self.paths.managed_profiles_root.display().to_string(),
            },
            "commands": {
                "open": "prodex dashboard",
                "login": "prodex login --profile <name>",
                "addManagedProfile": "prodex profile add <name> --activate",
                "importCurrent": "prodex profile import-current <name>",
                "quota": "prodex quota --all --once",
            }
        }))
    }

    fn accounts_json(&self) -> Result<Value> {
        let state = AppState::load(&self.paths)?;
        let accounts = collect_profile_summaries(&state)
            .into_iter()
            .map(|summary| {
                json!({
                    "name": summary.name,
                    "active": summary.active,
                    "managed": summary.managed,
                    "email": summary.email,
                    "provider": summary.provider.label(),
                    "providerName": summary.provider.display_name(),
                    "auth": {
                        "label": summary.auth.label,
                        "quotaCompatible": summary.auth.quota_compatible,
                    },
                    "codexHome": summary.codex_home.display().to_string(),
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({ "accounts": accounts }))
    }

    fn usage_json(&self) -> Result<Value> {
        let state = AppState::load(&self.paths)?;
        let reports = collect_quota_reports_with_filters(
            &state,
            self.base_url.as_deref(),
            &prodex_quota::QuotaAuthFilter::All,
            QuotaProviderFilter::All,
        );
        let mut ready = 0usize;
        let mut blocked = 0usize;
        let mut errors = 0usize;
        let mut profiles = Vec::new();

        for report in reports {
            let quota = match report.result {
                Ok(snapshot) => {
                    let summary = quota_summary(&snapshot);
                    if summary["status"]
                        .as_str()
                        .is_some_and(|status| status == "Ready")
                    {
                        ready += 1;
                    } else {
                        blocked += 1;
                    }
                    summary
                }
                Err(error) => {
                    errors += 1;
                    json!({
                        "status": "Error",
                        "main": "-",
                        "reset": null,
                        "error": error.lines().find(|line| !line.trim().is_empty()).unwrap_or("quota fetch failed"),
                    })
                }
            };
            profiles.push(json!({
                "name": report.name,
                "active": report.active,
                "provider": report.provider.label(),
                "providerName": report.provider.display_name(),
                "auth": report.auth.label,
                "workspaceId": report.workspace_id,
                "fetchedAt": report.fetched_at,
                "quota": quota,
            }));
        }

        Ok(json!({
            "summary": {
                "ready": ready,
                "blocked": blocked,
                "errors": errors,
                "total": profiles.len(),
            },
            "profiles": profiles,
        }))
    }

    fn providers_json(&self) -> Result<Value> {
        let state = AppState::load(&self.paths)?;
        Ok(json!({
            "activeProfile": state.active_profile,
            "providers": provider_presets(&state),
            "contracts": provider_adapter_contract_matrix(),
        }))
    }

    fn provider_presets_json(&self) -> Result<Value> {
        let state = AppState::load(&self.paths)?;
        Ok(json!({ "providers": provider_presets(&state) }))
    }

    fn models_json(&self) -> Result<Value> {
        let state = AppState::load(&self.paths)?;
        let mut models = Vec::new();
        for provider in DASHBOARD_PROVIDER_IDS {
            let default_model = provider_default_model(*provider);
            let availability =
                provider_available_through(*provider, provider_profile_count(&state, *provider));
            for mut model in provider_model_catalog_json(*provider) {
                let id = model
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let recommended = id == default_model;
                if let Some(object) = model.as_object_mut() {
                    object.insert(
                        "providerName".to_string(),
                        json!(provider_display_name(*provider)),
                    );
                    object.insert("recommended".to_string(), json!(recommended));
                    object.insert("default".to_string(), json!(recommended));
                    object.insert("availableThrough".to_string(), json!(availability));
                    object.insert(
                        "launchCommand".to_string(),
                        json!(provider_launch_command(*provider, Some(&id))),
                    );
                }
                models.push(model);
            }
        }
        Ok(json!({ "models": models, "providers": provider_presets(&state) }))
    }

    fn runtime_status_json(&self) -> Result<Value> {
        let log_dir = runtime_proxy_log_dir();
        let latest_pointer = log_dir.join(crate::RUNTIME_PROXY_LATEST_LOG_POINTER);
        let latest_log = runtime_proxy_latest_log_path_from_pointer();
        let latest_log_exists = latest_log.as_ref().is_some_and(|path| path.exists());

        Ok(json!({
            "runtime": {
                "status": if latest_log_exists { "log-available" } else { "not-running-or-no-log" },
                "logDir": log_dir.display().to_string(),
                "latestLogPointer": latest_pointer.display().to_string(),
                "latestLog": latest_log.map(|path| path.display().to_string()),
                "latestLogExists": latest_log_exists,
                "doctorCommand": "prodex doctor --runtime",
            },
            "gateway": {
                "status": "available-on-demand",
                "startCommand": "prodex gateway --provider <provider>",
                "providersCommand": "prodex gateway providers --json",
                "modelsCommand": "prodex gateway models --provider <provider> --json",
            }
        }))
    }

    fn handle_set_active(&self, mut request: Request) -> Result<()> {
        let payload: ActiveProfileRequest = match read_json_body(&mut request) {
            Ok(payload) => payload,
            Err(err) => return respond_error(request, dashboard_json_body_error_status(&err), err),
        };
        let mut state = match AppState::load(&self.paths) {
            Ok(state) => state,
            Err(err) => return respond_error(request, StatusCode(500), err),
        };
        if !state.profiles.contains_key(&payload.profile) {
            return respond_error(
                request,
                StatusCode(404),
                anyhow!("profile '{}' is missing", payload.profile),
            );
        }
        state.active_profile = Some(payload.profile);
        if let Err(err) = state.save(&self.paths) {
            return respond_error(request, StatusCode(500), err);
        }
        respond_json(
            request,
            json!({ "status": "ok", "activeProfile": state.active_profile }),
        )
    }

    fn handle_add_profile(&self, mut request: Request) -> Result<()> {
        let payload: AddProfileRequest = match read_json_body(&mut request) {
            Ok(payload) => payload,
            Err(err) => return respond_error(request, dashboard_json_body_error_status(&err), err),
        };
        let name = payload.name.trim().to_string();
        if let Err(err) = prodex_profile_identity::validate_profile_name(&name) {
            return respond_error(request, StatusCode(400), err);
        }
        let mut state = match AppState::load(&self.paths) {
            Ok(state) => state,
            Err(err) => return respond_error(request, StatusCode(500), err),
        };
        if state.profiles.contains_key(&name) {
            return respond_error(
                request,
                StatusCode(409),
                anyhow!("profile '{}' already exists", name),
            );
        }
        let codex_home = match managed_profile_home_path(&self.paths, &name) {
            Ok(path) => path,
            Err(err) => return respond_error(request, StatusCode(500), err),
        };
        for result in [
            create_codex_home_if_missing(&codex_home),
            prepare_managed_codex_home(&self.paths, &codex_home),
            ensure_path_is_unique(&state, &codex_home),
        ] {
            if let Err(err) = result {
                return respond_error(request, StatusCode(500), err);
            }
        }
        state.profiles.insert(
            name.clone(),
            ProfileEntry {
                codex_home: codex_home.clone(),
                managed: true,
                email: None,
                provider: ProfileProvider::Openai,
            },
        );
        if payload.activate || state.active_profile.is_none() {
            state.active_profile = Some(name.clone());
        }
        if let Err(err) = state.save(&self.paths) {
            return respond_error(request, StatusCode(500), err);
        }
        respond_json(
            request,
            json!({
                "status": "ok",
                "profile": name,
                "activeProfile": state.active_profile,
                "codexHome": codex_home.display().to_string(),
            }),
        )
    }

    fn handle_remove_profile(&self, request: Request, raw_name: String) -> Result<()> {
        let name = percent_decode(&raw_name);
        let mut state = match AppState::load(&self.paths) {
            Ok(state) => state,
            Err(err) => return respond_error(request, StatusCode(500), err),
        };
        if state.profiles.remove(&name).is_none() {
            return respond_status(
                request,
                StatusCode(404),
                "application/json",
                br#"{"error":"profile_not_found"}"#.to_vec(),
            );
        }
        if state.active_profile.as_deref() == Some(name.as_str()) {
            state.active_profile = state.profiles.keys().next().cloned();
        }
        if let Err(err) = state.save(&self.paths) {
            return respond_error(request, StatusCode(500), err);
        }
        respond_json(
            request,
            json!({ "status": "ok", "activeProfile": state.active_profile }),
        )
    }
}

fn provider_presets(state: &AppState) -> Vec<Value> {
    DASHBOARD_PROVIDER_IDS
        .iter()
        .copied()
        .map(|provider| {
            let configured_profiles = provider_profile_count(state, provider);
            let default_model = provider_default_model(provider);
            json!({
                "id": provider.label(),
                "label": provider_display_name(provider),
                "auth": provider_auth_summary(provider),
                "defaultModel": default_model,
                "recommendedModel": default_model,
                "modelCount": provider_model_catalog_json(provider).len(),
                "configuredProfiles": configured_profiles,
                "active": provider_has_active_profile(state, provider),
                "availableThrough": provider_available_through(provider, configured_profiles),
                "commands": {
                    "setup": provider_setup_commands(provider),
                    "launch": provider_launch_command(provider, None),
                    "quota": provider_quota_command(provider),
                    "gateway": provider_gateway_command(provider),
                },
                "notes": provider_notes(provider),
            })
        })
        .collect()
}

fn provider_profile_count(state: &AppState, provider: ProviderId) -> usize {
    state
        .profiles
        .values()
        .filter(|profile| profile_catalog_provider(profile) == Some(provider))
        .count()
}

fn provider_has_active_profile(state: &AppState, provider: ProviderId) -> bool {
    state
        .active_profile
        .as_ref()
        .and_then(|name| state.profiles.get(name))
        .and_then(profile_catalog_provider)
        == Some(provider)
}

fn profile_catalog_provider(profile: &ProfileEntry) -> Option<ProviderId> {
    match &profile.provider {
        ProfileProvider::Openai => {
            let model_provider = crate::codex_non_openai_model_provider(&profile.codex_home, None);
            match model_provider
                .as_ref()
                .map(|provider| provider.provider_id.as_str())
            {
                Some(id) if id.eq_ignore_ascii_case(crate::SUPER_DEEPSEEK_PROVIDER_ID) => {
                    Some(ProviderId::DeepSeek)
                }
                Some(id) if id.eq_ignore_ascii_case(crate::SUPER_LOCAL_PROVIDER_ID) => {
                    Some(ProviderId::Local)
                }
                _ => Some(ProviderId::OpenAi),
            }
        }
        ProfileProvider::Gemini { .. } => Some(ProviderId::Gemini),
        ProfileProvider::Anthropic { .. } => Some(ProviderId::Anthropic),
        ProfileProvider::Copilot { .. } => Some(ProviderId::Copilot),
        ProfileProvider::Kiro { .. } => Some(ProviderId::Kiro),
        ProfileProvider::Agy { .. } => None,
    }
}

fn provider_display_name(provider: ProviderId) -> &'static str {
    match provider {
        ProviderId::OpenAi => "OpenAI / ChatGPT Codex",
        ProviderId::Gemini => "Google Gemini",
        ProviderId::Anthropic => "Anthropic Claude",
        ProviderId::Copilot => "GitHub Copilot",
        ProviderId::DeepSeek => "DeepSeek",
        ProviderId::Kiro => "Kiro CLI",
        ProviderId::Local => "Local OpenAI-compatible",
    }
}

fn provider_auth_summary(provider: ProviderId) -> &'static str {
    match provider {
        ProviderId::OpenAi => "ChatGPT login, device code, or API key profile",
        ProviderId::Gemini => "Google OAuth profile or GEMINI_API_KEY(S)",
        ProviderId::Anthropic => "Claude OAuth import or ANTHROPIC_API_KEY(S)",
        ProviderId::Copilot => "Copilot CLI import or GITHUB_COPILOT_API_KEY(S)",
        ProviderId::DeepSeek => "DEEPSEEK_API_KEY(S)",
        ProviderId::Kiro => "Kiro CLI import",
        ProviderId::Local => "Local base URL; API key optional",
    }
}

fn provider_default_model(provider: ProviderId) -> &'static str {
    match provider {
        ProviderId::OpenAi => "gpt-5.3-codex",
        ProviderId::Gemini => crate::SUPER_GEMINI_DEFAULT_MODEL,
        ProviderId::Anthropic => crate::SUPER_ANTHROPIC_DEFAULT_MODEL,
        ProviderId::Copilot => crate::SUPER_COPILOT_DEFAULT_MODEL,
        ProviderId::DeepSeek => crate::SUPER_DEEPSEEK_DEFAULT_MODEL,
        ProviderId::Kiro => crate::SUPER_KIRO_DEFAULT_MODEL,
        ProviderId::Local => crate::SUPER_DEFAULT_LOCAL_MODEL,
    }
}

fn provider_available_through(
    provider: ProviderId,
    configured_profiles: usize,
) -> Vec<&'static str> {
    let mut routes = Vec::new();
    if configured_profiles > 0 {
        routes.push("profile-backed routing");
    }
    match provider {
        ProviderId::OpenAi => {
            routes.push("gateway");
        }
        ProviderId::Gemini
        | ProviderId::Anthropic
        | ProviderId::Copilot
        | ProviderId::DeepSeek
        | ProviderId::Kiro => {
            routes.push("runtime provider launch");
            routes.push("gateway");
        }
        ProviderId::Local => {
            routes.push("local URL");
            routes.push("gateway");
        }
    }
    routes
}

fn provider_setup_commands(provider: ProviderId) -> Vec<&'static str> {
    match provider {
        ProviderId::OpenAi => vec![
            "prodex profile add openai-main --activate",
            "prodex login --profile openai-main",
            "prodex profile import-current openai-main",
        ],
        ProviderId::Gemini => vec![
            "prodex login --with-google",
            "GEMINI_API_KEY=... prodex s gemini --model auto",
        ],
        ProviderId::Anthropic => vec![
            "prodex login --with-claude",
            "prodex profile import claude --activate",
            "ANTHROPIC_API_KEY=... prodex s --provider anthropic --model claude-sonnet-4-6",
        ],
        ProviderId::Copilot => vec![
            "prodex profile import copilot --activate",
            "GITHUB_COPILOT_API_KEY=... prodex s --provider copilot --model gpt-5.3-codex",
        ],
        ProviderId::DeepSeek => {
            vec!["DEEPSEEK_API_KEY=... prodex s deepseek --model deepseek-v4-pro"]
        }
        ProviderId::Kiro => vec![
            "prodex profile import kiro --activate",
            "prodex s --provider kiro --model claude-sonnet-4",
        ],
        ProviderId::Local => {
            vec!["prodex super --url http://127.0.0.1:8131 --model unsloth/qwen3.5-35b-a3b"]
        }
    }
}

fn provider_launch_command(provider: ProviderId, model: Option<&str>) -> String {
    let model = model.unwrap_or_else(|| provider_default_model(provider));
    match provider {
        ProviderId::OpenAi => format!("prodex s -m {model}"),
        ProviderId::Gemini => format!("prodex s gemini --model {model}"),
        ProviderId::Anthropic => format!("prodex s --provider anthropic --model {model}"),
        ProviderId::Copilot => format!("prodex s --provider copilot --model {model}"),
        ProviderId::DeepSeek => format!("prodex s deepseek --model {model}"),
        ProviderId::Kiro => format!("prodex s --provider kiro --model {model}"),
        ProviderId::Local => format!("prodex super --url http://127.0.0.1:8131 --model {model}"),
    }
}

fn provider_quota_command(provider: ProviderId) -> &'static str {
    match provider {
        ProviderId::OpenAi => "prodex quota --all --provider openai --once",
        ProviderId::Gemini => "prodex quota --all --provider gemini --once",
        ProviderId::Anthropic => "prodex quota --all --provider anthropic --once",
        ProviderId::Copilot => "prodex quota --all --provider copilot --once",
        ProviderId::DeepSeek => "prodex quota --all --provider deepseek --once",
        ProviderId::Kiro => "prodex quota --all --provider kiro --once",
        ProviderId::Local => {
            "prodex quota --all --provider local --base-url http://127.0.0.1:8131/v1 --once"
        }
    }
}

fn provider_gateway_command(provider: ProviderId) -> &'static str {
    match provider {
        ProviderId::OpenAi => "prodex gateway",
        ProviderId::Gemini => "prodex gateway --provider gemini",
        ProviderId::Anthropic => "prodex gateway --provider anthropic",
        ProviderId::Copilot => "prodex gateway --provider copilot",
        ProviderId::DeepSeek => "prodex gateway --provider deepseek",
        ProviderId::Kiro => "prodex gateway --provider kiro",
        ProviderId::Local => "prodex gateway --base-url http://127.0.0.1:8131/v1",
    }
}

fn provider_notes(provider: ProviderId) -> &'static str {
    match provider {
        ProviderId::OpenAi => {
            "Prodex profile pool keeps quota-aware rotation and continuation affinity."
        }
        ProviderId::Gemini => {
            "OAuth profiles use Code Assist; API-key launches use the Gemini OpenAI-compatible endpoint."
        }
        ProviderId::Anthropic => {
            "Command generation only; dashboard does not store Anthropic secrets."
        }
        ProviderId::Copilot => {
            "Imported profiles keep Copilot credentials in Copilot-owned storage."
        }
        ProviderId::DeepSeek => {
            "API-key runtime bridge; no profile secret is stored by this dashboard."
        }
        ProviderId::Kiro => "Import snapshots Kiro CLI auth for Prodex routing.",
        ProviderId::Local => "Point Prodex at a local OpenAI-compatible /v1 server.",
    }
}

fn quota_summary(snapshot: &ProviderQuotaSnapshot) -> Value {
    match snapshot {
        ProviderQuotaSnapshot::OpenAi(usage) => {
            let blocked = crate::collect_blocked_limits(usage, false);
            json!({
                "account": usage.email,
                "plan": usage.plan_type,
                "status": if prodex_quota::openai_quota_has_ready_limit(usage) {
                    "Ready".to_string()
                } else {
                    format!("Blocked ({})", crate::format_blocked_limits(&blocked))
                },
                "main": format_main_windows(usage),
                "reset": prodex_quota::format_main_reset_summary(usage),
                "windows": {
                    "fiveHour": usage.rate_limit.as_ref().and_then(|rate| rate.primary_window.as_ref()).map(window_json),
                    "weekly": usage.rate_limit.as_ref().and_then(|rate| rate.secondary_window.as_ref()).map(window_json),
                }
            })
        }
        ProviderQuotaSnapshot::Copilot(info) => json!({
            "account": info.login,
            "plan": info.copilot_plan.as_ref().or(info.access_type_sku.as_ref()),
            "status": format_copilot_quota_status(info),
            "main": format_copilot_main_quota(info),
            "reset": format_copilot_reset_summary(info),
        }),
        ProviderQuotaSnapshot::Gemini(info) => json!({
            "account": info.email,
            "plan": info.plan,
            "project": info.project_id,
            "status": format_gemini_quota_status(info),
            "main": format_gemini_main_quota(info),
            "reset": format_gemini_reset_summary(info),
        }),
        ProviderQuotaSnapshot::External(info) => json!({
            "account": info.account,
            "plan": info.plan,
            "status": info.status,
            "main": info.main,
            "reset": info.reset,
            "details": info.details,
        }),
    }
}

fn window_json(window: &prodex_quota::UsageWindow) -> Value {
    json!({
        "usedPercent": window.used_percent,
        "remainingPercent": prodex_quota::remaining_percent(window.used_percent),
        "resetAt": window.reset_at,
        "windowSeconds": window.limit_window_seconds,
    })
}

fn read_json_body<T: for<'de> Deserialize<'de>>(request: &mut Request) -> Result<T> {
    let body =
        read_dashboard_json_body_limited(request.as_reader(), DASHBOARD_MAX_JSON_BODY_BYTES)?;
    serde_json::from_slice(&body).context("invalid JSON request body")
}

fn dashboard_json_body_error_status(err: &anyhow::Error) -> StatusCode {
    if err.downcast_ref::<DashboardJsonBodyTooLarge>().is_some() {
        StatusCode(413)
    } else {
        StatusCode(400)
    }
}

fn read_dashboard_json_body_limited(reader: impl Read, limit: usize) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    let mut reader = reader.take((limit as u64).saturating_add(1));
    reader
        .read_to_end(&mut body)
        .context("failed to read dashboard request body")?;
    if body.len() > limit {
        return Err(DashboardJsonBodyTooLarge { limit }.into());
    }
    Ok(body)
}

fn respond_json(request: Request, value: Value) -> Result<()> {
    let body = serde_json::to_vec(&value).context("failed to serialize dashboard JSON")?;
    respond_status(request, StatusCode(200), "application/json", body)
}

fn respond_json_result(request: Request, result: Result<Value>) -> Result<()> {
    match result {
        Ok(value) => respond_json(request, value),
        Err(err) => respond_error(request, StatusCode(500), err),
    }
}

fn respond_error(request: Request, status: StatusCode, err: anyhow::Error) -> Result<()> {
    respond_status(
        request,
        status,
        "application/json",
        serde_json::to_vec(&json!({ "error": err.to_string() }))
            .context("failed to serialize dashboard error")?,
    )
}

fn respond_html(request: Request, html: &str) -> Result<()> {
    respond_status(
        request,
        StatusCode(200),
        "text/html; charset=utf-8",
        html.as_bytes().to_vec(),
    )
}

fn respond_status(
    request: Request,
    status: StatusCode,
    content_type: &'static str,
    body: Vec<u8>,
) -> Result<()> {
    let content_type = Header::from_bytes("content-type", content_type)
        .map_err(|_| anyhow!("failed to build dashboard content-type header"))?;
    let cache_control = Header::from_bytes("cache-control", "no-store")
        .map_err(|_| anyhow!("failed to build dashboard cache-control header"))?;
    let response = Response::from_data(body)
        .with_status_code(status)
        .with_header(content_type)
        .with_header(cache_control);
    request
        .respond(response)
        .map_err(|err| anyhow!("failed to send dashboard response: {err}"))
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
        {
            output.push((high << 4) | low);
            index += 3;
            continue;
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_status_fields_contain_url_and_warning() {
        let fields = dashboard_status_fields(
            "http://127.0.0.1:8765",
            Some("dashboard has no password auth"),
        );

        assert!(fields.contains(&("URL".to_string(), "http://127.0.0.1:8765".to_string())));
        assert!(fields.contains(&(
            "Warning".to_string(),
            "dashboard has no password auth".to_string()
        )));
    }

    #[test]
    fn dashboard_json_body_limit_rejects_limit_plus_one() {
        let err = read_dashboard_json_body_limited(std::io::Cursor::new(vec![b'a'; 5]), 4)
            .expect_err("body above limit should be rejected");

        assert_eq!(dashboard_json_body_error_status(&err), StatusCode(413));
    }
}
