use std::io::IsTerminal;

use crate::{ExportProfileArgs, ProfileExportPayload, print_stderr_line, print_stderr_prompt};
use anyhow::{Context, Result, bail};
use std::path::Path;
use std::{env, io};

const PROFILE_EXPORT_PASSWORD_ENV: &str = "PRODEX_PROFILE_EXPORT_PASSWORD";
const PROFILE_IMPORT_PASSWORD_ENV: &str = "PRODEX_PROFILE_IMPORT_PASSWORD";

pub(super) fn resolve_export_password_mode(args: &ExportProfileArgs) -> Result<bool> {
    if args.password_protect {
        return Ok(true);
    }
    if args.no_password {
        return Ok(false);
    }
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        return Ok(false);
    }
    prompt_yes_no("Password-protect export file? [y/N]: ", false)
}

pub(super) fn resolve_export_password() -> Result<String> {
    if let Ok(password) = env::var(PROFILE_EXPORT_PASSWORD_ENV)
        && !password.trim().is_empty()
    {
        return Ok(password);
    }
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        bail!(
            "password protection requested but no interactive terminal is available; set {}",
            PROFILE_EXPORT_PASSWORD_ENV
        );
    }

    let password = rpassword::prompt_password("Export password: ")
        .context("failed to read export password")?;
    if password.is_empty() {
        bail!("export password cannot be empty");
    }
    let confirmation = rpassword::prompt_password("Confirm export password: ")
        .context("failed to read export password confirmation")?;
    if password != confirmation {
        bail!("export passwords did not match");
    }
    Ok(password)
}

pub(super) fn resolve_import_password() -> Result<String> {
    if let Ok(password) = env::var(PROFILE_IMPORT_PASSWORD_ENV)
        && !password.trim().is_empty()
    {
        return Ok(password);
    }
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        bail!(
            "profile export bundle is password-protected; set {} or rerun in a terminal",
            PROFILE_IMPORT_PASSWORD_ENV
        );
    }

    let password = rpassword::prompt_password("Export password: ")
        .context("failed to read import password")?;
    if password.is_empty() {
        bail!("import password cannot be empty");
    }
    Ok(password)
}

fn prompt_yes_no(prompt: &str, default: bool) -> Result<bool> {
    let mut input = String::new();
    loop {
        print_stderr_prompt(prompt)?;
        input.clear();
        io::stdin()
            .read_line(&mut input)
            .context("failed to read prompt response")?;
        match input.trim().to_ascii_lowercase().as_str() {
            "" => return Ok(default),
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => {
                print_stderr_line("Please answer yes or no.");
            }
        }
    }
}

pub(super) fn read_profile_export_payload(path: &Path) -> Result<(ProfileExportPayload, bool)> {
    let (envelope, encrypted) = prodex_profile_export::read_profile_export_envelope(path)?;
    let payload =
        prodex_profile_export::decode_profile_export_envelope(envelope, resolve_import_password)?;
    Ok((payload, encrypted))
}
