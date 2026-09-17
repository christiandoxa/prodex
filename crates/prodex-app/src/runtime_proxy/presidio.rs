//! Runtime Presidio facade; transport adaptation and engine concerns live in focused modules.

mod analyzer;
mod engine;
mod findings;
mod http;
mod json_body;
pub(crate) mod local;
mod registry;
mod telemetry;
mod websocket;

pub(crate) use http::apply_runtime_presidio_redaction_to_request;
pub(crate) use registry::{
    register_runtime_presidio_redaction_proxy_state,
    unregister_runtime_presidio_redaction_proxy_state,
};
pub(crate) use websocket::apply_runtime_presidio_redaction_to_websocket_text;

#[cfg(test)]
mod tests;
