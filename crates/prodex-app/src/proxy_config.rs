use crate::core_constants::{QUOTA_HTTP_CONNECT_TIMEOUT_MS, QUOTA_HTTP_READ_TIMEOUT_MS};
use crate::runtime_config::RuntimeConfig;
use anyhow::Result;
use reqwest::blocking::Client;

mod client;

pub(crate) fn runtime_upstream_proxy_mode_label(explicit_no_proxy: bool) -> &'static str {
    client::UpstreamProxyMode::from_explicit_no_proxy(explicit_no_proxy).label()
}

pub(crate) fn build_runtime_upstream_async_http_client(
    explicit_no_proxy: bool,
    runtime_config: &RuntimeConfig,
) -> Result<reqwest::Client> {
    client::build_runtime_upstream_async_http_client(client::AsyncUpstreamClientConfig {
        proxy_mode: client::UpstreamProxyMode::from_explicit_no_proxy(explicit_no_proxy),
        connect_timeout: std::time::Duration::from_millis(
            runtime_config.tuning.http_connect_timeout_ms,
        ),
        response_timeout: client::AsyncResponseTimeout::ReadIdle(std::time::Duration::from_millis(
            runtime_config.tuning.stream_idle_timeout_ms,
        )),
    })
}

pub(crate) fn build_runtime_upstream_async_http_compact_client(
    explicit_no_proxy: bool,
    runtime_config: &RuntimeConfig,
) -> Result<reqwest::Client> {
    client::build_runtime_upstream_async_http_client(client::AsyncUpstreamClientConfig {
        proxy_mode: client::UpstreamProxyMode::from_explicit_no_proxy(explicit_no_proxy),
        connect_timeout: std::time::Duration::from_millis(
            runtime_config.tuning.http_connect_timeout_ms,
        ),
        response_timeout: client::AsyncResponseTimeout::Request(std::time::Duration::from_millis(
            runtime_config.compact_request_timeout_ms,
        )),
    })
}

pub(crate) fn build_upstream_blocking_http_client(
    context_label: &'static str,
    explicit_no_proxy: bool,
) -> Result<Client> {
    client::build_upstream_blocking_http_client(client::BlockingUpstreamClientConfig {
        context_label,
        proxy_mode: client::UpstreamProxyMode::from_explicit_no_proxy(explicit_no_proxy),
        connect_timeout: std::time::Duration::from_millis(QUOTA_HTTP_CONNECT_TIMEOUT_MS),
        request_timeout: std::time::Duration::from_millis(QUOTA_HTTP_READ_TIMEOUT_MS),
    })
}
