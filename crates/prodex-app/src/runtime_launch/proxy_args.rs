use super::*;
#[cfg(test)]
pub(crate) use prodex_runtime_launch::normalize_run_codex_args;
#[cfg(test)]
pub(crate) use prodex_runtime_launch::{
    runtime_proxy_codex_args, runtime_proxy_codex_args_with_mount_path,
};

pub(crate) fn runtime_proxy_codex_passthrough_args(
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
    user_args: &[OsString],
) -> Vec<OsString> {
    prodex_runtime_launch::runtime_proxy_codex_passthrough_args(
        runtime_proxy.map(|proxy| prodex_runtime_launch::RuntimeProxyCodexEndpoint {
            listen_addr: proxy.listen_addr,
            openai_mount_path: &proxy.openai_mount_path,
            local_model_provider_id: proxy.local_model_provider_id.as_deref(),
        }),
        user_args,
    )
}
