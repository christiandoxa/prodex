use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, OwnedHandle};

const OPENAI_TUNNEL_CLIENT_RELEASE: &str = "v0.0.13";
const OPENAI_TUNNEL_CLIENT_COMMIT: &str = "4b5267f823be0b046bb883aacb51603cfde3a0ea";
const OPENAI_TUNNEL_CLIENT_READY_TIMEOUT: Duration = if cfg!(test) {
    Duration::from_secs(3)
} else {
    Duration::from_secs(20)
};
const OPENAI_TUNNEL_CLIENT_READY_POLL: Duration = Duration::from_millis(100);
const OPENAI_TUNNEL_CLIENT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const OPENAI_TUNNEL_HEALTH_REQUEST_TIMEOUT: Duration = Duration::from_millis(750);
const OPENAI_TUNNEL_HEALTH_URL_MAX_BYTES: u64 = 4096;
const OPENAI_TUNNEL_ID_LENGTH: usize = "tunnel_".len() + 32;
const OPENAI_TUNNEL_VERSION_MAX_BYTES: usize = 32;
const OPENAI_TUNNEL_API_KEY_MAX_BYTES: usize = 4096;
static NEXT_OPENAI_TUNNEL_CONFIG_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::expose) struct OpenAiTunnelStatus {
    pub(in crate::expose) tunnel_id: String,
    pub(in crate::expose) client_version: String,
}

pub(in crate::expose) struct OpenAiTunnelCredentials {
    tunnel_id: String,
    api_key: Zeroizing<String>,
}

impl fmt::Debug for OpenAiTunnelCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenAiTunnelCredentials")
            .field("tunnel_id", &self.tunnel_id)
            .field("api_key", &"<redacted>")
            .finish()
    }
}

impl OpenAiTunnelCredentials {
    pub(in crate::expose) fn new(
        tunnel_id: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Result<Self> {
        let tunnel_id = tunnel_id.into().trim().to_owned();
        validate_openai_tunnel_id(&tunnel_id)?;
        let api_key = Zeroizing::new(api_key.into());
        if api_key.is_empty()
            || api_key.len() > OPENAI_TUNNEL_API_KEY_MAX_BYTES
            || api_key.chars().any(char::is_control)
        {
            bail!("OpenAI Secure MCP Tunnel API key is invalid")
        }
        Ok(Self { tunnel_id, api_key })
    }

    pub(in crate::expose) fn tunnel_id(&self) -> &str {
        &self.tunnel_id
    }

    pub(in crate::expose) fn api_key(&self) -> &str {
        &self.api_key
    }
}

pub(in crate::expose) struct OpenAiTunnel {
    child: std::process::Child,
    pub(in crate::expose) status: OpenAiTunnelStatus,
    files: OpenAiTunnelFiles,
    shut_down: bool,
    #[cfg(windows)]
    process_job: Option<OwnedHandle>,
}

struct OpenAiTunnelFiles {
    directory: PathBuf,
    config: PathBuf,
    mcp_url: PathBuf,
    health_url: PathBuf,
    log: PathBuf,
    cleaned: bool,
}

impl OpenAiTunnelFiles {
    fn create(local_mcp_url: &str, tunnel_id: &str) -> Result<Self> {
        let local_mcp_url = validate_local_mcp_url(local_mcp_url)?;
        validate_openai_tunnel_id(tunnel_id)?;
        let unique_id = NEXT_OPENAI_TUNNEL_CONFIG_ID.fetch_add(1, Ordering::Relaxed);
        let mut directory = None;
        for attempt in 0..16 {
            let candidate = std::env::temp_dir().join(format!(
                "prodex-openai-tunnel-{}-{unique_id}-{attempt}",
                std::process::id()
            ));
            match fs::create_dir(&candidate) {
                Ok(()) => {
                    directory = Some(candidate);
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => {
                    return Err(error).context("failed to create private OpenAI tunnel directory");
                }
            }
        }
        let directory = directory.context("failed to create private OpenAI tunnel directory")?;
        let files = Self {
            config: directory.join("config.yaml"),
            mcp_url: directory.join("mcp-url"),
            health_url: directory.join("health-url"),
            log: directory.join("tunnel-client.log"),
            directory,
            cleaned: false,
        };
        secret_store::ensure_private_directory(&files.directory)
            .context("failed to secure private OpenAI tunnel directory")?;
        for path in [&files.config, &files.mcp_url, &files.health_url, &files.log] {
            if path
                .to_str()
                .is_none_or(|value| value.chars().any(char::is_control))
            {
                bail!("private OpenAI tunnel path is not safe for configuration")
            }
            secret_store::write_private_file_create_new(path, &[])
                .context("failed to create private OpenAI tunnel file")?;
        }
        secret_store::write_private_file_atomic(&files.mcp_url, local_mcp_url.as_bytes())
            .context("failed to write private MCP endpoint reference")?;
        secret_store::write_private_file_atomic(
            &files.config,
            openai_tunnel_config(&files.mcp_url).as_bytes(),
        )
        .context("failed to write private OpenAI tunnel configuration")?;
        Ok(files)
    }

    fn cleanup(&mut self) {
        if self.cleaned {
            return;
        }
        self.cleaned = true;
        let _ = fs::remove_dir_all(&self.directory);
    }
}

impl Drop for OpenAiTunnelFiles {
    fn drop(&mut self) {
        self.cleanup();
    }
}

pub(in crate::expose) fn resolve_openai_tunnel_id(explicit: Option<&str>) -> Result<String> {
    let value = explicit
        .map(str::to_owned)
        .or_else(|| std::env::var("CONTROL_PLANE_TUNNEL_ID").ok())
        .context(
            "OpenAI Secure MCP Tunnel requires CONTROL_PLANE_TUNNEL_ID or --openai-tunnel-id",
        )?;
    let value = value.trim();
    if value.is_empty() {
        bail!(
            "OpenAI Secure MCP Tunnel requires a non-empty CONTROL_PLANE_TUNNEL_ID or --openai-tunnel-id"
        )
    }
    validate_openai_tunnel_id(value)?;
    Ok(value.to_owned())
}

pub(in crate::expose) fn ensure_openai_tunnel_available(tunnel_id: &str) -> Result<String> {
    validate_openai_tunnel_id(tunnel_id)?;
    let mut command = openai_tunnel_client_command()?;
    command.arg("--version");
    remove_inherited_tunnel_configuration(&mut command);
    let output = crate::command_probe_output(&mut command, "tunnel-client version probe")
        .map_err(|_| openai_tunnel_install_error())?;
    if !output.status.success() {
        bail!(
            "tunnel-client --version failed; install the official openai/tunnel-client {OPENAI_TUNNEL_CLIENT_RELEASE} release"
        )
    }
    safe_client_version(&output).context("tunnel-client did not report a supported version")
}

pub(in crate::expose) fn openai_tunnel_credentials_from_env(
    tunnel_id: &str,
) -> Result<OpenAiTunnelCredentials> {
    let api_key = std::env::var("CONTROL_PLANE_API_KEY").map_err(|_| {
        anyhow::anyhow!(
            "OpenAI Secure MCP Tunnel requires CONTROL_PLANE_API_KEY in noninteractive mode"
        )
    })?;
    OpenAiTunnelCredentials::new(tunnel_id.to_owned(), api_key)
}

pub(in crate::expose) fn openai_tunnel_client_command() -> Result<Command> {
    let configured = std::env::var_os("PRODEX_TUNNEL_CLIENT_BIN")
        .unwrap_or_else(|| OsString::from("tunnel-client"));
    let binary =
        prodex_core::resolve_binary_path(&configured).ok_or_else(openai_tunnel_install_error)?;
    Ok(Command::new(binary))
}

pub(in crate::expose) fn start_openai_tunnel(
    local_mcp_url: &str,
    credentials: OpenAiTunnelCredentials,
    client_version: String,
    cancelled: &dyn Fn() -> bool,
) -> Result<OpenAiTunnel> {
    let tunnel_id = credentials.tunnel_id().to_owned();
    validate_openai_tunnel_id(&tunnel_id)?;
    let mut files = OpenAiTunnelFiles::create(local_mcp_url, &tunnel_id)?;
    let config_path = files
        .config
        .to_str()
        .context("OpenAI tunnel configuration path is not UTF-8")?;
    let health_url_path = files
        .health_url
        .to_str()
        .context("OpenAI tunnel health URL path is not UTF-8")?;
    let log_path = files
        .log
        .to_str()
        .context("OpenAI tunnel log path is not UTF-8")?;
    let mut command = openai_tunnel_client_command()?;
    remove_inherited_tunnel_configuration(&mut command);
    command
        .args(["run", "--config", config_path])
        .args(["--control-plane.base-url", "https://api.openai.com"])
        .args(["--control-plane.tunnel-id", tunnel_id.as_str()])
        .args(["--control-plane.api-key", "env:CONTROL_PLANE_API_KEY"])
        .args(["--health.listen-addr", "127.0.0.1:0"])
        .args(["--health.url-file", health_url_path])
        .args(["--log.file", log_path])
        // Pinned tunnel-client logs resolved MCP bearer URLs at INFO.
        .args(["--log.level", "warn"])
        .env("CONTROL_PLANE_API_KEY", credentials.api_key())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    crate::configure_child_process_group(&mut command, true);
    crate::configure_child_parent_death(&mut command);
    let mut child = command.spawn().context("failed to spawn tunnel-client")?;
    #[cfg(windows)]
    let process_job = super::assign_expose_process_job(child.as_raw_handle()).ok();
    match wait_for_openai_tunnel_ready(&mut child, &files.health_url, cancelled) {
        Ok(_) => {}
        Err(error) => {
            stop_child(&mut child);
            #[cfg(windows)]
            drop(process_job);
            files.cleanup();
            return Err(error);
        }
    };
    if let Some(status) = child
        .try_wait()
        .context("failed to inspect tunnel-client after readiness")?
    {
        stop_child(&mut child);
        #[cfg(windows)]
        drop(process_job);
        files.cleanup();
        bail!(
            "tunnel-client exited before local readiness completed (status {})",
            exit_status_label(status)
        )
    }
    Ok(OpenAiTunnel {
        child,
        status: OpenAiTunnelStatus {
            tunnel_id,
            client_version: safe_version_label(&client_version),
        },
        files,
        shut_down: false,
        #[cfg(windows)]
        process_job,
    })
}

impl OpenAiTunnel {
    pub(in crate::expose) fn exited(&mut self) -> Option<std::process::ExitStatus> {
        self.child.try_wait().ok().flatten()
    }

    pub(in crate::expose) fn shutdown(&mut self) {
        if self.shut_down {
            return;
        }
        self.shut_down = true;
        stop_child(&mut self.child);
        #[cfg(windows)]
        drop(self.process_job.take());
        self.files.cleanup();
    }
}

impl Drop for OpenAiTunnel {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn openai_tunnel_install_error() -> anyhow::Error {
    anyhow::anyhow!(
        "tunnel-client is required for OpenAI Secure MCP Tunnel mode; install the official openai/tunnel-client {OPENAI_TUNNEL_CLIENT_RELEASE} release ({OPENAI_TUNNEL_CLIENT_COMMIT}) from https://platform.openai.com/settings/organization/tunnels or set PRODEX_TUNNEL_CLIENT_BIN"
    )
}

fn remove_inherited_tunnel_configuration(command: &mut Command) {
    for variable in [
        "OPENAI_API_KEY",
        "OPENAI_API_KEYS",
        "OPENAI_ADMIN_KEY",
        "CONTROL_PLANE_API_KEY",
        "TUNNEL_CLIENT_CONFIG",
        "TUNNEL_CLIENT_PROFILE",
        "TUNNEL_CLIENT_PROFILE_FILE",
        "TUNNEL_CLIENT_PROFILE_DIR",
        "CONTROL_PLANE_BASE_URL",
        "CONTROL_PLANE_URL_PATH",
        "CONTROL_PLANE_TUNNEL_ID",
        "CONTROL_PLANE_POLL_CHANNELS",
        "MCP_SERVER_URL",
        "MCP_COMMAND",
        "HEALTH_LISTEN_ADDR",
        "HEALTH_UNIX_SOCKET",
        "HEALTH_URL_FILE",
        "LOG_FILE",
        "LOG_HTTP_RAW_UNSAFE",
        "CLOUDFLARED_TUNNEL_TOKEN",
        "CLOUDFLARED_MANAGED",
        "CLOUDFLARED_PATH",
        "CLOUDFLARED_READY_TIMEOUT",
        "TUNNEL_CONFIG",
        "TUNNEL_CERT",
        "TUNNEL_ORIGIN_CERT",
        "TUNNEL_CRED_FILE",
        "TUNNEL_HOSTNAME",
        "TUNNEL_NAME",
        "TUNNEL_TOKEN",
        "TUNNEL_TRANSPORT_PROTOCOL",
    ] {
        command.env_remove(variable);
    }
}

fn validate_openai_tunnel_id(value: &str) -> Result<()> {
    if value.len() != OPENAI_TUNNEL_ID_LENGTH
        || !value.starts_with("tunnel_")
        || !value["tunnel_".len()..]
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    {
        bail!("OpenAI tunnel id must match tunnel_<32 lowercase letters or digits>")
    }
    Ok(())
}

fn validate_local_mcp_url(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty()
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        bail!("OpenAI tunnel MCP endpoint must be a loopback HTTP URL")
    }
    let parsed = url::Url::parse(value)
        .map_err(|_| anyhow::anyhow!("OpenAI tunnel MCP endpoint must be a loopback HTTP URL"))?;
    let host = parsed
        .host_str()
        .context("OpenAI tunnel MCP endpoint must have a loopback host")?;
    let ip = host
        .parse::<IpAddr>()
        .map_err(|_| anyhow::anyhow!("OpenAI tunnel MCP endpoint must use a loopback IP"))?;
    if !ip.is_loopback()
        || !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.port().is_none_or(|port| port == 0)
    {
        bail!("OpenAI tunnel MCP endpoint must be a loopback HTTP URL")
    }
    Ok(value.to_owned())
}

fn openai_tunnel_config(mcp_url: &Path) -> String {
    format!(
        "config_version: 1\nmcp:\n  server_urls:\n    - channel: main\n      url: '{}'\n",
        yaml_quote(&format!("file:{}", mcp_url.to_string_lossy())),
    )
}

fn yaml_quote(value: &str) -> String {
    value.replace('\'', "''")
}

fn safe_client_version(output: &Output) -> Option<String> {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    text.split_whitespace().find_map(|token| {
        let token = token.trim_matches(|character: char| {
            !character.is_ascii_alphanumeric() && character != '.' && character != '-'
        });
        let token = token.strip_prefix('v').unwrap_or(token);
        let version = token.split(['-', '+']).next().unwrap_or_default();
        let parts = version.split('.').collect::<Vec<_>>();
        (parts.len() == 3
            && version.len() <= OPENAI_TUNNEL_VERSION_MAX_BYTES
            && parts
                .iter()
                .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit())))
        .then_some(version.to_owned())
    })
}

fn safe_version_label(value: &str) -> String {
    let output = Output {
        status: std::process::ExitStatus::default(),
        stdout: value.as_bytes().to_vec(),
        stderr: Vec::new(),
    };
    safe_client_version(&output).unwrap_or_else(|| "unknown".to_string())
}

fn wait_for_openai_tunnel_ready(
    child: &mut std::process::Child,
    health_url_path: &Path,
    cancelled: &dyn Fn() -> bool,
) -> Result<String> {
    let deadline = Instant::now() + OPENAI_TUNNEL_CLIENT_READY_TIMEOUT;
    let mut health = None;
    loop {
        if cancelled() {
            bail!("OpenAI Secure MCP Tunnel startup cancelled")
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                bail!(
                    "tunnel-client exited before local readiness (status {})",
                    exit_status_label(status)
                )
            }
            Ok(None) => {}
            Err(error) => return Err(error).context("failed to inspect tunnel-client startup"),
        }
        if health.is_none()
            && let Some(base_url) = read_health_base_url(health_url_path)?
        {
            let client = Client::builder()
                .no_proxy()
                .timeout(OPENAI_TUNNEL_HEALTH_REQUEST_TIMEOUT)
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .context("failed to initialize tunnel-client health probe")?;
            health = Some((client, base_url));
        }
        if let Some((client, base_url)) = health.as_ref()
            && tunnel_health_ready(client, base_url)
        {
            if let Some(status) = child
                .try_wait()
                .context("failed to inspect tunnel-client after readiness")?
            {
                bail!(
                    "tunnel-client exited before local readiness completed (status {})",
                    exit_status_label(status)
                )
            }
            return Ok(base_url.clone());
        }
        if Instant::now() >= deadline {
            bail!(
                "tunnel-client did not become locally ready within {} seconds; verify outbound HTTPS/TCP 443, tunnel permissions, and the configured MCP endpoint",
                OPENAI_TUNNEL_CLIENT_READY_TIMEOUT.as_secs()
            )
        }
        thread::sleep(OPENAI_TUNNEL_CLIENT_READY_POLL);
    }
}

fn read_health_base_url(path: &Path) -> Result<Option<String>> {
    let Some(bytes) =
        secret_store::read_private_file_bounded(path, OPENAI_TUNNEL_HEALTH_URL_MAX_BYTES)
            .context("failed to read tunnel-client health URL file")?
    else {
        return Ok(None);
    };
    let value = std::str::from_utf8(&bytes)
        .map_err(|_| anyhow::anyhow!("tunnel-client health URL file is not valid UTF-8"))?;
    if value.trim().is_empty() {
        return Ok(None);
    }
    parse_health_base_url(value).map(Some)
}

fn parse_health_base_url(raw: &str) -> Result<String> {
    let value = raw.trim().trim_end_matches('/');
    if value.is_empty()
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        bail!("tunnel-client health URL must be a loopback HTTP base URL")
    }
    let parsed = url::Url::parse(value).map_err(|_| {
        anyhow::anyhow!("tunnel-client health URL must be a loopback HTTP base URL")
    })?;
    let host = parsed
        .host_str()
        .context("tunnel-client health URL must have a loopback host")?;
    let ip = host
        .parse::<IpAddr>()
        .map_err(|_| anyhow::anyhow!("tunnel-client health URL must use a loopback IP"))?;
    if !ip.is_loopback()
        || parsed.scheme() != "http"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !matches!(parsed.path(), "" | "/")
        || parsed.port().is_none_or(|port| port == 0)
    {
        bail!("tunnel-client health URL must be a loopback HTTP base URL")
    }
    Ok(value.to_owned())
}

fn tunnel_health_ready(client: &Client, base_url: &str) -> bool {
    ["/healthz", "/readyz"].into_iter().all(|path| {
        client
            .get(format!("{base_url}{path}"))
            .send()
            .is_ok_and(|response| response.status().is_success())
    })
}

fn exit_status_label(status: std::process::ExitStatus) -> String {
    status
        .code()
        .map_or_else(|| "signal".to_string(), |code| code.to_string())
}

fn stop_child(child: &mut std::process::Child) {
    let _ = crate::terminate_child_process_tree(child, true);
    let deadline = Instant::now() + OPENAI_TUNNEL_CLIENT_SHUTDOWN_TIMEOUT;
    while Instant::now() < deadline {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[cfg(test)]
#[path = "openai_tunnel_tests.rs"]
mod tests;
