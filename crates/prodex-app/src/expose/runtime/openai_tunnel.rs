use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use std::ffi::OsString;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const OPENAI_TUNNEL_READY_TIMEOUT: Duration = if cfg!(test) {
    Duration::from_secs(3)
} else {
    Duration::from_secs(20)
};
const OPENAI_TUNNEL_READY_POLL: Duration = Duration::from_millis(100);
const OPENAI_TUNNEL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const OPENAI_TUNNEL_ID_MAX_BYTES: usize = 128;
const OPENAI_TUNNEL_VERSION_MAX_BYTES: usize = 128;
const OPENAI_TUNNEL_HEALTH_URL_MAX_BYTES: u64 = 4096;
static NEXT_OPENAI_TUNNEL_CONFIG_ID: AtomicU64 = AtomicU64::new(1);

#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, OwnedHandle};

pub(in crate::expose) struct OpenAiTunnel {
    child: std::process::Child,
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
        let directory = (0..16)
            .map(|attempt| {
                std::env::temp_dir().join(format!(
                    "prodex-openai-tunnel-{}-{unique_id}-{attempt}",
                    std::process::id()
                ))
            })
            .find(|directory| std::fs::create_dir(directory).is_ok())
            .context("failed to create private OpenAI tunnel directory")?;
        let files = Self {
            config: directory.join("config.yaml"),
            mcp_url: directory.join("mcp-url"),
            health_url: directory.join("health-url"),
            log: directory.join("tunnel-client.log"),
            directory,
            cleaned: false,
        };
        let result = (|| {
            secret_store::ensure_private_directory(&files.directory)
                .context("failed to secure OpenAI tunnel directory")?;
            for path in [&files.config, &files.mcp_url, &files.health_url, &files.log] {
                secret_store::write_private_file_create_new(path, &[])
                    .context("failed to create private OpenAI tunnel file")?;
            }
            secret_store::write_private_file_atomic(&files.mcp_url, local_mcp_url.as_bytes())
                .context("failed to write private MCP endpoint reference")?;
            secret_store::write_private_file_atomic(
                &files.config,
                openai_tunnel_config(tunnel_id, &files.mcp_url, &files.health_url, &files.log)
                    .as_bytes(),
            )
            .context("failed to write private OpenAI tunnel configuration")?;
            Ok::<(), anyhow::Error>(())
        })();
        result?;
        Ok(files)
    }

    fn cleanup(&mut self) {
        if self.cleaned {
            return;
        }
        self.cleaned = true;
        let _ = std::fs::remove_dir_all(&self.directory);
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
        .or_else(|| {
            std::env::var("CONTROL_PLANE_TUNNEL_ID")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .context(
            "OpenAI Secure MCP Tunnel requires CONTROL_PLANE_TUNNEL_ID or --openai-tunnel-id",
        )?;
    validate_openai_tunnel_id(&value)?;
    Ok(value)
}

pub(in crate::expose) fn ensure_openai_tunnel_available(tunnel_id: &str) -> Result<String> {
    validate_openai_tunnel_id(tunnel_id)?;
    if std::env::var_os("CONTROL_PLANE_API_KEY")
        .map(|value| value.to_string_lossy().trim().is_empty())
        .unwrap_or(true)
    {
        bail!(
            "OpenAI Secure MCP Tunnel requires CONTROL_PLANE_API_KEY; configure the tunnel runtime key outside argv"
        );
    }
    let mut command = openai_tunnel_client_command();
    command.arg("--version");
    remove_inherited_tunnel_configuration(&mut command);
    command.env_remove("CONTROL_PLANE_API_KEY");
    let output = crate::command_probe_output(&mut command, "tunnel-client version probe")
        .map_err(|_| {
            anyhow::anyhow!(
                "tunnel-client is required for OpenAI Secure MCP Tunnel mode; install the official openai/tunnel-client release (stable v0.0.13 or newer) or set PRODEX_TUNNEL_CLIENT_BIN"
            )
        })?;
    if !output.status.success() {
        bail!(
            "tunnel-client --version failed; install the official openai/tunnel-client release (stable v0.0.13 or newer)"
        );
    }
    Ok(safe_client_version(&output).unwrap_or_else(|| "unknown".to_string()))
}

pub(in crate::expose) fn openai_tunnel_client_command() -> Command {
    let configured = std::env::var_os("PRODEX_TUNNEL_CLIENT_BIN")
        .unwrap_or_else(|| OsString::from("tunnel-client"));
    Command::new(prodex_core::resolve_binary_path(&configured).unwrap_or_else(|| configured.into()))
}

pub(in crate::expose) fn start_openai_tunnel(
    local_mcp_url: &str,
    tunnel_id: String,
    cancelled: &dyn Fn() -> bool,
) -> Result<OpenAiTunnel> {
    let mut files = OpenAiTunnelFiles::create(local_mcp_url, &tunnel_id)?;
    let config_path = files
        .config
        .to_str()
        .context("OpenAI tunnel configuration path is not UTF-8")?;
    let mut command = openai_tunnel_client_command();
    command
        .args(["run", "--config", config_path])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    remove_inherited_tunnel_configuration(&mut command);
    crate::configure_child_process_group(&mut command, true);
    crate::configure_child_parent_death(&mut command);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            files.cleanup();
            return Err(error).context("failed to spawn tunnel-client");
        }
    };
    #[cfg(windows)]
    let process_job = super::assign_expose_process_job(child.as_raw_handle()).ok();
    if let Err(error) = wait_for_openai_tunnel_ready(&mut child, &files.health_url, cancelled) {
        stop_child(&mut child);
        #[cfg(windows)]
        drop(process_job);
        files.cleanup();
        return Err(error);
    }
    if child
        .try_wait()
        .context("failed to inspect tunnel-client after readiness")?
        .is_some()
    {
        stop_child(&mut child);
        #[cfg(windows)]
        drop(process_job);
        files.cleanup();
        bail!("tunnel-client exited before local readiness completed")
    }
    Ok(OpenAiTunnel {
        child,
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

fn validate_openai_tunnel_id(value: &str) -> Result<()> {
    let value = value.trim();
    if value.len() > OPENAI_TUNNEL_ID_MAX_BYTES
        || !value.starts_with("tunnel_")
        || value.len() == "tunnel_".len()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        bail!("OpenAI tunnel id must be a valid tunnel_... identifier")
    }
    Ok(())
}

fn validate_local_mcp_url(value: &str) -> Result<String> {
    let value = value.trim();
    let parsed =
        url::Url::parse(value).context("OpenAI tunnel MCP endpoint must be a loopback HTTP URL")?;
    let host = parsed
        .host_str()
        .context("OpenAI tunnel MCP endpoint must have a loopback host")?;
    let loopback = host == "localhost"
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if !loopback
        || parsed.scheme() != "http"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_none_or(|port| port == 0)
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        bail!("OpenAI tunnel MCP endpoint must be a loopback HTTP URL")
    }
    Ok(value.to_string())
}

fn remove_inherited_tunnel_configuration(command: &mut Command) {
    for variable in [
        "OPENAI_API_KEY",
        "OPENAI_API_KEYS",
        "OPENAI_ADMIN_KEY",
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

fn openai_tunnel_config(tunnel_id: &str, mcp_url: &Path, health_url: &Path, log: &Path) -> String {
    format!(
        "config_version: 1\ncontrol_plane:\n  base_url: 'https://api.openai.com'\n  tunnel_id: '{}'\n  api_key: 'env:CONTROL_PLANE_API_KEY'\nmcp:\n  server_urls:\n    - channel: main\n      url: '{}'\nhealth:\n  listen_addr: '127.0.0.1:0'\n  url_file: '{}'\nlog:\n  level: warn\n  format: struct-text\n  file: '{}'\n",
        yaml_quote(tunnel_id),
        yaml_quote(&format!("file:{}", yaml_path(mcp_url))),
        yaml_quote(&yaml_path(health_url)),
        yaml_quote(&yaml_path(log)),
    )
}

fn yaml_path(path: &Path) -> String {
    path.to_string_lossy().replace('\'', "''")
}

fn yaml_quote(value: &str) -> String {
    value.replace('\'', "''")
}

fn safe_client_version(output: &std::process::Output) -> Option<String> {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    text.split_whitespace()
        .map(|token| {
            token.trim_matches(|character: char| {
                !character.is_ascii_alphanumeric() && character != '.' && character != '-'
            })
        })
        .find(|token| {
            !token.is_empty()
                && token.len() <= OPENAI_TUNNEL_VERSION_MAX_BYTES
                && token.chars().any(|character| character.is_ascii_digit())
                && token.contains('.')
        })
        .map(str::to_string)
}

fn wait_for_openai_tunnel_ready(
    child: &mut std::process::Child,
    health_url_path: &Path,
    cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    let deadline = Instant::now() + OPENAI_TUNNEL_READY_TIMEOUT;
    let mut client = None;
    loop {
        if cancelled() {
            bail!("OpenAI Secure MCP Tunnel startup cancelled")
        }
        if let Ok(Some(status)) = child.try_wait() {
            bail!(
                "tunnel-client exited before local readiness (status {})",
                status
                    .code()
                    .map_or_else(|| "signal".to_string(), |code| code.to_string())
            )
        }
        if client.is_none()
            && let Ok(Some(bytes)) = secret_store::read_private_file_bounded(
                health_url_path,
                OPENAI_TUNNEL_HEALTH_URL_MAX_BYTES,
            )
            && let Ok(value) = String::from_utf8(bytes.to_vec())
            && let Ok(base_url) = parse_health_base_url(&value)
        {
            client = Client::builder()
                .no_proxy()
                .timeout(Duration::from_millis(750))
                .build()
                .ok()
                .map(|client| (client, base_url));
        }
        if let Some((client, base_url)) = client.as_ref()
            && tunnel_health_ready(client, base_url)
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!(
                "tunnel-client did not become ready within {} seconds; verify outbound HTTPS/TCP 443 and the configured tunnel",
                OPENAI_TUNNEL_READY_TIMEOUT.as_secs()
            )
        }
        thread::sleep(OPENAI_TUNNEL_READY_POLL);
    }
}

fn parse_health_base_url(raw: &str) -> Result<String> {
    let value = raw.trim().trim_end_matches('/');
    let parsed = url::Url::parse(value).context("tunnel-client health URL is invalid")?;
    let host = parsed
        .host_str()
        .context("tunnel-client health URL has no host")?;
    let loopback = host == "localhost"
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if !loopback
        || parsed.scheme() != "http"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !matches!(parsed.path(), "" | "/")
        || parsed.port() == Some(0)
    {
        bail!("tunnel-client health URL must be a loopback HTTP base URL")
    }
    Ok(value.to_string())
}

fn tunnel_health_ready(client: &Client, base_url: &str) -> bool {
    ["/healthz", "/readyz"].into_iter().all(|path| {
        client
            .get(format!("{base_url}{path}"))
            .send()
            .is_ok_and(|response| response.status().is_success())
    })
}

fn stop_child(child: &mut std::process::Child) {
    let _ = crate::terminate_child_process_tree(child, true);
    let deadline = Instant::now() + OPENAI_TUNNEL_SHUTDOWN_TIMEOUT;
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
mod tests {
    use super::{
        openai_tunnel_config, parse_health_base_url, safe_client_version, validate_openai_tunnel_id,
    };
    use std::path::Path;
    use std::process::Output;

    #[test]
    fn validates_tunnel_id_without_accepting_secrets_or_urls() {
        assert!(validate_openai_tunnel_id("tunnel_test-123").is_ok());
        assert!(validate_openai_tunnel_id("https://example.com/tunnel_test").is_err());
        assert!(validate_openai_tunnel_id("tunnel_").is_err());
    }

    #[test]
    fn config_uses_secret_and_endpoint_references() {
        let config = openai_tunnel_config(
            "tunnel_test",
            Path::new("/tmp/private/mcp-url"),
            Path::new("/tmp/private/health-url"),
            Path::new("/tmp/private/tunnel-client.log"),
        );
        assert!(config.contains("api_key: 'env:CONTROL_PLANE_API_KEY'"));
        assert!(config.contains("url: 'file:/tmp/private/mcp-url'"));
        assert!(!config.contains("/pdx/v1/"));
    }

    #[test]
    fn health_url_is_loopback_only() {
        assert_eq!(
            parse_health_base_url("http://127.0.0.1:1234/\n").unwrap(),
            "http://127.0.0.1:1234"
        );
        assert!(parse_health_base_url("https://example.com:1234").is_err());
    }

    #[test]
    fn version_parser_does_not_echo_arbitrary_output() {
        let output = Output {
            status: std::process::ExitStatus::default(),
            stdout: b"tunnel-client 0.0.13\n".to_vec(),
            stderr: Vec::new(),
        };
        assert_eq!(safe_client_version(&output).as_deref(), Some("0.0.13"));
    }
}
