use anyhow::{Context, Result};
use prodex_cli::ProdexUpdateArgs;
use prodex_update_notice::{ProdexUpdateDecision, prodex_update_decision};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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
            println!("Prodex {current_version} is already up to date.");
            Ok(None)
        }
        ProdexUpdateDecision::LocalNewer(_) => {
            println!(
                "Installed Prodex {current_version} is newer than latest stable {target_version}."
            );
            println!("No changes made.");
            Ok(None)
        }
        ProdexUpdateDecision::UpdateAvailable(_) => {
            let install_lock = prodex_update_notice::acquire_prodex_update_lock(&paths)?;
            let installed_version = prodex_version_from_binary(&running_exe)?;
            match prodex_update_decision(&installed_version, &target_version)? {
                ProdexUpdateDecision::UpToDate => {
                    println!("Prodex {installed_version} is already up to date.");
                    Ok(None)
                }
                ProdexUpdateDecision::LocalNewer(_) => {
                    println!(
                        "Installed Prodex {installed_version} is newer than latest stable {target_version}."
                    );
                    println!("No changes made.");
                    Ok(None)
                }
                ProdexUpdateDecision::UpdateAvailable(_) => {
                    println!("Updating Prodex {installed_version} → {target_version}...");
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
        .spawn()
        .context("failed to start the embedded Prodex installer with sh")?;
    child
        .stdin
        .take()
        .context("failed to open Prodex installer stdin")?
        .write_all(include_bytes!("../../../../install.sh"))
        .context("failed to send the embedded Prodex installer to sh")?;
    let status = child
        .wait()
        .context("failed to wait for Prodex installer")?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("Prodex installer exited with {status}")
    }
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
        .spawn()
        .context("failed to start the embedded Prodex installer with PowerShell")?;
    child
        .stdin
        .take()
        .context("failed to open Prodex installer stdin")?
        .write_all(include_bytes!("../../../../install.ps1"))
        .context("failed to send the embedded Prodex installer to PowerShell")?;
    let status = child
        .wait()
        .context("failed to wait for Prodex installer")?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("Prodex installer exited with {status}")
    }
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn handle_prodex_update(_args: ProdexUpdateArgs) -> Result<()> {
    anyhow::bail!(
        "prodex update supports macOS, Linux, and Windows; download a binary from https://github.com/christiandoxa/prodex/releases/latest"
    )
}
