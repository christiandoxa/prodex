use std::time::Duration;

use anyhow::{Context, Result};

pub fn runtime_upstream_no_proxy(explicit_no_proxy: bool) -> bool {
    explicit_no_proxy
}

pub fn runtime_upstream_proxy_mode_label(explicit_no_proxy: bool) -> &'static str {
    if runtime_upstream_no_proxy(explicit_no_proxy) {
        "disabled"
    } else {
        "system"
    }
}

pub fn build_runtime_upstream_async_http_client(
    explicit_no_proxy: bool,
    connect_timeout_ms: u64,
    stream_idle_timeout_ms: u64,
) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_millis(connect_timeout_ms))
        .read_timeout(Duration::from_millis(stream_idle_timeout_ms));
    if runtime_upstream_no_proxy(explicit_no_proxy) {
        builder = builder.no_proxy();
    }
    builder
        .build()
        .context("failed to build runtime auto-rotate async HTTP client")
}

pub fn build_upstream_blocking_http_client(
    context_label: &'static str,
    explicit_no_proxy: bool,
    connect_timeout_ms: u64,
    read_timeout_ms: u64,
) -> Result<reqwest::blocking::Client> {
    let mut builder = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_millis(connect_timeout_ms))
        .timeout(Duration::from_millis(read_timeout_ms));
    if runtime_upstream_no_proxy(explicit_no_proxy) {
        builder = builder.no_proxy();
    }
    builder
        .build()
        .with_context(|| format!("failed to build {context_label} client"))
}
