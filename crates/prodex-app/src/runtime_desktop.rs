use crate::AppPaths;
use anyhow::{Context, Result, bail};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[cfg(target_os = "linux")]
const LINUX_DESKTOP_PROJECT: &str = "https://github.com/ilysenko/codex-desktop-linux";
const DESKTOP_HISTORY_REPAIR_PAGE_LIMIT: u64 = 100;
const DESKTOP_HISTORY_REPAIR_TIMEOUT: Duration = Duration::from_secs(60);

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
    sqlite_home: Option<&Path>,
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
    if let Some(sqlite_home) = sqlite_home {
        let sqlite_home = sqlite_home
            .to_str()
            .context("shared Codex SQLite home must be UTF-8")?;
        config
            .as_table_mut()
            .context("Codex desktop config must be a TOML table")?
            .insert(
                "sqlite_home".to_string(),
                toml::Value::String(sqlite_home.to_string()),
            );
    }

    let rendered = toml::to_string_pretty(&config).context("failed to render desktop config")?;
    fs::write(&config_path, rendered)
        .with_context(|| format!("failed to write {}", config_path.display()))
}

pub(crate) fn repair_desktop_thread_index(
    codex_binary: &OsStr,
    codex_home: &Path,
    sqlite_home: &Path,
) -> Result<()> {
    let mut child = Command::new(codex_binary)
        .arg("app-server")
        .env("CODEX_HOME", codex_home)
        .env("CODEX_SQLITE_HOME", sqlite_home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| {
            format!(
                "failed to start {} app-server for Desktop history repair",
                codex_binary.to_string_lossy()
            )
        })?;
    let stdin = child
        .stdin
        .take()
        .context("failed to capture Desktop history repair stdin")?;
    let stdout = child
        .stdout
        .take()
        .context("failed to capture Desktop history repair stdout")?;
    let (completion_tx, completion_rx) = mpsc::channel();
    let worker = thread::Builder::new()
        .name("prodex-desktop-history-repair".to_string())
        .spawn(move || {
            let result = repair_desktop_thread_index_protocol(
                &mut BufReader::new(stdout),
                &mut BufWriter::new(stdin),
            );
            let _ = completion_tx.send(result);
        });
    let worker = match worker {
        Ok(worker) => worker,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error).context("failed to start Desktop history repair worker");
        }
    };

    let result = match completion_rx.recv_timeout(DESKTOP_HISTORY_REPAIR_TIMEOUT) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            Err(anyhow::anyhow!("Desktop history repair timed out"))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err(anyhow::anyhow!("Desktop history repair worker stopped"))
        }
    };
    let _ = child.kill();
    let _ = child.wait();
    worker
        .join()
        .map_err(|_| anyhow::anyhow!("Desktop history repair worker panicked"))?;
    result
}

fn repair_desktop_thread_index_protocol(
    reader: &mut impl BufRead,
    writer: &mut impl Write,
) -> Result<()> {
    let mut request_id = 1_u64;
    write_app_server_message(
        writer,
        &serde_json::json!({
            "id": request_id,
            "method": "initialize",
            "params": {
                "clientInfo": {
                    "name": "prodex-desktop-history-repair",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            }
        }),
    )?;
    read_app_server_response(reader, request_id)?;
    write_app_server_message(writer, &serde_json::json!({"method": "initialized"}))?;

    for archived in [false, true] {
        let mut cursor = None;
        let mut seen_cursors = std::collections::HashSet::new();
        loop {
            request_id += 1;
            write_app_server_message(
                writer,
                &serde_json::json!({
                    "id": request_id,
                    "method": "thread/list",
                    "params": {
                        "archived": archived,
                        "cursor": cursor,
                        "limit": DESKTOP_HISTORY_REPAIR_PAGE_LIMIT,
                        "modelProviders": [],
                        "sortKey": "updated_at",
                        "sourceKinds": [],
                        "useStateDbOnly": false,
                    }
                }),
            )?;
            let result = read_app_server_response(reader, request_id)?;
            let next_cursor = match result.get("nextCursor") {
                None | Some(serde_json::Value::Null) => None,
                Some(serde_json::Value::String(cursor)) => Some(cursor.clone()),
                Some(_) => bail!("Codex app-server returned an invalid thread list cursor"),
            };
            let Some(next_cursor) = next_cursor else {
                break;
            };
            if !seen_cursors.insert(next_cursor.clone()) {
                bail!("Codex app-server repeated a thread list cursor");
            }
            cursor = Some(next_cursor);
        }
    }
    Ok(())
}

fn write_app_server_message(writer: &mut impl Write, message: &serde_json::Value) -> Result<()> {
    serde_json::to_writer(&mut *writer, message).context("failed to encode app-server request")?;
    writer.write_all(b"\n")?;
    writer.flush().context("failed to send app-server request")
}

fn read_app_server_response(
    reader: &mut impl BufRead,
    request_id: u64,
) -> Result<serde_json::Value> {
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            bail!("Codex app-server stopped during Desktop history repair");
        }
        let mut message: serde_json::Value = serde_json::from_str(&line)
            .context("Codex app-server returned invalid JSON during Desktop history repair")?;
        if message.get("id").and_then(serde_json::Value::as_u64) != Some(request_id) {
            continue;
        }
        if let Some(error) = message.get("error") {
            let detail = error
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown app-server error");
            bail!("Desktop history repair failed: {detail}");
        }
        return message
            .get_mut("result")
            .map(serde_json::Value::take)
            .context("Codex app-server response is missing its result");
    }
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

        let sqlite_home = home.join("shared-sqlite");
        configure_desktop_codex_home(&home, &args, true, Some(&sqlite_home)).unwrap();

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
        assert_eq!(
            config["sqlite_home"].as_str(),
            sqlite_home.to_str(),
            "Desktop must read the cross-profile SQLite index"
        );
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn desktop_history_repair_scans_every_active_and_archived_page() {
        let responses = concat!(
            "{\"id\":1,\"result\":{}}\n",
            "{\"method\":\"remoteControl/status/changed\",\"params\":{}}\n",
            "{\"id\":2,\"result\":{\"data\":[],\"nextCursor\":\"active-next\"}}\n",
            "{\"id\":3,\"result\":{\"data\":[],\"nextCursor\":null}}\n",
            "{\"id\":4,\"result\":{\"data\":[],\"nextCursor\":null}}\n",
        );
        let mut reader = std::io::Cursor::new(responses.as_bytes());
        let mut written = Vec::new();

        repair_desktop_thread_index_protocol(&mut reader, &mut written).unwrap();

        let requests = String::from_utf8(written)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(requests.len(), 5);
        assert_eq!(requests[0]["method"], "initialize");
        assert_eq!(requests[1]["method"], "initialized");
        assert_eq!(requests[2]["method"], "thread/list");
        assert_eq!(requests[2]["params"]["archived"], false);
        assert_eq!(requests[2]["params"]["cursor"], serde_json::Value::Null);
        assert_eq!(requests[2]["params"]["useStateDbOnly"], false);
        assert_eq!(
            requests[2]["params"]["modelProviders"],
            serde_json::json!([])
        );
        assert_eq!(requests[3]["params"]["cursor"], "active-next");
        assert_eq!(requests[4]["params"]["archived"], true);
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

        let err = configure_desktop_codex_home(&home, &[], false, None).unwrap_err();

        assert!(err.to_string().contains("refusing to write symlinked"));
        assert_eq!(fs::read_to_string(outside).unwrap(), "model = \"safe\"\n");
        let _ = fs::remove_dir_all(home);
    }
}
