//! Runtime state and scheduled-save data structures.
//!
//! This crate intentionally owns side-effect-free state containers only. Live
//! proxy handles, transport clients, and persistence behavior stay in the
//! binary crate until their dependencies can be split safely.

mod admission;
mod background;
mod continuations;
mod lineage;
mod quota;
mod route;

pub use admission::*;
pub use background::*;
pub use continuations::*;
pub use lineage::*;
pub use quota::*;
pub use route::*;

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
