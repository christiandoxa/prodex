use anyhow::{Context, Result, bail};
use base64::Engine as _;
use prodex_cli::SuperExposeArgs;
use redaction::redaction_redact_secret_like_text;
use serde_json::json;
use std::io::Read;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, TryRecvError};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tiny_http::{Method, Server};

#[path = "super_expose/exec.rs"]
mod exec;
#[path = "super_expose/logging.rs"]
mod logging;
#[path = "super_expose/openai_tunnel.rs"]
mod openai_tunnel;
#[path = "super_expose/protocol.rs"]
mod protocol;
#[path = "super_expose/run.rs"]
mod run;

const BODY_MAX_BYTES: u64 = 1024 * 1024;
static CLOCK_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(crate) fn handle_super_expose(mut expose: SuperExposeArgs) -> Result<()> {
    let (openai_tunnel_id, listen) = prepare_super_expose(&mut expose)?;
    if expose.super_args.dry_run {
        print_super_expose_dry_run(&expose, openai_tunnel_id.as_deref());
        return Ok(());
    }

    let workspace = std::env::current_dir()
        .context("failed to resolve expose workspace")?
        .canonicalize()
        .context("failed to canonicalize expose workspace")?;
    let audit = logging::ExposeAuditLog::new()?;
    audit.event(
        "super_expose_starting",
        [
            crate::runtime_proxy_log_field("mode", expose.mode.as_str()),
            crate::runtime_proxy_log_field("bind", "loopback"),
        ],
    );
    let token = capability_token()?;
    let server = Server::http(listen)
        .map_err(|error| anyhow::anyhow!("failed to bind Super expose: {error}"))?;
    let address = server
        .server_addr()
        .to_ip()
        .context("Super expose did not bind an IP address")?;
    let endpoint = format!("http://{address}/mcp/{token}");
    let display_name = expose_display_name(&expose, &workspace);

    let mut openai_tunnel_rx =
        start_openai_tunnel_async(&expose, openai_tunnel_id.as_deref(), &endpoint, &audit)?;
    let mut openai_tunnel = None;
    announce_super_expose(&expose, display_name, &endpoint, address.port(), &audit)?;

    let manager = run::RunManager::new(workspace.clone(), expose.super_args, audit.clone());
    let expected_path = format!("/mcp/{token}");
    loop {
        poll_openai_tunnel(&mut openai_tunnel_rx, &mut openai_tunnel, &audit)?;
        check_openai_tunnel_exit(openai_tunnel.as_mut(), &audit)?;

        let request = server
            .recv_timeout(Duration::from_millis(100))
            .map_err(|error| anyhow::anyhow!("Super expose receive failed: {error}"))?;
        let Some(request) = request else {
            continue;
        };
        handle_super_expose_request(
            request,
            &expected_path,
            &manager,
            &workspace,
            expose.mode,
            &audit,
        );
    }
}

fn prepare_super_expose(expose: &mut SuperExposeArgs) -> Result<(Option<String>, SocketAddr)> {
    if expose.tunnel {
        bail!(
            "legacy --tunnel mode is not part of the lean 0.430 expose surface; use --openai-tunnel-id for OpenAI Secure MCP Tunnel or omit tunnel flags for local-only access"
        );
    }
    let openai_tunnel_id = expose
        .openai_tunnel_id
        .as_deref()
        .map(|value| openai_tunnel::resolve_openai_tunnel_id(Some(value)))
        .transpose()?;
    expose
        .super_args
        .extract_provider_overrides_from_codex_args()
        .map_err(anyhow::Error::msg)?;
    expose
        .super_args
        .validate_urls()
        .map_err(anyhow::Error::msg)?;

    let listen: SocketAddr = expose
        .listen
        .parse()
        .with_context(|| format!("invalid expose listen address {}", expose.listen))?;
    if !listen.ip().is_loopback() {
        bail!("Super expose only binds loopback addresses in 0.430");
    }
    Ok((openai_tunnel_id, listen))
}

fn print_super_expose_dry_run(expose: &SuperExposeArgs, tunnel_id: Option<&str>) {
    let label = expose_label(expose);
    match tunnel_id {
        Some(tunnel_id) => println!(
            "{label}: OpenAI Secure MCP Tunnel {tunnel_id} -> local MCP on {}",
            expose.listen
        ),
        None => println!("{label}: local MCP on {}", expose.listen),
    }
}

fn expose_label(expose: &SuperExposeArgs) -> &'static str {
    if expose.mode.exec_only() {
        "Prodex Super expose exec"
    } else {
        "Prodex Super expose"
    }
}

fn expose_display_name<'a>(expose: &'a SuperExposeArgs, workspace: &'a std::path::Path) -> &'a str {
    expose
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| workspace.file_name().and_then(|name| name.to_str()))
        .unwrap_or("workspace")
}

fn start_openai_tunnel_async(
    expose: &SuperExposeArgs,
    tunnel_id: Option<&str>,
    endpoint: &str,
    audit: &logging::ExposeAuditLog,
) -> Result<Option<mpsc::Receiver<Result<openai_tunnel::OpenAiTunnel>>>> {
    let Some(tunnel_id) = tunnel_id else {
        return Ok(None);
    };
    audit.event(
        "super_expose_openai_tunnel_starting",
        [
            crate::runtime_proxy_log_field("mode", expose.mode.as_str()),
            crate::runtime_proxy_log_field("provider", "openai"),
        ],
    );
    let client_version = openai_tunnel::ensure_openai_tunnel_available(tunnel_id)?;
    let credentials = openai_tunnel::openai_tunnel_credentials_from_env(tunnel_id)?;
    let local_endpoint = endpoint.to_string();
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("prodex-openai-tunnel-start".to_string())
        .spawn(move || {
            let result = openai_tunnel::start_openai_tunnel(
                &local_endpoint,
                credentials,
                client_version,
                &|| false,
            );
            let _ = sender.send(result);
        })
        .context("failed to start OpenAI tunnel supervisor")?;
    eprintln!("OpenAI Secure MCP Tunnel starting for {tunnel_id}.");
    Ok(Some(receiver))
}

fn announce_super_expose(
    expose: &SuperExposeArgs,
    display_name: &str,
    endpoint: &str,
    port: u16,
    audit: &logging::ExposeAuditLog,
) -> Result<()> {
    println!("{} ({display_name}): {endpoint}", expose_label(expose));
    eprintln!("Capability URL: keep it secret; Ctrl-C stops the endpoint.");
    audit.event(
        "super_expose_started",
        [
            crate::runtime_proxy_log_field("mode", expose.mode.as_str()),
            crate::runtime_proxy_log_field("bind", "loopback"),
            crate::runtime_proxy_log_field("port", port.to_string()),
        ],
    );
    audit.flush()
}

fn poll_openai_tunnel(
    receiver: &mut Option<mpsc::Receiver<Result<openai_tunnel::OpenAiTunnel>>>,
    tunnel: &mut Option<openai_tunnel::OpenAiTunnel>,
    audit: &logging::ExposeAuditLog,
) -> Result<()> {
    let Some(active_receiver) = receiver.as_ref() else {
        return Ok(());
    };
    match active_receiver.try_recv() {
        Ok(Ok(ready)) => {
            audit.event(
                "super_expose_openai_tunnel_ready",
                [
                    crate::runtime_proxy_log_field("provider", "openai"),
                    crate::runtime_proxy_log_field(
                        "client_version",
                        ready.status.client_version.clone(),
                    ),
                ],
            );
            eprintln!(
                "OpenAI Secure MCP Tunnel ready: {} (client {}).",
                ready.status.tunnel_id, ready.status.client_version
            );
            *tunnel = Some(ready);
            *receiver = None;
            audit.flush()?;
            Ok(())
        }
        Ok(Err(error)) => {
            audit.event(
                "super_expose_openai_tunnel_failed",
                [
                    crate::runtime_proxy_log_field("provider", "openai"),
                    crate::runtime_proxy_log_field("reason", "startup_failed"),
                ],
            );
            audit.flush()?;
            Err(error)
        }
        Err(TryRecvError::Empty) => Ok(()),
        Err(TryRecvError::Disconnected) => {
            bail!("OpenAI tunnel supervisor stopped before reporting readiness")
        }
    }
}

fn check_openai_tunnel_exit(
    tunnel: Option<&mut openai_tunnel::OpenAiTunnel>,
    audit: &logging::ExposeAuditLog,
) -> Result<()> {
    let Some(status) = tunnel.and_then(openai_tunnel::OpenAiTunnel::exited) else {
        return Ok(());
    };
    audit.event(
        "super_expose_openai_tunnel_exited",
        [
            crate::runtime_proxy_log_field("provider", "openai"),
            crate::runtime_proxy_log_field(
                "exit_code",
                status
                    .code()
                    .map_or_else(|| "signal".to_string(), |code| code.to_string()),
            ),
        ],
    );
    audit.flush()?;
    bail!("OpenAI tunnel-client exited unexpectedly")
}

fn handle_super_expose_request(
    mut request: tiny_http::Request,
    expected_path: &str,
    manager: &run::RunManager,
    workspace: &std::path::Path,
    mode: prodex_cli::SuperExposeMode,
    audit: &logging::ExposeAuditLog,
) {
    if request.url().split('?').next() != Some(expected_path) {
        audit.event(
            "super_expose_http_rejected",
            [crate::runtime_proxy_log_field("reason", "not_found")],
        );
        let _ = request.respond(protocol::json_response(404, json!({"error":"not_found"})));
        return;
    }
    if request.method() != &Method::Post {
        audit.event(
            "super_expose_http_rejected",
            [crate::runtime_proxy_log_field(
                "reason",
                "method_not_allowed",
            )],
        );
        let _ = request.respond(protocol::json_response(
            405,
            json!({"error":"method_not_allowed"}),
        ));
        return;
    }

    let mut body = Vec::new();
    let read = request
        .as_reader()
        .take(BODY_MAX_BYTES.saturating_add(1))
        .read_to_end(&mut body);
    if read.is_err() || body.len() as u64 > BODY_MAX_BYTES {
        audit.event(
            "super_expose_http_rejected",
            [
                crate::runtime_proxy_log_field("reason", "invalid_body"),
                crate::runtime_proxy_log_field("body_bytes", body.len().to_string()),
            ],
        );
        let _ = request.respond(protocol::json_response(
            400,
            json!({"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"request body is invalid or too large"}}),
        ));
        return;
    }
    let _ = request.respond(protocol::dispatch(&body, manager, workspace, mode, audit));
}

fn capability_token() -> Result<String> {
    let mut bytes = [0_u8; 24];
    getrandom::fill(&mut bytes).context("failed to generate expose capability")?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

pub(super) fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
        .saturating_add(CLOCK_SEQUENCE.fetch_add(1, Ordering::Relaxed))
}

pub(super) fn bounded_redacted_text(bytes: &[u8], max_bytes: usize) -> String {
    let bytes = &bytes[..bytes.len().min(max_bytes)];
    redaction_redact_secret_like_text(&String::from_utf8_lossy(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expose_args(extra: &[&str]) -> SuperExposeArgs {
        let mut argv = vec!["prodex", "s", "expose", "--no-presidio", "--dry-run"];
        argv.extend_from_slice(extra);
        let prodex_cli::Commands::SuperExpose(args) =
            prodex_cli::parse_cli_command_from(argv).expect("expose args should parse")
        else {
            panic!("expected SuperExpose");
        };
        *args
    }

    #[test]
    fn local_expose_dry_run_is_side_effect_free_and_supported() {
        handle_super_expose(expose_args(&["--listen", "127.0.0.1:0"]))
            .expect("local expose dry-run should succeed");
    }

    #[test]
    fn public_tunnel_is_explicitly_rejected_in_0430() {
        let error = handle_super_expose(expose_args(&["--tunnel"]))
            .expect_err("public tunnel should not be silently enabled");
        assert!(error.to_string().contains("legacy --tunnel mode"));
    }

    #[test]
    fn exec_openai_tunnel_dry_run_accepts_explicit_tunnel_id() {
        let args = expose_args(&[
            "exec",
            "--openai-tunnel-id",
            "tunnel_0123456789abcdef0123456789abcdef",
        ]);
        assert_eq!(args.mode, prodex_cli::SuperExposeMode::Exec);
        assert_eq!(
            args.openai_tunnel_id.as_deref(),
            Some("tunnel_0123456789abcdef0123456789abcdef")
        );
        handle_super_expose(args).expect("OpenAI tunnel dry-run should validate without spawning");
    }

    #[test]
    fn openai_tunnel_id_rejects_invalid_values_before_spawn() {
        let error =
            handle_super_expose(expose_args(&["exec", "--openai-tunnel-id", "tunnel_short"]))
                .expect_err("invalid tunnel id should fail");
        assert!(error.to_string().contains("OpenAI tunnel id"));
    }
}
