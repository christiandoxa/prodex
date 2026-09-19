use anyhow::{Context, Result, bail};
use base64::Engine as _;
use prodex_cli::SuperExposeArgs;
use redaction::redaction_redact_secret_like_text;
use serde_json::json;
use std::io::Read;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tiny_http::{Method, Server};

#[path = "super_expose/exec.rs"]
mod exec;
#[path = "super_expose/logging.rs"]
mod logging;
#[path = "super_expose/protocol.rs"]
mod protocol;
#[path = "super_expose/run.rs"]
mod run;

const BODY_MAX_BYTES: u64 = 1024 * 1024;
static CLOCK_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(crate) fn handle_super_expose(mut expose: SuperExposeArgs) -> Result<()> {
    if expose.tunnel {
        bail!(
            "public --tunnel mode is not part of the lean 0.430 expose surface; use local prodex s expose or --no-tunnel"
        );
    }
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
    if expose.super_args.dry_run {
        let label = if expose.mode.exec_only() {
            "Prodex Super expose exec"
        } else {
            "Prodex Super expose"
        };
        println!("{label}: local MCP on {}", expose.listen);
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
    let server = Server::http(expose.listen.as_str())
        .map_err(|error| anyhow::anyhow!("failed to bind Super expose: {error}"))?;
    let address = server
        .server_addr()
        .to_ip()
        .context("Super expose did not bind an IP address")?;
    let endpoint = format!("http://{address}/mcp/{token}");
    let display_name = expose
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| workspace.file_name().and_then(|name| name.to_str()))
        .unwrap_or("workspace");

    let label = if expose.mode.exec_only() {
        "Prodex Super expose exec"
    } else {
        "Prodex Super expose"
    };
    println!("{label} ({display_name}): {endpoint}");
    eprintln!("Capability URL: keep it secret; Ctrl-C stops the endpoint.");
    audit.event(
        "super_expose_started",
        [
            crate::runtime_proxy_log_field("mode", expose.mode.as_str()),
            crate::runtime_proxy_log_field("bind", "loopback"),
            crate::runtime_proxy_log_field("port", address.port().to_string()),
        ],
    );
    audit.flush()?;

    let manager = run::RunManager::new(workspace.clone(), expose.super_args, audit.clone());
    for mut request in server.incoming_requests() {
        let expected_path = format!("/mcp/{token}");
        if request.url().split('?').next() != Some(expected_path.as_str()) {
            audit.event(
                "super_expose_http_rejected",
                [crate::runtime_proxy_log_field("reason", "not_found")],
            );
            let _ = request.respond(protocol::json_response(404, json!({"error":"not_found"})));
            continue;
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
            continue;
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
            continue;
        }
        let _ = request.respond(protocol::dispatch(
            &body,
            &manager,
            &workspace,
            expose.mode,
            &audit,
        ));
    }
    Ok(())
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
        assert!(error.to_string().contains("public --tunnel mode"));
    }
}
