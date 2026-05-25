use anyhow::{Context, Result};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::hook_trust::configure_trusted_session_start_command_hook;

const AUTO_WRAPPER: &str = "prodex-claw-compactor-auto";
const SESSION_HOOK: &str = "prodex-claw-compactor-sessionstart";
const SESSION_WRAPPER: &str = "prodex-claw-compactor-sessionstart";
const SESSION_MARKER: &str = ".prodex-hooks/claw-compactor-sessionstart";

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
        write_session_wrapper(&bin_dir.join(SESSION_WRAPPER))?;
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
        write_session_wrapper(&bin_dir.join(SESSION_WRAPPER))?;
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

run_claw() {{
  target="$1"
  if output=$({command} benchmark "$target" --json 2>/dev/null); then
    printf 'CLAW_COMPACTOR_ACTIVE %.12000s\n' "$output"
    return 0
  elif output=$({command} "$target" benchmark --json 2>/dev/null); then
    printf 'CLAW_COMPACTOR_ACTIVE %.12000s\n' "$output"
    return 0
  fi
  return 1
}}

has_claw_markdown() {{
  target="$1"
  if find "$target" -maxdepth 1 -type f -name '*.md' -print -quit 2>/dev/null | grep -q .; then
    return 0
  fi
  if [ -d "$target/memory" ] && find "$target/memory" -maxdepth 1 -type f -name '*.md' ! -name '.*' -print -quit 2>/dev/null | grep -q .; then
    return 0
  fi
  return 1
}}

write_shadow_memory() {{
  original="$1"
  output="$2"
  {{
    printf '# Prodex Claw Shadow Workspace\n\n'
    printf 'Original workspace: `%s`\n\n' "$original"
    printf '%s\n\n' 'This temporary Markdown file lets claw-compactor benchmark a directory that has no Markdown memory files. Prodex generated it outside the original workspace.'
    printf '## Top-Level Entries\n\n'
    if [ -d "$original" ]; then
      find "$original" -maxdepth 1 -mindepth 1 \( -name '.git' -o -name 'node_modules' -o -name 'target' -o -name 'dist' -o -name 'build' -o -name '.venv' -o -name 'venv' -o -name '__pycache__' \) -prune -o -print 2>/dev/null | sort | head -80 | sed 's/^/- /'
    fi
    printf '\n## Important Files\n\n'
    if [ -d "$original" ]; then
      find "$original" -maxdepth 3 \( -type d \( -name '.git' -o -name 'node_modules' -o -name 'target' -o -name 'dist' -o -name 'build' -o -name '.venv' -o -name 'venv' -o -name '__pycache__' -o -name 'vendor' \) -prune \) -o \( -type f \( -name 'Cargo.toml' -o -name 'package.json' -o -name 'pyproject.toml' -o -name 'go.mod' -o -name 'pom.xml' -o -name 'build.gradle' -o -name 'Makefile' -o -name 'Dockerfile' \) -print \) 2>/dev/null | sort | head -80 | sed 's/^/- /'
    fi
    printf '\n## Code File Sample\n\n'
    if [ -d "$original" ]; then
      find "$original" -maxdepth 3 \( -type d \( -name '.git' -o -name 'node_modules' -o -name 'target' -o -name 'dist' -o -name 'build' -o -name '.venv' -o -name 'venv' -o -name '__pycache__' -o -name 'vendor' \) -prune \) -o \( -type f \( -name '*.rs' -o -name '*.py' -o -name '*.ts' -o -name '*.tsx' -o -name '*.js' -o -name '*.jsx' -o -name '*.go' -o -name '*.java' -o -name '*.kt' -o -name '*.swift' -o -name '*.c' -o -name '*.cpp' -o -name '*.h' -o -name '*.hpp' \) -print \) 2>/dev/null | sort | head -120 | sed 's/^/- /'
    fi
  }} > "$output"
}}

run_shadow_claw() {{
  target="$1"
  [ -d "$target" ] || return 1
  has_claw_markdown "$target" && return 1
  shadow="$(mktemp -d "${{TMPDIR:-/tmp}}/prodex-claw-shadow.XXXXXX")" || return 1
  trap 'rm -rf "$shadow"' EXIT HUP INT TERM
  write_shadow_memory "$target" "$shadow/MEMORY.md" || return 1
  run_claw "$shadow"
}}

if run_claw "$workspace"; then
  exit 0
fi
if run_shadow_claw "$workspace"; then
  exit 0
fi
printf '%s\n' 'CLAW_COMPACTOR_UNAVAILABLE'
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

run_claw() {{
  target="$1"
  if output=$({python} {script_path} "$target" benchmark --json 2>/dev/null); then
    printf 'CLAW_COMPACTOR_ACTIVE %.12000s\n' "$output"
    return 0
  fi
  return 1
}}

has_claw_markdown() {{
  target="$1"
  if find "$target" -maxdepth 1 -type f -name '*.md' -print -quit 2>/dev/null | grep -q .; then
    return 0
  fi
  if [ -d "$target/memory" ] && find "$target/memory" -maxdepth 1 -type f -name '*.md' ! -name '.*' -print -quit 2>/dev/null | grep -q .; then
    return 0
  fi
  return 1
}}

write_shadow_memory() {{
  original="$1"
  output="$2"
  {{
    printf '# Prodex Claw Shadow Workspace\n\n'
    printf 'Original workspace: `%s`\n\n' "$original"
    printf '%s\n\n' 'This temporary Markdown file lets claw-compactor benchmark a directory that has no Markdown memory files. Prodex generated it outside the original workspace.'
    printf '## Top-Level Entries\n\n'
    if [ -d "$original" ]; then
      find "$original" -maxdepth 1 -mindepth 1 \( -name '.git' -o -name 'node_modules' -o -name 'target' -o -name 'dist' -o -name 'build' -o -name '.venv' -o -name 'venv' -o -name '__pycache__' \) -prune -o -print 2>/dev/null | sort | head -80 | sed 's/^/- /'
    fi
    printf '\n## Important Files\n\n'
    if [ -d "$original" ]; then
      find "$original" -maxdepth 3 \( -type d \( -name '.git' -o -name 'node_modules' -o -name 'target' -o -name 'dist' -o -name 'build' -o -name '.venv' -o -name 'venv' -o -name '__pycache__' -o -name 'vendor' \) -prune \) -o \( -type f \( -name 'Cargo.toml' -o -name 'package.json' -o -name 'pyproject.toml' -o -name 'go.mod' -o -name 'pom.xml' -o -name 'build.gradle' -o -name 'Makefile' -o -name 'Dockerfile' \) -print \) 2>/dev/null | sort | head -80 | sed 's/^/- /'
    fi
    printf '\n## Code File Sample\n\n'
    if [ -d "$original" ]; then
      find "$original" -maxdepth 3 \( -type d \( -name '.git' -o -name 'node_modules' -o -name 'target' -o -name 'dist' -o -name 'build' -o -name '.venv' -o -name 'venv' -o -name '__pycache__' -o -name 'vendor' \) -prune \) -o \( -type f \( -name '*.rs' -o -name '*.py' -o -name '*.ts' -o -name '*.tsx' -o -name '*.js' -o -name '*.jsx' -o -name '*.go' -o -name '*.java' -o -name '*.kt' -o -name '*.swift' -o -name '*.c' -o -name '*.cpp' -o -name '*.h' -o -name '*.hpp' \) -print \) 2>/dev/null | sort | head -120 | sed 's/^/- /'
    fi
  }} > "$output"
}}

run_shadow_claw() {{
  target="$1"
  [ -d "$target" ] || return 1
  has_claw_markdown "$target" && return 1
  shadow="$(mktemp -d "${{TMPDIR:-/tmp}}/prodex-claw-shadow.XXXXXX")" || return 1
  trap 'rm -rf "$shadow"' EXIT HUP INT TERM
  write_shadow_memory "$target" "$shadow/MEMORY.md" || return 1
  run_claw "$shadow"
}}

if run_claw "$workspace"; then
  exit 0
fi
if run_shadow_claw "$workspace"; then
  exit 0
fi
printf '%s\n' 'CLAW_COMPACTOR_UNAVAILABLE'
"#
    );
    write_executable_script(path, &script)
}

fn write_session_wrapper(path: &Path) -> Result<()> {
    let script = format!(
        r#"#!/usr/bin/env sh
codex_home="${{CODEX_HOME:-${{HOME:-}}/.codex}}"
marker="$codex_home/{SESSION_MARKER}"
marker_dir=$(dirname "$marker")
mkdir -p "$marker_dir" 2>/dev/null || true
if [ -e "$marker" ]; then
  exit 0
fi
: > "$marker" 2>/dev/null || exit 0
exec {AUTO_WRAPPER} "${{1:-$(pwd)}}"
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
    #[cfg(unix)]
    use std::process::Command;
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
        configure_session_hook(&codex_home, std::slice::from_ref(&path_root), &[])
            .expect("claw hook should write");
        configure_session_hook(&codex_home, std::slice::from_ref(&path_root), &[])
            .expect("claw hook should be idempotent");

        let wrapper = fs::read_to_string(codex_home.join("bin").join(AUTO_WRAPPER))
            .expect("auto wrapper should exist");
        assert!(wrapper.contains("benchmark"));
        assert!(wrapper.contains("--json"));
        assert!(wrapper.contains("Prodex Claw Shadow Workspace"));
        assert!(wrapper.contains("MEMORY.md"));
        assert!(wrapper.contains("mktemp -d"));
        assert!(wrapper.contains("has_claw_markdown"));
        let session_wrapper = fs::read_to_string(codex_home.join("bin").join(SESSION_WRAPPER))
            .expect("session wrapper should exist");
        assert!(session_wrapper.contains(AUTO_WRAPPER));
        assert!(session_wrapper.contains(SESSION_MARKER));

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

    #[cfg(unix)]
    #[test]
    fn auto_wrapper_uses_shadow_workspace_when_markdown_is_missing() {
        let root = temp_dir("shadow-wrapper");
        let workspace = root.join("plain-workspace");
        let bin_dir = root.join("bin");
        let fake_claw = command_path(bin_dir.join("claw-compactor"));
        let wrapper = bin_dir.join(AUTO_WRAPPER);
        fs::create_dir_all(&workspace).expect("plain workspace should exist");
        fs::write(workspace.join("main.rs"), "fn main() {}\n")
            .expect("sample source should be written");
        write_executable_script(
            &fake_claw,
            r#"#!/usr/bin/env sh
if [ "$1" = "benchmark" ]; then
  target="$2"
else
  target="$1"
fi
if [ -f "$target/MEMORY.md" ]; then
  printf '{"shadow":true,"target":"%s"}\n' "$target"
  exit 0
fi
exit 1
"#,
        )
        .expect("fake claw should be executable");
        write_auto_wrapper_for_binary(&wrapper, &fake_claw)
            .expect("auto wrapper should be written");

        let output = Command::new(&wrapper)
            .arg(&workspace)
            .output()
            .expect("wrapper should run");
        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
        assert!(stdout.contains("CLAW_COMPACTOR_ACTIVE"));
        assert!(stdout.contains(r#""shadow":true"#));
        assert!(
            !workspace.join("MEMORY.md").exists(),
            "shadow fallback must not write into the original workspace"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn session_wrapper_outputs_once_per_codex_home() {
        let root = temp_dir("session-wrapper-once");
        let codex_home = root.join("codex-home");
        let workspace = root.join("workspace");
        let bin_dir = root.join("bin");
        let fake_auto = bin_dir.join(AUTO_WRAPPER);
        let wrapper = bin_dir.join(SESSION_WRAPPER);
        fs::create_dir_all(&codex_home).expect("codex home should exist");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        write_executable_script(
            &fake_auto,
            r#"#!/usr/bin/env sh
printf 'CLAW_COMPACTOR_ACTIVE {"target":"%s"}\n' "$1"
"#,
        )
        .expect("fake auto wrapper should be executable");
        write_session_wrapper(&wrapper).expect("session wrapper should be written");

        let mut path_entries = vec![bin_dir.clone()];
        if let Some(existing) = env::var_os("PATH") {
            path_entries.extend(env::split_paths(&existing));
        }
        let path = env::join_paths(path_entries).expect("PATH should join");

        let first = Command::new(&wrapper)
            .arg(&workspace)
            .env("CODEX_HOME", &codex_home)
            .env("PATH", &path)
            .output()
            .expect("first session wrapper should run");
        assert!(first.status.success());
        let first_stdout = String::from_utf8(first.stdout).expect("first stdout should be utf8");
        assert!(first_stdout.contains("CLAW_COMPACTOR_ACTIVE"));

        let second = Command::new(&wrapper)
            .arg(&workspace)
            .env("CODEX_HOME", &codex_home)
            .env("PATH", &path)
            .output()
            .expect("second session wrapper should run");
        assert!(second.status.success());
        assert!(
            second.stdout.is_empty(),
            "Claw SessionStart wrapper should not replay after marker"
        );

        let _ = fs::remove_dir_all(root);
    }
}
