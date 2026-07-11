use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::fs_ops::{read_text_file_limited, write_executable_file, write_text_file};
use crate::localization::localize_text_file;
use crate::{AGENTS_MD, PRODEX_RTK_CODEX_AWARENESS, RTK_MD};

const PRODEX_HOME_ENV: &str = "PRODEX_HOME";
const RTK_STATE_DIR_NAME: &str = "rtk";

pub fn configure_rtk_codex_home(codex_home: &Path) -> Result<()> {
    prodex_shared_codex_fs::create_codex_home_if_missing(codex_home)?;
    let rtk_md_path = codex_home.join(RTK_MD);
    write_text_file(&rtk_md_path, PRODEX_RTK_CODEX_AWARENESS)?;
    localize_text_file(&codex_home.join(AGENTS_MD))?;
    ensure_rtk_agents_reference(codex_home, &rtk_md_path)?;
    configure_rtk_wrapper(codex_home)
}

fn ensure_rtk_agents_reference(codex_home: &Path, rtk_md_path: &Path) -> Result<()> {
    let agents_path = codex_home.join(AGENTS_MD);
    let reference = format!("@{}", rtk_md_path.display());
    let contents = read_text_file_limited(&agents_path)?.unwrap_or_default();
    if contents.lines().any(|line| line.trim() == reference) {
        return Ok(());
    }

    let mut updated = String::new();
    if contents.trim().is_empty() {
        updated.push_str(&reference);
        updated.push('\n');
    } else {
        updated.push_str(contents.trim_end());
        updated.push_str("\n\n");
        updated.push_str(&reference);
        updated.push('\n');
    }
    write_text_file(&agents_path, &updated)?;
    Ok(())
}

fn configure_rtk_wrapper(codex_home: &Path) -> Result<()> {
    let bin_dir = codex_home.join("bin");
    fs::create_dir_all(&bin_dir)
        .with_context(|| format!("failed to create {}", bin_dir.display()))?;
    let Some(rtk) = find_path_command("rtk") else {
        write_unavailable_command_wrapper(&bin_dir.join("rtk"), "rtk")?;
        write_unavailable_command_wrapper(&bin_dir.join("prodex-rtk"), "rtk")?;
        return Ok(());
    };
    write_shell_wrapper(&bin_dir.join("rtk"), &rtk, &[])?;
    write_shell_wrapper(&bin_dir.join("prodex-rtk"), &rtk, &[])?;
    configure_rtk_command_wrappers(&bin_dir, &rtk)?;
    Ok(())
}

fn configure_rtk_command_wrappers(bin_dir: &Path, rtk: &Path) -> Result<()> {
    for wrapper in rtk_command_wrappers() {
        let Some(command) = find_path_command(wrapper.command) else {
            continue;
        };
        if command.parent() == Some(bin_dir) {
            continue;
        }
        write_rtk_command_wrapper(
            &bin_dir.join(wrapper.command),
            rtk,
            &command,
            wrapper.subcommands,
        )?;
    }
    Ok(())
}

struct RtkCommandWrapper {
    command: &'static str,
    subcommands: &'static [&'static str],
}

fn rtk_command_wrappers() -> &'static [RtkCommandWrapper] {
    &[
        RtkCommandWrapper {
            command: "git",
            subcommands: &["diff", "show", "log", "status", "grep", "blame"],
        },
        RtkCommandWrapper {
            command: "cargo",
            subcommands: &["test", "build", "check", "clippy", "bench", "run"],
        },
        RtkCommandWrapper {
            command: "npm",
            subcommands: &["test", "run", "build", "install", "ci", "update", "audit"],
        },
        RtkCommandWrapper {
            command: "yarn",
            subcommands: &["test", "run", "build", "install", "add", "upgrade"],
        },
        RtkCommandWrapper {
            command: "pnpm",
            subcommands: &["test", "run", "build", "install", "add", "update"],
        },
        RtkCommandWrapper {
            command: "bun",
            subcommands: &["test", "run", "build", "install", "add"],
        },
        RtkCommandWrapper {
            command: "pytest",
            subcommands: &[],
        },
        RtkCommandWrapper {
            command: "go",
            subcommands: &["test", "build", "vet"],
        },
        RtkCommandWrapper {
            command: "docker",
            subcommands: &["build", "compose", "logs", "pull", "push", "run"],
        },
        RtkCommandWrapper {
            command: "kubectl",
            subcommands: &["logs", "describe", "get", "events", "top"],
        },
        RtkCommandWrapper {
            command: "rg",
            subcommands: &[],
        },
        RtkCommandWrapper {
            command: "find",
            subcommands: &[],
        },
        RtkCommandWrapper {
            command: "ls",
            subcommands: &[],
        },
        RtkCommandWrapper {
            command: "tree",
            subcommands: &[],
        },
    ]
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
            let candidate = dir.join(format!("{command}.exe"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn write_rtk_command_wrapper(
    path: &Path,
    rtk: &Path,
    command: &Path,
    subcommands: &[&str],
) -> Result<()> {
    let rtk = shell_single_quote(&rtk.display().to_string());
    let command = shell_single_quote(&command.display().to_string());
    let rtk_env = rtk_state_exports();
    let routing = if subcommands.is_empty() {
        format!("export PRODEX_RTK_AUTO_WRAP_DEPTH=1\n{rtk_env}exec {rtk} {command} \"$@\"\n")
    } else {
        let pattern = subcommands
            .iter()
            .map(|subcommand| subcommand.replace('|', "\\|"))
            .collect::<Vec<_>>()
            .join("|");
        format!(
            "for prodex_rtk_arg in \"$@\"; do\n  case \"$prodex_rtk_arg\" in\n    {pattern})\n      export PRODEX_RTK_AUTO_WRAP_DEPTH=1\n      {rtk_env}exec {rtk} {command} \"$@\"\n      ;;\n  esac\ndone\nexec {command} \"$@\"\n",
        )
    };
    let script = format!(
        "#!/usr/bin/env sh\nif [ \"${{PRODEX_RTK_DISABLE_AUTO_WRAP:-}}\" = \"1\" ] || [ -n \"${{PRODEX_RTK_AUTO_WRAP_DEPTH:-}}\" ]; then\n  exec {command} \"$@\"\nfi\n{routing}",
    );
    write_executable_script(path, &script)
}

fn write_shell_wrapper(path: &Path, command: &Path, args: &[&str]) -> Result<()> {
    let args = args
        .iter()
        .map(|arg| format!(" {}", shell_single_quote(arg)))
        .collect::<String>();
    let rtk_env = rtk_state_exports();
    let script = format!(
        "#!/usr/bin/env sh\n{}exec {}{} \"$@\"\n",
        rtk_env,
        shell_single_quote(&command.display().to_string()),
        args
    );
    write_executable_script(path, &script)
}

fn write_unavailable_command_wrapper(path: &Path, command: &str) -> Result<()> {
    let message = shell_single_quote(&format!(
        "prodex: {command} requested but '{command}' is not installed or not on PATH"
    ));
    let script = format!("#!/usr/bin/env sh\necho {message} >&2\nexit 127\n");
    write_executable_script(path, &script)
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn rtk_state_exports() -> String {
    let Some(root) =
        prodex_home_from_env().map(|home| home.join("optimizer-state").join(RTK_STATE_DIR_NAME))
    else {
        return String::new();
    };
    [
        ("RTK_DB_PATH", root.join("history.db")),
        ("RTK_TEE_DIR", root.join("tee")),
        ("RTK_AUDIT_DIR", root.join("audit")),
    ]
    .into_iter()
    .map(|(key, value)| {
        format!(
            "export {}={}\n",
            key,
            shell_single_quote(&value.display().to_string())
        )
    })
    .collect()
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
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        env::temp_dir().join(format!(
            "prodex-rtk-wrapper-{name}-{}-{stamp}",
            std::process::id()
        ))
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
        assert!(script.contains("optimizer-state/rtk/history.db"));
        assert!(script.contains("test|build|check"));
        assert!(script.contains("exec '/opt/rtk/bin/rtk' '/usr/bin/cargo' \"$@\""));
        assert!(script.contains("exec '/usr/bin/cargo' \"$@\""));

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
        assert!(script.contains("exec '/opt/rtk/bin/rtk' '/usr/bin/rg' \"$@\""));
        assert!(script.contains("exec '/usr/bin/rg' \"$@\""));

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
}
