use super::*;
use prodex_state::{ProfileProvider, ResponseProfileBinding};
use std::path::PathBuf;

fn profile() -> ProfileEntry {
    ProfileEntry {
        codex_home: PathBuf::from("/tmp/profile"),
        managed: true,
        email: None,
        provider: ProfileProvider::Openai,
    }
}

#[path = "lib/backoffs.rs"]
mod backoffs;
#[path = "lib/continuations.rs"]
mod continuations;
#[path = "lib/selected_snapshot.rs"]
mod selected_snapshot;
#[path = "smart_context.rs"]
mod smart_context;
