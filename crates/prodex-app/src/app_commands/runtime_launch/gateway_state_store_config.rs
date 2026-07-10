use super::*;
use std::{env, path::PathBuf};

pub(crate) fn gateway_state_store_config(
    paths: &AppPaths,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<RuntimeGatewayStateStore> {
    match policy
        .state
        .backend
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("file")
        .to_ascii_lowercase()
        .as_str()
    {
        "file" => Ok(RuntimeGatewayStateStore::file(paths)),
        "sqlite" => {
            let configured = policy
                .state
                .sqlite_path
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("gateway-state.sqlite");
            let path = PathBuf::from(configured);
            let path = if path.is_absolute() {
                path
            } else {
                paths.root.join(path)
            };
            Ok(RuntimeGatewayStateStore::sqlite(path))
        }
        "postgres" => {
            let env_name = policy
                .state
                .postgres_url_env
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("PRODEX_GATEWAY_POSTGRES_URL");
            let url = env::var(env_name)
                .with_context(|| format!("gateway.state.backend=postgres requires {env_name}"))?
                .trim()
                .to_string();
            if url.is_empty() {
                bail!("gateway.state.backend=postgres env {env_name} cannot be empty");
            }
            Ok(RuntimeGatewayStateStore::postgres(
                env_name.to_string(),
                url,
            ))
        }
        "redis" => {
            let env_name = policy
                .state
                .redis_url_env
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("PRODEX_GATEWAY_REDIS_URL");
            let url = env::var(env_name)
                .with_context(|| format!("gateway.state.backend=redis requires {env_name}"))?
                .trim()
                .to_string();
            if url.is_empty() {
                bail!("gateway.state.backend=redis env {env_name} cannot be empty");
            }
            Ok(RuntimeGatewayStateStore::redis(env_name.to_string(), url))
        }
        other => {
            bail!("gateway.state.backend must be file, sqlite, postgres, or redis, got {other:?}")
        }
    }
}
