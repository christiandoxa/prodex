use super::tools::mcp_tool_names;
use super::{
    MCP_CURRENT_PROTOCOL_VERSION, MCP_PROTOCOL_VERSIONS, MCP_PUBLIC_INITIALIZE_TIMEOUT,
    MCP_PUBLIC_READY_STEP, MCP_PUBLIC_TOOLS_TIMEOUT,
};
use anyhow::{Context, Result, bail};
use reqwest::blocking::{Client, Response};
use serde_json::{Value, json};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProbePhase {
    LocalInitialize,
    LocalTools,
    PublicInitialize,
    PublicTools,
}

impl ProbePhase {
    const fn label(self) -> &'static str {
        match self {
            Self::LocalInitialize => "local MCP initialize",
            Self::LocalTools => "local MCP tools/list",
            Self::PublicInitialize => "public MCP initialize",
            Self::PublicTools => "public MCP tools/list",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProbeFailureKind {
    Transport,
    Http(u16),
    EventStream,
    InvalidJson,
    InvalidProtocol,
    MissingTools,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProbeFailure {
    phase: ProbePhase,
    kind: ProbeFailureKind,
}

impl ProbeFailure {
    const fn retryable(self) -> bool {
        matches!(
            self.kind,
            ProbeFailureKind::Transport | ProbeFailureKind::Http(502..=504)
        )
    }
}

impl std::fmt::Display for ProbeFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            ProbeFailureKind::Transport => write!(
                formatter,
                "{} could not reach the endpoint (DNS, TLS, connection, or timeout)",
                self.phase.label()
            ),
            ProbeFailureKind::Http(404) => write!(
                formatter,
                "{} returned HTTP 404; Prodex Host/capability routing rejected the route",
                self.phase.label()
            ),
            ProbeFailureKind::Http(status) => {
                write!(formatter, "{} returned HTTP {status}", self.phase.label())
            }
            ProbeFailureKind::EventStream => write!(
                formatter,
                "{} returned text/event-stream; Quick Tunnel mode requires JSON",
                self.phase.label()
            ),
            ProbeFailureKind::InvalidJson => write!(
                formatter,
                "{} returned invalid JSON-RPC",
                self.phase.label()
            ),
            ProbeFailureKind::InvalidProtocol => write!(
                formatter,
                "{} returned an invalid MCP protocol response",
                self.phase.label()
            ),
            ProbeFailureKind::MissingTools => write!(
                formatter,
                "{} did not advertise the required Prodex Super tools",
                self.phase.label()
            ),
        }
    }
}

pub(crate) fn verify_local_mcp(url: &str) -> Result<()> {
    let started = Instant::now();
    let client = Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .context("failed to initialize local MCP probe")?;
    probe_initialize(&client, url, ProbePhase::LocalInitialize)
        .map_err(|failure| anyhow::anyhow!("{failure}"))?;
    probe_tools(&client, url, ProbePhase::LocalTools)
        .map_err(|failure| anyhow::anyhow!("{failure}"))?;
    crate::runtime_launch::emit_runtime_timing("expose.local_mcp_ready_ms", started);
    Ok(())
}

pub(crate) fn verify_public_mcp(url: &str) -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .context("failed to initialize public MCP probe")?;
    let started = Instant::now();
    run_public_phase(
        &client,
        url,
        ProbePhase::PublicInitialize,
        MCP_PUBLIC_INITIALIZE_TIMEOUT,
        |client, url| probe_initialize(client, url, ProbePhase::PublicInitialize),
    )?;
    run_public_phase(
        &client,
        url,
        ProbePhase::PublicTools,
        MCP_PUBLIC_TOOLS_TIMEOUT,
        |client, url| probe_tools(client, url, ProbePhase::PublicTools),
    )?;
    crate::runtime_launch::emit_runtime_timing("expose.public_mcp_ready_ms", started);
    Ok(())
}

fn run_public_phase<F>(
    client: &Client,
    url: &str,
    phase: ProbePhase,
    timeout: Duration,
    mut probe: F,
) -> Result<()>
where
    F: FnMut(&Client, &str) -> std::result::Result<(), ProbeFailure>,
{
    crate::print_launch_status(&format!("Validating {}...", phase.label()));
    let deadline = Instant::now() + timeout;
    let mut delay = MCP_PUBLIC_READY_STEP;
    loop {
        let failure = match probe(client, url) {
            Ok(()) => return Ok(()),
            Err(failure) if failure.retryable() => failure,
            Err(failure) => bail!("{failure}"),
        };
        if probe_cancelled() {
            bail!("{} cancelled", phase.label())
        }
        if Instant::now() >= deadline {
            bail!(
                "{} did not become ready within {} seconds: {failure}",
                phase.label(),
                timeout.as_secs()
            )
        }
        if !wait_for_probe_retry(delay) {
            bail!("{} cancelled", phase.label())
        }
        delay = (delay * 2).min(Duration::from_secs(2));
    }
}

fn probe_initialize(
    client: &Client,
    url: &str,
    phase: ProbePhase,
) -> std::result::Result<(), ProbeFailure> {
    let body = mcp_probe_request(client, url, "initialize", mcp_initialize_body(), phase)?;
    let version = body
        .get("result")
        .and_then(|result| result.get("protocolVersion"))
        .and_then(Value::as_str);
    if version.is_some_and(|version| MCP_PROTOCOL_VERSIONS.contains(&version)) {
        Ok(())
    } else {
        Err(ProbeFailure {
            phase,
            kind: ProbeFailureKind::InvalidProtocol,
        })
    }
}

fn probe_tools(
    client: &Client,
    url: &str,
    phase: ProbePhase,
) -> std::result::Result<(), ProbeFailure> {
    let body = mcp_probe_request(client, url, "tools/list", mcp_metadata_body(), phase)?;
    let Some(tools) = body
        .get("result")
        .and_then(|result| result.get("tools"))
        .and_then(Value::as_array)
    else {
        return Err(ProbeFailure {
            phase,
            kind: ProbeFailureKind::InvalidProtocol,
        });
    };
    if mcp_tool_names().iter().all(|expected| {
        tools
            .iter()
            .any(|tool| tool.get("name").and_then(Value::as_str) == Some(expected))
    }) {
        Ok(())
    } else {
        Err(ProbeFailure {
            phase,
            kind: ProbeFailureKind::MissingTools,
        })
    }
}

fn mcp_probe_request(
    client: &Client,
    url: &str,
    method: &str,
    body: Value,
    phase: ProbePhase,
) -> std::result::Result<Value, ProbeFailure> {
    let response = client
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(
            reqwest::header::ACCEPT,
            "application/json, text/event-stream",
        )
        .header("MCP-Protocol-Version", MCP_CURRENT_PROTOCOL_VERSION)
        .header("Mcp-Method", method)
        .json(&body)
        .send()
        .map_err(|_error| ProbeFailure {
            phase,
            kind: ProbeFailureKind::Transport,
        })?;
    mcp_probe_json(response, phase)
}

fn mcp_probe_json(
    response: Response,
    phase: ProbePhase,
) -> std::result::Result<Value, ProbeFailure> {
    if !response.status().is_success() {
        return Err(ProbeFailure {
            phase,
            kind: ProbeFailureKind::Http(response.status().as_u16()),
        });
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if content_type.contains("text/event-stream") {
        return Err(ProbeFailure {
            phase,
            kind: ProbeFailureKind::EventStream,
        });
    }
    response.json().map_err(|_| ProbeFailure {
        phase,
        kind: ProbeFailureKind::InvalidJson,
    })
}

fn wait_for_probe_retry(delay: Duration) -> bool {
    let deadline = Instant::now() + delay;
    while Instant::now() < deadline {
        if probe_cancelled() {
            return false;
        }
        thread::sleep(
            Duration::from_millis(50).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
    true
}

fn probe_cancelled() -> bool {
    #[cfg(unix)]
    {
        crate::InteractiveSigintGuard::count() > 0
    }
    #[cfg(not(unix))]
    false
}

fn mcp_initialize_body() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": MCP_CURRENT_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {"name": "prodex-probe", "version": env!("CARGO_PKG_VERSION")}
        }
    })
}

fn mcp_metadata_body() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {"_meta": {"io.modelcontextprotocol/protocolVersion": MCP_CURRENT_PROTOCOL_VERSION, "io.modelcontextprotocol/clientInfo": {"name": "prodex-probe", "version": env!("CARGO_PKG_VERSION")}, "io.modelcontextprotocol/clientCapabilities": {}}}
    })
}
