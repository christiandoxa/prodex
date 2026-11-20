use std::sync::Arc;

use super::local_rewrite_options::{
    RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyStartOptions,
};
use crate::runtime_local_provider_config::local_long_context_stream_idle_timeout_ms;
use crate::{RuntimeConfig, RuntimeRotationProxy, validate_credential_free_http_url};

pub(crate) fn start_runtime_local_rewrite_proxy(
    options: RuntimeLocalRewriteProxyStartOptions<'_>,
) -> anyhow::Result<RuntimeRotationProxy> {
    validate_credential_free_http_url(&options.upstream_base_url, "runtime upstream base URL")?;
    let stream_idle_timeout_default_ms = match &options.provider {
        RuntimeLocalRewriteProviderOptions::OpenAiResponses { local: true, .. } => options
            .model_context_window_tokens
            .and_then(local_long_context_stream_idle_timeout_ms),
        _ => None,
    };
    let runtime_config = Arc::new(
        RuntimeConfig::from_env_policy_and_cli_with_stream_idle_default(
            options.paths,
            stream_idle_timeout_default_ms,
        )?,
    );
    super::local_rewrite::start_runtime_local_rewrite_proxy_with_file_access(
        options,
        runtime_config,
        true,
    )
}
