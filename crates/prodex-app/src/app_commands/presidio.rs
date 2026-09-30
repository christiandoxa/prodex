use crate::{AppPaths, print_launch_status};
use anyhow::{Context, Result, bail};
use prodex_mojo_core::rich::ascii_casefold_equal_exact;
use prodex_mojo_core::super_provider_config::runtime_bool_token;
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

pub(crate) fn presidio_expose_health() -> (bool, String) {
    let result = (|| -> Result<bool> {
        let paths = AppPaths::discover()?;
        let mut config = load_presidio_config(&paths)?.unwrap_or_default();
        config.timeout_ms = config.timeout_ms.clamp(100, 1_000);
        let client = PresidioBlockingClient::from_config(&config)?;
        let analyzer = client.probe_health(&config.analyzer_url);
        let anonymizer = client.probe_health(&config.anonymizer_url);
        Ok(analyzer.ok && anonymizer.ok)
    })();
    match result {
        Ok(true) => (
            true,
            "Presidio Analyzer and Anonymizer are healthy".to_string(),
        ),
        Ok(false) => (
            false,
            "Presidio Analyzer or Anonymizer is not healthy".to_string(),
        ),
        Err(_) => (false, "Presidio health could not be validated".to_string()),
    }
}

pub(crate) fn ensure_presidio_services_for_super_launch(paths: &AppPaths) -> Result<()> {
    ensure_presidio_services_for_super_launch_inner(paths, false)
}

pub(crate) fn ensure_required_presidio_services_for_super_launch(paths: &AppPaths) -> Result<()> {
    ensure_presidio_services_for_super_launch_inner(paths, true)
}

fn ensure_presidio_services_for_super_launch_inner(paths: &AppPaths, required: bool) -> Result<()> {
    let config = load_presidio_config(paths)?.unwrap_or_default();
    if required
        && !ascii_casefold_equal_exact(&config.fail_mode, "closed")
            .expect("Mojo Presidio fail-mode comparison failed")
    {
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
        .is_some_and(|value| presidio_auto_start_disabled_value(&value))
}

fn presidio_auto_start_disabled_value(value: &str) -> bool {
    runtime_bool_token(value.trim())
        .expect("runtime boolean token classification should accept Rust strings")
        == Some(false)
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

#[cfg(test)]
mod tests {
    use super::presidio_auto_start_disabled_value;

    #[test]
    fn presidio_auto_start_disabled_value_uses_mojo_boolean_tokens() {
        for value in ["0", "FALSE", " no ", "\u{2003}OFF\u{2003}"] {
            assert!(presidio_auto_start_disabled_value(value));
        }
        for value in ["1", "true", "YES", "on", "", "unknown"] {
            assert!(!presidio_auto_start_disabled_value(value));
        }
    }
}
