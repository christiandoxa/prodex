//! Audit domain types grouped by event, query, retention, and public error mapping.
//!
//! Crate-root re-exports preserve the original `prodex_domain::*` API.

mod digest;
mod errors;
mod event;
mod query;
mod retention;

pub use digest::*;
pub use errors::*;
pub use event::*;
pub use query::*;
pub use retention::*;
