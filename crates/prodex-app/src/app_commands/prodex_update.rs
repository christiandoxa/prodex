use anyhow::{Context, Result};
use prodex_cli::ProdexUpdateArgs;
use prodex_update_notice::{ProdexUpdateDecision, prodex_update_decision};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

struct ProdexUpdatePreparation {
    running_exe: PathBuf,
    target_version: String,
    _install_lock: File,
}

fn prepare_prodex_update() -> Result<Option<ProdexUpdatePreparation>> {
    let running_exe = std::env::current_exe().context("failed to locate current prodex binary")?;
    let current_version = prodex_update_notice::current_prodex_version();
    let paths = prodex_core::AppPaths::discover()?;
    let target_version = prodex_update_notice::latest_prodex_version_for_update(&paths)?;
    match prodex_update_decision(current_version, &target_version)? {
        ProdexUpdateDecision::UpToDate => {
            print_update_panel(
                "up to date",
                current_version,
                &target_version,
                &[format!("Prodex {current_version} is already up to date.")],
            )?;
            Ok(None)
        }
        ProdexUpdateDecision::LocalNewer(_) => {
            print_update_panel(
                "local version is newer",
                current_version,
                &target_version,
                &[
                    format!(
                        "Installed Prodex {current_version} is newer than latest stable {target_version}."
                    ),
                    "No changes made.".to_string(),
                ],
            )?;
            Ok(None)
        }
        ProdexUpdateDecision::UpdateAvailable(_) => {
            let install_lock = prodex_update_notice::acquire_prodex_update_lock(&paths)?;
            let installed_version = prodex_version_from_binary(&running_exe)?;
            match prodex_update_decision(&installed_version, &target_version)? {
                ProdexUpdateDecision::UpToDate => {
                    print_update_panel(
                        "up to date",
                        &installed_version,
                        &target_version,
                        &[format!("Prodex {installed_version} is already up to date.")],
                    )?;
                    Ok(None)
                }
                ProdexUpdateDecision::LocalNewer(_) => {
                    print_update_panel(
                        "local version is newer",
                        &installed_version,
                        &target_version,
                        &[
                            format!(
                                "Installed Prodex {installed_version} is newer than latest stable {target_version}."
                            ),
                            "No changes made.".to_string(),
                        ],
                    )?;
                    Ok(None)
                }
                ProdexUpdateDecision::UpdateAvailable(_) => {
                    print_update_panel(
                        "updating",
                        &installed_version,
                        &target_version,
                        &[format!(
                            "Updating Prodex {installed_version} → {target_version}..."
                        )],
                    )?;
                    Ok(Some(ProdexUpdatePreparation {
                        running_exe,
                        target_version,
                        _install_lock: install_lock,
                    }))
                }
            }
        }
    }
}

fn print_update_panel(
    status: &str,
    installed_version: &str,
    target_version: &str,
    fallback_lines: &[String],
) -> Result<()> {
    super::print_user_stdout_panel(
        "Prodex Update",
        &[
            ("Installed".to_string(), installed_version.to_string()),
            ("Latest".to_string(), target_version.to_string()),
            ("Status".to_string(), status.to_string()),
        ],
        fallback_lines,
    )
}

fn prodex_version_from_binary(path: &Path) -> Result<String> {
    let output = Command::new(path)
        .arg("--version")
        .output()
        .with_context(|| format!("failed to inspect installed Prodex at {}", path.display()))?;
    if !output.status.success() {
        anyhow::bail!("installed Prodex version probe failed");
    }
    let output =
        String::from_utf8(output.stdout).context("installed Prodex version was not UTF-8")?;
    let version = output
        .trim()
        .strip_prefix("prodex ")
        .context("installed Prodex version probe returned unexpected output")?;
    prodex_update_decision(version, version)
        .context("installed Prodex version probe returned an invalid version")?;
    Ok(version.to_string())
}

#[cfg(unix)]
pub(crate) fn handle_prodex_update(_args: ProdexUpdateArgs) -> Result<()> {
    let Some(preparation) = prepare_prodex_update()? else {
        return Ok(());
    };
    let mut child = Command::new("sh")
        .arg("-s")
        .arg("--")
        .env("PRODEX_RUNNING_EXE", &preparation.running_exe)
        .env("PRODEX_RELEASE", &preparation.target_version)
        .env("PRODEX_MIGRATE", "1")
        .env("PRODEX_NON_INTERACTIVE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to start the embedded Prodex installer with sh")?;
    child
        .stdin
        .take()
        .context("failed to open Prodex installer stdin")?
        .write_all(include_bytes!("../../../../install.sh"))
        .context("failed to send the embedded Prodex installer to sh")?;
    finish_prodex_update(child, &preparation.target_version)
}

#[cfg(windows)]
pub(crate) fn handle_prodex_update(_args: ProdexUpdateArgs) -> Result<()> {
    let Some(preparation) = prepare_prodex_update()? else {
        return Ok(());
    };
    let mut child = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "-",
        ])
        .env("PRODEX_RUNNING_EXE", &preparation.running_exe)
        .env("PRODEX_RELEASE", &preparation.target_version)
        .env("PRODEX_MIGRATE", "1")
        .env("PRODEX_NON_INTERACTIVE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to start the embedded Prodex installer with PowerShell")?;
    child
        .stdin
        .take()
        .context("failed to open Prodex installer stdin")?
        .write_all(include_bytes!("../../../../install.ps1"))
        .context("failed to send the embedded Prodex installer to PowerShell")?;
    finish_prodex_update(child, &preparation.target_version)
}

fn finish_prodex_update(child: Child, target_version: &str) -> Result<()> {
    let output = child
        .wait_with_output()
        .context("failed to wait for Prodex installer")?;
    let stdout = bounded_update_output(&output.stdout);
    let stderr = bounded_update_output(&output.stderr);
    if !stdout.trim().is_empty() {
        super::print_user_stdout_text_panel("Prodex Update Output", &stdout)?;
    }
    if !stderr.trim().is_empty() {
        super::print_user_stderr_text_panel("Prodex Update Diagnostics", &stderr)?;
    }
    if output.status.success() {
        print_update_panel(
            "updated",
            target_version,
            target_version,
            &[format!("Prodex updated to {target_version}.")],
        )
    } else {
        anyhow::bail!("Prodex installer exited with {}", output.status)
    }
}

fn bounded_update_output(bytes: &[u8]) -> String {
    const MAX_CHARS: usize = 16 * 1024;
    let text = String::from_utf8_lossy(bytes);
    let redacted = crate::redaction_redact_secret_like_text(&text);
    let mut sanitized = redacted
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .take(MAX_CHARS)
        .collect::<String>();
    if redacted.chars().count() > MAX_CHARS {
        sanitized.push_str("\n… output truncated …");
    }
    sanitized
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn handle_prodex_update(_args: ProdexUpdateArgs) -> Result<()> {
    anyhow::bail!(
        "prodex update supports macOS, Linux, and Windows; download a binary from https://github.com/christiandoxa/prodex/releases/latest"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_output_is_redacted_bounded_and_terminal_safe() {
        let payload = format!(
            "Authorization: Bearer fixture-update-token\n{}\u{1b}",
            "x".repeat(20_000)
        );
        let output = bounded_update_output(payload.as_bytes());
        assert!(!output.contains("fixture-update-token"));
        assert!(!output.contains('\u{1b}'));
        assert!(output.contains("output truncated"));
        assert!(output.chars().count() < 17_000);
    }
}
