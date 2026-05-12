use super::*;

mod admission;
mod affinity;
mod attempt_outcome;
mod buffered_response;
mod chain_log;
mod classification;
mod continuation;
mod cookies;
mod dispatch;
mod failure_response;
mod health;
mod lifecycle;
mod lineage;
mod path;
mod payload_detection;
mod prefetch;
mod previous_response_log;
mod previous_response_orchestration;
mod profile_state;
mod quota;
mod response_forwarding;
mod responses;
mod selection;
mod selection_plan;
mod smart_context;
mod standard;
mod transport_failure;
mod upstream;
mod websocket;
mod websocket_message;

pub(crate) use self::admission::*;
pub(crate) use self::affinity::*;
pub(crate) use self::attempt_outcome::*;
pub(super) use self::buffered_response::*;
pub(crate) use self::chain_log::*;
pub(crate) use self::classification::*;
pub(super) use self::continuation::*;
pub(crate) use self::cookies::*;
pub(crate) use self::dispatch::*;
pub(crate) use self::failure_response::*;
pub(super) use self::health::*;
pub(crate) use self::lifecycle::*;
pub(super) use self::lineage::*;
pub(super) use self::path::*;
pub(super) use self::payload_detection::*;
pub(super) use self::prefetch::*;
pub(crate) use self::previous_response_log::*;
pub(crate) use self::previous_response_orchestration::*;
pub(super) use self::profile_state::*;
pub(super) use self::quota::*;
pub(crate) use self::response_forwarding::*;
#[cfg(test)]
pub(crate) use self::responses::attempt_runtime_responses_request;
pub(crate) use self::responses::proxy_runtime_responses_request;
pub(crate) use self::selection::*;
pub(crate) use self::selection_plan::*;
use self::smart_context::*;
pub(crate) use self::smart_context::{
    observe_runtime_smart_context_token_usage_for_bucket, prepare_runtime_smart_context_http_body,
    prepare_runtime_smart_context_http_body_for_profile,
    register_runtime_smart_context_proxy_state,
};
pub(crate) use self::standard::*;
pub(super) use self::transport_failure::*;
pub(crate) use self::upstream::*;
pub(crate) use self::websocket::*;
use self::websocket_message::runtime_profile_uncached_auth_summary_for_selection;
use self::websocket_message::{
    RuntimeWebsocketTextMessageInput, proxy_runtime_websocket_text_message,
};

pub(super) use runtime_proxy_crate::{
    runtime_noncompact_session_priority_profile,
    runtime_proxy_allows_direct_current_profile_fallback, runtime_proxy_has_continuation_priority,
    runtime_proxy_precommit_budget, runtime_wait_affinity_owner,
};

pub(super) fn runtime_proxy_local_selection_failure_message() -> &'static str {
    "Runtime proxy could not secure a healthy upstream profile before the pre-commit retry budget was exhausted. Retry the request."
}

pub(super) fn await_runtime_proxy_async_task<T, F>(
    shared: &RuntimeRotationProxyShared,
    task_name: &'static str,
    future: F,
) -> Result<T>
where
    T: Send + 'static,
    F: std::future::Future<Output = Result<T>> + Send + 'static,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        bail!(
            "refusing to synchronously wait for runtime proxy async task '{task_name}' from inside a Tokio runtime"
        );
    }

    let (sender, receiver) = mpsc::sync_channel(1);
    let task = shared.async_runtime.spawn(async move {
        let result = future.await;
        let _ = sender.send(result);
    });
    drop(task);

    receiver
        .recv()
        .with_context(|| format!("runtime proxy async task '{task_name}' did not return"))?
}
