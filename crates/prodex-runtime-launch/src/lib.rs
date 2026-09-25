#[cfg(not(feature = "mojo"))]
compile_error!("prodex-runtime-launch requires the Mojo launch planner");

mod args;
mod args_mojo;
mod child;
mod dry_run;
mod profile;
mod quota_plan;
mod types;

#[cfg(test)]
use std::ffi::OsString;
#[cfg(test)]
use std::path::{Path, PathBuf};

pub use self::args::*;
pub use self::child::*;
pub use self::dry_run::*;
pub use self::profile::*;
pub use self::quota_plan::*;
pub use self::types::*;

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
