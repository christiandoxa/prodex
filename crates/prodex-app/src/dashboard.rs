use anyhow::Result;
use prodex_provider_core::{provider_adapter_contract_matrix, provider_model_catalog_json};
use serde_json::{Value, json};
use tiny_http::{Method, Request, StatusCode};

mod mutations;
mod payloads;
mod providers;
mod server;

use payloads::quota_summary;
use providers::*;
pub(crate) use server::serve_dashboard;
#[cfg(test)]
use server::{
    dashboard_json_body_error_status, dashboard_status_fields, read_dashboard_json_body_limited,
};
use server::{respond_html, respond_json, respond_json_result, respond_status};

use crate::dashboard_html::DASHBOARD_HTML;
use crate::{
    AppPaths, AppState, AppStateIoExt, QuotaProviderFilter, collect_profile_summaries,
    collect_quota_reports_with_filters,
};

#[derive(Debug)]
struct DashboardServer {
    paths: AppPaths,
    base_url: Option<String>,
}

impl DashboardServer {
    fn handle(&self, request: Request) -> Result<()> {
        let method = request.method().clone();
        let path = request.url().split('?').next().unwrap_or("/").to_string();
        match (method, path.as_str()) {
            (Method::Get, "/") | (Method::Get, "/dashboard") => {
                respond_html(request, DASHBOARD_HTML)
            }
            (Method::Get, "/healthz") => respond_json(
                request,
                json!({
                    "status": "ok",
                    "service": "prodex-dashboard",
                    "version": env!("CARGO_PKG_VERSION"),
                }),
            ),
            (Method::Get, "/third-party-notices") => respond_status(
                request,
                StatusCode(200),
                "text/plain; charset=utf-8",
                include_str!("../../../THIRD_PARTY_NOTICES.md")
                    .as_bytes()
                    .to_vec(),
            ),
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
            (Method::Get, "/api/logs") => respond_json_result(request, self.logs_json()),
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
            "providers": provider_presets(&state)?,
            "contracts": provider_adapter_contract_matrix(),
        }))
    }

    fn provider_presets_json(&self) -> Result<Value> {
        let state = AppState::load(&self.paths)?;
        Ok(json!({ "providers": provider_presets(&state)? }))
    }

    fn models_json(&self) -> Result<Value> {
        let state = AppState::load(&self.paths)?;
        let mut models = Vec::new();
        for provider in DASHBOARD_PROVIDER_IDS {
            let default_model = provider_default_model(*provider);
            let availability =
                provider_available_through(*provider, provider_profile_count(&state, *provider)?);
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
        Ok(json!({ "models": models, "providers": provider_presets(&state)? }))
    }
}

#[cfg(test)]
#[path = "dashboard/lifecycle_tests.rs"]
mod lifecycle_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProfileEntry, ProfileProvider};
    use prodex_provider_core::ProviderId;
    use std::collections::{BTreeSet, HashMap};
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    use std::ffi::OsString;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    pub(super) fn dashboard_json_request(
        dashboard: &DashboardServer,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> (u16, Value) {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("test server should bind");
        let address = server.server_addr().to_ip().expect("address should be IP");
        let url = format!("http://{address}{path}");
        let client = std::thread::spawn(move || {
            let request = reqwest::blocking::Client::new().request(method, url);
            let response = match body {
                Some(body) => request.json(&body),
                None => request,
            }
            .send()
            .expect("dashboard request should complete");
            let status = response.status().as_u16();
            let body = response.json().expect("dashboard response should be JSON");
            (status, body)
        });
        let request = server.recv().expect("dashboard request should arrive");
        dashboard.handle(request).expect("dashboard should respond");
        client.join().expect("dashboard client should finish")
    }

    #[test]
    fn dashboard_profile_mutations_succeed_when_local_audit_persistence_fails() {
        let paths = dashboard_test_paths("audit-best-effort");
        let dashboard = DashboardServer {
            paths: paths.clone(),
            base_url: None,
        };
        let audit_blocker = paths.root.join("audit-blocker");
        fs::write(&audit_blocker, "blocked").expect("audit blocker should be written");
        let _audit_guard = crate::TestEnvVarGuard::set(
            "PRODEX_AUDIT_LOG_DIR",
            &audit_blocker.display().to_string(),
        );

        for name in ["main", "second"] {
            let (status, _) = dashboard_json_request(
                &dashboard,
                reqwest::Method::POST,
                "/api/profile",
                Some(json!({ "name": name, "activate": name == "main" })),
            );
            assert_eq!(status, 200, "profile add should report success");
        }
        let (status, _) = dashboard_json_request(
            &dashboard,
            reqwest::Method::POST,
            "/api/profile/active",
            Some(json!({ "profile": "second" })),
        );
        assert_eq!(status, 200, "profile activation should report success");
        let (status, _) = dashboard_json_request(
            &dashboard,
            reqwest::Method::DELETE,
            "/api/profile/second",
            None,
        );
        assert_eq!(status, 200, "profile removal should report success");

        let state = AppState::load(&paths).expect("committed state should load");
        assert!(state.profiles.contains_key("main"));
        assert!(!state.profiles.contains_key("second"));
        assert_eq!(state.active_profile.as_deref(), Some("main"));
        fs::remove_dir_all(paths.root).expect("test root should be removed");
    }

    #[test]
    fn dashboard_status_fields_contain_url_and_warning() {
        let fields = dashboard_status_fields(
            "http://127.0.0.1:8765",
            Some("port 8765 was unavailable; using an OS-assigned port"),
        );

        assert!(fields.contains(&("URL".to_string(), "http://127.0.0.1:8765".to_string())));
        assert!(fields.contains(&(
            "Warning".to_string(),
            "port 8765 was unavailable; using an OS-assigned port".to_string()
        )));
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    #[test]
    fn dashboard_browser_uses_xdg_open_on_linux_and_unix() {
        let (program, args) = server::dashboard_browser_command("http://127.0.0.1:8765");

        assert_eq!(program, "xdg-open");
        assert_eq!(args, vec![OsString::from("http://127.0.0.1:8765")]);
    }

    #[test]
    fn dashboard_state_json_works_with_empty_state() {
        let paths = dashboard_test_paths("state-json-empty-state");
        let dashboard = DashboardServer {
            paths: paths.clone(),
            base_url: None,
        };

        let value = dashboard.state_json().expect("state json should build");
        assert_eq!(value["activeProfile"], Value::Null);
        assert_eq!(value["profileCount"], 0);
        assert_eq!(
            value["paths"]["stateFile"],
            paths.state_file.display().to_string()
        );
        assert_eq!(
            value["paths"]["managedProfilesRoot"],
            paths.managed_profiles_root.display().to_string()
        );

        let commands = &value["commands"];
        assert_eq!(commands["open"], "prodex dashboard");
        assert!(commands["quota"].as_str().is_some());
    }

    #[test]
    fn dashboard_provider_endpoints_cover_supported_providers() {
        let paths = dashboard_test_paths("provider-endpoints");
        let server = DashboardServer {
            paths: paths.clone(),
            base_url: None,
        };

        write_test_state(&paths, sample_dashboard_state(&paths));

        let providers = server
            .providers_json()
            .expect("providers endpoint should build");
        let preset_values = providers["providers"]
            .as_array()
            .expect("providers list should be array");
        assert_eq!(preset_values.len(), DASHBOARD_PROVIDER_IDS.len());

        let mut seen_ids = BTreeSet::new();
        for value in preset_values {
            let id = value["id"].as_str().expect("provider id should be present");
            assert!(
                DASHBOARD_PROVIDER_IDS
                    .iter()
                    .any(|provider| provider.label() == id)
            );
            assert!(
                seen_ids.insert(id.to_string()),
                "duplicate provider id in providers payload: {id}"
            );

            assert!(value["commands"]["setup"].is_array());
            assert!(!value["commands"]["setup"].as_array().unwrap().is_empty());
            assert!(value["commands"]["launch"].as_str().is_some_and(|value| {
                value.starts_with("prodex s") || value.starts_with("prodex super")
            }));
        }

        let contracts = providers["contracts"]
            .as_array()
            .expect("contracts should be array");
        assert!(
            !contracts.is_empty(),
            "provider contract matrix should be present"
        );
    }

    #[test]
    fn dashboard_models_json_exposes_recommended_and_launch_commands() {
        let paths = dashboard_test_paths("models-endpoint");
        let mut state = sample_dashboard_state(&paths);
        state.active_profile = Some("main-openai".to_string());
        write_test_state(&paths, state);
        let dashboard = DashboardServer {
            paths: paths.clone(),
            base_url: None,
        };

        let models = dashboard
            .models_json()
            .expect("models endpoint should build");
        let model_values = models["models"].as_array().expect("models should be array");
        assert!(!model_values.is_empty());

        let mut by_provider = HashMap::new();
        for value in model_values {
            let provider_name = value["providerName"]
                .as_str()
                .expect("providerName should be present");
            let by = by_provider
                .entry(provider_name.to_string())
                .or_insert(0usize);
            *by += 1;

            assert!(value["launchCommand"].as_str().is_some_and(|value| {
                value.contains("prodex s") || value.contains("prodex super")
            }));

            let available_through = value["availableThrough"]
                .as_array()
                .expect("availableThrough should be array");
            assert!(
                !available_through.is_empty(),
                "each model should expose routing availability"
            );

            if value["recommended"].as_bool().unwrap_or(false) {
                assert!(value["default"].as_bool().unwrap_or(false));
            }
        }

        for provider in DASHBOARD_PROVIDER_IDS {
            let provider_name = provider_display_name(*provider);
            let expected_default = provider_default_model(*provider);
            let provider_models: Vec<&Value> = model_values
                .iter()
                .filter(|value| value["providerName"] == provider_name)
                .collect();
            if provider_models.is_empty() {
                continue;
            }

            let has_default_model = provider_models.iter().any(|value| {
                value
                    .get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|value| value == expected_default)
            });
            if *provider != ProviderId::Local {
                assert!(
                    has_default_model,
                    "provider default model should be present: {provider_name}"
                );
            }
            assert_eq!(
                by_provider.get(provider_name).copied(),
                Some(provider_models.len())
            );
        }
    }

    #[test]
    fn dashboard_usage_endpoint_empty_state_is_redacted() {
        let paths = dashboard_test_paths("usage-empty-state");
        let dashboard = DashboardServer {
            paths: paths.clone(),
            base_url: None,
        };

        let value = dashboard.usage_json().expect("usage endpoint should build");
        let summary = &value["summary"];
        assert_eq!(summary["total"], 0);
        assert_eq!(summary["ready"], 0);

        let rendered = serde_json::to_string(&value).expect("render json");
        assert!(!rendered.contains("Bearer "));
        assert!(!rendered.contains("Authorization:"));
        assert!(!rendered.contains("api_key"));
        assert!(!rendered.contains("secret"));
    }

    #[test]
    fn dashboard_provider_presets_has_setup_and_launch_commands() {
        let paths = dashboard_test_paths("provider-presets");
        write_test_state(&paths, sample_dashboard_state(&paths));
        let dashboard = DashboardServer {
            paths: paths.clone(),
            base_url: None,
        };

        let presets = dashboard
            .provider_presets_json()
            .expect("provider presets endpoint should build");
        let preset_values = presets["providers"]
            .as_array()
            .expect("provider presets should be array");
        assert_eq!(preset_values.len(), DASHBOARD_PROVIDER_IDS.len());

        for value in preset_values {
            let commands = value["commands"]
                .as_object()
                .expect("commands should be object");
            let setup = commands["setup"].as_array().expect("setup should be array");
            assert!(!setup.is_empty(), "setup commands should exist");
            for command in setup {
                let value = command.as_str().expect("setup command should be string");
                assert!(!contains_secret_marker(value));
            }

            let launch = commands["launch"]
                .as_str()
                .expect("launch should be string");
            assert!(launch.contains("prodex s") || launch.contains("prodex super"));
            assert!(!contains_secret_marker(launch));
        }

        let gemini = preset_values
            .iter()
            .find(|value| value["id"] == "gemini")
            .expect("Gemini preset should be present");
        assert!(
            gemini["auth"]
                .as_str()
                .is_some_and(|value| value.contains("GEMINI_API_KEY"))
        );
        assert!(
            gemini["notes"]
                .as_str()
                .is_some_and(|value| value.contains("OAuth profiles are disabled"))
        );
        assert!(
            !gemini["availableThrough"]
                .as_array()
                .expect("Gemini availability should be an array")
                .iter()
                .any(|value| value == "profile-backed routing")
        );
    }

    #[test]
    fn dashboard_runtime_status_payload_is_non_secret() {
        let paths = dashboard_test_paths("runtime-status");
        let dashboard = DashboardServer {
            paths: paths.clone(),
            base_url: None,
        };

        let status = dashboard
            .runtime_status_json()
            .expect("runtime status endpoint should build");

        let gateway = &status["gateway"];
        let runtime = &status["runtime"];
        assert!(gateway["startCommand"].as_str().is_some());
        assert!(gateway["providersCommand"].as_str().is_some());
        assert!(runtime["logDir"].as_str().is_some());
        assert!(runtime["latestLogPointer"].as_str().is_some());

        assert!(!contains_secret_marker(
            &serde_json::to_string(&status).expect("serialize status")
        ));
    }

    #[test]
    fn dashboard_models_payload_has_recommended_model_per_provider_where_available() {
        let paths = dashboard_test_paths("provider-models");
        let state = sample_dashboard_state(&paths);
        write_test_state(&paths, state);
        let dashboard = DashboardServer {
            paths: paths.clone(),
            base_url: None,
        };

        let value = dashboard
            .models_json()
            .expect("models endpoint should build");
        let models = value["models"].as_array().expect("models should be array");
        let mut has_recommended = 0usize;

        for model in models {
            if model["recommended"].as_bool().unwrap_or(false) {
                has_recommended += 1;
            }
            assert!(
                model["availableThrough"]
                    .as_array()
                    .is_some_and(|paths| !paths.is_empty())
            );
            assert!(!contains_secret_marker(
                &serde_json::to_string(model).expect("serialize model payload")
            ));
        }

        assert!(
            has_recommended > 0,
            "recommended model markers should exist where model catalog exists"
        );
    }

    #[test]
    fn dashboard_handles_empty_state_endpoints() {
        let paths = dashboard_test_paths("empty-endpoints");
        let dashboard = DashboardServer {
            paths: paths.clone(),
            base_url: None,
        };

        for value in [
            dashboard.state_json().expect("state should render"),
            dashboard.providers_json().expect("providers should render"),
            dashboard
                .provider_presets_json()
                .expect("preset should render"),
            dashboard.models_json().expect("models should render"),
            dashboard.usage_json().expect("usage should render"),
            dashboard
                .runtime_status_json()
                .expect("runtime status should render"),
        ] {
            assert!(!contains_secret_marker(
                &serde_json::to_string(&value).expect("serialize value")
            ));
        }
    }

    #[test]
    fn dashboard_json_body_limit_rejects_limit_plus_one() {
        let err = read_dashboard_json_body_limited(std::io::Cursor::new(vec![b'a'; 5]), 4)
            .expect_err("body above limit should be rejected");

        assert_eq!(dashboard_json_body_error_status(&err), StatusCode(413));
    }

    fn contains_secret_marker(value: &str) -> bool {
        const SECRET_MARKERS: [&str; 6] = [
            "Bearer ",
            "api_key",
            "\"Authorization\"",
            "access_token",
            "refresh_token",
            "\"refresh\"",
        ];
        SECRET_MARKERS.iter().any(|marker| value.contains(marker))
    }

    pub(super) fn dashboard_test_paths(name: &str) -> AppPaths {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be available")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "prodex-dashboard-tests-{name}-{stamp}-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        fs::create_dir_all(root.join("profiles")).expect("profiles directory should be created");
        AppPaths {
            root: root.clone(),
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("shared"),
            legacy_shared_codex_root: root.join("legacy"),
        }
    }

    fn sample_dashboard_state(paths: &AppPaths) -> AppState {
        AppState {
            active_profile: Some("main-openai".to_string()),
            profiles: [
                (
                    "main-openai".to_string(),
                    ProfileEntry {
                        codex_home: paths.managed_profiles_root.join("main-openai"),
                        managed: true,
                        email: Some("openai@example".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                ),
                (
                    "main-gemini".to_string(),
                    ProfileEntry {
                        codex_home: paths.managed_profiles_root.join("main-gemini"),
                        managed: true,
                        email: Some("gemini@example".to_string()),
                        provider: ProfileProvider::Gemini {
                            email: "gemini@example".to_string(),
                            project_id: None,
                        },
                    },
                ),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        }
    }

    fn write_test_state(paths: &AppPaths, state: AppState) {
        state.save(paths).expect("state should be written");
    }
}
