use std::sync::Arc;

use super::local_rewrite_options::RuntimeLocalRewriteProxyStartOptions;
use crate::{RuntimeConfig, RuntimeRotationProxy, validate_credential_free_http_url};

pub(crate) fn start_runtime_local_rewrite_proxy(
    options: RuntimeLocalRewriteProxyStartOptions<'_>,
) -> anyhow::Result<RuntimeRotationProxy> {
    validate_credential_free_http_url(&options.upstream_base_url, "runtime upstream base URL")?;
    let runtime_config = Arc::new(RuntimeConfig::from_env_policy_and_cli(options.paths)?);
    super::local_rewrite::start_runtime_local_rewrite_proxy_with_file_access(
        options,
        runtime_config,
        true,
    )
}
