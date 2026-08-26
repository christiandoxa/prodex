use super::tools::mcp_tool_names;
use super::{
    MCP_CURRENT_PROTOCOL_VERSION, MCP_PROTOCOL_VERSIONS, MCP_PUBLIC_READY_STEP,
    MCP_PUBLIC_READY_TIMEOUT,
};
use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::thread;
use std::time::{Duration, Instant};

pub(crate) fn verify_public_mcp(url: &str) -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .context("failed to initialize public MCP probe")?;
    let deadline = Instant::now() + MCP_PUBLIC_READY_TIMEOUT;
    let mut delay = MCP_PUBLIC_READY_STEP;
    while Instant::now() < deadline {
        let modern_ready = mcp_probe_request(&client, url, "server/discover", mcp_discover_body())
            .and_then(mcp_probe_json)
            .is_some_and(|body| {
                body.get("result")
                    .and_then(|result| result.get("supportedVersions"))
                    .and_then(Value::as_array)
                    .is_some_and(|versions| {
                        versions
                            .iter()
                            .any(|version| version.as_str() == Some(MCP_CURRENT_PROTOCOL_VERSION))
                    })
            });
        let legacy_ready = !modern_ready
            && mcp_probe_request(&client, url, "initialize", mcp_initialize_body())
                .and_then(mcp_probe_json)
                .is_some_and(|body| {
                    body.get("result")
                        .and_then(|result| result.get("protocolVersion"))
                        .and_then(Value::as_str)
                        .is_some_and(|version| MCP_PROTOCOL_VERSIONS.contains(&version))
                });
        if (modern_ready || legacy_ready)
            && mcp_probe_request(&client, url, "tools/list", mcp_metadata_body())
                .and_then(mcp_probe_json)
                .is_some_and(|body| {
                    let Some(tools) = body
                        .get("result")
                        .and_then(|result| result.get("tools"))
                        .and_then(Value::as_array)
                    else {
                        return false;
                    };
                    mcp_tool_names().iter().all(|expected| {
                        tools
                            .iter()
                            .any(|tool| tool.get("name").and_then(Value::as_str) == Some(expected))
                    })
                })
        {
            return Ok(());
        }
        thread::sleep(delay);
        delay = (delay * 2).min(Duration::from_secs(2));
    }
    bail!("public MCP endpoint did not become ready; rerun expose")
}

fn mcp_probe_request(
    client: &Client,
    url: &str,
    method: &str,
    body: Value,
) -> Option<reqwest::blocking::Response> {
    client
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
        .ok()
}

fn mcp_probe_json(response: reqwest::blocking::Response) -> Option<Value> {
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    (response.status().is_success() && !content_type.contains("text/event-stream"))
        .then(|| response.json().ok())
        .flatten()
}

fn mcp_discover_body() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 0,
        "method": "server/discover",
        "params": {"_meta": {"io.modelcontextprotocol/protocolVersion": MCP_CURRENT_PROTOCOL_VERSION, "io.modelcontextprotocol/clientInfo": {"name": "prodex-probe", "version": env!("CARGO_PKG_VERSION")}, "io.modelcontextprotocol/clientCapabilities": {}}}
    })
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
