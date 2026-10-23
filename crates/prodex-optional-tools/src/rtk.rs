use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::fs_ops::{write_executable_file, write_text_file};
use crate::localization::ensure_agents_reference;
use crate::{PRODEX_RTK_CODEX_AWARENESS, RTK_MD};

const PRODEX_HOME_ENV: &str = "PRODEX_HOME";
const RTK_STATE_DIR_NAME: &str = "rtk";

pub fn configure_rtk_codex_home(codex_home: &Path) -> Result<()> {
    let rtk = find_path_command("rtk");
    configure_rtk_codex_home_with_command(codex_home, rtk.as_deref())
}

pub(crate) fn configure_rtk_codex_home_with_command(
    codex_home: &Path,
    rtk: Option<&Path>,
) -> Result<()> {
    prodex_shared_codex_fs::create_codex_home_if_missing(codex_home)?;
    let rtk_md_path = codex_home.join(RTK_MD);
    write_text_file(&rtk_md_path, PRODEX_RTK_CODEX_AWARENESS)?;
    ensure_agents_reference(codex_home, &rtk_md_path)?;
    configure_rtk_wrapper(codex_home, rtk)
}

fn configure_rtk_wrapper(codex_home: &Path, rtk: Option<&Path>) -> Result<()> {
    let bin_dir = codex_home.join("bin");
    fs::create_dir_all(&bin_dir)
        .with_context(|| format!("failed to create {}", bin_dir.display()))?;
    let Some(rtk) = rtk else {
        write_unavailable_command_wrapper(&wrapper_path(&bin_dir, "rtk"), "rtk")?;
        write_unavailable_command_wrapper(&wrapper_path(&bin_dir, "prodex-rtk"), "rtk")?;
        return Ok(());
    };
    write_shell_wrapper(&wrapper_path(&bin_dir, "rtk"), rtk, &[])?;
    write_shell_wrapper(&wrapper_path(&bin_dir, "prodex-rtk"), rtk, &[])?;
    configure_rtk_command_wrappers(&bin_dir, rtk)?;
    Ok(())
}

fn configure_rtk_command_wrappers(bin_dir: &Path, rtk: &Path) -> Result<()> {
    for wrapper in prodex_mojo_core::rtk_noisy::rtk_auto_wrapper_manifest().map_err(|error| {
        anyhow::anyhow!("compiled Mojo RTK command wrapper policy failed: {error:?}")
    })? {
        let Some(command) = find_path_command(&wrapper.command) else {
            continue;
        };
        if command.parent() == Some(bin_dir) {
            continue;
        }
        let subcommands = wrapper
            .subcommands
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        write_rtk_command_wrapper(
            &wrapper_path(bin_dir, &wrapper.command),
            rtk,
            &command,
            &subcommands,
        )?;
    }
    Ok(())
}

fn find_path_command(command: &str) -> Option<PathBuf> {
    for dir in env::var_os("PATH")
        .map(|path| env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default()
    {
        let candidate = dir.join(command);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            for extension in ["exe", "cmd", "bat", "com"] {
                let candidate = dir.join(format!("{command}.{extension}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

#[cfg(windows)]
fn wrapper_path(bin_dir: &Path, command: &str) -> PathBuf {
    bin_dir.join(format!("{command}.cmd"))
}

#[cfg(not(windows))]
fn wrapper_path(bin_dir: &Path, command: &str) -> PathBuf {
    bin_dir.join(command)
}

fn write_rtk_command_wrapper(
    path: &Path,
    rtk: &Path,
    command: &Path,
    subcommands: &[&str],
) -> Result<()> {
    let script = render_wrapper(
        prodex_mojo_core::rtk_noisy::RtkWrapperKind::Command,
        &rtk.display().to_string(),
        &command.display().to_string(),
        subcommands,
    )?;
    write_executable_script(path, &script)
}

fn write_shell_wrapper(path: &Path, command: &Path, args: &[&str]) -> Result<()> {
    let script = render_wrapper(
        prodex_mojo_core::rtk_noisy::RtkWrapperKind::Shell,
        "",
        &command.display().to_string(),
        args,
    )?;
    write_executable_script(path, &script)
}

fn write_unavailable_command_wrapper(path: &Path, command: &str) -> Result<()> {
    let script = render_wrapper(
        prodex_mojo_core::rtk_noisy::RtkWrapperKind::Unavailable,
        "",
        command,
        &[],
    )?;
    write_executable_script(path, &script)
}

fn render_wrapper(
    kind: prodex_mojo_core::rtk_noisy::RtkWrapperKind,
    rtk: &str,
    command: &str,
    arguments: &[&str],
) -> Result<String> {
    use prodex_mojo_core::rtk_noisy::{RtkWrapperInput, RtkWrapperPlatform, render_rtk_wrapper};
    let state_paths = match prodex_home_from_env() {
        Some(home) => {
            let root = home.join("optimizer-state").join(RTK_STATE_DIR_NAME);
            ["history.db", "tee", "audit"]
                .map(|name| root.join(name).display().to_string())
                .to_vec()
        }
        None => Vec::new(),
    };
    let state_paths = state_paths.iter().map(String::as_str).collect::<Vec<_>>();
    render_rtk_wrapper(RtkWrapperInput {
        platform: if cfg!(windows) {
            RtkWrapperPlatform::Windows
        } else {
            RtkWrapperPlatform::Posix
        },
        kind,
        rtk,
        command,
        arguments,
        state_paths: &state_paths,
    })
    .map_err(|error| anyhow::anyhow!("compiled Mojo RTK wrapper rendering failed: {error:?}"))
}

fn prodex_home_from_env() -> Option<PathBuf> {
    env::var_os(PRODEX_HOME_ENV)
        .map(PathBuf::from)
        .map(absolutize_path_lossy)
        .or_else(|| home_dir_from_env().map(|home| home.join(".prodex")))
}

fn absolutize_path_lossy(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        return path;
    }
    env::current_dir()
        .map(|current_dir| current_dir.join(&path))
        .unwrap_or(path)
}

fn home_dir_from_env() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
}

fn write_executable_script(path: &Path, script: &str) -> Result<()> {
    write_executable_file(path, script)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(any(unix, windows))]
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        #[cfg(windows)]
        let temp_root = env::temp_dir();
        #[cfg(not(windows))]
        let temp_root = env::temp_dir()
            .canonicalize()
            .expect("temp dir should resolve");
        temp_root.join(format!(
            "prodex-rtk-wrapper-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    #[test]
    #[cfg(unix)]
    fn compiled_shell_wrapper_preserves_quoted_literal_arguments() {
        let dir = temp_dir("quoted-shell-arguments");
        let wrapper = dir.join("wrapper");
        let argument = "single'quote $HOME `id` 10% 账户🙂";
        write_shell_wrapper(&wrapper, Path::new("/usr/bin/printf"), &["%s", argument])
            .expect("wrapper should render and write");
        let output = Command::new(&wrapper)
            .output()
            .expect("wrapper should execute");
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap(), argument);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rtk_auto_wrapper_creation_uses_compiled_manifest_not_host_command_table() {
        let dir = temp_dir("compiled-rtk-wrapper-manifest");
        let wrappers = prodex_mojo_core::rtk_noisy::rtk_auto_wrapper_manifest()
            .expect("compiled Mojo wrapper manifest");
        let cargo = wrappers
            .iter()
            .find(|wrapper| wrapper.command == "cargo")
            .unwrap();
        let subcommands = cargo
            .subcommands
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let script_path = dir.join("cargo");
        write_rtk_command_wrapper(
            &script_path,
            Path::new("/opt/rtk/bin/rtk"),
            Path::new("/usr/bin/cargo"),
            &subcommands,
        )
        .expect("compiled RTK policy should render a wrapper");
        let script = fs::read_to_string(&script_path).unwrap();
        #[cfg(not(windows))]
        {
            assert!(script.contains("test|build|check|clippy|bench|run"));
            assert!(script.contains("exec '/usr/bin/cargo'"));
        }
        #[cfg(windows)]
        {
            assert!(script.contains("if /I \"%~1\"==\"test\" goto prodex_rtk_wrap"));
            assert!(script.contains("if /I \"%~1\"==\"clippy\" goto prodex_rtk_wrap"));
        }
        let _ = fs::remove_file(&script_path);
    }

    #[test]
    fn rtk_selective_command_wrapper_routes_matching_subcommands() {
        let dir = temp_dir("selective");
        let wrapper = dir.join("cargo");
        write_rtk_command_wrapper(
            &wrapper,
            Path::new("/opt/rtk/bin/rtk"),
            Path::new("/usr/bin/cargo"),
            &["test", "build", "check"],
        )
        .expect("wrapper should write");

        let script = fs::read_to_string(&wrapper).expect("wrapper should exist");
        assert!(script.contains("PRODEX_RTK_DISABLE_AUTO_WRAP"));
        assert!(script.contains("PRODEX_RTK_AUTO_WRAP_DEPTH"));
        assert!(script.contains("RTK_DB_PATH"));
        #[cfg(not(windows))]
        {
            assert!(script.contains("optimizer-state/rtk/history.db"));
            assert!(script.contains("test|build|check"));
            assert!(script.contains("exec '/opt/rtk/bin/rtk' '/usr/bin/cargo' \"$@\""));
            assert!(script.contains("exec '/usr/bin/cargo' \"$@\""));
        }
        #[cfg(windows)]
        {
            assert!(script.contains("if /I \"%~1\"==\"test\" goto prodex_rtk_wrap"));
            assert!(script.contains("shift\r\ngoto prodex_rtk_scan"));
            assert!(script.contains("\"/opt/rtk/bin/rtk\" \"/usr/bin/cargo\" %*"));
            assert!(script.contains("\"/usr/bin/cargo\" %*"));
        }

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rtk_always_command_wrapper_routes_through_rtk() {
        let dir = temp_dir("always");
        let wrapper = dir.join("rg");
        write_rtk_command_wrapper(
            &wrapper,
            Path::new("/opt/rtk/bin/rtk"),
            Path::new("/usr/bin/rg"),
            &[],
        )
        .expect("wrapper should write");

        let script = fs::read_to_string(&wrapper).expect("wrapper should exist");
        assert!(script.contains("RTK_DB_PATH"));
        #[cfg(not(windows))]
        {
            assert!(script.contains("exec '/opt/rtk/bin/rtk' '/usr/bin/rg' \"$@\""));
            assert!(script.contains("exec '/usr/bin/rg' \"$@\""));
        }
        #[cfg(windows)]
        {
            assert!(script.contains("\"/opt/rtk/bin/rtk\" \"/usr/bin/rg\" %*"));
            assert!(script.contains("\"/usr/bin/rg\" %*"));
        }

        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn unavailable_command_wrapper_fails_loudly() {
        let dir = temp_dir("unavailable");
        let wrapper = dir.join("rtk");
        write_unavailable_command_wrapper(&wrapper, "rtk").expect("wrapper should write");

        let output = Command::new(&wrapper)
            .output()
            .expect("wrapper should execute");
        assert_eq!(output.status.code(), Some(127));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("rtk requested"));
        assert!(stderr.contains("not installed"));

        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn rtk_selective_command_wrapper_executes_rtk_for_noisy_subcommand_only() {
        let dir = temp_dir("exec-selective");
        fs::create_dir_all(&dir).expect("temp dir should exist");
        let fake_rtk = dir.join("rtk");
        let fake_cargo = dir.join("cargo-real");
        let marker = dir.join("marker");
        write_executable_script(
            &fake_rtk,
            "#!/usr/bin/env sh\nprintf 'rtk:%s\\n' \"$*\" > \"$PRODEX_RTK_MARKER\"\n",
        )
        .expect("fake rtk should write");
        write_executable_script(
            &fake_cargo,
            "#!/usr/bin/env sh\nprintf 'raw:%s\\n' \"$*\"\n",
        )
        .expect("fake cargo should write");

        let wrapper = dir.join("cargo");
        write_rtk_command_wrapper(&wrapper, &fake_rtk, &fake_cargo, &["test"])
            .expect("wrapper should write");

        let output = Command::new(&wrapper)
            .arg("test")
            .env("PRODEX_RTK_MARKER", &marker)
            .env_remove("PRODEX_RTK_AUTO_WRAP_DEPTH")
            .env_remove("PRODEX_RTK_DISABLE_AUTO_WRAP")
            .output()
            .expect("wrapper should run");
        assert!(output.status.success());
        let marked = fs::read_to_string(&marker).expect("rtk marker should exist");
        assert!(marked.contains("rtk:"));
        assert!(marked.contains("cargo-real test"));

        let _ = fs::remove_file(&marker);
        let output = Command::new(&wrapper)
            .arg("fmt")
            .env("PRODEX_RTK_MARKER", &marker)
            .env_remove("PRODEX_RTK_AUTO_WRAP_DEPTH")
            .env_remove("PRODEX_RTK_DISABLE_AUTO_WRAP")
            .output()
            .expect("wrapper should run raw command");
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "raw:fmt\n");
        assert!(!marker.exists());

        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(windows)]
    #[test]
    fn rtk_selective_command_wrapper_executes_on_windows() {
        let dir = temp_dir("exec-selective-windows");
        fs::create_dir_all(&dir).expect("temp dir should exist");
        let fake_rtk = dir.join("rtk.cmd");
        let fake_cargo = dir.join("cargo-real.cmd");
        let marker = dir.join("marker");
        write_executable_script(
            &fake_rtk,
            "@echo off\r\n> \"%PRODEX_RTK_MARKER%\" echo rtk:%*\r\nexit /b 0\r\n",
        )
        .expect("fake rtk should write");
        write_executable_script(&fake_cargo, "@echo off\r\necho raw:%*\r\nexit /b 0\r\n")
            .expect("fake cargo should write");

        let wrapper = dir.join("cargo.cmd");
        write_rtk_command_wrapper(&wrapper, &fake_rtk, &fake_cargo, &["test"])
            .expect("wrapper should write");

        let output = Command::new(&wrapper)
            .args(["-q", "test", "prompt with spaces"])
            .env("PRODEX_RTK_MARKER", &marker)
            .env_remove("PRODEX_RTK_AUTO_WRAP_DEPTH")
            .env_remove("PRODEX_RTK_DISABLE_AUTO_WRAP")
            .output()
            .expect("wrapper should run");
        assert!(output.status.success());
        let marked = fs::read_to_string(&marker).expect("rtk marker should exist");
        assert!(marked.contains("rtk:"));
        assert!(marked.contains("cargo-real.cmd"));
        assert!(marked.contains("-q test \"prompt with spaces\""));

        let _ = fs::remove_file(&marker);
        let output = Command::new(&wrapper)
            .args(["-q", "fmt"])
            .env("PRODEX_RTK_MARKER", &marker)
            .env_remove("PRODEX_RTK_AUTO_WRAP_DEPTH")
            .env_remove("PRODEX_RTK_DISABLE_AUTO_WRAP")
            .output()
            .expect("wrapper should run raw command");
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
            "raw:-q fmt\n"
        );
        assert!(!marker.exists());

        let _ = fs::remove_dir_all(dir);
    }
}
