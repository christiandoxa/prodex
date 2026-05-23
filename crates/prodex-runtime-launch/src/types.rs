use std::net::SocketAddr;
use std::path::PathBuf;

pub struct RuntimeLaunchRequest<'a> {
    pub profile: Option<&'a str>,
    pub allow_auto_rotate: bool,
    pub skip_quota_check: bool,
    pub base_url: Option<&'a str>,
    pub upstream_no_proxy: bool,
    pub include_code_review: bool,
    pub smart_context_enabled: bool,
    pub presidio_redaction_enabled: bool,
    pub model_context_window_tokens: Option<u64>,
    pub force_runtime_proxy: bool,
    pub model_provider_override: Option<&'a str>,
    pub profile_v2_name: Option<&'a str>,
}

pub struct PreparedRuntimeLaunch<P, E> {
    pub paths: P,
    pub codex_home: PathBuf,
    pub managed: bool,
    pub runtime_proxy: Option<E>,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeProxyCodexEndpoint<'a> {
    pub listen_addr: SocketAddr,
    pub openai_mount_path: &'a str,
    pub local_model_provider_id: Option<&'a str>,
}
