use crate::runtime_broker_shared::RuntimeProxyEndpoint;
#[cfg(test)]
pub(crate) use prodex_runtime_launch::normalize_run_codex_args;
use std::ffi::OsString;

pub(crate) fn runtime_proxy_codex_passthrough_args(
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
    user_args: &[OsString],
) -> Vec<OsString> {
    prodex_runtime_launch::runtime_proxy_codex_passthrough_args(
        runtime_proxy.map(|proxy| prodex_runtime_launch::RuntimeProxyCodexEndpoint {
            listen_addr: proxy.listen_addr,
            openai_mount_path: &proxy.openai_mount_path,
            local_model_provider_id: proxy.local_model_provider_id.as_deref(),
            force_http_responses: runtime_proxy_force_http_responses(proxy, user_args),
            realtime_ws_base_url: proxy.realtime_ws_base_url.as_deref(),
            realtime_ws_model: proxy.realtime_ws_model.as_deref(),
        }),
        user_args,
    )
}

pub(crate) fn runtime_proxy_force_http_responses(
    proxy: &RuntimeProxyEndpoint,
    user_args: &[OsString],
) -> bool {
    proxy.force_http_responses || sub_agent_exec_requires_http_response_transport(user_args)
}

fn sub_agent_exec_requires_http_response_transport(user_args: &[OsString]) -> bool {
    std::env::var_os(crate::SUB_AGENT_RECURSION_MARKER).is_some()
        && std::env::var_os(crate::SUB_AGENT_LAUNCHER_MARKER).is_none()
        && prodex_runtime_launch::is_codex_exec_invocation(user_args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TestEnvVarGuard;

    #[test]
    fn recursive_exec_http_transport_is_scoped_to_its_own_launch() {
        let _lock = TestEnvVarGuard::lock();
        let _recursive = TestEnvVarGuard::set(crate::SUB_AGENT_RECURSION_MARKER, "1");
        let _launcher = TestEnvVarGuard::unset(crate::SUB_AGENT_LAUNCHER_MARKER);
        let endpoint = RuntimeProxyEndpoint {
            listen_addr: "127.0.0.1:4455".parse().unwrap(),
            openai_mount_path: runtime_proxy_crate::RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string(),
            local_model_provider_id: None,
            force_http_responses: false,
            realtime_ws_base_url: None,
            realtime_ws_model: None,
            lease_dir: std::path::PathBuf::new(),
            broker_session_affinity_control: None,
            _lease: None,
            _direct_proxy: None,
        };
        let exec_args = [OsString::from("exec"), OsString::from("hello")];
        let recursive_args = runtime_proxy_codex_passthrough_args(Some(&endpoint), &exec_args);
        assert!(recursive_args.iter().any(|arg| {
            arg == "openai_base_url=\"http://127.0.0.1:4455/_prodex/http/backend-api/prodex\""
        }));
        assert!(
            recursive_args
                .iter()
                .any(|arg| arg == "model_provider=\"openai\"")
        );

        let tui_args = runtime_proxy_codex_passthrough_args(
            Some(&endpoint),
            &[OsString::from("resume"), OsString::from("--last")],
        );
        assert!(
            tui_args.iter().any(|arg| {
                arg == "openai_base_url=\"http://127.0.0.1:4455/backend-api/prodex\""
            })
        );

        let _launcher = TestEnvVarGuard::set(crate::SUB_AGENT_LAUNCHER_MARKER, "1");
        let launcher_args = runtime_proxy_codex_passthrough_args(Some(&endpoint), &exec_args);
        assert!(
            launcher_args.iter().any(|arg| {
                arg == "openai_base_url=\"http://127.0.0.1:4455/backend-api/prodex\""
            })
        );
    }
}
