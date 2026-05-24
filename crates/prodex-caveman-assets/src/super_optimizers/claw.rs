use anyhow::{Context, Result};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::hook_trust::configure_trusted_session_start_command_hook;

const AUTO_WRAPPER: &str = "prodex-claw-compactor-auto";
const SESSION_HOOK: &str = "prodex-claw-compactor-auto \"$(pwd)\"";

pub(super) fn configure_command_wrappers(
    bin_dir: &Path,
    path_dirs: &[PathBuf],
    optimizer_roots: &[PathBuf],
) -> Result<()> {
    if let Some(command) =
        super::find_optimizer_command("claw-compactor", path_dirs, optimizer_roots)
    {
        super::write_shell_wrapper(&bin_dir.join("claw-compactor"), &command, &[])?;
        super::write_shell_wrapper(&bin_dir.join("prodex-claw-compactor"), &command, &[])?;
        write_auto_wrapper_for_binary(&bin_dir.join(AUTO_WRAPPER), &command)?;
    } else if let Some((python, script)) = find_script(optimizer_roots) {
        super::write_shell_wrapper(
            &bin_dir.join("claw-compactor"),
            &python,
            &[script.to_string_lossy().as_ref()],
        )?;
        super::write_shell_wrapper(
            &bin_dir.join("prodex-claw-compactor"),
            &python,
            &[script.to_string_lossy().as_ref()],
        )?;
        write_auto_wrapper_for_script(&bin_dir.join(AUTO_WRAPPER), &python, &script)?;
    }
    Ok(())
}

pub(super) fn configure_session_hook(
    codex_home: &Path,
    path_dirs: &[PathBuf],
    optimizer_roots: &[PathBuf],
) -> Result<()> {
    let has_claw_compactor =
        super::find_optimizer_command("claw-compactor", path_dirs, optimizer_roots).is_some()
            || find_script(optimizer_roots).is_some();
    if !has_claw_compactor {
        return Ok(());
    }

    let config_path = codex_home.join("config.toml");
    let contents = fs::read_to_string(&config_path).unwrap_or_default();
    let mut table = if contents.trim().is_empty() {
        toml::Table::new()
    } else {
        match toml::from_str::<toml::Value>(&contents)
            .with_context(|| format!("failed to parse {}", config_path.display()))?
        {
            toml::Value::Table(table) => table,
            _ => anyhow::bail!("{} did not parse as a TOML table", config_path.display()),
        }
    };

    if !session_start_hook_contains_command(&table, SESSION_HOOK) {
        configure_trusted_session_start_command_hook(&mut table, &config_path, SESSION_HOOK)?;
    }

    let rendered = toml::to_string(&toml::Value::Table(table))
        .context("failed to render Claw-Compactor session hook config")?;
    fs::write(&config_path, rendered)
        .with_context(|| format!("failed to write {}", config_path.display()))?;
    Ok(())
}

fn session_start_hook_contains_command(table: &toml::Table, command: &str) -> bool {
    table
        .get("hooks")
        .and_then(toml::Value::as_table)
        .and_then(|hooks| hooks.get("SessionStart"))
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .any(|group| {
            group
                .get("hooks")
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
                .any(|hook| hook.get("command").and_then(toml::Value::as_str) == Some(command))
        })
}

fn find_script(optimizer_roots: &[PathBuf]) -> Option<(PathBuf, PathBuf)> {
    for root in optimizer_roots {
        let checkout = root.join("claw-compactor");
        let script = checkout.join("scripts").join("mem_compress.py");
        if !script.is_file() {
            continue;
        }
        let python = [
            checkout.join(".venv").join("bin").join("python"),
            checkout.join("venv").join("bin").join("python"),
            PathBuf::from("python3"),
        ]
        .into_iter()
        .find(|candidate| candidate.is_file() || candidate == &PathBuf::from("python3"))?;
        return Some((python, script));
    }
    None
}

fn write_auto_wrapper_for_binary(path: &Path, command: &Path) -> Result<()> {
    let command = shell_single_quote(&command.display().to_string());
    let script = format!(
        r#"#!/usr/bin/env sh
workspace="${{1:-$(pwd)}}"
if output=$({command} benchmark "$workspace" --json 2>/dev/null); then
  printf 'CLAW_COMPACTOR_ACTIVE %.12000s\n' "$output"
elif output=$({command} "$workspace" benchmark --json 2>/dev/null); then
  printf 'CLAW_COMPACTOR_ACTIVE %.12000s\n' "$output"
else
  printf '%s\n' 'CLAW_COMPACTOR_UNAVAILABLE'
fi
"#
    );
    write_executable_script(path, &script)
}

fn write_auto_wrapper_for_script(path: &Path, python: &Path, script_path: &Path) -> Result<()> {
    let python = shell_single_quote(&python.display().to_string());
    let script_path = shell_single_quote(&script_path.display().to_string());
    let script = format!(
        r#"#!/usr/bin/env sh
workspace="${{1:-$(pwd)}}"
if output=$({python} {script_path} "$workspace" benchmark --json 2>/dev/null); then
  printf 'CLAW_COMPACTOR_ACTIVE %.12000s\n' "$output"
else
  printf '%s\n' 'CLAW_COMPACTOR_UNAVAILABLE'
fi
"#
    );
    write_executable_script(path, &script)
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn write_executable_script(path: &Path, script: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(path, script).with_context(|| format!("failed to write {}", path.display()))?;
    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(path)
            .with_context(|| format!("failed to stat {}", path.display()))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)
            .with_context(|| format!("failed to chmod {}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        env::temp_dir().join(format!(
            "prodex-claw-compactor-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn command_path(path: PathBuf) -> PathBuf {
        #[cfg(windows)]
        if path.extension().is_none() {
            return path.with_extension("exe");
        }
        path
    }

    #[test]
    fn managed_discovery_finds_venv_binary() {
        let root = temp_dir("root");
        let command = command_path(
            root.join("claw-compactor")
                .join(".venv")
                .join("bin")
                .join("claw-compactor"),
        );
        fs::create_dir_all(command.parent().expect("command parent should exist"))
            .expect("command parent should be created");
        fs::write(&command, "").expect("fake command should be written");

        assert_eq!(
            super::super::find_managed_optimizer_command(
                "claw-compactor",
                std::slice::from_ref(&root),
            ),
            Some(command)
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn command_wrappers_register_script_checkout() {
        let codex_home = temp_dir("codex-home");
        let optimizer_root = temp_dir("optimizer-root");
        let script = optimizer_root
            .join("claw-compactor")
            .join("scripts")
            .join("mem_compress.py");
        fs::create_dir_all(&codex_home).expect("codex home should exist");
        fs::create_dir_all(script.parent().expect("script parent should exist"))
            .expect("script parent should be created");
        fs::write(&script, "print('ok')\n").expect("fake script should be written");

        configure_command_wrappers(
            &codex_home.join("bin"),
            &[],
            std::slice::from_ref(&optimizer_root),
        )
        .expect("claw wrapper should write");

        let wrapper = fs::read_to_string(codex_home.join("bin").join("claw-compactor"))
            .expect("wrapper should exist");
        assert!(wrapper.contains("python3"));
        assert!(wrapper.contains("mem_compress.py"));
        assert!(
            codex_home
                .join("bin")
                .join("prodex-claw-compactor")
                .is_file()
        );
        assert!(codex_home.join("bin").join(AUTO_WRAPPER).is_file());

        let _ = fs::remove_dir_all(codex_home);
        let _ = fs::remove_dir_all(optimizer_root);
    }

    #[test]
    fn session_hook_registers_trusted_runtime_probe() {
        let codex_home = temp_dir("hook-codex-home");
        let path_root = temp_dir("hook-path-root");
        let command = command_path(path_root.join("claw-compactor"));
        fs::create_dir_all(&codex_home).expect("codex home should exist");
        fs::create_dir_all(command.parent().expect("command parent should exist"))
            .expect("command parent should be created");
        fs::write(&command, "").expect("fake command should be written");

        configure_command_wrappers(
            &codex_home.join("bin"),
            std::slice::from_ref(&path_root),
            &[],
        )
        .expect("claw wrappers should write");
        configure_session_hook(&codex_home, &[path_root.clone()], &[])
            .expect("claw hook should write");
        configure_session_hook(&codex_home, &[path_root.clone()], &[])
            .expect("claw hook should be idempotent");

        let wrapper = fs::read_to_string(codex_home.join("bin").join(AUTO_WRAPPER))
            .expect("auto wrapper should exist");
        assert!(wrapper.contains("benchmark"));
        assert!(wrapper.contains("--json"));

        let config_path = codex_home.join("config.toml");
        let config = fs::read_to_string(&config_path).expect("config.toml should be written");
        let table = toml::from_str::<toml::Value>(&config).expect("config should parse");
        let hooks = table
            .get("hooks")
            .and_then(toml::Value::as_table)
            .and_then(|hooks| hooks.get("SessionStart"))
            .and_then(toml::Value::as_array)
            .expect("SessionStart hooks should exist");
        let matching_hooks = hooks
            .iter()
            .filter(|group| {
                group
                    .get("hooks")
                    .and_then(toml::Value::as_array)
                    .into_iter()
                    .flatten()
                    .any(|hook| {
                        hook.get("command").and_then(toml::Value::as_str) == Some(SESSION_HOOK)
                    })
            })
            .count();
        assert_eq!(matching_hooks, 1);
        let hook_key = format!("{}:session_start:0:0", config_path.display());
        assert!(
            table["hooks"]["state"][&hook_key]["trusted_hash"]
                .as_str()
                .is_some_and(|hash| hash.starts_with("sha256:"))
        );

        let _ = fs::remove_dir_all(codex_home);
        let _ = fs::remove_dir_all(path_root);
    }
}
