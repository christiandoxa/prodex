use super::cloudflared::{
    CLOUDFLARED_EVENT_PREFIX, CLOUDFLARED_TRANSPORT_NEGOTIATION_TIMEOUT, CloudflaredTransport,
    CloudflaredTunnel, cloudflared_command, expose_scan_cloudflared_output,
};
use crate::ExposeArgs;
use anyhow::{Context, Result, bail};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::io::AsRawHandle;

const CLOUDFLARE_CONFIG_MAX_BYTES: u64 = 1024 * 1024;
const DEFAULT_EXISTING_ORIGIN_PORT: u16 = 8765;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::expose) struct ExistingCloudflareSelection {
    pub(in crate::expose) config_path: Option<PathBuf>,
    pub(in crate::expose) tunnel: Option<String>,
    pub(in crate::expose) token_file: Option<PathBuf>,
    pub(in crate::expose) hostname: String,
    pub(in crate::expose) origin_port: u16,
}

#[derive(Default)]
struct CloudflareConfigInfo {
    tunnel: Option<String>,
    routes: Vec<CloudflareRoute>,
}

struct CloudflareRoute {
    hostname: String,
    origin_port: Option<u16>,
}

pub(in crate::expose) fn resolve_existing_cloudflare_selection(
    args: &ExposeArgs,
) -> Result<ExistingCloudflareSelection> {
    let config_path = args
        .cloudflare_config
        .clone()
        .or_else(|| env_path("PRODEX_CLOUDFLARE_CONFIG"))
        .or_else(|| env_path("TUNNEL_CONFIG"));
    let token_file = args
        .cloudflare_token_file
        .clone()
        .or_else(|| env_path("PRODEX_CLOUDFLARE_TOKEN_FILE"))
        .or_else(|| env_path("TUNNEL_TOKEN_FILE"));
    if config_path.is_some() && token_file.is_some() {
        bail!("existing Cloudflare mode accepts either a config file or a token file, not both")
    }

    if let Some(token_file) = token_file {
        ensure_regular_file(&token_file, "Cloudflare tunnel token file")?;
        let hostname = explicit_hostname(args)?
            .context("token-based Cloudflare mode requires --cloudflare-hostname or PRODEX_CLOUDFLARE_HOSTNAME")?;
        let origin_port = explicit_origin_port(args)?.unwrap_or(DEFAULT_EXISTING_ORIGIN_PORT);
        return Ok(ExistingCloudflareSelection {
            config_path: None,
            tunnel: args
                .cloudflare_tunnel
                .clone()
                .or_else(|| env_string("PRODEX_CLOUDFLARE_TUNNEL")),
            token_file: Some(token_file),
            hostname,
            origin_port,
        });
    }

    let (config_path, config) = load_selected_config(config_path.as_deref())?;
    let hostname = explicit_hostname(args)?
        .or_else(|| unique_local_hostname(&config))
        .context(
            "existing Cloudflare config must contain one loopback HTTP ingress hostname; set --cloudflare-hostname when multiple routes exist",
        )?;
    let route = config
        .routes
        .iter()
        .find(|route| route.hostname.eq_ignore_ascii_case(&hostname))
        .context("existing Cloudflare config has no ingress route for the selected hostname")?;
    let configured_port = route.origin_port.context(
        "existing Cloudflare ingress must map the selected hostname to a loopback HTTP service",
    )?;
    let origin_port = explicit_origin_port(args)?.unwrap_or(configured_port);
    if origin_port != configured_port {
        bail!(
            "existing Cloudflare route for {hostname} uses loopback port {configured_port}, but {origin_port} was requested; Prodex does not mutate ingress configuration"
        )
    }
    let tunnel = args
        .cloudflare_tunnel
        .clone()
        .or(config.tunnel)
        .or_else(|| env_string("PRODEX_CLOUDFLARE_TUNNEL"))
        .context("existing Cloudflare config must provide a tunnel name or UUID")?;
    Ok(ExistingCloudflareSelection {
        config_path: Some(config_path),
        tunnel: Some(tunnel),
        token_file: None,
        hostname,
        origin_port,
    })
}

pub(in crate::expose) fn discover_existing_cloudflare()
-> Result<Option<ExistingCloudflareSelection>> {
    if env_path("PRODEX_CLOUDFLARE_CONFIG").is_some() || env_path("TUNNEL_CONFIG").is_some() {
        return resolve_existing_cloudflare_selection(&empty_expose_args()).map(Some);
    }
    if env_path("PRODEX_CLOUDFLARE_TOKEN_FILE").is_some() || env_path("TUNNEL_TOKEN_FILE").is_some()
    {
        return resolve_existing_cloudflare_selection(&empty_expose_args()).map(Some);
    }
    let mut selections = Vec::new();
    for path in default_config_paths() {
        if !path.is_file() {
            continue;
        }
        if let Ok((config_path, config)) = load_config(&path)
            && let Some(hostname) = unique_local_hostname(&config)
            && let Some(route) = config
                .routes
                .iter()
                .find(|route| route.hostname.eq_ignore_ascii_case(&hostname))
            && route.origin_port.is_some()
            && config.tunnel.is_some()
        {
            selections.push(ExistingCloudflareSelection {
                config_path: Some(config_path),
                tunnel: config.tunnel,
                token_file: None,
                hostname,
                origin_port: route.origin_port.unwrap_or(DEFAULT_EXISTING_ORIGIN_PORT),
            });
        }
    }
    match selections.as_slice() {
        [] => Ok(None),
        [selection] => Ok(Some(selection.clone())),
        _ => bail!(
            "multiple usable Cloudflare configs detected; set --cloudflare-config or PRODEX_CLOUDFLARE_CONFIG"
        ),
    }
}

pub(in crate::expose) fn start_existing_cloudflared_tunnel(
    selection: &ExistingCloudflareSelection,
    cancelled: &dyn Fn() -> bool,
) -> Result<CloudflaredTunnel> {
    let mut command = cloudflared_command();
    match (
        &selection.config_path,
        &selection.tunnel,
        &selection.token_file,
    ) {
        (Some(config_path), Some(tunnel), None) => {
            let config_path = config_path
                .to_str()
                .context("Cloudflare config path is not UTF-8")?;
            command.args([
                "tunnel",
                "--config",
                config_path,
                "--no-autoupdate",
                "run",
                tunnel,
            ]);
        }
        (None, _, Some(token_file)) => {
            let token_file = token_file
                .to_str()
                .context("Cloudflare token file path is not UTF-8")?;
            command.args([
                "tunnel",
                "--no-autoupdate",
                "run",
                "--token-file",
                token_file,
            ]);
        }
        _ => bail!("existing Cloudflare selection has incomplete credentials"),
    }
    remove_inherited_cloudflare_configuration(&mut command);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::configure_child_process_group(&mut command, true);
    crate::configure_child_parent_death(&mut command);
    let mut child = command
        .spawn()
        .context("failed to spawn existing cloudflared tunnel")?;
    #[cfg(windows)]
    let process_job = super::assign_expose_process_job(child.as_raw_handle()).ok();
    let (tx, rx) = mpsc::sync_channel(8);
    let mut reader_threads = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        reader_threads.push(expose_scan_cloudflared_output(stdout, tx.clone()));
    }
    if let Some(stderr) = child.stderr.take() {
        reader_threads.push(expose_scan_cloudflared_output(stderr, tx));
    }

    let result = wait_for_existing_registration(&mut child, &rx, cancelled);
    let transport = match result {
        Ok(transport) => transport,
        Err(error) => {
            stop_child(&mut child);
            for reader in reader_threads {
                let _ = reader.join();
            }
            return Err(error);
        }
    };
    #[cfg(windows)]
    let mut tunnel = CloudflaredTunnel::from_existing(child, transport, reader_threads);
    #[cfg(not(windows))]
    let tunnel = CloudflaredTunnel::from_existing(child, transport, reader_threads);
    #[cfg(windows)]
    {
        tunnel.attach_process_job(process_job);
    }
    Ok(tunnel)
}

fn wait_for_existing_registration(
    child: &mut std::process::Child,
    rx: &mpsc::Receiver<String>,
    cancelled: &dyn Fn() -> bool,
) -> Result<CloudflaredTransport> {
    let deadline = Instant::now() + CLOUDFLARED_TRANSPORT_NEGOTIATION_TIMEOUT;
    loop {
        if cancelled() {
            bail!("existing Cloudflare Tunnel startup cancelled")
        }
        if let Ok(event) = rx.recv_timeout(Duration::from_millis(100))
            && let Some(protocol) = event.strip_prefix(CLOUDFLARED_EVENT_PREFIX)
        {
            let transport = match protocol {
                "quic" => CloudflaredTransport::Quic,
                "http2" => CloudflaredTransport::Http2,
                _ => continue,
            };
            return Ok(transport);
        }
        if let Some(status) = child
            .try_wait()
            .context("failed to inspect existing cloudflared startup")?
        {
            bail!(
                "existing Cloudflare Tunnel exited before transport registration (status {})",
                status
                    .code()
                    .map_or_else(|| "signal".to_string(), |code| code.to_string())
            )
        }
        if Instant::now() >= deadline {
            bail!(
                "existing Cloudflare Tunnel did not register a transport within {} seconds",
                CLOUDFLARED_TRANSPORT_NEGOTIATION_TIMEOUT.as_secs()
            )
        }
    }
}

fn stop_child(child: &mut std::process::Child) {
    let _ = crate::terminate_child_process_tree(child, true);
    let deadline = Instant::now() + Duration::from_secs(2);
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

fn remove_inherited_cloudflare_configuration(command: &mut std::process::Command) {
    for variable in [
        "TUNNEL_CONFIG",
        "TUNNEL_CERT",
        "TUNNEL_ORIGIN_CERT",
        "TUNNEL_CRED_FILE",
        "TUNNEL_TOKEN",
        "TUNNEL_TOKEN_FILE",
        "TUNNEL_TRANSPORT_PROTOCOL",
    ] {
        command.env_remove(variable);
    }
}

fn load_selected_config(explicit: Option<&Path>) -> Result<(PathBuf, CloudflareConfigInfo)> {
    if let Some(path) = explicit {
        return load_config(path);
    }
    let candidates = default_config_paths()
        .into_iter()
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [] => bail!(
            "no Cloudflare config detected; set --cloudflare-config or PRODEX_CLOUDFLARE_CONFIG, or use --cloudflare-token-file"
        ),
        [path] => load_config(path),
        _ => {
            let usable = candidates
                .iter()
                .filter_map(|path| load_config(path).ok())
                .filter(|(_, config)| unique_local_hostname(config).is_some())
                .collect::<Vec<_>>();
            match usable.as_slice() {
                [(path, config)] => Ok((path.clone(), config.clone_for_selection())),
                _ => bail!(
                    "multiple Cloudflare configs detected; set --cloudflare-config or PRODEX_CLOUDFLARE_CONFIG"
                ),
            }
        }
    }
}

fn load_config(path: &Path) -> Result<(PathBuf, CloudflareConfigInfo)> {
    ensure_regular_file(path, "Cloudflare config file")?;
    let path = path.canonicalize().with_context(|| {
        format!(
            "failed to resolve Cloudflare config file {}",
            path.display()
        )
    })?;
    let file = fs::File::open(&path).context("failed to open Cloudflare config file")?;
    let mut contents = String::new();
    file.take(CLOUDFLARE_CONFIG_MAX_BYTES + 1)
        .read_to_string(&mut contents)
        .context("failed to read Cloudflare config file")?;
    if contents.len() as u64 > CLOUDFLARE_CONFIG_MAX_BYTES {
        bail!("Cloudflare config file exceeds the supported size")
    }
    Ok((path, parse_config(&contents)))
}

fn parse_config(contents: &str) -> CloudflareConfigInfo {
    let mut config = CloudflareConfigInfo::default();
    let mut current_hostname = None;
    let mut current_origin_port = None;
    for raw_line in contents.lines() {
        let line = raw_line.split_once('#').map_or(raw_line, |(line, _)| line);
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("tunnel:")
            && line.chars().take_while(|ch| ch.is_whitespace()).count() == 0
        {
            config.tunnel = non_empty_yaml_scalar(value);
        }
        if let Some(value) = trimmed.strip_prefix("- hostname:") {
            push_route(
                &mut config,
                current_hostname.take(),
                current_origin_port.take(),
            );
            current_hostname = non_empty_yaml_scalar(value);
            continue;
        }
        if current_hostname.is_some()
            && let Some(value) = trimmed.strip_prefix("service:")
        {
            current_origin_port = parse_loopback_service_port(value);
        }
    }
    push_route(&mut config, current_hostname, current_origin_port);
    config
}

fn push_route(
    config: &mut CloudflareConfigInfo,
    hostname: Option<String>,
    origin_port: Option<u16>,
) {
    if let Some(hostname) = hostname
        && super::super_expose::validate_existing_cloudflare_hostname(&hostname).is_ok()
    {
        config.routes.push(CloudflareRoute {
            hostname,
            origin_port,
        });
    }
}

fn parse_loopback_service_port(raw: &str) -> Option<u16> {
    let value = non_empty_yaml_scalar(raw)?;
    let value = if value.contains("://") {
        value
    } else {
        format!("http://{value}")
    };
    let parsed = url::Url::parse(&value).ok()?;
    let host = parsed.host_str()?;
    let local = host == "localhost"
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback());
    (local
        && parsed.scheme() == "http"
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && matches!(parsed.path(), "" | "/")
        && parsed.query().is_none()
        && parsed.fragment().is_none())
    .then(|| parsed.port().filter(|port| *port > 0))
    .flatten()
}

fn non_empty_yaml_scalar(raw: &str) -> Option<String> {
    let value = raw.trim();
    let value = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
        .trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn unique_local_hostname(config: &CloudflareConfigInfo) -> Option<String> {
    let mut hostnames = config
        .routes
        .iter()
        .filter(|route| route.origin_port.is_some())
        .map(|route| route.hostname.clone())
        .collect::<Vec<_>>();
    hostnames.sort_unstable();
    hostnames.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    (hostnames.len() == 1).then(|| hostnames.remove(0))
}

fn explicit_hostname(args: &ExposeArgs) -> Result<Option<String>> {
    let value = args
        .cloudflare_hostname
        .clone()
        .or_else(|| env_string("PRODEX_CLOUDFLARE_HOSTNAME"));
    value
        .map(|value| super::super_expose::validate_existing_cloudflare_hostname(value.trim()))
        .transpose()
}

fn explicit_origin_port(args: &ExposeArgs) -> Result<Option<u16>> {
    let value = args.cloudflare_origin_port.or_else(|| {
        env_string("PRODEX_CLOUDFLARE_ORIGIN_PORT").and_then(|value| value.parse().ok())
    });
    if let Some(port) = value {
        if port == 0 {
            bail!("Cloudflare origin port must be between 1 and 65535")
        }
        return Ok(Some(port));
    }
    Ok(None)
}

fn default_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = dirs::home_dir() {
        paths.extend([
            home.join(".cloudflared/config.yml"),
            home.join(".cloudflared/config.yaml"),
        ]);
    }
    #[cfg(unix)]
    paths.extend([
        PathBuf::from("/etc/cloudflared/config.yml"),
        PathBuf::from("/etc/cloudflared/config.yaml"),
        PathBuf::from("/usr/local/etc/cloudflared/config.yml"),
        PathBuf::from("/usr/local/etc/cloudflared/config.yaml"),
    ]);
    paths
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn env_string(name: &str) -> Option<String> {
    std::env::var(name).ok().and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn ensure_regular_file(path: &Path, label: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path).with_context(|| format!("{label} is unavailable"))?;
    if !metadata.file_type().is_file() {
        bail!("{label} must be a regular file")
    }
    Ok(())
}

impl CloudflareConfigInfo {
    fn clone_for_selection(&self) -> Self {
        Self {
            tunnel: self.tunnel.clone(),
            routes: self
                .routes
                .iter()
                .map(|route| CloudflareRoute {
                    hostname: route.hostname.clone(),
                    origin_port: route.origin_port,
                })
                .collect(),
        }
    }
}

fn empty_expose_args() -> ExposeArgs {
    ExposeArgs {
        command: None,
        cols: 100,
        rows: 32,
        max_clients: 4,
        tunnel: false,
        no_tunnel: false,
        tunnel_provider: None,
        cloudflare_config: None,
        cloudflare_tunnel: None,
        cloudflare_hostname: None,
        cloudflare_origin_port: None,
        cloudflare_token_file: None,
        openai_tunnel_id: None,
        name: None,
        invocation: prodex_cli::ExposeInvocation::Standalone,
        super_args: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TestEnvVarGuard, test_temp_root, write_test_python_executable};

    fn fixture_args(config_path: &Path) -> ExposeArgs {
        let mut args = empty_expose_args();
        args.cloudflare_config = Some(config_path.to_path_buf());
        args
    }

    #[test]
    fn config_selection_uses_only_the_configured_loopback_route() {
        let root = test_temp_root().join(format!("prodex-existing-config-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let config = root.join("config.yml");
        fs::write(
            &config,
            "tunnel: prodex-main\ncredentials-file: credential-sentinel.json\ningress:\n  - hostname: prodex.example.com\n    service: http://127.0.0.1:43123\n  - service: http_status:404\n",
        )
        .unwrap();

        let selection = resolve_existing_cloudflare_selection(&fixture_args(&config)).unwrap();
        assert_eq!(selection.tunnel.as_deref(), Some("prodex-main"));
        assert_eq!(selection.hostname, "prodex.example.com");
        assert_eq!(selection.origin_port, 43123);
        assert!(!format!("{selection:?}").contains("credential-sentinel"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn named_tunnel_run_does_not_use_quick_tunnel_url_flags() {
        let root = test_temp_root().join(format!("prodex-existing-child-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let marker = root.join("args.txt");
        let script = write_test_python_executable(
            &root,
            "cloudflared",
            r#"#!/usr/bin/env python3
import os
import sys
import time

with open(os.environ["PRODEX_EXISTING_CLOUDFLARE_ARGS"], "w", encoding="utf-8") as file:
    file.write(repr(sys.argv[1:]))
print("Registered tunnel connection protocol=quic", flush=True)
time.sleep(30)
"#,
        );
        let _env_lock = TestEnvVarGuard::lock();
        let _script =
            TestEnvVarGuard::set("PRODEX_TEST_CLOUDFLARED_SCRIPT", &script.to_string_lossy());
        let _marker =
            TestEnvVarGuard::set("PRODEX_EXISTING_CLOUDFLARE_ARGS", &marker.to_string_lossy());
        let selection = ExistingCloudflareSelection {
            config_path: Some(root.join("config.yml")),
            tunnel: Some("prodex-main".to_string()),
            token_file: None,
            hostname: "prodex.example.com".to_string(),
            origin_port: 43123,
        };
        let mut tunnel = start_existing_cloudflared_tunnel(&selection, &|| false).unwrap();
        let args = fs::read_to_string(&marker).unwrap();
        assert!(args.contains("'run'"));
        assert!(args.contains("'prodex-main'"));
        assert!(!args.contains("--url"));
        tunnel.shutdown();
        fs::remove_dir_all(root).unwrap();
    }
}
