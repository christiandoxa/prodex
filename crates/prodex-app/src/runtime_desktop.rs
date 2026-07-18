use crate::AppPaths;
use anyhow::{Context, Result, bail};
#[cfg(target_os = "linux")]
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
const LINUX_DESKTOP_PROJECT: &str = "https://github.com/ilysenko/codex-desktop-linux";

#[derive(Clone)]
pub(crate) struct DesktopGuiCommand {
    pub(crate) binary: OsString,
    pub(crate) args: Vec<OsString>,
}

pub(crate) fn desktop_gui_command() -> Result<DesktopGuiCommand> {
    desktop_gui_command_for_platform()
}

pub(crate) fn configure_desktop_codex_home(
    codex_home: &Path,
    codex_args: &[OsString],
    full_access: bool,
) -> Result<()> {
    let config_path = codex_home.join("config.toml");
    if fs::symlink_metadata(&config_path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        bail!(
            "refusing to write symlinked desktop config {}",
            config_path.display()
        );
    }

    let raw = match fs::read_to_string(&config_path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read {}", config_path.display()));
        }
    };
    let mut config = if raw.trim().is_empty() {
        toml::Value::Table(toml::map::Map::new())
    } else {
        toml::from_str(&raw)
            .with_context(|| format!("failed to parse {}", config_path.display()))?
    };

    for assignment in desktop_config_assignments(codex_args)? {
        let patch: toml::Value = toml::from_str(&format!("{assignment}\n"))
            .map_err(|_| anyhow::anyhow!("invalid Codex desktop config override"))?;
        merge_toml(&mut config, patch);
    }
    if full_access {
        let patch: toml::Value =
            toml::from_str("approval_policy = \"never\"\nsandbox_mode = \"danger-full-access\"\n")?;
        merge_toml(&mut config, patch);
    }

    let rendered = toml::to_string_pretty(&config).context("failed to render desktop config")?;
    fs::write(&config_path, rendered)
        .with_context(|| format!("failed to write {}", config_path.display()))
}

pub(crate) fn prepare_runtime_overlay_home(
    paths: &AppPaths,
    base_codex_home: &Path,
) -> Result<PathBuf> {
    let sessions_are_managed = prodex_core::same_path(
        &base_codex_home.join("sessions"),
        &paths.shared_codex_root.join("sessions"),
    );
    if sessions_are_managed {
        prodex_shared_codex_fs::maintain_managed_codex_sessions(paths)?;
        return prodex_caveman_assets::prepare_runtime_overlay_home_from_prepared_base(
            &paths.managed_profiles_root,
            base_codex_home,
        );
    }
    prodex_caveman_assets::prepare_runtime_overlay_home(
        &paths.managed_profiles_root,
        base_codex_home,
    )
}

pub(crate) fn prepare_desktop_overlay_home(
    paths: &AppPaths,
    base_codex_home: &Path,
    configure_prodex: bool,
) -> Result<PathBuf> {
    let sessions_are_managed = prodex_core::same_path(
        &base_codex_home.join("sessions"),
        &paths.shared_codex_root.join("sessions"),
    );
    if sessions_are_managed {
        prodex_shared_codex_fs::maintain_managed_codex_sessions(paths)?;
        return prodex_caveman_assets::prepare_desktop_overlay_home_from_prepared_base(
            &paths.managed_profiles_root,
            base_codex_home,
            configure_prodex,
        );
    }
    prodex_caveman_assets::prepare_desktop_overlay_home(
        &paths.managed_profiles_root,
        base_codex_home,
        configure_prodex,
    )
}

fn desktop_config_assignments(args: &[OsString]) -> Result<Vec<&str>> {
    let mut assignments = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let Some(arg) = args[index].to_str() else {
            index += 1;
            continue;
        };
        if matches!(arg, "-c" | "--config") {
            let value = args
                .get(index + 1)
                .context("Codex desktop config flag is missing its value")?
                .to_str()
                .context("Codex desktop config override must be UTF-8")?;
            assignments.push(value);
            index += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--config=") {
            assignments.push(value);
        } else if let Some(value) = arg.strip_prefix("-c")
            && !value.is_empty()
        {
            assignments.push(value);
        }
        index += 1;
    }
    Ok(assignments)
}

fn merge_toml(target: &mut toml::Value, patch: toml::Value) {
    match (target, patch) {
        (toml::Value::Table(target), toml::Value::Table(patch)) => {
            for (key, value) in patch {
                match target.get_mut(&key) {
                    Some(current) => merge_toml(current, value),
                    None => {
                        target.insert(key, value);
                    }
                }
            }
        }
        (target, patch) => *target = patch,
    }
}

#[cfg(target_os = "linux")]
fn desktop_gui_command_for_platform() -> Result<DesktopGuiCommand> {
    desktop_gui_linux_command_in_path(std::env::var_os("PATH").as_deref())
}

#[cfg(target_os = "linux")]
fn desktop_gui_linux_command_in_path(path: Option<&OsStr>) -> Result<DesktopGuiCommand> {
    let binary = prodex_core::resolve_binary_path_in_path(&OsString::from("codex-desktop"), path)
        .with_context(|| {
        format!("codex-desktop is not installed; install it from {LINUX_DESKTOP_PROJECT}")
    })?;
    Ok(DesktopGuiCommand {
        binary: binary.into_os_string(),
        args: vec![OsString::from("--new-instance")],
    })
}

#[cfg(target_os = "macos")]
fn desktop_gui_command_for_platform() -> Result<DesktopGuiCommand> {
    let binary = macos_codex_app_executable().context(
        "Codex Desktop is not installed; run `codex app`, complete installation, close the app, then retry",
    )?;
    if macos_codex_app_is_running(&binary) {
        bail!("close the running Codex Desktop app before launching it through Prodex")
    }
    Ok(DesktopGuiCommand {
        binary: binary.into_os_string(),
        args: Vec::new(),
    })
}

#[cfg(target_os = "macos")]
fn macos_codex_app_is_running(binary: &Path) -> bool {
    use std::process::{Command, Stdio};

    let Some(name) = binary.file_name() else {
        return false;
    };
    Command::new("/usr/bin/pgrep")
        .arg("-x")
        .arg(name)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(target_os = "macos")]
fn macos_codex_app_executable() -> Option<PathBuf> {
    let mut application_dirs = vec![PathBuf::from("/Applications")];
    if let Some(home) = std::env::var_os("HOME") {
        application_dirs.push(PathBuf::from(home).join("Applications"));
    }
    application_dirs.into_iter().find_map(|dir| {
        ["ChatGPT.app", "Codex.app"]
            .into_iter()
            .find_map(|name| macos_bundle_executable(&dir.join(name)))
    })
}

#[cfg(target_os = "macos")]
fn macos_bundle_executable(app: &Path) -> Option<PathBuf> {
    use std::process::Command;

    let plist = app.join("Contents/Info.plist");
    let bundle_id = Command::new("/usr/bin/plutil")
        .args(["-extract", "CFBundleIdentifier", "raw", "-o", "-"])
        .arg(&plist)
        .output()
        .ok()?;
    if !bundle_id.status.success()
        || String::from_utf8_lossy(&bundle_id.stdout).trim() != "com.openai.codex"
    {
        return None;
    }
    let executable = Command::new("/usr/bin/plutil")
        .args(["-extract", "CFBundleExecutable", "raw", "-o", "-"])
        .arg(plist)
        .output()
        .ok()?;
    if !executable.status.success() {
        return None;
    }
    let path = app
        .join("Contents/MacOS")
        .join(String::from_utf8_lossy(&executable.stdout).trim());
    path.is_file().then_some(path)
}

#[cfg(target_os = "windows")]
fn desktop_gui_command_for_platform() -> Result<DesktopGuiCommand> {
    let binary = prodex_core::resolve_binary_path(&OsString::from("powershell.exe"))
        .context("PowerShell is required to launch Codex Desktop")?;
    Ok(DesktopGuiCommand {
        binary: binary.into_os_string(),
        args: [
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            WINDOWS_DESKTOP_LAUNCH_SCRIPT,
        ]
        .into_iter()
        .map(OsString::from)
        .collect(),
    })
}

#[cfg(target_os = "windows")]
const WINDOWS_DESKTOP_LAUNCH_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
if (Get-Process -Name ChatGPT,Codex -ErrorAction SilentlyContinue) {
  throw 'Close the running Codex Desktop app before launching it through Prodex.'
}
$pkg = Get-AppxPackage | Where-Object { $_.PackageFamilyName -like 'OpenAI.Codex_*' } | Sort-Object Version -Descending | Select-Object -First 1
if (-not $pkg) {
  throw 'Codex Desktop is not installed. Run `codex app`, complete installation, close the app, then retry.'
}
$manifest = $pkg | Get-AppxPackageManifest
$application = @($manifest.Package.Applications.Application | Where-Object { $_.Id -eq 'App' })[0]
if (-not $application) {
  throw 'The installed Codex Desktop package has no launchable App entry.'
}
$executable = Join-Path $pkg.InstallLocation ([string]$application.Executable)
$process = Start-Process -FilePath $executable -Wait -PassThru
exit $process.ExitCode
"#;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn desktop_gui_command_for_platform() -> Result<DesktopGuiCommand> {
    bail!("Codex Desktop is supported by Prodex only on Linux, macOS, and Windows")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "prodex-desktop-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn desktop_config_persists_proxy_and_full_access_overrides() {
        let home = temp_dir("config");
        fs::write(
            home.join("config.toml"),
            "model = \"old\"\n[features]\napps = true\n",
        )
        .unwrap();
        let args = [
            OsString::from("-c"),
            OsString::from("model=\"new\""),
            OsString::from("--config=features.apps=false"),
            OsString::from("-copenai_base_url=\"http://127.0.0.1:2455/v1\""),
        ];

        configure_desktop_codex_home(&home, &args, true).unwrap();

        let config: toml::Value =
            toml::from_str(&fs::read_to_string(home.join("config.toml")).unwrap()).unwrap();
        assert_eq!(config["model"].as_str(), Some("new"));
        assert_eq!(config["features"]["apps"].as_bool(), Some(false));
        assert_eq!(
            config["openai_base_url"].as_str(),
            Some("http://127.0.0.1:2455/v1")
        );
        assert_eq!(config["approval_policy"].as_str(), Some("never"));
        assert_eq!(config["sandbox_mode"].as_str(), Some("danger-full-access"));
        let _ = fs::remove_dir_all(home);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_desktop_command_uses_installed_new_instance_launcher() {
        use std::os::unix::fs::PermissionsExt;

        let bin = temp_dir("linux-bin");
        let launcher = bin.join("codex-desktop");
        fs::write(&launcher, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&launcher, fs::Permissions::from_mode(0o755)).unwrap();

        let command = desktop_gui_linux_command_in_path(Some(bin.as_os_str())).unwrap();

        assert_eq!(PathBuf::from(command.binary), launcher);
        assert_eq!(command.args, [OsString::from("--new-instance")]);
        let _ = fs::remove_dir_all(bin);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_desktop_resolves_the_official_codex_bundle() {
        let root = temp_dir("macos-bundle");
        let app = root.join("Codex.app");
        let macos = app.join("Contents/MacOS");
        fs::create_dir_all(&macos).unwrap();
        fs::write(
            app.join("Contents/Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.openai.codex</string>
<key>CFBundleExecutable</key><string>Codex</string>
</dict></plist>"#,
        )
        .unwrap();
        let executable = macos.join("Codex");
        fs::write(&executable, "fixture").unwrap();

        assert_eq!(macos_bundle_executable(&app), Some(executable));
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_desktop_uses_the_official_msix_launcher() {
        let command = desktop_gui_command_for_platform().unwrap();
        let binary = PathBuf::from(command.binary);

        assert!(binary.file_name().is_some_and(|name| {
            name.to_string_lossy()
                .eq_ignore_ascii_case("powershell.exe")
        }));
        assert_eq!(
            command.args,
            [
                OsString::from("-NoProfile"),
                OsString::from("-NonInteractive"),
                OsString::from("-Command"),
                OsString::from(WINDOWS_DESKTOP_LAUNCH_SCRIPT),
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn desktop_config_rejects_symlink_target() {
        use std::os::unix::fs::symlink;

        let home = temp_dir("symlink");
        let outside = home.join("outside.toml");
        fs::write(&outside, "model = \"safe\"\n").unwrap();
        symlink(&outside, home.join("config.toml")).unwrap();

        let err = configure_desktop_codex_home(&home, &[], false).unwrap_err();

        assert!(err.to_string().contains("refusing to write symlinked"));
        assert_eq!(fs::read_to_string(outside).unwrap(), "model = \"safe\"\n");
        let _ = fs::remove_dir_all(home);
    }
}
