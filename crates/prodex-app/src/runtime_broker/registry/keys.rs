use crate::{
    RUNTIME_BROKER_IDLE_GRACE_SECONDS, RUNTIME_PROXY_OPENAI_MOUNT_PATH,
    runtime_broker_ready_timeout_ms, runtime_current_prodex_binary_identity,
    runtime_prodex_binary_identity_key,
};

#[cfg(test)]
pub(crate) fn runtime_broker_key_for_binary_identity(
    upstream_base_url: &str,
    include_code_review: bool,
    upstream_no_proxy: bool,
    binary_identity_key: &str,
) -> String {
    runtime_broker_key_for_binary_identity_with_smart_context(
        upstream_base_url,
        include_code_review,
        upstream_no_proxy,
        false,
        None,
        binary_identity_key,
    )
}

pub(crate) fn runtime_broker_key_for_binary_identity_with_smart_context(
    upstream_base_url: &str,
    include_code_review: bool,
    upstream_no_proxy: bool,
    smart_context_enabled: bool,
    model_context_window_tokens: Option<u64>,
    binary_identity_key: &str,
) -> String {
    prodex_runtime_broker::runtime_broker_key_for_binary_identity(
        upstream_base_url,
        include_code_review,
        upstream_no_proxy,
        smart_context_enabled,
        model_context_window_tokens,
        RUNTIME_PROXY_OPENAI_MOUNT_PATH,
        binary_identity_key,
    )
}

pub(crate) fn runtime_broker_current_binary_identity_key() -> String {
    runtime_prodex_binary_identity_key(&runtime_current_prodex_binary_identity())
}

#[cfg(test)]
pub(crate) fn runtime_broker_key(
    upstream_base_url: &str,
    include_code_review: bool,
    upstream_no_proxy: bool,
) -> String {
    runtime_broker_key_with_smart_context(
        upstream_base_url,
        include_code_review,
        upstream_no_proxy,
        false,
        None,
    )
}

pub(crate) fn runtime_broker_key_with_smart_context(
    upstream_base_url: &str,
    include_code_review: bool,
    upstream_no_proxy: bool,
    smart_context_enabled: bool,
    model_context_window_tokens: Option<u64>,
) -> String {
    runtime_broker_key_for_binary_identity_with_smart_context(
        upstream_base_url,
        include_code_review,
        upstream_no_proxy,
        smart_context_enabled,
        model_context_window_tokens,
        &runtime_broker_current_binary_identity_key(),
    )
}

pub(crate) fn runtime_broker_startup_grace_seconds() -> i64 {
    prodex_runtime_broker::runtime_broker_startup_grace_seconds(
        runtime_broker_ready_timeout_ms(),
        RUNTIME_BROKER_IDLE_GRACE_SECONDS,
    )
}
