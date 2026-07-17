use anyhow::{Context, Result};
use dirs::{data_local_dir, home_dir};
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub(super) fn discover_kiro_database_path() -> Result<PathBuf> {
    for variable in ["KIRO_DATA_DIR", "Q_CLI_DATA_DIR"] {
        if let Some(path) = env::var_os(variable) {
            let candidate = PathBuf::from(path).join("data.sqlite3");
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    let mut candidates = Vec::new();
    if let Some(data_dir) = data_local_dir() {
        candidates.push(data_dir.join("kiro-cli").join("data.sqlite3"));
        candidates.push(data_dir.join("amazon-q").join("data.sqlite3"));
    }
    if let Some(home) = home_dir() {
        for name in ["kiro-cli", "amazon-q"] {
            candidates.push(
                home.join(".local")
                    .join("share")
                    .join(name)
                    .join("data.sqlite3"),
            );
        }
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .context("failed to find Kiro auth database; expected ~/.local/share/kiro-cli/data.sqlite3 or ~/.local/share/amazon-q/data.sqlite3")
}

pub(crate) fn kiro_cli_data_dir_env(data_dir: &Path) -> Vec<(OsString, OsString)> {
    let value = data_dir.as_os_str().to_os_string();
    vec![
        (OsString::from("KIRO_DATA_DIR"), value.clone()),
        (OsString::from("Q_CLI_DATA_DIR"), value),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_env_supports_current_and_legacy_clients() {
        let env = kiro_cli_data_dir_env(Path::new("/tmp/kiro-data"));
        assert_eq!(env[0].0, "KIRO_DATA_DIR");
        assert_eq!(env[1].0, "Q_CLI_DATA_DIR");
        assert_eq!(env[0].1, env[1].1);
    }
}
