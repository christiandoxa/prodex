use super::{
    CLOUDFLARED_EVENT_PREFIX, CLOUDFLARED_TRANSPORT_NEGOTIATION_TIMEOUT,
    CloudflaredConfigIsolation, CloudflaredTransport, cloudflared_command,
    expose_scan_cloudflared_output,
};
use anyhow::{Context, Result};
use std::process::Stdio;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub(super) struct CloudflaredStartup {
    pub(super) url: Option<String>,
    pub(super) effective_transport: Option<CloudflaredTransport>,
    pub(super) startup_timed_out: bool,
    pub(super) startup_failure: Option<String>,
}

pub(super) fn spawn(
    local_url: &str,
    transport: CloudflaredTransport,
    config: &CloudflaredConfigIsolation,
) -> Result<(
    std::process::Child,
    mpsc::Receiver<String>,
    Vec<JoinHandle<()>>,
)> {
    let mut command = cloudflared_command();
    command
        .args([
            "--config",
            config
                .path
                .to_str()
                .context("cloudflared config path is not UTF-8")?,
            "tunnel",
            "--no-autoupdate",
            "--protocol",
            transport.as_str(),
            "--url",
            local_url,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for variable in [
        "TUNNEL_CONFIG",
        "TUNNEL_ORIGIN_CERT",
        "TUNNEL_CRED_FILE",
        "TUNNEL_HOSTNAME",
        "TUNNEL_NAME",
        "TUNNEL_TOKEN",
        "TUNNEL_TRANSPORT_PROTOCOL",
    ] {
        command.env_remove(variable);
    }
    // Quick Tunnel must not consult a user's named-tunnel home/configuration.  The explicit
    // --config is authoritative; private homes also cover cloudflared lookups outside that flag.
    command.env("HOME", &config.directory);
    command.env("USERPROFILE", &config.directory);
    crate::configure_child_process_group(&mut command, true);
    crate::configure_child_parent_death(&mut command);
    let mut child = command.spawn().context("failed to spawn cloudflared")?;
    let (tx, rx) = mpsc::sync_channel(4);
    let mut reader_threads = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        reader_threads.push(expose_scan_cloudflared_output(stdout, tx.clone()));
    }
    if let Some(stderr) = child.stderr.take() {
        reader_threads.push(expose_scan_cloudflared_output(stderr, tx));
    }
    Ok((child, rx, reader_threads))
}

pub(super) fn wait(
    child: &mut std::process::Child,
    rx: &mpsc::Receiver<String>,
    transport: CloudflaredTransport,
) -> CloudflaredStartup {
    let deadline = Instant::now() + CLOUDFLARED_TRANSPORT_NEGOTIATION_TIMEOUT;
    let mut startup = CloudflaredStartup {
        url: None,
        effective_transport: None,
        startup_timed_out: false,
        startup_failure: None,
    };
    loop {
        if let Ok(event) = rx.recv_timeout(Duration::from_millis(100)) {
            record_event(&mut startup, event);
            if startup.url.is_some() && startup.effective_transport.is_some() {
                break;
            }
        }
        if let Ok(Some(status)) = child.try_wait() {
            startup.startup_failure = Some(format!(
                "cloudflared exited before {} transport registration ({status})",
                transport.as_str()
            ));
            break;
        }
        if Instant::now() >= deadline {
            startup.startup_timed_out = true;
            startup.startup_failure = Some(format!(
                "cloudflared {} transport negotiation timed out after {} seconds",
                transport.as_str(),
                CLOUDFLARED_TRANSPORT_NEGOTIATION_TIMEOUT.as_secs()
            ));
            break;
        }
    }
    startup
}

fn record_event(startup: &mut CloudflaredStartup, event: String) {
    if let Some(protocol) = event.strip_prefix(CLOUDFLARED_EVENT_PREFIX) {
        startup.effective_transport = match protocol {
            "quic" => Some(CloudflaredTransport::Quic),
            "http2" => Some(CloudflaredTransport::Http2),
            _ => startup.effective_transport,
        };
    } else if startup.url.is_none() {
        startup.url = Some(event);
    }
}
