use crate::{AppPaths, print_launch_status};
use anyhow::{Context, Result, bail};
use prodex_presidio::{
    PresidioBlockingClient, PresidioHealth, ProdexPresidioRuntimeFileConfig,
    validate_presidio_file_config,
};
use std::{env, fs, path::PathBuf};

mod docker;
mod startup;

const PRODEX_PRESIDIO_FILE_NAME: &str = "presidio.toml";
const DEFAULT_PRESIDIO_ANALYZER_URL: &str = "http://localhost:5002";
const DEFAULT_PRESIDIO_ANONYMIZER_URL: &str = "http://localhost:5001";
const PRESIDIO_AUTO_START_ENV: &str = "PRODEX_PRESIDIO_AUTO_START";

type ProdexPresidioConfig = ProdexPresidioRuntimeFileConfig;

pub(crate) fn stored_presidio_preference() -> Result<Option<bool>> {
    let paths = AppPaths::discover()?;
    Ok(load_presidio_config(&paths)?.map(|config| config.enabled))
}

pub(crate) fn ensure_presidio_services_for_super_launch(paths: &AppPaths) -> Result<()> {
    ensure_presidio_services_for_super_launch_inner(paths, false)
}

pub(crate) fn ensure_required_presidio_services_for_super_launch(paths: &AppPaths) -> Result<()> {
    ensure_presidio_services_for_super_launch_inner(paths, true)
}

fn ensure_presidio_services_for_super_launch_inner(paths: &AppPaths, required: bool) -> Result<()> {
    let config = load_presidio_config(paths)?.unwrap_or_default();
    if required && !config.fail_mode.eq_ignore_ascii_case("closed") {
        bail!(
            "--require-tool presidio requires fail_mode = \"closed\" in {}",
            presidio_config_path(paths).display()
        );
    }
    let analyzer_url = &config.analyzer_url;
    let anonymizer_url = &config.anonymizer_url;
    print_launch_status(&format!(
        "Presidio redaction enabled; checking Analyzer={} Anonymizer={} ...",
        analyzer_url, anonymizer_url
    ));
    let client = PresidioBlockingClient::from_config(&config)?;
    let analyzer = client.probe_health(analyzer_url);
    let anonymizer = client.probe_health(anonymizer_url);
    if analyzer.ok && anonymizer.ok {
        print_launch_status("Presidio services are ready.");
        return Ok(());
    }

    if let Some((message, reason)) = startup::presidio_startup_blocker(analyzer_url, anonymizer_url)
    {
        if required {
            return required_presidio_services_error(&analyzer, &anonymizer, reason);
        }
        print_launch_status(message);
        return Ok(());
    }

    let changes = match startup::start_presidio_containers(&analyzer, &anonymizer) {
        Ok(changes) => changes,
        Err(error) => return startup::presidio_startup_failure(required, &config.fail_mode, error),
    };

    print_launch_status("waiting for Presidio services to become ready...");
    if startup::wait_for_presidio_services(&client, analyzer_url, anonymizer_url) {
        print_launch_status("Presidio services are ready.");
        return Ok(());
    }

    let cleanup_error = startup::rollback_presidio_containers(&changes).err();
    if required {
        let reason = cleanup_error
            .map(|error| {
                format!("services did not become healthy before launch; cleanup failed: {error}")
            })
            .unwrap_or_else(|| "services did not become healthy before launch".to_string());
        return required_presidio_services_error(&analyzer, &anonymizer, &reason);
    }
    if let Some(error) = cleanup_error {
        print_launch_status(&format!(
            "Presidio startup cleanup failed; continuing with runtime fail_mode={}: {error}",
            config.fail_mode
        ));
    }
    print_launch_status(&format!(
        "Presidio services did not become healthy before launch; continuing with runtime fail_mode={}",
        config.fail_mode
    ));
    Ok(())
}

fn required_presidio_services_error(
    analyzer: &PresidioHealth,
    anonymizer: &PresidioHealth,
    reason: &str,
) -> Result<()> {
    bail!(
        "required Presidio services are not ready: Analyzer {}; Anonymizer {}; {reason}",
        presidio_health_label(analyzer),
        presidio_health_label(anonymizer),
    )
}

fn presidio_auto_start_disabled() -> bool {
    env::var(PRESIDIO_AUTO_START_ENV)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "no" | "off"
            )
        })
        .unwrap_or(false)
}

fn presidio_health_label(health: &PresidioHealth) -> String {
    if health.ok {
        format!("ok ({})", health.message)
    } else {
        format!("failed ({})", health.message)
    }
}

fn load_presidio_config(paths: &AppPaths) -> Result<Option<ProdexPresidioConfig>> {
    let path = presidio_config_path(paths);
    if !path.exists() {
        return Ok(None);
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let config = toml::from_str::<ProdexPresidioConfig>(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    validate_presidio_file_config(&config)?;
    Ok(Some(config))
}

fn presidio_config_path(paths: &AppPaths) -> PathBuf {
    paths.root.join(PRODEX_PRESIDIO_FILE_NAME)
}
