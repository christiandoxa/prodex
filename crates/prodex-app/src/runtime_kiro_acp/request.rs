use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeKiroAcpClientInfo<'a> {
    pub(crate) name: &'a str,
    pub(crate) title: &'a str,
    pub(crate) version: &'a str,
}

pub(crate) fn runtime_kiro_acp_initialize_request(
    id: u64,
    client_info: RuntimeKiroAcpClientInfo<'_>,
) -> Value {
    prodex_provider_core::kiro_provider_core_acp_initialize_request(
        id,
        client_info.name,
        client_info.title,
        client_info.version,
    )
}

pub(crate) fn runtime_kiro_acp_session_new_request(id: u64, cwd: &Path) -> Value {
    prodex_provider_core::kiro_provider_core_acp_session_new_request(id, &cwd.to_string_lossy())
}

pub(crate) fn runtime_kiro_acp_session_prompt_request(
    id: u64,
    session_id: &str,
    prompt: &str,
) -> Value {
    prodex_provider_core::kiro_provider_core_acp_session_prompt_request(id, session_id, prompt)
}
