use super::*;
use std::borrow::Cow;

use prodex_mojo_core::rich::SmartContextNormalizationMode;

mod artifacts;
mod rewrite_policy;
mod static_context;
mod token_budget;

pub(super) use artifacts::*;
pub use artifacts::{
    smart_context_hash_matches_text, smart_context_hash_text,
    smart_context_normalized_command_output_hash_text,
};
pub(super) use rewrite_policy::*;
pub(super) use static_context::*;
pub(super) use token_budget::*;

fn normalize_volatile(text: &str, mode: SmartContextNormalizationMode) -> Cow<'_, str> {
    Cow::Owned(
        prodex_mojo_core::rich::normalize_smart_context_volatile(text, mode)
            .expect("Mojo Smart Context volatile normalizer returned invalid output"),
    )
}

pub fn smart_context_normalize_volatile_command_output(text: &str) -> Cow<'_, str> {
    normalize_volatile(text, SmartContextNormalizationMode::CommandOutput)
}

pub fn smart_context_normalize_volatile_static_context(text: &str) -> Cow<'_, str> {
    normalize_volatile(text, SmartContextNormalizationMode::StaticContext)
}

#[cfg(test)]
#[path = "normalization/volatile_tests.rs"]
mod volatile_tests;
