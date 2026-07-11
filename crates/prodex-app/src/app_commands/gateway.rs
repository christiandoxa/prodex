use super::{
    GatewayArgs, GatewayCommands, GatewayProviderFilterArgs, GatewayProvidersArgs, Result,
};
use crate::profile_commands::{KIRO_MODEL_CATALOG_FILE, parse_kiro_model_catalog_text};
use crate::{AppPaths, AppState, AppStateIoExt, ProfileProvider};
use anyhow::{Context, anyhow};
use prodex_provider_core::{
    ProviderAdapterContract, ProviderAdapterContractSpec, ProviderId, provider_adapter,
    provider_adapter_contract_matrix, provider_model_catalog_json,
};
use std::collections::BTreeSet;
use std::fs;
use terminal_ui::print_stdout_line;

pub(crate) fn handle_gateway(args: GatewayArgs) -> Result<()> {
    match &args.command {
        Some(GatewayCommands::Providers(command)) => handle_gateway_providers(command),
        Some(GatewayCommands::Capabilities(command)) => handle_gateway_capabilities(command),
        Some(GatewayCommands::Models(command)) => handle_gateway_models(command),
        None => super::runtime_launch::handle_gateway(args),
    }
}

fn handle_gateway_providers(args: &GatewayProvidersArgs) -> Result<()> {
    let providers = provider_adapter_contract_matrix();
    if args.json {
        print_stdout_line(
            &serde_json::to_string_pretty(&providers)
                .context("failed to serialize provider contracts")?,
        );
        return Ok(());
    }
    for provider in providers {
        print_stdout_line(&format!(
            "{}: {}; models={}; endpoints={}",
            provider.provider,
            provider.transform_status,
            provider.model_count,
            provider.supported_endpoints.join(",")
        ));
    }
    Ok(())
}

fn handle_gateway_capabilities(args: &GatewayProviderFilterArgs) -> Result<()> {
    let provider = parse_gateway_provider(&args.provider)?;
    let spec = gateway_capabilities_spec(provider)?;
    if args.json {
        print_stdout_line(
            &serde_json::to_string_pretty(&spec)
                .context("failed to serialize provider capabilities")?,
        );
        return Ok(());
    }
    print_stdout_line(&format!(
        "{}: {}; streaming={}; fallback={}",
        spec.provider, spec.transform_status, spec.supports_streaming, spec.supports_model_fallback
    ));
    for endpoint in spec.endpoint_status {
        print_stdout_line(&format!(
            "  {}: {}; streaming={}; tested={}",
            endpoint.endpoint, endpoint.status, endpoint.streaming, endpoint.tested
        ));
    }
    Ok(())
}

fn gateway_capabilities_spec(provider: ProviderId) -> Result<ProviderAdapterContractSpec> {
    let mut spec = prodex_provider_core::provider_adapter_contract_spec(provider);
    if provider == ProviderId::Kiro {
        spec.model_count = gateway_kiro_model_catalog_json()?.len();
    }
    Ok(spec)
}

fn handle_gateway_models(args: &GatewayProviderFilterArgs) -> Result<()> {
    let provider = parse_gateway_provider(&args.provider)?;
    if provider == ProviderId::Kiro {
        return handle_gateway_kiro_models(args);
    }
    let models = provider_model_catalog_json(provider);
    if args.json {
        print_stdout_line(
            &serde_json::to_string_pretty(&models)
                .context("failed to serialize provider model catalog")?,
        );
        return Ok(());
    }
    let adapter = provider_adapter(provider);
    for model in adapter.model_catalog() {
        print_stdout_line(&format!(
            "{}: {} ({})",
            model.id,
            model.display_name,
            model
                .endpoints
                .iter()
                .map(|endpoint| endpoint.label())
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    Ok(())
}

fn handle_gateway_kiro_models(args: &GatewayProviderFilterArgs) -> Result<()> {
    let models = gateway_kiro_model_catalog_json()?;
    if args.json {
        print_stdout_line(
            &serde_json::to_string_pretty(&models)
                .context("failed to serialize Kiro model catalog")?,
        );
        return Ok(());
    }
    for model in &models {
        let id = model
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let name = model
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(id);
        print_stdout_line(&format!("{id}: {name} (imported kiro profile snapshot)"));
    }
    Ok(())
}

fn gateway_kiro_model_catalog_json() -> Result<Vec<serde_json::Value>> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let mut seen = BTreeSet::new();
    let mut models = Vec::new();
    for profile in state.profiles.values() {
        if !matches!(profile.provider, ProfileProvider::Kiro { .. }) {
            continue;
        }
        let path = profile.codex_home.join(KIRO_MODEL_CATALOG_FILE);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(catalog) = parse_kiro_model_catalog_text(&text) else {
            continue;
        };
        for model in catalog {
            let Some(id) = model.get("id").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let id = id.trim();
            if id.is_empty() || !seen.insert(id.to_ascii_lowercase()) {
                continue;
            }
            models.push(model);
        }
    }
    Ok(models)
}

fn parse_gateway_provider(value: &str) -> Result<ProviderId> {
    ProviderId::parse(value).ok_or_else(|| {
        anyhow!(
            "unknown provider '{}'; expected openai, anthropic, copilot, deepseek, gemini, kiro, or local",
            value
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TestEnvLockGuard, acquire_test_env_lock, create_codex_home_if_missing};
    use std::collections::BTreeMap;
    use std::env;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct EnvGuard {
        _lock: TestEnvLockGuard,
        previous_home: Option<std::ffi::OsString>,
        previous_xdg: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(root: &std::path::Path) -> Self {
            let lock = acquire_test_env_lock();
            let previous_home = env::var_os("HOME");
            let previous_xdg = env::var_os("XDG_CONFIG_HOME");
            unsafe {
                env::set_var("HOME", root.join("home"));
                env::set_var("XDG_CONFIG_HOME", root.join("config"));
            }
            Self {
                _lock: lock,
                previous_home,
                previous_xdg,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.previous_home.take() {
                Some(value) => unsafe { env::set_var("HOME", value) },
                None => unsafe { env::remove_var("HOME") },
            }
            match self.previous_xdg.take() {
                Some(value) => unsafe { env::set_var("XDG_CONFIG_HOME", value) },
                None => unsafe { env::remove_var("XDG_CONFIG_HOME") },
            }
        }
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos();
        let dir = env::temp_dir().join(format!(
            "prodex-gateway-kiro-{name}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("temp dir should exist");
        dir
    }

    #[test]
    fn parse_gateway_provider_accepts_kiro() {
        assert_eq!(
            parse_gateway_provider("kiro").expect("kiro should parse"),
            ProviderId::Kiro
        );
    }

    #[test]
    fn gateway_kiro_model_catalog_json_merges_imported_profile_snapshots() {
        let _guard = acquire_test_env_lock();
        let root = temp_dir("catalog");
        let _env = EnvGuard::set(&root);

        let paths = AppPaths::discover().expect("paths should resolve");
        let first_home = paths.managed_profiles_root.join("kiro-a");
        let second_home = paths.managed_profiles_root.join("kiro-b");
        create_codex_home_if_missing(&first_home).expect("first home should exist");
        create_codex_home_if_missing(&second_home).expect("second home should exist");
        fs::write(
            first_home.join(KIRO_MODEL_CATALOG_FILE),
            serde_json::json!({
                "models": [
                    { "id": "claude-sonnet-4", "name": "claude-sonnet-4", "owned_by": "kiro-cli" }
                ]
            })
            .to_string(),
        )
        .expect("first catalog should be written");
        fs::write(
            second_home.join(KIRO_MODEL_CATALOG_FILE),
            serde_json::json!({
                "models": [
                    { "id": "claude-sonnet-4", "name": "claude-sonnet-4", "owned_by": "kiro-cli" },
                    { "id": "claude-sonnet-4.5", "name": "claude-sonnet-4.5", "owned_by": "kiro-cli" }
                ]
            })
            .to_string(),
        )
        .expect("second catalog should be written");
        AppState {
            active_profile: Some("kiro-a".to_string()),
            profiles: BTreeMap::from([
                (
                    "kiro-a".to_string(),
                    crate::ProfileEntry {
                        codex_home: first_home,
                        managed: true,
                        email: Some("a@example.com".to_string()),
                        provider: ProfileProvider::Kiro {
                            auth_key: "key-a".to_string(),
                            auth_kind: Some("builder-id".to_string()),
                            profile_arn: None,
                            profile_name: None,
                            start_url: None,
                            region: None,
                        },
                    },
                ),
                (
                    "kiro-b".to_string(),
                    crate::ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("b@example.com".to_string()),
                        provider: ProfileProvider::Kiro {
                            auth_key: "key-b".to_string(),
                            auth_kind: Some("builder-id".to_string()),
                            profile_arn: None,
                            profile_name: None,
                            start_url: None,
                            region: None,
                        },
                    },
                ),
            ]),
            ..AppState::default()
        }
        .save(&paths)
        .expect("state should save");

        let models = gateway_kiro_model_catalog_json().expect("catalog should load");
        assert_eq!(models.len(), 2);
        assert_eq!(models[0]["id"], "claude-sonnet-4");
        assert_eq!(models[1]["id"], "claude-sonnet-4.5");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_kiro_capabilities_json_reports_runtime_surface() {
        let _guard = acquire_test_env_lock();
        let root = temp_dir("capabilities");
        let _env = EnvGuard::set(&root);

        let paths = AppPaths::discover().expect("paths should resolve");
        let kiro_home = paths.managed_profiles_root.join("kiro-cap");
        create_codex_home_if_missing(&kiro_home).expect("kiro home should exist");
        fs::write(
            kiro_home.join(KIRO_MODEL_CATALOG_FILE),
            serde_json::json!({
                "models": [
                    { "id": "claude-sonnet-4", "name": "claude-sonnet-4", "owned_by": "kiro-cli" }
                ]
            })
            .to_string(),
        )
        .expect("catalog should be written");
        AppState {
            active_profile: Some("kiro-cap".to_string()),
            profiles: BTreeMap::from([(
                "kiro-cap".to_string(),
                crate::ProfileEntry {
                    codex_home: kiro_home,
                    managed: true,
                    email: Some("kiro@example.com".to_string()),
                    provider: ProfileProvider::Kiro {
                        auth_key: "key-cap".to_string(),
                        auth_kind: Some("builder-id".to_string()),
                        profile_arn: None,
                        profile_name: None,
                        start_url: None,
                        region: None,
                    },
                },
            )]),
            ..Default::default()
        }
        .save(&paths)
        .expect("state should save");

        let spec = gateway_capabilities_spec(ProviderId::Kiro).expect("capabilities should build");
        let spec = serde_json::to_value(spec).expect("spec should serialize");
        assert_eq!(spec["provider"], "kiro");
        assert_eq!(spec["supports_streaming"], true);
        assert_eq!(spec["transform_status"], "translated");
        assert_eq!(spec["model_count"], 1);
        assert_eq!(spec["endpoint_status"][2]["endpoint"], "chat-completions");
        assert_eq!(spec["endpoint_status"][2]["streaming"], true);
        assert_eq!(spec["endpoint_status"][2]["tested"], true);
        assert!(
            spec["endpoint_status"][2]["unsupported_params"]
                .as_array()
                .expect("unsupported_params should be an array")
                .iter()
                .any(|value| value == "temperature")
        );
        assert!(
            spec["endpoint_status"][2]["unsupported_params"]
                .as_array()
                .expect("unsupported_params should be an array")
                .iter()
                .any(|value| value == "parallel_tool_calls")
        );
        assert!(
            spec["endpoint_status"][2]["unsupported_params"]
                .as_array()
                .expect("unsupported_params should be an array")
                .iter()
                .any(|value| value == "user")
        );
        assert!(
            spec["endpoint_status"][2]["unsupported_params"]
                .as_array()
                .expect("unsupported_params should be an array")
                .iter()
                .any(|value| value == "max_output_tokens/max_tokens/max_completion_tokens")
        );
        assert_eq!(spec["endpoint_status"][1]["endpoint"], "responses/compact");
        assert_eq!(spec["endpoint_status"][1]["status"], "emulated");
        assert_eq!(spec["endpoint_status"][1]["tested"], true);
        assert!(
            spec["supported_endpoints"]
                .as_array()
                .expect("supported_endpoints should be an array")
                .iter()
                .any(|value| value == "responses")
        );
        assert!(
            spec["supported_endpoints"]
                .as_array()
                .expect("supported_endpoints should be an array")
                .iter()
                .any(|value| value == "chat-completions")
        );
    }

    #[test]
    fn gateway_copilot_capabilities_report_compact_surface() {
        let spec = prodex_provider_core::provider_adapter_contract_spec(ProviderId::Copilot);
        assert!(spec.supported_endpoints.contains(&"responses/compact"));
        let compact = spec
            .endpoint_status
            .iter()
            .find(|endpoint| endpoint.endpoint == "responses/compact")
            .expect("copilot compact endpoint should be reported");
        assert_eq!(compact.status, "passthrough");
        assert!(!compact.streaming);
        assert!(compact.tested);
    }
}
