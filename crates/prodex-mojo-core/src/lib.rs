#![allow(unsafe_code)]

//! Safe, stateless wrappers around the narrow Rust-to-Mojo C ABI.
//!
//! This crate is the only owner of unsafe FFI declarations. Callers exchange
//! bounded integers and caller-owned slices; no Rust heap object or Mojo
//! collection crosses the ABI.

pub const MOJO_ACTIVE: bool = cfg!(prodex_mojo_active);
pub const MOJO_FALLBACK: bool = cfg!(prodex_mojo_fallback);
pub const MOJO_REQUIRED: bool = cfg!(prodex_mojo_required);
pub const MOJO_VERSION: Option<&str> = option_env!("PRODEX_MOJO_VERSION");

#[cfg(feature = "mojo-quota")]
pub mod quota;
#[cfg(feature = "mojo-routing")]
pub mod routing;
#[cfg(feature = "mojo-runtime")]
pub mod runtime;
