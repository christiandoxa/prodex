use crate::core_constants::{QUOTA_HTTP_CONNECT_TIMEOUT_MS, QUOTA_HTTP_READ_TIMEOUT_MS};
use crate::runtime_config::{
    runtime_proxy_compact_request_timeout_ms, runtime_proxy_http_connect_timeout_ms,
    runtime_proxy_stream_idle_timeout_ms,
};
use anyhow::Result;
use reqwest::blocking::Client;

pub(crate) fn runtime_upstream_proxy_mode_label(explicit_no_proxy: bool) -> &'static str {
    prodex_proxy_config::runtime_upstream_proxy_mode_label(explicit_no_proxy)
}

pub(crate) fn build_runtime_upstream_async_http_client(
    explicit_no_proxy: bool,
) -> Result<reqwest::Client> {
    prodex_proxy_config::build_runtime_upstream_async_http_client(
        explicit_no_proxy,
        runtime_proxy_http_connect_timeout_ms(),
        runtime_proxy_stream_idle_timeout_ms(),
    )
}

pub(crate) fn build_runtime_upstream_async_http_compact_client(
    explicit_no_proxy: bool,
) -> Result<reqwest::Client> {
    prodex_proxy_config::build_runtime_upstream_async_http_compact_client(
        explicit_no_proxy,
        runtime_proxy_http_connect_timeout_ms(),
        runtime_proxy_compact_request_timeout_ms(),
    )
}

pub(crate) fn build_upstream_blocking_http_client(
    context_label: &'static str,
    explicit_no_proxy: bool,
) -> Result<Client> {
    prodex_proxy_config::build_upstream_blocking_http_client(
        context_label,
        explicit_no_proxy,
        QUOTA_HTTP_CONNECT_TIMEOUT_MS,
        QUOTA_HTTP_READ_TIMEOUT_MS,
    )
}
