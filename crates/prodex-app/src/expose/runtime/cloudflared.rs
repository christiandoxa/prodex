use super::super::EXPOSE_CLOUDFLARED_LINE_MAX_BYTES;
use super::cloudflared_startup;
use super::hostname;
use anyhow::{Context, Result, bail};
use std::io::{self, Read};
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, OwnedHandle};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub(in crate::expose) struct CloudflaredTunnel {
    pub(in crate::expose) child: std::process::Child,
    pub(in crate::expose) url: Option<String>,
    pub(in crate::expose) effective_transport: Option<CloudflaredTransport>,
    reader_threads: Vec<JoinHandle<()>>,
    config: Option<CloudflaredConfigIsolation>,
    shut_down: bool,
    #[cfg(windows)]
    process_job: Option<std::os::windows::io::OwnedHandle>,
    pub(in crate::expose) startup_timed_out: bool,
    pub(in crate::expose) startup_failure: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::expose) enum CloudflaredTransport {
    Auto,
    Quic,
    Http2,
}

impl CloudflaredTransport {
    pub(in crate::expose) fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Quic => "quic",
            Self::Http2 => "http2",
        }
    }
}

pub(in crate::expose) const CLOUDFLARED_TRANSPORT_NEGOTIATION_TIMEOUT: Duration = if cfg!(test) {
    if cfg!(windows) {
        Duration::from_secs(30)
    } else {
        Duration::from_secs(5)
    }
} else {
    Duration::from_secs(20)
};
pub(in crate::expose) const CLOUDFLARED_EVENT_PREFIX: &str = "\0prodex-cloudflared-transport=";
static NEXT_CLOUDFLARED_CONFIG_ID: AtomicU64 = AtomicU64::new(1);

pub(in crate::expose) struct CloudflaredConfigIsolation {
    directory: std::path::PathBuf,
    pub(in crate::expose) path: std::path::PathBuf,
    cleaned: bool,
}

impl CloudflaredConfigIsolation {
    pub(in crate::expose) fn create() -> Result<Self> {
        let unique_id = NEXT_CLOUDFLARED_CONFIG_ID.fetch_add(1, Ordering::Relaxed);
        let directory = (0..16)
            .map(|attempt| {
                std::env::temp_dir().join(format!(
                    "prodex-cloudflared-{}-{unique_id}-{attempt}",
                    std::process::id()
                ))
            })
            .find(|directory| std::fs::create_dir(directory).is_ok())
            .context("failed to create private cloudflared config directory")?;
        let path = directory.join("config.yaml");
        let isolation = Self {
            directory,
            path,
            cleaned: false,
        };
        secret_store::ensure_private_directory(&isolation.directory)
            .context("failed to secure private cloudflared config directory")?;
        secret_store::write_private_file_create_new(&isolation.path, &[])
            .context("failed to create private cloudflared config")?;
        let metadata = std::fs::symlink_metadata(&isolation.path)
            .context("failed to inspect private cloudflared config")?;
        if !metadata.file_type().is_file() {
            anyhow::bail!("private cloudflared config is not a regular file");
        }
        Ok(isolation)
    }

    pub(in crate::expose) fn directory(&self) -> &Path {
        &self.directory
    }

    fn cleanup(&mut self) {
        if self.cleaned {
            return;
        }
        self.cleaned = true;
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

impl Drop for CloudflaredConfigIsolation {
    fn drop(&mut self) {
        self.cleanup();
    }
}

pub(in crate::expose) fn cloudflared_command() -> Command {
    #[cfg(test)]
    if let Some(script) = std::env::var_os("PRODEX_TEST_CLOUDFLARED_SCRIPT") {
        #[cfg(windows)]
        {
            if Path::new(&script)
                .extension()
                .is_some_and(|extension| extension.to_string_lossy().eq_ignore_ascii_case("cmd"))
            {
                let mut command = Command::new("cmd.exe");
                command.arg("/C").arg(script);
                return command;
            }
            let mut command = Command::new("python");
            command.arg(script);
            return command;
        }
        #[cfg(not(windows))]
        {
            let mut command = Command::new("python3");
            command.arg(script);
            return command;
        }
    }
    Command::new("cloudflared")
}

impl CloudflaredTunnel {
    #[cfg(windows)]
    pub(in crate::expose) fn attach_process_job(&mut self, process_job: Option<OwnedHandle>) {
        self.process_job = process_job;
    }

    pub(in crate::expose) fn from_existing(
        child: std::process::Child,
        effective_transport: CloudflaredTransport,
        reader_threads: Vec<JoinHandle<()>>,
    ) -> Self {
        Self {
            child,
            url: None,
            effective_transport: Some(effective_transport),
            reader_threads,
            config: None,
            shut_down: false,
            #[cfg(windows)]
            process_job: None,
            startup_timed_out: false,
            startup_failure: None,
        }
    }

    pub(in crate::expose) fn exited(&mut self) -> Option<std::process::ExitStatus> {
        self.child.try_wait().ok().flatten()
    }

    pub(in crate::expose) fn shutdown(&mut self) {
        if self.shut_down {
            return;
        }
        self.shut_down = true;
        let _ = crate::terminate_child_process_tree(&mut self.child, true);
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            match self.child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => thread::sleep(Duration::from_millis(20)),
                Err(_) => break,
            }
        }
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        #[cfg(windows)]
        drop(self.process_job.take());
        for thread in self.reader_threads.drain(..) {
            let _ = crate::join_thread_with_timeout(
                thread,
                Duration::from_secs(1),
                "cloudflared output reader",
            );
        }
        if let Some(config) = self.config.as_mut() {
            config.cleanup();
        }
    }
}

impl Drop for CloudflaredTunnel {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub(in crate::expose) fn start_cloudflared_tunnel(local_url: &str) -> Result<CloudflaredTunnel> {
    let mut progress = |message: &str| crate::print_launch_status(message);
    start_cloudflared_tunnel_with_cancel_and_progress(local_url, &|| false, &mut progress)
}

pub(in crate::expose) fn start_cloudflared_tunnel_with_cancel_and_progress(
    local_url: &str,
    cancelled: &dyn Fn() -> bool,
    progress: &mut dyn FnMut(&str),
) -> Result<CloudflaredTunnel> {
    let mut auto = start_cloudflared_attempt(local_url, CloudflaredTransport::Auto, cancelled)?;
    if cancelled() {
        auto.startup_failure = Some("cloudflared startup cancelled".to_string());
        auto.shutdown();
        return Ok(auto);
    }
    if auto.effective_transport.is_some() {
        return Ok(auto);
    }
    if auto.url.is_some() && auto.startup_timed_out {
        auto.shutdown();
        if cancelled() {
            auto.startup_failure = Some("cloudflared startup cancelled".to_string());
            return Ok(auto);
        }
        progress("Cloudflare QUIC unavailable; using HTTP/2 fallback.");
        let mut http2 =
            start_cloudflared_attempt(local_url, CloudflaredTransport::Http2, cancelled)?;
        if http2.effective_transport.is_some() {
            return Ok(http2);
        }
        http2.startup_failure = Some(
            "Cloudflare edge transport unavailable: QUIC/UDP 7844 and HTTP/2/TCP 7844 failed"
                .to_string(),
        );
        http2.shutdown();
        return Ok(http2);
    }
    if auto.startup_failure.is_none() {
        auto.startup_failure =
            Some("Cloudflare Quick Tunnel did not register a transport connection".to_string());
    }
    auto.shutdown();
    Ok(auto)
}

fn start_cloudflared_attempt(
    local_url: &str,
    transport: CloudflaredTransport,
    cancelled: &dyn Fn() -> bool,
) -> Result<CloudflaredTunnel> {
    let config = CloudflaredConfigIsolation::create()?;
    let (mut child, rx, reader_threads) =
        cloudflared_startup::spawn(local_url, transport, &config)?;
    #[cfg(windows)]
    let process_job = super::assign_expose_process_job(child.as_raw_handle()).ok();
    let startup = cloudflared_startup::wait(&mut child, &rx, transport, cancelled);
    Ok(CloudflaredTunnel {
        child,
        url: startup.url,
        effective_transport: startup.effective_transport,
        reader_threads,
        config: Some(config),
        shut_down: false,
        #[cfg(windows)]
        process_job,
        startup_timed_out: startup.startup_timed_out,
        startup_failure: startup.startup_failure,
    })
}

pub(in crate::expose) fn expose_scan_cloudflared_output<R>(
    reader: R,
    tx: mpsc::SyncSender<String>,
) -> JoinHandle<()>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut reader = io::BufReader::new(reader);
        let mut bytes = [0_u8; 1024];
        let mut line = Vec::with_capacity(EXPOSE_CLOUDFLARED_LINE_MAX_BYTES);
        loop {
            match reader.read(&mut bytes) {
                Ok(0) => break,
                Ok(size) => {
                    for byte in &bytes[..size] {
                        expose_scan_cloudflared_byte(&mut line, *byte, &tx);
                    }
                }
                Err(_) => break,
            }
        }
        expose_scan_cloudflared_line(&line, &tx);
    })
}

fn expose_scan_cloudflared_byte(line: &mut Vec<u8>, byte: u8, tx: &mpsc::SyncSender<String>) {
    if line.len() < EXPOSE_CLOUDFLARED_LINE_MAX_BYTES {
        line.push(byte);
    }
    if byte == b'\n' {
        expose_scan_cloudflared_line(line, tx);
        line.clear();
    }
}

fn expose_scan_cloudflared_line(line: &[u8], tx: &mpsc::SyncSender<String>) {
    let Ok(line) = std::str::from_utf8(line) else {
        return;
    };
    if line.contains("Registered tunnel connection") {
        let protocol = line.split_whitespace().find_map(|part| {
            part.strip_prefix("protocol=")
                .filter(|value| matches!(*value, "quic" | "http2"))
        });
        if let Some(protocol) = protocol {
            let _ = tx.try_send(format!("{CLOUDFLARED_EVENT_PREFIX}{protocol}"));
        }
    }
    if let Some(url) = expose_find_trycloudflare_url(line) {
        let _ = tx.try_send(url);
    }
}

pub(in crate::expose) fn expose_find_trycloudflare_url(line: &str) -> Option<String> {
    line.split_whitespace().find_map(|part| {
        let candidate = part.trim_matches(|ch| matches!(ch, ',' | ';' | '(' | ')' | '[' | ']'));
        expose_public_host(candidate).map(|host| format!("https://{host}"))
    })
}

pub(in crate::expose) fn expose_public_host(url: &str) -> Option<String> {
    if !url.is_ascii() {
        return None;
    }
    let authority = url
        .strip_prefix("https://")?
        .split(['/', '?', '#'])
        .next()?;
    if authority.contains([':', '@']) {
        return None;
    }
    let parsed = url::Url::parse(url).ok()?;
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !matches!(parsed.path(), "" | "/")
    {
        return None;
    }
    let host = parsed.host_str()?.to_ascii_lowercase();
    (host.ends_with(".trycloudflare.com")
        && host != "trycloudflare.com"
        && hostname::expose_valid_dns_hostname(&host))
    .then_some(host)
}

pub(in crate::expose) fn expose_valid_dns_hostname(host: &str) -> bool {
    hostname::expose_valid_dns_hostname(host)
}

pub(in crate::expose) fn cloudflared_start_failure(
    tunnel: &mut CloudflaredTunnel,
) -> anyhow::Error {
    if let Some(status) = tunnel.exited() {
        return anyhow::anyhow!(
            "Cloudflare Quick Tunnel exited before obtaining a public hostname (status {})",
            status
                .code()
                .map_or_else(|| "signal".to_string(), |code| code.to_string())
        );
    }
    if tunnel.startup_timed_out {
        return anyhow::anyhow!(
            "Cloudflare Quick Tunnel did not complete transport negotiation; check outbound UDP/TCP 7844 connectivity"
        );
    }
    anyhow::anyhow!("Cloudflare Quick Tunnel did not report a public hostname")
}

pub(in crate::expose) fn ensure_cloudflared_available() -> anyhow::Result<()> {
    let mut command = cloudflared_command();
    command.arg("--version");
    command.env_remove("CONTROL_PLANE_API_KEY");
    let output = crate::command_probe_output(&mut command, "cloudflared version probe");
    match output {
        Ok(output) if output.status.success() && cloudflared_version_is_parseable(&output) => {}
        Ok(_) => bail!(
            "cloudflared --version failed or reported an unsupported version; install a current cloudflared (no account or init is required)"
        ),
        Err(_) => bail!(
            "cloudflared is required for Quick Tunnel mode; install it from https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/ (no account or init is required)"
        ),
    }
    let mut help = cloudflared_command();
    help.args(["tunnel", "--help"]);
    help.env_remove("CONTROL_PLANE_API_KEY");
    let help = crate::command_probe_output(&mut help, "cloudflared Quick Tunnel capability probe")
        .context("cloudflared Quick Tunnel capability probe failed; upgrade cloudflared")?;
    let help = String::from_utf8_lossy(&help.stdout);
    if !help.contains("--config") || !help.contains("--url") {
        bail!(
            "installed cloudflared does not support isolated Quick Tunnel configuration; upgrade cloudflared"
        );
    }
    Ok(())
}

fn cloudflared_version_is_parseable(output: &std::process::Output) -> bool {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    [stdout.as_ref(), stderr.as_ref()].into_iter().any(|text| {
        text.split_whitespace().any(|token| {
            let token = token.trim_matches(|character: char| {
                !character.is_ascii_alphanumeric() && character != '.'
            });
            let token = token.strip_prefix('v').unwrap_or(token);
            let token = token.split(['-', '+']).next().unwrap_or_default();
            let parts = token.split('.').collect::<Vec<_>>();
            parts.len() >= 2
                && parts
                    .iter()
                    .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        })
    })
}
