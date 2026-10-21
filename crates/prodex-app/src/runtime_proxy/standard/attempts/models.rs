//! Standard-route response forwarding.

use super::{
    RuntimeRotationProxyShared, await_runtime_proxy_async_task, forward_runtime_proxy_response,
};
use anyhow::Result;

pub(super) fn forward_runtime_standard_success_response(
    shared: &RuntimeRotationProxyShared,
    response: reqwest::Response,
) -> Result<tiny_http::ResponseBox> {
    await_runtime_proxy_async_task(
        shared,
        "standard_forward_response",
        forward_runtime_proxy_response(response, Vec::new()),
    )
}
