#![forbid(unsafe_code)]
//! Observability boundary primitives.
//!
//! This crate assembles trace propagation, span descriptors, and metric label
//! validation without depending on OpenTelemetry SDKs, HTTP frameworks, storage,
//! filesystem, or async runtimes.
//!
//! The public API stays flat while private modules group plans by owned signal
//! family: trace, configuration, runtime, API, provider, audit, security,
//! accounting, and operational lifecycle.

mod accounting;
mod api;
mod audit;
mod configuration;
mod operations;
mod provider;
mod runtime;
mod security;
mod trace;

pub use accounting::*;
pub use api::*;
pub use audit::*;
pub use configuration::*;
pub use operations::*;
pub use provider::*;
pub use runtime::*;
pub use security::*;
pub use trace::*;
