use super::*;

pub(crate) fn runtime_upstream_no_proxy(explicit_no_proxy: bool) -> bool {
    explicit_no_proxy
}

pub(crate) fn runtime_upstream_proxy_mode_label(explicit_no_proxy: bool) -> &'static str {
    if runtime_upstream_no_proxy(explicit_no_proxy) {
        "disabled"
    } else {
        "system"
    }
}

pub(crate) fn build_runtime_upstream_async_http_client(
    explicit_no_proxy: bool,
) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_millis(
            runtime_proxy_http_connect_timeout_ms(),
        ))
        .read_timeout(Duration::from_millis(runtime_proxy_stream_idle_timeout_ms()));
    if runtime_upstream_no_proxy(explicit_no_proxy) {
        builder = builder.no_proxy();
    }
    builder
        .build()
        .context("failed to build runtime auto-rotate async HTTP client")
}

pub(crate) fn build_upstream_blocking_http_client(
    context_label: &'static str,
    explicit_no_proxy: bool,
) -> Result<Client> {
    let mut builder = Client::builder()
        .connect_timeout(Duration::from_millis(QUOTA_HTTP_CONNECT_TIMEOUT_MS))
        .timeout(Duration::from_millis(QUOTA_HTTP_READ_TIMEOUT_MS));
    if runtime_upstream_no_proxy(explicit_no_proxy) {
        builder = builder.no_proxy();
    }
    builder
        .build()
        .with_context(|| format!("failed to build {context_label} client"))
}
