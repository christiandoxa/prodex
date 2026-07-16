use std::sync::atomic::AtomicU64;

mod artifact;
mod file_store;
mod stale;
mod types;

pub use artifact::*;
pub use file_store::*;
pub use stale::*;
pub use types::*;

pub(super) static RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
