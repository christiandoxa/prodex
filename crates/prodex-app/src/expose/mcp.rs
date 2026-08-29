use super::http::ExposeHttpRequest;
use super::run_manager::ExposeRunManager;
use super::runtime::ExposeShared;
use super::session::ExposeDigest;
use super::ui::expose_text_response;
use anyhow::{Context, Result};
use base64::Engine;
use prodex_cli::SuperArgs;
use std::fmt;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

mod probe;
pub(super) use probe::{verify_local_mcp_with_progress, verify_public_mcp_with_progress};
mod handler;
mod protocol;
mod tools;
use tools::main_provider;

const MCP_PATH_PREFIX: &str = "/pdx/v1/";
const MCP_PATH_SUFFIX: &str = "/mcp";
const MCP_CURRENT_PROTOCOL_VERSION: &str = "2026-07-28";
const MCP_PROTOCOL_VERSIONS: [&str; 5] = [
    MCP_CURRENT_PROTOCOL_VERSION,
    "2025-11-25",
    "2025-06-18",
    "2025-03-26",
    "2024-11-05",
];
const MCP_RATE_LIMIT: usize = 120;
const MCP_RATE_WINDOW: Duration = Duration::from_secs(1);
const MCP_PUBLIC_INITIALIZE_TIMEOUT: Duration = Duration::from_secs(45);
const MCP_PUBLIC_TOOLS_TIMEOUT: Duration = Duration::from_secs(20);
const MCP_PUBLIC_READY_STEP: Duration = Duration::from_millis(250);
const MCP_MAX_TASK_BYTES: usize = 64 * 1024;
const MCP_MAX_JSON_NESTING: usize = 64;
const MCP_MAX_MODEL_BYTES: usize = 256;
const MCP_MAX_PROFILE_BYTES: usize = 128;
const MCP_MAX_EVENT_PAGE: usize = 64;
const MCP_ERROR_UNSUPPORTED_VERSION: i64 = -32022;
const MCP_ERROR_HEADER_MISMATCH: i64 = -32020;

pub(super) struct ExposeMcpEndpoint {
    capability_digest: ExposeDigest,
    pub(super) run_manager: ExposeRunManager,
    pub(super) server_name: String,
    pub(super) workspace_name: String,
    pub(super) instance_id: String,
    pub(super) defaults: SuperArgs,
    rate: Mutex<McpRateLimit>,
}

impl fmt::Debug for ExposeMcpEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExposeMcpEndpoint")
            .field("capability", &"<redacted>")
            .field("server_name", &self.server_name)
            .field("workspace_name", &self.workspace_name)
            .field("instance_id", &self.instance_id)
            .field("run_count", &self.run_manager.list().len())
            .finish()
    }
}

struct McpRateLimit {
    started: Instant,
    requests: usize,
}

pub(super) fn handle_mcp_route(request: ExposeHttpRequest, shared: &Arc<ExposeShared>, host: &str) {
    if shared.shutdown.load(Ordering::SeqCst) {
        let _ = request.respond(expose_text_response(404, "not found"));
        return;
    }
    if let Some(mcp) = shared.mcp.as_ref() {
        mcp.handle(request, host);
    } else {
        let _ = request.respond(expose_text_response(404, "not found"));
    }
}

pub(super) fn expose_main_provider(args: &SuperArgs) -> prodex_provider_core::ProviderId {
    main_provider(args)
}

/// The complete bearer-style MCP endpoint kept as one logical value.
pub(super) struct PublicMcpEndpoint(String);

impl PublicMcpEndpoint {
    pub(super) fn new(tunnel_origin: &str, capability: &str) -> Result<Self> {
        let origin = tunnel_origin.trim_end_matches('/');
        let parsed = url::Url::parse(origin).context("invalid MCP endpoint origin")?;
        if origin.is_empty()
            || origin.chars().any(char::is_control)
            || origin.chars().any(char::is_whitespace)
            || !matches!(parsed.scheme(), "http" | "https")
            || !origin.split_once("://").is_some_and(|(_, authority)| {
                !authority.is_empty() && !authority.contains(['/', '\\', '?', '#'])
            })
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || !matches!(parsed.path(), "" | "/")
            || parsed.port() == Some(0)
            || capability.is_empty()
            || !capability
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            anyhow::bail!("invalid MCP endpoint components")
        }
        let url = format!("{origin}{MCP_PATH_PREFIX}{capability}{MCP_PATH_SUFFIX}");
        if url.bytes().any(|byte| byte == 10 || byte == 13) {
            anyhow::bail!("MCP endpoint must be a single line")
        }
        Ok(Self(url))
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for PublicMcpEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("PublicMcpEndpoint")
            .field(&"<redacted>")
            .finish()
    }
}

pub(super) fn expose_instance_id() -> Result<String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).context("failed to generate expose instance id")?;
    Ok(format!(
        "pdxi_{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    ))
}

#[cfg(test)]
#[path = "mcp/unit_tests.rs"]
mod tests;
