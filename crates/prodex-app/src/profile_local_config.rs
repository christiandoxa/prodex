use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const PRODEX_OPENAI_COMPAT_PROVIDER_ID: &str = "prodex-openai-compatible";

const PRODEX_PROFILE_CONFIG_FILE: &str = ".prodex-profile.toml";

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct ProdexProfileLocalConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    openai_compatible_base_url: Option<String>,
}

pub(crate) fn profile_local_config_path(codex_home: &Path) -> PathBuf {
    codex_home.join(PRODEX_PROFILE_CONFIG_FILE)
}

pub(crate) fn read_profile_openai_compatible_base_url(codex_home: &Path) -> Option<String> {
    let config_path = profile_local_config_path(codex_home);
    let contents = fs::read_to_string(config_path).ok()?;
    let config: ProdexProfileLocalConfig = toml::from_str(&contents).ok()?;
    config
        .openai_compatible_base_url
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn write_profile_openai_compatible_base_url(
    codex_home: &Path,
    base_url: Option<&str>,
) -> Result<()> {
    let config_path = profile_local_config_path(codex_home);
    let base_url = base_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    if base_url.is_none() {
        match fs::remove_file(&config_path) {
            Ok(()) => return Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("failed to remove {}", config_path.display()));
            }
        }
    }

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let config = ProdexProfileLocalConfig {
        openai_compatible_base_url: base_url,
    };
    let contents = toml::to_string_pretty(&config).context("failed to serialize profile config")?;
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(&secret_store::SecretLocation::file(&config_path), contents)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("failed to write {}", config_path.display()))
}

pub(crate) fn profile_openai_compatible_codex_args(
    codex_home: &Path,
    user_args: &[OsString],
) -> Vec<OsString> {
    if codex_config::codex_cli_config_override_value(user_args, "model_provider").is_some() {
        return user_args.to_vec();
    }

    let Some(base_url) = read_profile_openai_compatible_base_url(codex_home) else {
        return user_args.to_vec();
    };

    let overrides = [
        format!(
            "model_provider={}",
            toml_string_literal(PRODEX_OPENAI_COMPAT_PROVIDER_ID)
        ),
        format!(
            "model_providers.{PRODEX_OPENAI_COMPAT_PROVIDER_ID}.name={}",
            toml_string_literal("OpenAI-compatible")
        ),
        format!(
            "model_providers.{PRODEX_OPENAI_COMPAT_PROVIDER_ID}.base_url={}",
            toml_string_literal(&base_url)
        ),
        format!("model_providers.{PRODEX_OPENAI_COMPAT_PROVIDER_ID}.wire_api=\"responses\""),
        format!("model_providers.{PRODEX_OPENAI_COMPAT_PROVIDER_ID}.requires_openai_auth=true"),
        format!("model_providers.{PRODEX_OPENAI_COMPAT_PROVIDER_ID}.supports_websockets=false"),
    ];

    let mut args = Vec::with_capacity(user_args.len() + overrides.len() * 2);
    for override_entry in overrides {
        args.push(OsString::from("-c"));
        args.push(OsString::from(override_entry));
    }
    args.extend(user_args.iter().cloned());
    args
}

fn toml_string_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "prodex-profile-local-config-{name}-{}-{nanos}",
            std::process::id()
        ))
    }

    #[cfg(unix)]
    #[test]
    fn write_profile_config_replaces_symlink_without_touching_target() {
        let root = temp_dir("symlink");
        fs::create_dir_all(&root).unwrap();
        let target = root.join("target.toml");
        let config_path = profile_local_config_path(&root);
        fs::write(&target, "do_not_touch = true\n").unwrap();
        std::os::unix::fs::symlink(&target, &config_path).unwrap();

        write_profile_openai_compatible_base_url(&root, Some("http://127.0.0.1:11434")).unwrap();

        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            "do_not_touch = true\n"
        );
        assert!(
            !fs::symlink_metadata(&config_path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            read_profile_openai_compatible_base_url(&root).as_deref(),
            Some("http://127.0.0.1:11434")
        );
        let _ = fs::remove_dir_all(root);
    }
}
